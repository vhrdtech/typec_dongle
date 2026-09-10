//! Command-line interface, global settings and built-in command tree.
//!
//! Every node supports `--help`, unknown top-level subcommands are captured for external
//! plugin dispatch (`donguru my-command` → `donguru-my-command` on `PATH`).

use std::ffi::OsString;

use crate::theme;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};

/// Command-line interface for the Donguru USB-C dongle.
#[derive(Debug, Parser)]
#[command(
    name = "donguru",
    bin_name = "donguru",
    version,
    about = "Interact with a Donguru USB-dongle",
    subcommand_required = true,
    arg_required_else_help = true,
    allow_external_subcommands = true
)]
pub struct Cli {
    #[command(flatten)]
    pub globals: GlobalSettings,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Apply the CLI color theme before and parse arguments.
    ///
    /// As we want to downgrade the color theme depending on the terminal's capabilities,
    /// we manually apply the style rather than using `#[command(styles = ...)]` which
    /// requires a `const` and is applied "statically".
    pub fn parse_styled() -> Self {
        let matches = Self::command().styles(theme::select_style()).get_matches();
        match Self::from_arg_matches(&matches) {
            Ok(cli) => cli,
            Err(err) => err.exit(),
        }
    }
}

/// Settings accepted at every level of the command tree.
#[derive(Debug, Args)]
pub struct GlobalSettings {
    /// Select the target dongle (serial number, index or USB path)
    #[arg(
        short,
        long,
        global = true,
        env = "DONGURU_DEVICE",
        value_name = "SELECTOR"
    )]
    pub device: Option<String>,

    /// Output format
    #[arg(
        short,
        long,
        global = true,
        env = "DONGURU_OUTPUT",
        value_enum,
        default_value_t = Format::Text
    )]
    pub format: Format,

    /// When to colourise output
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto, value_name = "WHEN")]
    pub color: ColorMode,

    /// Increase diagnostic detail on stderr (repeatable)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbosity: u8,

    /// Suppress non-essential stderr output
    #[arg(short, long, global = true, conflicts_with = "verbosity")]
    pub quiet: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Human-readable, aligned, colourised on a TTY
    Text,
    /// One JSON value
    Json,
    /// Newline-delimited JSON (e.g. for streaming commands)
    Jsonl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    /// Colourise only when stderr/stdout is a TTY that supports it
    Auto,
    /// Always emit colour escape sequences
    Always,
    /// Never emit colour escape sequences
    Never,
}

/// Top-level command tree.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Provides general information about the available dongles and the environment
    #[command(subcommand)]
    Info,

    /// Enumerate and inspect dongles
    #[command(subcommand)]
    Device,

    /// Control downstream USB port(s)
    #[command(subcommand)]
    Usb,

    /// Read and drive general-purpose I/O pins
    #[command(subcommand)]
    Gpio,

    /// Generate Udev rules
    #[command(display_order = 70)]
    Udev,

    /// Generate a default config, and inspect/evaluate the effective one
    #[command(subcommand, display_order = 80)]
    Config,

    /// Generate shell completions
    #[command(display_order = 90)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// External plugin: `donguru my-command ...` runs `donguru-my-command ...` from PATH
    #[command(external_subcommand)]
    External(Vec<OsString>),
}
