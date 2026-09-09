use crate::cli::{Cli, ColorMode, Format};

/// Everything a command handler needs beyond its own arguments.
#[derive(Debug)]
#[allow(dead_code)] // scaffolding: fields consumed once handlers are real
pub struct Context {
    /// Device selector from `--device` / `DONGURU_DEVICE` / config (TBD).
    pub device: Option<String>,
    /// Selected output format.
    pub format: Format,
    /// Colour policy (applied only when rendering `text` to a TTY).
    pub color: ColorMode,
    /// Verbosity level (`-v` count); `quiet` wins when set.
    pub verbosity: u8,
    pub quiet: bool,
}

impl Context {
    pub fn from_cli(cli: &Cli) -> Self {
        // TODO: merge config file layers here (see docs/design/cli/index.md,
        // "Configuration"). Flags/env already win because clap resolves them.
        Self {
            device: cli.globals.device.clone(),
            format: cli.globals.format,
            color: cli.globals.color,
            verbosity: cli.globals.verbosity,
            quiet: cli.globals.quiet,
        }
    }
}
