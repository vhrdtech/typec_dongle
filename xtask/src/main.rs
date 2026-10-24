//! Generates man pages for the whole `donguru` command tree (PKG-3): one `.1` file per
//! command and subcommand, from the same [`clap::Command`] the binary itself parses with.
//!
//! Run via `just man` or as part of `just package`; `xtask` is not installed anywhere.

use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use donguru_cli::cli::Cli;

#[derive(Parser)]
struct Args {
    /// Directory the man pages (donguru.1, donguru-device.1, ...) are written into
    #[arg(default_value = "target/dist/man")]
    out_dir: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    std::fs::create_dir_all(&args.out_dir)?;
    clap_mangen::generate_to(Cli::command(), &args.out_dir)?;
    println!("xtask: wrote man pages to {}", args.out_dir.display());
    Ok(())
}
