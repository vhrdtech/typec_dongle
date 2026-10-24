//! `donguru config` - generate, locate and inspect the configuration.
//!
//! Settings are merged from several sources, with later ones overriding earlier
//! ones: built-in defaults, the system config file (`/etc/donguru/donguru.toml`),
//! the users config file (`$XDG_CONFIG_HOME/donguru/donguru.toml`), the project file
//! (`./donguru.toml`), the `DONGURU_*` environment variables and finally the command line.
//! Because of that layering it is often hard to tell which value a command actually uses,
//! in case of doubt use `donguru config show` answers that question.

use std::path::PathBuf;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use anyhow::Context as _;
use clap::{Subcommand, ValueEnum};
use donguru_core::config::{self, Layer};

/// One of the configuration files in the search path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scope {
    /// `./donguru.toml` - settings for the current project
    #[value(
        alias = "local",
        help = "Project configuration, ./donguru.toml [alias: local]"
    )]
    Project,

    /// `$XDG_CONFIG_HOME/donguru/donguru.toml` - settings for the current user
    #[value(
        alias = "global",
        help = "User configuration, $XDG_CONFIG_HOME/donguru/donguru.toml [alias: global]"
    )]
    User,

    /// `/etc/donguru/donguru.toml` - settings for every user on the machine
    #[value(
        alias = "etc",
        help = "System configuration, /etc/donguru/donguru.toml [alias: etc]"
    )]
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
        #[arg(
            value_enum,
            value_name = "SCOPE",
            help = "Print only the path of this scope [aliases: local, global, etc]"
        )]
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

impl Scope {
    fn layer(self) -> Layer {
        match self {
            Scope::Project => Layer::Project,
            Scope::User => Layer::User,
            Scope::System => Layer::System,
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The file of every scope, highest precedence first.
fn search_path() -> Vec<(Scope, PathBuf)> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let xdg = env_path("XDG_CONFIG_HOME");
    let home = env_path("HOME");
    [Scope::Project, Scope::User, Scope::System]
        .into_iter()
        .filter_map(|scope| {
            config::path_of(scope.layer(), &cwd, xdg.as_deref(), home.as_deref())
                .map(|path| (scope, path))
        })
        .collect()
}

fn generate(output: Option<PathBuf>, force: bool) -> anyhow::Result<ExitCode> {
    let text = config::generate();
    let Some(path) = output else {
        print!("{text}");
        return Ok(ExitCode::Success);
    };
    if path.exists() && !force {
        eprintln!(
            "donguru: {} exists; `--force` overwrites it",
            path.display()
        );
        return Ok(ExitCode::Failure);
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    std::fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
    eprintln!("donguru: wrote {}", path.display());
    Ok(ExitCode::Success)
}

fn show(ctx: &Context, key: Option<String>, origin: bool) -> anyhow::Result<ExitCode> {
    if let Some(name) = key {
        let Some(entry) = ctx.config.iter().find(|e| e.key.name == name) else {
            eprintln!(
                "donguru: unknown key `{name}` (known: {})",
                config::KEYS
                    .iter()
                    .map(|k| k.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return Ok(ExitCode::BadArgs);
        };
        return Ok(match (&entry.value, ctx.format) {
            (Some(v), Format::Text) => {
                println!("{v}");
                ExitCode::Success
            }
            (Some(v), _) => {
                println!("{}", serde_json::json!(v));
                ExitCode::Success
            }
            // set nowhere: exit 1 with no output, so `if v=$(dg config show device)` works
            (None, _) => ExitCode::Failure,
        });
    }
    match ctx.format {
        Format::Text => {
            let width = config::KEYS.iter().map(|k| k.name.len()).max().unwrap_or(0);
            for e in &ctx.config {
                let value = e
                    .value
                    .as_deref()
                    .map_or("<unset>".to_string(), |v| format!("{v:?}"));
                if origin {
                    println!("{:width$} = {value}  # {}", e.key.name, e.layer);
                } else {
                    println!("{:width$} = {value}", e.key.name);
                }
            }
        }
        Format::Json | Format::Jsonl => {
            let obj: serde_json::Map<_, _> = ctx
                .config
                .iter()
                .map(|e| {
                    let v = if origin {
                        serde_json::json!({"value": e.value, "origin": e.layer.to_string()})
                    } else {
                        serde_json::json!(e.value)
                    };
                    (e.key.name.to_string(), v)
                })
                .collect();
            println!("{}", serde_json::Value::Object(obj));
        }
    }
    Ok(ExitCode::Success)
}

fn path(ctx: &Context, scope: Option<Scope>) -> anyhow::Result<ExitCode> {
    let all = search_path();
    if let Some(scope) = scope {
        if let Some((_, p)) = all.iter().find(|(s, _)| *s == scope) {
            println!("{}", p.display());
            return Ok(ExitCode::Success);
        }
        eprintln!(
            "donguru: no {} configuration directory (HOME is not set)",
            fmt_enum(scope)
        );
        return Ok(ExitCode::Failure);
    }
    match ctx.format {
        Format::Text => {
            for (scope, p) in &all {
                let mark = if p.is_file() { "exists" } else { "missing" };
                println!("{:8} {mark:8} {}", fmt_enum(scope), p.display());
            }
        }
        Format::Json | Format::Jsonl => {
            let list: Vec<_> = all
                .iter()
                .map(|(scope, p)| {
                    serde_json::json!({"scope": fmt_enum(scope), "path": p, "exists": p.is_file()})
                })
                .collect();
            println!("{}", serde_json::Value::Array(list));
        }
    }
    Ok(ExitCode::Success)
}

fn validate(path: Option<PathBuf>) -> anyhow::Result<ExitCode> {
    let files: Vec<PathBuf> = match path {
        Some(p) => vec![p],
        None => search_path()
            .into_iter()
            .map(|(_, p)| p)
            .filter(|p| p.is_file())
            .collect(),
    };
    let mut bad = false;
    for file in &files {
        match std::fs::read_to_string(file)
            .map_err(|e| config::ConfigError::Io {
                path: file.display().to_string(),
                source: e,
            })
            .and_then(|text| config::Settings::parse(&file.display().to_string(), &text))
        {
            Ok(_) => println!("{}: ok", file.display()),
            Err(e) => {
                bad = true;
                eprintln!("donguru: {e}");
            }
        }
    }
    if files.is_empty() {
        eprintln!(
            "donguru: no configuration file found; `donguru config path` lists where they are looked for"
        );
    }
    Ok(if bad {
        ExitCode::Failure
    } else {
        ExitCode::Success
    })
}

/// Lowercase name of a `ValueEnum` variant.
fn fmt_enum<T: std::fmt::Debug>(value: T) -> String {
    format!("{value:?}").to_lowercase()
}

pub fn run(ctx: &Context, command: Config) -> anyhow::Result<ExitCode> {
    match command {
        Config::Generate { output, force } => generate(output, force),
        Config::Show { key, origin } => show(ctx, key, origin),
        Config::Path { scope } => path(ctx, scope),
        Config::Validate { path } => validate(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_refuses_to_overwrite_without_force() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub/donguru.toml");
        assert!(matches!(
            generate(Some(file.clone()), false).unwrap(),
            ExitCode::Success
        ));
        assert!(std::fs::read_to_string(&file).unwrap().contains("# output"));
        assert!(matches!(
            generate(Some(file.clone()), false).unwrap(),
            ExitCode::Failure
        ));
        assert!(matches!(
            generate(Some(file.clone()), true).unwrap(),
            ExitCode::Success
        ));
    }

    #[test]
    fn validate_flags_a_bad_file() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.toml");
        let bad = dir.path().join("bad.toml");
        std::fs::write(&good, "output = \"json\"\n").unwrap();
        std::fs::write(&bad, "nope = 1\n").unwrap();
        assert!(matches!(validate(Some(good)).unwrap(), ExitCode::Success));
        assert!(matches!(validate(Some(bad)).unwrap(), ExitCode::Failure));
    }
}
