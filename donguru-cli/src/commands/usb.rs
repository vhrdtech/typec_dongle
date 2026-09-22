//! `donguru usb` - control downstream usb port(s).

use crate::context::Context;
use crate::exit::ExitCode;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Port {
    #[value(aliases = ["0", "p0"], help = "First downstream port [aliases: 0, p0]")]
    Port0,
    #[value(aliases = ["1", "p1"], help = "Second downstream port [aliases: 1, p1]")]
    Port1,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PowerState {
    #[value(aliases = ["enable", "attach"], help = "Enable power [aliases: enable, attach]")]
    On,
    #[value(aliases = ["disable", "detach"], help = "Disable power [aliases: disable, detach]")]
    Off,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DataState {
    #[value(aliases = ["enable", "attach"], help = "Connect data lines [aliases: enable, attach]")]
    Connect,
    #[value(aliases = ["disable", "detach"], help = "Disconnect data lines [aliases: disable, detach]")]
    Disconnect,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct UsbPort {
    #[arg(
        short,
        long,
        value_enum,
        value_name = "PORT",
        env = "DONGURU_USB_PORT",
        help = "Downstream USB port to interact with [aliases: 0|p0, 1|p1]"
    )]
    /// Downstream USB port to interact with
    ///
    /// Each port can also be given by its short alias, e.g. `-p 0` or `-p p1`.
    pub port: Option<Port>,
}

#[derive(Debug, Subcommand)]
pub enum Usb {
    #[command(alias = "ls")]
    /// List ports and their current status
    List,
    /// Emulate cable removal
    Detach {
        #[command(flatten)]
        port: UsbPort,
    },
    /// Emulate a cable insertion
    Attach {
        #[command(flatten)]
        port: UsbPort,
    },
    /// Control the USB power lines of a specific USB port
    Power {
        #[arg(value_enum)]
        action: PowerState,

        #[command(flatten)]
        port: UsbPort,
    },
    /// Control the USB data lines of a specific USB port
    Data {
        #[arg(value_enum)]
        action: DataState,

        #[command(flatten)]
        port: UsbPort,
    },
}

pub fn run(_ctx: &Context, command: Usb) -> anyhow::Result<ExitCode> {
    match command {
        Usb::List => super::stub("usb list", ExitCode::Failure),
        Usb::Detach { port } => super::stub(&format!("usb detach {port:?}"), ExitCode::Failure),
        Usb::Attach { port } => super::stub(&format!("usb attach {port:?}"), ExitCode::Failure),
        Usb::Power { port, action } => super::stub(
            &format!("usb power {action:?} [PORT: {port:?}]"),
            ExitCode::Failure,
        ),
        Usb::Data { port, action } => super::stub(
            &format!("usb data {action:?} [PORT: {port:?}]"),
            ExitCode::Failure,
        ),
    }
}
