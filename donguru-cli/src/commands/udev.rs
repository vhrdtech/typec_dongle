//! `donguru udev` - manage the Linux udev rules that grant access to a dongle.
//!
//! Without a matching rule the character device is owned by `root`, so every command
//! that talks to the hardware fails with a permission error unless run via `sudo`.

use std::path::{Path, PathBuf};

use crate::cli::Format;
use crate::{context::Context, exit::ExitCode};
use clap::{Args, Subcommand, ValueEnum, builder::ArgPredicate};
use donguru_core::udev;

/// Default location of the generated rules file.
///
/// The `60-` prefix keeps the rule ahead of the systemd defaults that would otherwise
/// claim the device first.
const DEFAULT_RULES_PATH: &str = udev::DEFAULT_RULES_PATH;

/// Default group for [`Access::Group`].
const DEFAULT_GROUP: &str = udev::DEFAULT_GROUP;

/// How the generated rule hands out access to the device node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Access {
    /// `TAG+="uaccess"` - access for the user logged in at the seat
    #[value(
        aliases = ["logind", "seat"],
        help = "Grant access to the locally logged-in user, TAG+=\"uaccess\" [aliases: logind, seat]"
    )]
    Uaccess,

    /// `GROUP="<group>", MODE="0660"` - access for members of a group
    #[value(
        aliases = ["plugdev"],
        help = "Grant access to members of --group, MODE=\"0660\" [aliases: plugdev]"
    )]
    Group,

    /// `MODE="0666"` - access for every local user
    #[value(help = "Grant access to every local user, MODE=\"0666\"")]
    All,
}

/// Knobs that shape the content of the generated rule.
#[derive(Debug, Clone, Args)]
pub struct RuleOptions {
    /// Access policy encoded in the rule
    ///
    /// `uaccess` is the modern systemd-logind mechanism and the safest default, but it
    /// only grants access to a user logged in at a local seat, so it does nothing on a
    /// headless or SSH-only machine. `group` works everywhere but requires the group to
    /// exist and the user to be a member (`plugdev` is absent on some distributions).
    /// `all` is maximally compatible and least restrictive - it exposes the dongle to
    /// every local user, so prefer it only on single-user or lab machines.
    #[arg(
        short,
        long,
        value_enum,
        default_value_t = Access::Uaccess,
        default_value_if("group", ArgPredicate::IsPresent, "group"),
        value_name = "POLICY",
        env = "DONGURU_UDEV_ACCESS",
        help = "Access policy encoded in the rule [aliases: logind|seat, group|plugdev, all]"
    )]
    pub access: Access,

    /// Group granted access to the device node
    ///
    /// Implies `--access group`, so `--group dialout` is enough; the group must exist
    /// and the user must be a member of it. Ignored by the other access policies.
    #[arg(
        short,
        long,
        value_name = "GROUP",
        env = "DONGURU_UDEV_GROUP",
        help = "Group granted access, implies --access group [default: plugdev]"
    )]
    pub group: Option<String>,
}

impl RuleOptions {
    /// Group used by the `group` access policy, falling back to [`DEFAULT_GROUP`].
    pub fn group(&self) -> &str {
        self.group.as_deref().unwrap_or(DEFAULT_GROUP)
    }
}

/// Knobs shared by the commands that touch the filesystem.
#[derive(Debug, Clone, Args)]
pub struct FileOptions {
    /// Path of the rules file to write or remove
    #[arg(
        short,
        long,
        default_value = DEFAULT_RULES_PATH,
        value_name = "PATH",
        env = "DONGURU_UDEV_RULES"
    )]
    pub path: PathBuf,

    /// Show what would happen without touching the filesystem
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Do not run `udevadm control --reload-rules && udevadm trigger` afterwards
    ///
    /// Without a reload the change only takes effect after the dongle is replugged
    /// or the machine is rebooted.
    #[arg(long)]
    pub no_reload: bool,
}

#[derive(Debug, Subcommand)]
pub enum Udev {
    /// Write the rules to stdout for inspection
    ///
    /// Nothing is installed; redirect the output yourself to review or hand-edit it:
    ///
    ///   donguru udev generate > 60-donguru.rules
    ///   sudo install -m 644 60-donguru.rules /etc/udev/rules.d/
    #[command(alias = "show", verbatim_doc_comment)]
    Generate {
        #[command(flatten)]
        rule: RuleOptions,

        /// Write to a file instead of stdout
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },

    /// Install the rules and reload udev
    ///
    /// Writes the rules file and reloads udev so the change applies to an already
    /// connected dongle. Requires root, so run it under `sudo` or use `generate` and
    /// install the file yourself.
    Install {
        #[command(flatten)]
        rule: RuleOptions,

        #[command(flatten)]
        file: FileOptions,

        /// Overwrite an existing rules file
        #[arg(long)]
        force: bool,
    },

    /// Remove previously installed rules and reload udev
    ///
    /// Requires root, same as `install`.
    #[command(alias = "remove")]
    Uninstall {
        #[command(flatten)]
        file: FileOptions,
    },

    /// Report whether the rules are installed and the device is accessible
    ///
    /// Useful when a command fails with a permission error: it reports the installed
    /// rule, the owner/mode of the device node and whether the current user matches.
    Status {
        /// Path of the rules file to look for
        #[arg(
            short,
            long,
            default_value = DEFAULT_RULES_PATH,
            value_name = "PATH",
            env = "DONGURU_UDEV_RULES"
        )]
        path: PathBuf,
    },
}

impl RuleOptions {
    fn access(&self) -> udev::Access {
        match self.access {
            Access::Uaccess => udev::Access::Uaccess,
            Access::Group => udev::Access::Group(self.group().to_string()),
            Access::All => udev::Access::All,
        }
    }

    /// The `--access` / `--group` arguments that give `self` back, for the root re-run.
    fn args(&self) -> Vec<String> {
        let mut args = vec![
            "--access".to_string(),
            match self.access {
                Access::Uaccess => "uaccess",
                Access::Group => "group",
                Access::All => "all",
            }
            .to_string(),
        ];
        if matches!(self.access, Access::Group) {
            args.extend(["--group".to_string(), self.group().to_string()]);
        }
        args
    }
}

pub fn run(ctx: &Context, command: Udev) -> anyhow::Result<ExitCode> {
    match command {
        Udev::Generate { rule, output } => generate(&rule, output.as_deref()),
        Udev::Install { rule, file, force } => install(&rule, &file, force),
        Udev::Uninstall { file } => uninstall(&file),
        Udev::Status { path } => status(ctx, &path),
    }
}

fn generate(rule: &RuleOptions, output: Option<&Path>) -> anyhow::Result<ExitCode> {
    let text = udev::generate(&rule.access());
    match output {
        Some(path) => std::fs::write(path, text)?,
        None => print!("{text}"),
    }
    Ok(ExitCode::Success)
}

/// `pkexec` on `PATH`, to get root for the one write.
fn find_pkexec() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join("pkexec"))
        .find(|p| p.is_file())
}

/// Runs this same command as root through `pkexec` (one password prompt), or tells what to run when there is no
/// `pkexec`. The exit status of the root run is ours.
fn as_root(args: &[String], what: &str, path: &Path) -> anyhow::Result<ExitCode> {
    let Some(pkexec) = find_pkexec() else {
        eprintln!(
            "donguru: {what} {} needs root and `pkexec` is not installed; run it as root yourself:\n  sudo {} {}",
            path.display(),
            std::env::current_exe()?.display(),
            args.join(" ")
        );
        return Ok(ExitCode::Failure);
    };
    eprintln!(
        "donguru: {what} {} needs root, asking through pkexec",
        path.display()
    );
    let status = std::process::Command::new(pkexec)
        .arg(std::env::current_exe()?)
        .args(args)
        .status()?;
    Ok(if status.success() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
}

/// The arguments of `donguru udev <verb>` for `file`, as the root re-run needs them.
fn file_args(verb: &str, file: &FileOptions) -> Vec<String> {
    let mut args = vec![
        "udev".to_string(),
        verb.to_string(),
        "--path".to_string(),
        file.path.display().to_string(),
    ];
    if file.no_reload {
        args.push("--no-reload".to_string());
    }
    args
}

/// Tells udev about the change so it applies to a dongle that is already plugged in.
fn reload() -> anyhow::Result<()> {
    let run = |args: &[&str]| -> anyhow::Result<()> {
        let status = std::process::Command::new("udevadm").args(args).status()?;
        anyhow::ensure!(status.success(), "`udevadm {}` failed", args.join(" "));
        Ok(())
    };
    run(&["control", "--reload-rules"])?;
    run(&["trigger", "--subsystem-match=usb", "--action=add"])
}

fn install(rule: &RuleOptions, file: &FileOptions, force: bool) -> anyhow::Result<ExitCode> {
    let text = udev::generate(&rule.access());
    if file.dry_run {
        println!(
            "would write {} ({}):\n{text}",
            file.path.display(),
            rule.access().name()
        );
        if !file.no_reload {
            println!("would reload udev rules and re-trigger usb devices");
        }
        return Ok(ExitCode::Success);
    }
    match udev::install(&file.path, &text, force) {
        Ok(done) => {
            eprintln!("donguru: {done:?}: {}", file.path.display());
            if !matches!(done, udev::Installed::Unchanged) && !file.no_reload {
                reload()?;
            }
            Ok(ExitCode::Success)
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let mut args = file_args("install", file);
            args.extend(rule.args());
            if force {
                args.push("--force".to_string());
            }
            as_root(&args, "installing", &file.path)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!("donguru: {e}; `--force` overwrites it");
            Ok(ExitCode::Failure)
        }
        Err(e) => Err(e.into()),
    }
}

fn uninstall(file: &FileOptions) -> anyhow::Result<ExitCode> {
    if file.dry_run {
        println!("would remove {}", file.path.display());
        return Ok(ExitCode::Success);
    }
    match udev::remove(&file.path) {
        Ok(removed) => {
            if removed {
                eprintln!("donguru: removed {}", file.path.display());
                if !file.no_reload {
                    reload()?;
                }
            } else {
                eprintln!("donguru: {} is not installed", file.path.display());
            }
            Ok(ExitCode::Success)
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            as_root(&file_args("uninstall", file), "removing", &file.path)
        }
        Err(e) => Err(e.into()),
    }
}

fn describe(status: &udev::Status) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if status.rules.is_empty() {
        let _ = writeln!(out, "rules: not installed (`donguru udev install`)");
    }
    for r in &status.rules {
        let policy = r.access.as_ref().map_or("custom", |a| a.name());
        let state = if r.current {
            ""
        } else {
            ", differs from this version's rule"
        };
        let _ = writeln!(out, "rules: {} ({policy}{state})", r.path.display());
    }
    if status.nodes.is_empty() {
        let _ = writeln!(out, "no dongle connected");
    }
    for n in &status.nodes {
        let mode = n.mode.map_or("?".to_string(), |m| format!("{m:04o}"));
        let ids = match (n.uid, n.gid) {
            (Some(u), Some(g)) => format!("uid {u} gid {g}"),
            _ => "owner unknown".to_string(),
        };
        let _ = writeln!(
            out,
            "dongle {}: {} mode {mode} {ids}, {}",
            n.usb_path,
            n.node.display(),
            if n.accessible {
                "accessible"
            } else {
                "NOT accessible to this user"
            }
        );
    }
    out
}

fn status_json(status: &udev::Status) -> serde_json::Value {
    serde_json::json!({
        "ok": status.ok(),
        "rules": status.rules.iter().map(|r| serde_json::json!({
            "path": r.path,
            "access": r.access.as_ref().map(|a| a.name()),
            "current": r.current,
        })).collect::<Vec<_>>(),
        "devices": status.nodes.iter().map(|n| serde_json::json!({
            "usb_path": n.usb_path,
            "node": n.node,
            "mode": n.mode,
            "uid": n.uid,
            "gid": n.gid,
            "accessible": n.accessible,
        })).collect::<Vec<_>>(),
    })
}

fn status(ctx: &Context, path: &Path) -> anyhow::Result<ExitCode> {
    let status = udev::status_here(path);
    match ctx.format {
        Format::Text => print!("{}", describe(&status)),
        Format::Json | Format::Jsonl => println!("{}", status_json(&status)),
    }
    Ok(if status.ok() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_rerun_args_round_trip() {
        let rule = RuleOptions {
            access: Access::Group,
            group: Some("dialout".into()),
        };
        assert_eq!(rule.args(), ["--access", "group", "--group", "dialout"]);
        assert_eq!(rule.access(), udev::Access::Group("dialout".into()));
        let file = FileOptions {
            path: "/x/60-donguru.rules".into(),
            dry_run: false,
            no_reload: true,
        };
        assert_eq!(
            file_args("install", &file),
            [
                "udev",
                "install",
                "--path",
                "/x/60-donguru.rules",
                "--no-reload"
            ]
        );
    }

    #[test]
    fn status_text_and_json_without_anything() {
        let none = udev::Status {
            rules: vec![],
            nodes: vec![],
        };
        assert!(describe(&none).contains("not installed"));
        assert_eq!(status_json(&none)["ok"], false);
    }
}
