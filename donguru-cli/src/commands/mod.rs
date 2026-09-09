use crate::cli::Command;
use crate::context::Context;
use crate::exit::ExitCode;

pub fn dispatch(_ctx: &Context, command: Command) -> anyhow::Result<ExitCode> {
    match command {
        _ => Ok(ExitCode::Failure),
    }
}

pub(crate) fn stub(what: &str) -> anyhow::Result<ExitCode> {
    eprintln!("donguru: `{what}` is not implemented yet");
    Ok(ExitCode::Success)
}
