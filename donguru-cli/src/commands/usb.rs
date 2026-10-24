//! `donguru usb` - USB data to the device under test: the plug's data switch (U701, CLI-6). Power to the DUT is
//! `donguru power`; the header's own switched VBUS and relay are `donguru header`.

use std::io::{self, Write};

use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;
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

/// Whether USB data reaches the DUT.
struct Data(DataPath);

impl Render for Data {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let text = fmt_data(self.0);
        match self.0 {
            DataPath::Connected => writeln!(out, "{}: {}", th.key("data"), th.ok(text)),
            DataPath::Cut => writeln!(out, "{}: {}", th.key("data"), th.value(text)),
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "data": fmt_data(self.0) })
    }
}

fn print_data(ctx: &Context, data: DataPath) -> anyhow::Result<()> {
    Output::stdout(ctx).emit(&Data(data))
}

pub fn run(ctx: &Context, command: Usb) -> anyhow::Result<ExitCode> {
    match command {
        Usb::Status => {
            let data = super::with_dongle(ctx, async |dongle| dongle.plug().data().await)?;
            print_data(ctx, data)?;
            Ok(ExitCode::Success)
        }
        Usb::Connect => {
            let data = super::with_dongle(ctx, async |dongle| {
                dongle.plug().set_data(DataPath::Connected).await?;
                dongle.plug().data().await
            })?;
            print_data(ctx, data)?;
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
            print_data(ctx, data)?;
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

    #[test]
    fn data_text_and_json() {
        use crate::cli::Format;
        use crate::output::render;
        assert_eq!(render(Format::Text, &Data(DataPath::Cut)), "data: cut\n");
        assert_eq!(
            render(Format::Json, &Data(DataPath::Connected)),
            "{\"data\":\"connected\"}\n"
        );
    }
}
