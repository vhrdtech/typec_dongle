//! `donguru usb` - USB data to the device under test: the plug's data switch (U701, CLI-6). Power to the DUT is
//! `donguru power`; the header's own switched VBUS and relay are `donguru header`.

use crate::context::Context;
use crate::exit::ExitCode;
use clap::Subcommand;
use donguru_core::api::DataPath;

#[derive(Debug, Subcommand)]
pub enum Usb {
    /// Show whether USB data reaches the DUT
    Status,
    /// Connect USB data (D+/D-) to the DUT
    Connect,
    /// Disconnect USB data from the DUT, without touching VBUS
    Disconnect,
}

fn fmt_data(data: DataPath) -> &'static str {
    match data {
        DataPath::Connected => "connected",
        DataPath::Cut => "cut",
    }
}

fn print_data(ctx: &Context, data: DataPath) {
    match ctx.format {
        crate::cli::Format::Text => println!("data: {}", fmt_data(data)),
        crate::cli::Format::Json | crate::cli::Format::Jsonl => {
            println!("{}", serde_json::json!({ "data": fmt_data(data) }))
        }
    }
}

pub fn run(ctx: &Context, command: Usb) -> anyhow::Result<ExitCode> {
    match command {
        Usb::Status => {
            let data = super::with_dongle(ctx, async |dongle| dongle.plug().data().await)?;
            print_data(ctx, data);
            Ok(ExitCode::Success)
        }
        Usb::Connect => {
            let data = super::with_dongle(ctx, async |dongle| {
                dongle.plug().set_data(DataPath::Connected).await?;
                dongle.plug().data().await
            })?;
            print_data(ctx, data);
            Ok(if matches!(data, DataPath::Connected) {
                ExitCode::Success
            } else {
                ExitCode::Failure
            })
        }
        Usb::Disconnect => {
            let data = super::with_dongle(ctx, async |dongle| {
                dongle.plug().set_data(DataPath::Cut).await?;
                dongle.plug().data().await
            })?;
            print_data(ctx, data);
            Ok(if matches!(data, DataPath::Cut) {
                ExitCode::Success
            } else {
                ExitCode::Failure
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::mock::MockDevice;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connect_and_disconnect() {
        let mock = MockDevice::start("cli/usb_data");
        let dongle = mock.open().await.unwrap();
        assert_eq!(dongle.plug().data().await.unwrap(), DataPath::Connected);
        dongle.plug().set_data(DataPath::Cut).await.unwrap();
        assert_eq!(dongle.plug().data().await.unwrap(), DataPath::Cut);
        dongle.plug().set_data(DataPath::Connected).await.unwrap();
        assert_eq!(dongle.plug().data().await.unwrap(), DataPath::Connected);
        dongle.close().await.unwrap();
    }
}
