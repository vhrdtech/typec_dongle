//! `donguru info` - Provide general information about the available dongles and the environment.

use crate::context::Context;
use crate::exit::ExitCode;

pub fn run(_ctx: &Context) -> anyhow::Result<ExitCode> {
    super::stub("info", ExitCode::Failure)
}
