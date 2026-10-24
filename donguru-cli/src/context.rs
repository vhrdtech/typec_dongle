use std::path::{Path, PathBuf};

use crate::cli::{Cli, ColorMode, Format};
use clap::ValueEnum;
use donguru_core::config::{self, Entry, Layer, Settings};

/// Everything a command handler needs beyond its own arguments.
#[derive(Debug)]
#[allow(dead_code)] // scaffolding: fields consumed once handlers are real
pub struct Context {
    /// Device selector: `--device`, `DONGURU_DEVICE`, then the `device` key of the project, user and system
    /// config files (CLI-9).
    pub device: Option<String>,
    /// Selected output format.
    pub format: Format,
    /// Colour policy (applied only when rendering `text` to a TTY).
    pub color: ColorMode,
    /// Verbosity level (`-v` count); `quiet` wins when set.
    pub verbosity: u8,
    pub quiet: bool,
    /// Every configurable key with its effective value and the layer it came from (`config show`).
    pub config: Vec<Entry>,
}

/// The config files that exist, as layers (lowest priority first). A file that cannot be read or has an unknown key
/// or invalid value is skipped and reported through `warn`: `donguru config validate` says more.
pub fn file_layers(
    cwd: &Path,
    xdg_config_home: Option<&Path>,
    home: Option<&Path>,
    mut warn: impl FnMut(String),
) -> Vec<(Layer, Settings)> {
    let mut layers = Vec::new();
    for layer in [Layer::System, Layer::User, Layer::Project] {
        let Some(path) = config::path_of(layer, cwd, xdg_config_home, home) else {
            continue;
        };
        match config::load(&path) {
            Ok(Some(settings)) => layers.push((layer, settings)),
            Ok(None) => {}
            Err(e) => warn(format!("ignoring the {layer}: {e}")),
        }
    }
    layers
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// All layers for this process: files, the environment, then the flags that were typed.
pub fn layers(cli: &Cli, warn: impl FnMut(String)) -> Vec<(Layer, Settings)> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut layers = file_layers(
        &cwd,
        env_path("XDG_CONFIG_HOME").as_deref(),
        env_path("HOME").as_deref(),
        warn,
    );
    layers.push((Layer::Env, Settings::from_env(|k| std::env::var(k).ok())));
    layers.push((Layer::Flags, flag_layer(cli)));
    layers
}

fn flag_layer(cli: &Cli) -> Settings {
    let mut flags = Settings::default();
    if cli.explicit.device
        && let Some(device) = &cli.globals.device
    {
        flags.set("device", device.clone());
    }
    let name = |v: &dyn ValueEnumName| v.name();
    if cli.explicit.format {
        flags.set("output", name(&cli.globals.format));
    }
    if cli.explicit.color {
        flags.set("color", name(&cli.globals.color));
    }
    flags
}

/// The command-line name of a `ValueEnum` variant.
trait ValueEnumName {
    fn name(&self) -> String;
}

impl<T: ValueEnum> ValueEnumName for T {
    fn name(&self) -> String {
        self.to_possible_value()
            .map(|v| v.get_name().to_string())
            .unwrap_or_default()
    }
}

impl Context {
    pub fn from_cli(cli: &Cli) -> Self {
        let quiet = cli.globals.quiet;
        let entries = config::resolve(&layers(cli, |w| {
            if !quiet {
                eprintln!("donguru: warning: {w}");
            }
        }));
        Self::from_entries(cli, entries)
    }

    fn from_entries(cli: &Cli, config: Vec<Entry>) -> Self {
        let value = |key: &str| {
            config
                .iter()
                .find(|e| e.key.name == key)
                .and_then(|e| e.value.clone())
        };
        let device = value("device");
        let format = value("output")
            .and_then(|v| Format::from_str(&v, true).ok())
            .unwrap_or(cli.globals.format);
        let color = value("color")
            .and_then(|v| ColorMode::from_str(&v, true).ok())
            .unwrap_or(cli.globals.color);
        Self {
            device,
            format,
            color,
            verbosity: cli.globals.verbosity,
            quiet: cli.globals.quiet,
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_layered_and_a_broken_one_is_skipped_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let xdg = dir.path().join("xdg");
        std::fs::create_dir_all(xdg.join("donguru")).unwrap();
        std::fs::write(
            xdg.join("donguru").join(config::FILE_NAME),
            "device = \"serial:U\"\noutput = \"json\"\n",
        )
        .unwrap();
        let proj = dir.path().join("proj");
        std::fs::create_dir(&proj).unwrap();
        std::fs::write(
            proj.join(config::FILE_NAME),
            "device = \"serial:P\"\nbogus = 1\n",
        )
        .unwrap();

        let mut warnings = Vec::new();
        let layers = file_layers(&proj, Some(&xdg), None, |w| warnings.push(w));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("unknown key `bogus`"));
        let r = config::resolve(&layers);
        assert_eq!(r[0].value.as_deref(), Some("serial:U"));
        assert_eq!(r[0].layer, Layer::User);
        assert_eq!(r[1].value.as_deref(), Some("json"));
    }
}
