//! `donguru udev` - manage the Linux udev rules that grant access to a dongle.
//!
//! Without a matching rule the character device is owned by `root`, so every command
//! that talks to the hardware fails with a permission error unless run via `sudo`.

use std::path::PathBuf;

use crate::{context::Context, exit::ExitCode};
use clap::{Args, Subcommand, ValueEnum, builder::ArgPredicate};

/// Default location of the generated rules file.
///
/// The `60-` prefix keeps the rule ahead of the systemd defaults that would otherwise
/// claim the device first.
const DEFAULT_RULES_PATH: &str = "/etc/udev/rules.d/60-donguru.rules";

/// Default group for [`Access::Group`].
const DEFAULT_GROUP: &str = "plugdev";

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

/// Describe the access policy for the stub messages.
fn fmt_access(rule: &RuleOptions) -> String {
    match rule.access {
        Access::Group => format!("group={}", rule.group()),
        other => format!("{other:?}").to_lowercase(),
    }
}

pub fn run(_ctx: &Context, command: Udev) -> anyhow::Result<ExitCode> {
    match command {
        Udev::Generate { rule, output } => {
            let target = output
                .as_deref()
                .map_or_else(|| "-".to_string(), |path| path.display().to_string());
            super::stub(
                &format!("udev generate [{}] -> {target}", fmt_access(&rule)),
                ExitCode::Failure,
            )
        }
        Udev::Install { rule, file, force } => super::stub(
            &format!(
                "udev install [{}] -> {} (dry_run: {}, reload: {}, force: {force})",
                fmt_access(&rule),
                file.path.display(),
                file.dry_run,
                !file.no_reload,
            ),
            ExitCode::Failure,
        ),
        Udev::Uninstall { file } => super::stub(
            &format!(
                "udev uninstall {} (dry_run: {}, reload: {})",
                file.path.display(),
                file.dry_run,
                !file.no_reload,
            ),
            ExitCode::Failure,
        ),
        Udev::Status { path } => super::stub(
            &format!("udev status {}", path.display()),
            ExitCode::Failure,
        ),
    }
}
