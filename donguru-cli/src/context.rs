use crate::cli::{Cli, ColorMode, Format};
use crate::project_config;

/// Everything a command handler needs beyond its own arguments.
#[derive(Debug)]
#[allow(dead_code)] // scaffolding: fields consumed once handlers are real
pub struct Context {
    /// Device selector from `--device` / `DONGURU_DEVICE` / `./donguru.toml` (`device select`, CLI-7); the full
    /// config layering of CLI-9 is still a stub.
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
        // `--device` and `DONGURU_DEVICE` are already resolved by clap (the `env` attribute); the project file is
        // the next, lowest-priority layer. The rest of the layering (system/user config, other keys) is CLI-9.
        let device = cli
            .globals
            .device
            .clone()
            .or_else(project_config::device_selector);
        Self {
            device,
            format: cli.globals.format,
            color: cli.globals.color,
            verbosity: cli.globals.verbosity,
            quiet: cli.globals.quiet,
        }
    }
}
