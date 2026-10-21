//! `donguru uart` - the UARTs of `uart[]` (CLI-20). So far `uart configure`: the line of the header UART
//! (`uart[1]`, IO7 TX / IO8 RX) until the dongle resets; the plug UART on SBU follows with API-14.

use clap::{Args, Subcommand, ValueEnum};
use clap_complete::engine::ArgValueCandidates;
use donguru_core::levels::Side;
use ww_si::{Baud, Prefix};
use ww_uart::{Mode, UartConfig};

use super::settings::{fmt_framing, parse_uart_value, uart_plug_json};
use crate::cli::Format;
use crate::completion;
use crate::context::Context;
use crate::exit::ExitCode;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Port {
    /// Header J101: IO7 (pin 1) TX, IO8 (pin 2) RX
    Header,
    /// The plug SBU lines (not implemented in the firmware yet)
    Plug,
}

impl From<Port> for Side {
    fn from(p: Port) -> Side {
        match p {
            Port::Header => Side::Header,
            Port::Plug => Side::Plug,
        }
    }
}

#[derive(Debug, Args)]
pub struct Configure {
    /// Baud rate; the dongle sets the nearest its divider can do and says which
    #[arg(value_name = "BAUD", add = ArgValueCandidates::new(completion::bauds))]
    baud: u32,
    /// Data bits, parity, stop bits, e.g. `8N1` or `7E2`
    #[arg(value_name = "FRAMING", default_value = "8N1", add = ArgValueCandidates::new(completion::framings))]
    framing: String,
    /// Which UART
    #[arg(long, value_enum, default_value = "header")]
    port: Port,
}

#[derive(Debug, Subcommand)]
pub enum Uart {
    /// Set the line (baud rate, framing) until the dongle resets, print what was set
    ///
    /// The header UART runs at 4 Bd ..= 8 Mbaud; above 4 Mbaud only 64 MHz / k is possible: 8, 7.11, 6.4,
    /// 5.82, 5.33 Mbaud. 9 data bits, 5 and 6 are refused.
    Configure(Configure),
}

pub fn run(ctx: &Context, command: Uart) -> anyhow::Result<ExitCode> {
    match command {
        Uart::Configure(args) => configure(ctx, args),
    }
}

fn configure(ctx: &Context, args: Configure) -> anyhow::Result<ExitCode> {
    let (baud, data_bits, parity, stop_bits) =
        parse_uart_value(&format!("{} {}", args.baud, args.framing)).map_err(anyhow::Error::msg)?;
    let asked = UartConfig {
        baud_rate: Baud::new(Prefix::Unit, baud),
        data_bits,
        parity,
        stop_bits,
        mode: Mode::Asynchronous,
        prevent_back_feed: false,
        io_level: None,
        idle_chars: ww_uart::IDLE_CHARS_DEFAULT,
    };
    let side = Side::from(args.port);
    let got = super::with_dongle(ctx, async |dongle| dongle.uart(side).configure(asked).await)?;
    print_configured(ctx, baud, &got);
    Ok(ExitCode::Success)
}

/// How far the rate set is from the one asked for, in percent.
fn error_pct(asked: u32, got: &UartConfig) -> f64 {
    (got.baud_rate.to_f64() / asked as f64 - 1.0) * 100.0
}

fn print_configured(ctx: &Context, asked: u32, got: &UartConfig) {
    let err = error_pct(asked, got);
    match ctx.format {
        Format::Text => println!("{}  ({err:+.2} % from {asked})", fmt_framing(got)),
        Format::Json | Format::Jsonl => {
            let mut json = uart_plug_json(got);
            json["baud_asked"] = asked.into();
            json["baud_error_pct"] = ((err * 100.0).round() / 100.0).into();
            println!("{json}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_error() {
        let mut got = UartConfig {
            baud_rate: Baud::new(Prefix::Unit, 5_818_182u32),
            data_bits: ww_uart::DataBits::Eight,
            parity: ww_uart::Parity::None,
            stop_bits: ww_uart::StopBits::Stop1,
            mode: Mode::Asynchronous,
            prevent_back_feed: false,
            io_level: None,
            idle_chars: 1,
        };
        assert!((error_pct(6_000_000, &got) - -3.03).abs() < 0.01);
        got.baud_rate = Baud::new(Prefix::Unit, 921_739u32);
        assert!((error_pct(921_600, &got) - 0.015).abs() < 0.001);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn configure_answers_the_rate_set() {
        let mock = donguru_core::mock::MockDevice::start("cli/uart");
        let dongle = mock.open().await.unwrap();
        let (baud, data_bits, parity, stop_bits) = parse_uart_value("6000000 8N1").unwrap();
        let asked = UartConfig {
            baud_rate: Baud::new(Prefix::Unit, baud),
            data_bits,
            parity,
            stop_bits,
            mode: Mode::Asynchronous,
            prevent_back_feed: false,
            io_level: None,
            idle_chars: ww_uart::IDLE_CHARS_DEFAULT,
        };
        let got = dongle
            .uart(Side::Header)
            .configure(asked.clone())
            .await
            .unwrap();
        assert_eq!(fmt_framing(&got), "5818182 8N1");
        let mut nine = asked;
        nine.data_bits = ww_uart::DataBits::Nine;
        assert!(dongle.uart(Side::Header).configure(nine).await.is_err());
        dongle.close().await.unwrap();
    }
}
