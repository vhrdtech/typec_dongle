pub mod completions;
pub mod config;
pub mod device;
pub mod external;
pub mod gpio;
pub mod info;
pub mod udev;
pub mod usb;

use crate::cli::Command;
use crate::context::Context;
use crate::exit::ExitCode;

pub fn dispatch(ctx: &Context, command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Info => info::run(ctx),
        Command::Device(cmd) => device::run(ctx, cmd),
        Command::Usb(cmd) => usb::run(ctx, cmd),
        Command::Gpio(cmd) => gpio::run(ctx, cmd),
        Command::Completions { shell } => completions::run(ctx, shell),
        Command::Udev(cmd) => udev::run(ctx, cmd),
        Command::Config(cmd) => config::run(ctx, cmd),
        Command::External(cmd) => external::run(ctx, cmd),
    }
}

pub(crate) fn stub(what: &str, exit_code: ExitCode) -> anyhow::Result<ExitCode> {
    eprintln!("donguru: `{what}` is not implemented yet");
    Ok(exit_code)
}
