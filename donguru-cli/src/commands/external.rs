//! External subcommand dispatch (Git/Cargo-style plugins).
//!
//! When the first argument is not a built-in subcommand e.g. `foo`, search `PATH`
//! for an executable named `donguru-foo` and run it, forwarding the remaining
//! arguments verbatim. Resolved context is passed via environment variables
//! (`DONGURU_CTX_DEVICE`, `DONGURU_CTX_OUTPUT`) so plugins inherit device selection
//! and output format.

use crate::exit::ExitCode;
use std::ffi::OsString;

use anyhow::bail;

use crate::context::Context;

pub fn run(ctx: &Context, args: Vec<OsString>) -> anyhow::Result<ExitCode> {
    let Some((name, rest)) = args.split_first() else {
        bail!("empty external subcommand");
    };
    let name = name.to_string_lossy();
    let plugin = format!("donguru-{name}");
    // TODO:
    //  - resolve `plugin` on PATH (error with a "no such command" hint,
    //    listing near-matches, when absent),
    //  - export DONGURU_DEVICE / DONGURU_OUTPUT from `ctx`,
    //  - exec (Unix) or spawn-and-wait (other platforms), forwarding `_rest`.
    let _ = ctx;
    eprintln!(
        "donguru: external subcommand dispatch is not implemented yet (would run `{plugin}`), args: {rest:?}"
    );
    Ok(ExitCode::Failure)
}
