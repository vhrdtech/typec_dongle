//! External subcommand dispatch (Git/Cargo-style plugins, CLI-4).
//!
//! When the first argument is not a built-in subcommand e.g. `foo`, search `PATH`
//! for an executable named `donguru-foo` and run it, forwarding the remaining
//! arguments verbatim. The resolved context reaches the plugin through the
//! environment: `DONGURU_DEVICE` (the selector, when one is set anywhere in the
//! config layering) and `DONGURU_OUTPUT` (`text`, `json` or `jsonl`), the same
//! variables the CLI itself reads, so a plugin that calls `donguru` again inherits them.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::exit::ExitCode;
use clap::ValueEnum;

const PREFIX: &str = "donguru-";

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Every `donguru-NAME` executable in the directories of `path_var`; the first directory wins for a repeated name.
pub fn list_plugins(path_var: &OsStr) -> BTreeMap<String, PathBuf> {
    let mut found = BTreeMap::new();
    for dir in std::env::split_paths(path_var) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let name = file.strip_prefix(PREFIX).unwrap_or_default();
            let name = name.strip_suffix(".exe").unwrap_or(name);
            if !name.is_empty() && is_executable(&entry.path()) {
                found.entry(name.to_string()).or_insert_with(|| entry.path());
            }
        }
    }
    found
}

/// Edit distance, for the "did you mean" hint.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The names in `candidates` close to `name`.
pub fn near_matches<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    candidates
        .filter(|c| distance(name, c) <= 2 || (name.len() >= 3 && c.starts_with(name)))
        .collect()
}

fn builtin_names() -> Vec<String> {
    use clap::CommandFactory;
    crate::cli::Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .filter(|n| n != "help")
        .collect()
}

/// The "External commands" block of `donguru help`; empty without plugins.
pub fn help_section(path_var: &OsStr) -> String {
    let plugins = list_plugins(path_var);
    if plugins.is_empty() {
        return String::new();
    }
    let mut out = String::from("External commands (donguru-NAME on PATH):\n");
    for (name, path) in plugins {
        out.push_str(&format!("  {name:12} {}\n", path.display()));
    }
    out
}

fn not_found(name: &str, path_var: &OsStr) -> String {
    let plugins = list_plugins(path_var);
    let builtins = builtin_names();
    let candidates = builtins
        .iter()
        .map(String::as_str)
        .chain(plugins.keys().map(String::as_str));
    let near = near_matches(name, candidates);
    let mut msg = format!(
        "no such command `{name}` (no built-in, and no `{PREFIX}{name}` on PATH)"
    );
    if !near.is_empty() {
        msg.push_str(&format!("\n\nDid you mean: {}?", near.join(", ")));
    }
    msg.push_str("\nSee `donguru help` for the commands.");
    msg
}

/// The environment a plugin gets: `(variable, value)`; `None` removes an inherited variable.
pub fn plugin_env(ctx: &Context) -> [(&'static str, Option<String>); 2] {
    let output = ctx
        .format
        .to_possible_value()
        .map(|v| v.get_name().to_string());
    [("DONGURU_DEVICE", ctx.device.clone()), ("DONGURU_OUTPUT", output)]
}

pub fn run(ctx: &Context, args: Vec<OsString>) -> anyhow::Result<ExitCode> {
    let Some((name, rest)) = args.split_first() else {
        anyhow::bail!("empty external subcommand");
    };
    let name = name.to_string_lossy();
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let Some(plugin) = list_plugins(&path_var).remove(name.as_ref()) else {
        eprintln!("donguru: error: {}", not_found(&name, &path_var));
        return Ok(ExitCode::BadArgs);
    };

    let mut command = std::process::Command::new(&plugin);
    command.args(rest);
    for (var, value) in plugin_env(ctx) {
        match value {
            Some(v) => command.env(var, v),
            None => command.env_remove(var),
        };
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // replaces this process: the plugin's exit status and signals are ours
        let err = command.exec();
        anyhow::bail!("cannot run {}: {err}", plugin.display());
    }
    #[cfg(not(unix))]
    {
        let status = command.status()?;
        Ok(match status.code() {
            Some(0) => ExitCode::Success,
            _ => ExitCode::Failure,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn plugin(dir: &Path, file: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(file);
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn finds_executables_first_directory_wins() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        plugin(a.path(), "donguru-flash", 0o755);
        plugin(b.path(), "donguru-flash", 0o755);
        plugin(b.path(), "donguru-bench", 0o755);
        plugin(b.path(), "donguru-notexec", 0o644);
        plugin(b.path(), "other-tool", 0o755);
        let path = std::env::join_paths([a.path(), b.path()]).unwrap();
        let found = list_plugins(&path);
        assert_eq!(found.keys().collect::<Vec<_>>(), ["bench", "flash"]);
        assert_eq!(found["flash"], a.path().join("donguru-flash"));
        let help = help_section(&path);
        assert!(help.starts_with("External commands") && help.contains("bench"), "{help}");
        assert_eq!(help_section(OsStr::new("")), "");
    }

    #[test]
    fn near_matches_hint() {
        let c = ["power", "gpio", "flash"];
        assert_eq!(near_matches("pwoer", c.iter().copied()), ["power"]);
        assert_eq!(near_matches("fla", c.iter().copied()), ["flash"]);
        assert!(near_matches("zzzzzz", c.iter().copied()).is_empty());
        let msg = not_found("pwoer", OsStr::new(""));
        assert!(msg.contains("Did you mean: power?"), "{msg}");
    }

    #[test]
    fn plugin_gets_device_and_output() {
        let ctx = Context {
            device: Some("serial:X".into()),
            format: crate::cli::Format::Jsonl,
            color: crate::cli::ColorMode::Never,
            verbosity: 0,
            quiet: true,
            config: Vec::new(),
        };
        let env = plugin_env(&ctx);
        assert_eq!(env[0], ("DONGURU_DEVICE", Some("serial:X".to_string())));
        assert_eq!(env[1], ("DONGURU_OUTPUT", Some("jsonl".to_string())));
    }
}
