//! Shell completions with live values (CLI-3, CLI-49).
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::Args;
use clap_complete::Shell;

use crate::completion;
use crate::context::Context;
use crate::exit::ExitCode;

/// The two names the same executable answers to.
const NAMES: [&str; 2] = ["donguru", "dg"];

#[derive(Debug, Args)]
pub struct Completions {
    /// Shell to generate completions for (with `--install`: just this one, default bash, zsh and fish)
    #[arg(value_enum, required_unless_present = "install")]
    shell: Option<Shell>,

    /// Write the scripts for `donguru` and `dg` into the shell's per-user completion directory instead of printing
    #[arg(long)]
    install: bool,

    /// Command name the script completes
    #[arg(
        long,
        value_name = "NAME",
        default_value = "donguru",
        conflicts_with = "install"
    )]
    bin: String,
}

pub fn run(_ctx: &Context, args: Completions) -> anyhow::Result<ExitCode> {
    if args.install {
        let shells = match args.shell {
            Some(s) => vec![s],
            None => vec![Shell::Bash, Shell::Zsh, Shell::Fish],
        };
        for shell in shells {
            install(shell)?;
        }
    } else if let Some(shell) = args.shell {
        completion::registration(&shell.to_string(), &args.bin, &mut std::io::stdout().lock())?;
    }
    Ok(ExitCode::Success)
}

fn home_dir(var: &str, fallback: &str) -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os(var).filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").context("neither HOME nor XDG directories are set")?;
    Ok(Path::new(&home).join(fallback))
}

/// Per-user completion file of `shell` for command `name`.
fn target(shell: Shell, name: &str) -> anyhow::Result<PathBuf> {
    Ok(match shell {
        Shell::Bash => home_dir("XDG_DATA_HOME", ".local/share")?
            .join("bash-completion/completions")
            .join(name),
        Shell::Fish => home_dir("XDG_CONFIG_HOME", ".config")?
            .join("fish/completions")
            .join(format!("{name}.fish")),
        Shell::Zsh => home_dir("XDG_DATA_HOME", ".local/share")?
            .join("zsh/site-functions")
            .join(format!("_{name}")),
        other => anyhow::bail!("--install knows bash, zsh and fish, not {other}"),
    })
}

fn install(shell: Shell) -> anyhow::Result<()> {
    for name in NAMES {
        let path = target(shell, name)?;
        let mut script = Vec::new();
        completion::registration(&shell.to_string(), name, &mut script)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::File::create(&path)
            .and_then(|mut f| f.write_all(&script))
            .with_context(|| format!("writing {}", path.display()))?;
        eprintln!("donguru: wrote {}", path.display());
    }
    if shell == Shell::Zsh {
        eprintln!("donguru: zsh needs ~/.local/share/zsh/site-functions in $fpath before compinit");
    }
    Ok(())
}
