//! Build-time helpers that are not part of any shipped crate (PKG-3): man pages and shell
//! completions generated straight from the same [`clap::Command`] the binary parses with, for
//! `cargo deb` / `cargo generate-rpm` to pick up as package assets.
//!
//! Run via `just package`; `xtask` is not installed anywhere.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand};
use donguru_cli::cli::Cli;
use donguru_cli::completion;

/// The two names the same executable is installed under.
const NAMES: [&str; 2] = ["donguru", "dg"];

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Write one man page per command/subcommand (donguru.1, donguru-device.1, ...)
    Man {
        #[arg(default_value = "target/dist/man")]
        out_dir: PathBuf,
    },
    /// Write bash, zsh and fish completions for `donguru` and `dg`
    Completions {
        #[arg(default_value = "target/dist/completions")]
        out_dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    match Args::parse().command {
        Command::Man { out_dir } => man(&out_dir),
        Command::Completions { out_dir } => completions(&out_dir),
    }
}

fn man(out_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    clap_mangen::generate_to(Cli::command(), out_dir)?;
    println!("xtask: wrote man pages to {}", out_dir.display());
    Ok(())
}

/// Per-shell completion filename convention: bash takes the bare command name, zsh prefixes
/// it with `_`, fish appends `.fish`.
fn completion_filename(shell: &str, name: &str) -> anyhow::Result<String> {
    Ok(match shell {
        "bash" => name.to_string(),
        "zsh" => format!("_{name}"),
        "fish" => format!("{name}.fish"),
        other => anyhow::bail!("no completions for shell {other:?}"),
    })
}

fn completions(out_dir: &Path) -> anyhow::Result<()> {
    for shell in ["bash", "zsh", "fish"] {
        let dir = out_dir.join(shell);
        std::fs::create_dir_all(&dir)?;
        for name in NAMES {
            let path = dir.join(completion_filename(shell, name)?);
            let mut script = Vec::new();
            completion::registration(shell, name, &mut script)?;
            std::fs::File::create(&path)?.write_all(&script)?;
        }
    }
    println!("xtask: wrote completions to {}", out_dir.display());
    Ok(())
}
