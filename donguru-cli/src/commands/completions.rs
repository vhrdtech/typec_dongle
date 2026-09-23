//! Generate shell completions.
use std::io;

use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;
use crate::context::Context;
use crate::exit::ExitCode;

pub fn run(_ctx: &Context, shell: Shell) -> anyhow::Result<ExitCode> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_bin_name().unwrap_or("donguru").to_string();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut io::stdout());
    Ok(ExitCode::Success)
}
