
pub mod info;
pub mod device;

use crate::cli::Command;
use crate::context::Context;
use crate::exit::ExitCode;


pub fn dispatch(ctx: &Context, command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Info => info::run(ctx),
        Command::Device(cmd) => device::run(ctx, cmd),
        _ => Ok(ExitCode::Failure),
    }
}

pub(crate) fn stub(what: &str, exit_code: ExitCode) -> anyhow::Result<ExitCode> {
    eprintln!("donguru: `{what}` is not implemented yet");
    Ok(exit_code)
}
