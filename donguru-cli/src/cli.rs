//! Command-line interface, global settings and built-in command tree.
//!
//! Every node supports `--help`, unknown top-level subcommands are captured for external
//! plugin dispatch (`donguru foo` → `donguru-foo` on `PATH`).

use std::ffi::OsString;

use clap::{Args, Parser, Subcommand, ValueEnum};

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
    Auto,
    Always,
    Never,
}

/// Top-level command tree.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Enumerate and inspect dongles
    #[command(subcommand)]
    Device,

    /// Read and drive general-purpose I/O pins
    #[command(subcommand)]
    Gpio,

    /// Control downstream USB hub ports
    #[command(subcommand)]
    Usb,

    /// Power switch and power meter
    #[command(subcommand)]
    Power,

    /// I2C bus bridge
    #[command(subcommand)]
    I2c,

    /// UART (USART) bridge
    #[command(subcommand)]
    Uart,

    /// Firmware version and update
    #[command(subcommand)]
    Fw,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Generate a default config, and inspect/evaluate the effective one
    #[command(subcommand)]
    Config,

    /// External plugin: `donguru foo ...` runs `donguru-foo ...` from PATH
    #[command(external_subcommand)]
    External(Vec<OsString>),
}
