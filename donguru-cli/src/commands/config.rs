//! `donguru config` - generate, locate and inspect the configuration.
//!
//! Settings are merged from several sources, with later ones overriding earlier
//! ones: built-in defaults, the system config file (`/etc/donguru/donguru.toml`),
//! the users config file (`$XDG_CONFIG_HOME/donguru/donguru.toml`), the project file
//! (`./donguru.toml`), the `DONGURU_*` environment variables and finally the command line.
//! Because of that layering it is often hard to tell which value a command actually uses,
//! in case of doubt use `donguru config show` answers that question.

use std::path::PathBuf;

use crate::context::Context;
use crate::exit::ExitCode;
use clap::{Subcommand, ValueEnum};

/// One of the configuration files in the search path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scope {
    /// `./donguru.toml` - settings for the current project
    #[value(alias = "local", help = "Project configuration, ./donguru.toml [alias: local]")]
    Project,

    /// `$XDG_CONFIG_HOME/donguru/donguru.toml` - settings for the current user
    #[value(
        alias = "global",
        help = "User configuration, $XDG_CONFIG_HOME/donguru/donguru.toml [alias: global]"
    )]
    User,

    /// `/etc/donguru/donguru.toml` - settings for every user on the machine
    #[value(alias = "etc", help = "System configuration, /etc/donguru/donguru.toml [alias: etc]")]
    System,
}

#[derive(Debug, Subcommand)]
pub enum Config {
    /// Write a documented default configuration file
    ///
    /// Without `--output` the file is printed to stdout, so it can be reviewed
    /// or redirected before it is installed:
    ///
    ///   donguru config generate > donguru.toml
    #[command(alias = "init", verbatim_doc_comment)]
    Generate {
        /// Write to a file instead of stdout
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,

        /// Overwrite an existing file
        #[arg(long)]
        force: bool,
    },

    /// Show the effective configuration and where every value comes from
    ///
    /// Merges the defaults, the configuration files, the `DONGURU_*` environment
    /// variables and the global command line flags, then prints the result -
    /// this is exactly what the other commands see.
    ///
    /// Pass a KEY to print a single value, which makes it usable in scripts:
    ///
    ///   donguru config show usb.port
    #[command(aliases = ["effective", "get"], verbatim_doc_comment)]
    Show {
        /// Print only this key, e.g. `usb.port` or `udev.access`
        #[arg(value_name = "KEY")]
        key: Option<String>,

        /// Annotate every value with the source it was taken from
        #[arg(short = 'O', long, alias = "source")]
        origin: bool,
    },

    /// List the configuration files that are searched, in precedence order
    ///
    /// Marks which candidates exist, so a file that is ignored because it sits
    /// in the wrong directory is easy to spot.
    #[command(aliases = ["paths", "where"])]
    Path {
        /// Print only the path of this scope
        #[arg(value_enum, value_name = "SCOPE", help = "Print only the path of this scope [aliases: local, global, etc]")]
        scope: Option<Scope>,
    },

    /// Check a configuration file for unknown keys and invalid values
    ///
    /// Without PATH every file in the search path is checked. Exits with a
    /// non-zero status when a file is rejected, so it works as a CI check.
    #[command(alias = "check")]
    Validate {
        /// File to check instead of the ones in the search path
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
}

/// Lowercase name of a `ValueEnum` variant, for the stub messages.
fn fmt_enum<T: std::fmt::Debug>(value: T) -> String {
    format!("{value:?}").to_lowercase()
}

pub fn run(_ctx: &Context, command: Config) -> anyhow::Result<ExitCode> {
    match command {
        Config::Generate { output, force } => super::stub(
            &format!(
                "config generate -> {} (force: {force})",
                output.as_deref().map_or_else(
                    || "-".to_string(),
                    |path| path.display().to_string()
                ),
            ),
            ExitCode::Failure,
        ),
        Config::Show { key, origin } => super::stub(
            &format!(
                "config show {} (origin: {origin})",
                key.as_deref().unwrap_or("<all keys>"),
            ),
            ExitCode::Failure,
        ),
        Config::Path { scope } => super::stub(
            &format!(
                "config path {}",
                scope.map_or_else(|| "<all scopes>".to_string(), fmt_enum),
            ),
            ExitCode::Failure,
        ),
        Config::Validate { path } => super::stub(
            &format!(
                "config validate {}",
                path.as_deref().map_or_else(
                    || "<search path>".to_string(),
                    |path| path.display().to_string()
                ),
            ),
            ExitCode::Failure,
        ),
    }
}
