//! Layered configuration (CLI-9): built-in defaults, the system, user and project `donguru.toml`, the `DONGURU_*`
//! environment and the command-line flags, later layers overriding earlier ones key by key.
//!
//! This module only knows files, text and precedence; the CLI feeds it the environment and the flags that were
//! given explicitly, so a command renders the result and the GUI and Python bindings can reuse it.

use std::fmt;
use std::path::{Path, PathBuf};

/// File name of every config file.
pub const FILE_NAME: &str = "donguru.toml";

/// One configurable key: its name, the values it accepts (empty: any text) and its built-in default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub name: &'static str,
    pub help: &'static str,
    pub choices: &'static [&'static str],
    pub default: Option<&'static str>,
    /// The environment variable that sets it.
    pub env: &'static str,
}

/// Every key the CLI reads, in the order `config show` prints them.
pub const KEYS: &[Key] = &[
    Key {
        name: "device",
        help: "Default device selector: a serial number, an index into `device list` or a USB path (`serial:S`, `index:N`, `path:P`)",
        choices: &[],
        default: None,
        env: "DONGURU_DEVICE",
    },
    Key {
        name: "output",
        help: "Output format of the commands",
        choices: &["text", "json", "jsonl"],
        default: Some("text"),
        env: "DONGURU_OUTPUT",
    },
    Key {
        name: "color",
        help: "When to colourise output",
        choices: &["auto", "always", "never"],
        default: Some("auto"),
        env: "DONGURU_COLOR",
    },
];

/// The layer a value was taken from, lowest priority first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Default,
    System,
    User,
    Project,
    Env,
    Flags,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Default => "default",
            Layer::System => "system file",
            Layer::User => "user file",
            Layer::Project => "project file",
            Layer::Env => "environment",
            Layer::Flags => "command line",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{path}: {message}")]
    Invalid { path: String, message: String },
    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

/// The values one layer sets (`None`: the layer says nothing about the key).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    values: Vec<(&'static str, String)>,
}

impl Settings {
    pub fn set(&mut self, key: &'static str, value: impl Into<String>) {
        self.values.retain(|(k, _)| *k != key);
        self.values.push((key, value.into()));
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The settings of the `DONGURU_*` variables, through `lookup` (so tests need no real environment).
    /// Invalid values are left out: the CLI's own parsing reports them.
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let mut settings = Self::default();
        for key in KEYS {
            if let Some(value) = lookup(key.env).filter(|v| !v.is_empty())
                && check(key, &value).is_ok()
            {
                settings.set(key.name, value);
            }
        }
        settings
    }

    /// Parses the text of a config file; an unknown key or a value outside the key's choices is an error.
    pub fn parse(path: &str, text: &str) -> Result<Self, ConfigError> {
        let bad = |message: String| ConfigError::Invalid {
            path: path.to_string(),
            message,
        };
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| {
            bad(e.to_string().trim_end().replace('\n', " "))
        })?;
        let mut settings = Self::default();
        for (name, value) in &table {
            let key = KEYS.iter().find(|k| k.name == name).ok_or_else(|| {
                bad(format!(
                    "unknown key `{name}` (known: {})",
                    KEYS.iter().map(|k| k.name).collect::<Vec<_>>().join(", ")
                ))
            })?;
            let toml::Value::String(value) = value else {
                return Err(bad(format!("`{name}` must be a string")));
            };
            check(key, value).map_err(bad)?;
            settings.set(key.name, value.clone());
        }
        Ok(settings)
    }
}

fn check(key: &Key, value: &str) -> Result<(), String> {
    if key.choices.is_empty() || key.choices.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "`{}` must be one of {}, not `{value}`",
            key.name,
            key.choices.join(", ")
        ))
    }
}

/// One resolved key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: &'static Key,
    pub value: Option<String>,
    pub layer: Layer,
}

/// Merges `layers` (any order; priority is the [`Layer`]) over the built-in defaults, per key.
pub fn resolve(layers: &[(Layer, Settings)]) -> Vec<Entry> {
    KEYS.iter()
        .map(|key| {
            let found = layers
                .iter()
                .filter_map(|(layer, s)| s.get(key.name).map(|v| (*layer, v)))
                .max_by_key(|(layer, _)| *layer);
            match found {
                Some((layer, v)) => Entry {
                    key,
                    value: Some(v.to_string()),
                    layer,
                },
                None => Entry {
                    key,
                    value: key.default.map(str::to_string),
                    layer: Layer::Default,
                },
            }
        })
        .collect()
}

/// The config file of a file layer; the project file is the nearest one above `cwd`, or `cwd/donguru.toml`
/// when there is none (where `config generate --output` would put it).
pub fn path_of(
    layer: Layer,
    cwd: &Path,
    xdg_config_home: Option<&Path>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    match layer {
        Layer::System => Some(PathBuf::from("/etc/donguru").join(FILE_NAME)),
        Layer::User => {
            let base = xdg_config_home
                .filter(|p| p.is_absolute())
                .map(Path::to_path_buf)
                .or_else(|| home.map(|h| h.join(".config")))?;
            Some(base.join("donguru").join(FILE_NAME))
        }
        Layer::Project => Some(find_project(cwd).unwrap_or_else(|| cwd.join(FILE_NAME))),
        _ => None,
    }
}

/// Walks up from `start` looking for `donguru.toml`, the way git finds `.git`.
pub fn find_project(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|d| d.join(FILE_NAME))
        .find(|c| c.is_file())
}

/// Reads and parses a config file; a missing file is `Ok(None)`.
pub fn load(path: &Path) -> Result<Option<Settings>, ConfigError> {
    let shown = path.display().to_string();
    match std::fs::read_to_string(path) {
        Ok(text) => Settings::parse(&shown, &text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Io {
            path: shown,
            source,
        }),
    }
}

/// A documented default config file: every key commented out with its default, so it changes nothing until
/// a line is uncommented, and still shows up to date what can be set.
pub fn generate() -> String {
    let mut out = String::from(
        "# Donguru configuration (donguru config generate).\n\
         #\n\
         # Layers, later ones override earlier ones key by key: built-in defaults, /etc/donguru/donguru.toml,\n\
         # ~/.config/donguru/donguru.toml, ./donguru.toml (found by walking up), DONGURU_* environment\n\
         # variables, command-line flags. `donguru config show --origin` tells which one a value comes from.\n",
    );
    for key in KEYS {
        out.push_str(&format!("\n# {}\n", key.help));
        if !key.choices.is_empty() {
            out.push_str(&format!("# One of: {}.\n", key.choices.join(", ")));
        }
        out.push_str(&format!("# Environment: {}\n", key.env));
        out.push_str(&format!(
            "# {} = \"{}\"\n",
            key.name,
            key.default.unwrap_or("")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(pairs: &[(&'static str, &str)]) -> Settings {
        let mut x = Settings::default();
        for (k, v) in pairs {
            x.set(k, *v);
        }
        x
    }

    #[test]
    fn higher_layers_win_per_key() {
        let layers = vec![
            (Layer::Project, s(&[("device", "serial:A"), ("output", "json")])),
            (Layer::User, s(&[("device", "serial:U"), ("color", "never")])),
            (Layer::Flags, s(&[("output", "jsonl")])),
        ];
        let r = resolve(&layers);
        let get = |n: &str| r.iter().find(|e| e.key.name == n).unwrap().clone();
        assert_eq!(get("device").value.as_deref(), Some("serial:A"));
        assert_eq!(get("device").layer, Layer::Project);
        assert_eq!(get("output").value.as_deref(), Some("jsonl"));
        assert_eq!(get("output").layer, Layer::Flags);
        assert_eq!(get("color").layer, Layer::User);
    }

    #[test]
    fn defaults_fill_the_rest() {
        let r = resolve(&[]);
        assert_eq!(r[0].value, None);
        assert_eq!(r[1].value.as_deref(), Some("text"));
        assert!(r.iter().all(|e| e.layer == Layer::Default));
    }

    #[test]
    fn parse_rejects_unknown_keys_and_bad_values() {
        assert!(Settings::parse("f", "device = \"x\"\noutput = \"json\"").is_ok());
        let e = Settings::parse("f", "colour = \"never\"").unwrap_err().to_string();
        assert!(e.contains("unknown key `colour`") && e.starts_with("f:"), "{e}");
        let e = Settings::parse("f", "output = \"yaml\"").unwrap_err().to_string();
        assert!(e.contains("one of text, json, jsonl"), "{e}");
        assert!(Settings::parse("f", "device = 3").is_err());
        assert!(Settings::parse("f", "device = ").is_err());
    }

    #[test]
    fn env_layer_skips_empty_and_invalid() {
        let env = |k: &str| match k {
            "DONGURU_DEVICE" => Some("serial:E".to_string()),
            "DONGURU_OUTPUT" => Some("yaml".to_string()),
            "DONGURU_COLOR" => Some(String::new()),
            _ => None,
        };
        let e = Settings::from_env(env);
        assert_eq!(e.get("device"), Some("serial:E"));
        assert_eq!(e.get("output"), None);
        assert_eq!(e.get("color"), None);
    }

    #[test]
    fn generated_file_parses_and_sets_nothing() {
        let text = generate();
        assert_eq!(Settings::parse("g", &text).unwrap(), Settings::default());
        assert!(text.contains("# output = \"text\""));
        // uncommenting a line makes it a real setting
        let on = text.replace("# output = \"text\"", "output = \"json\"");
        assert_eq!(Settings::parse("g", &on).unwrap().get("output"), Some("json"));
    }

    #[test]
    fn paths_and_project_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(path_of(Layer::Project, &sub, None, None), Some(sub.join(FILE_NAME)));
        std::fs::write(dir.path().join(FILE_NAME), "").unwrap();
        assert_eq!(
            path_of(Layer::Project, &sub, None, None),
            Some(dir.path().join(FILE_NAME))
        );
        assert_eq!(
            path_of(Layer::User, &sub, Some(Path::new("/x")), Some(Path::new("/h"))),
            Some(PathBuf::from("/x/donguru/donguru.toml"))
        );
        assert_eq!(
            path_of(Layer::User, &sub, None, Some(Path::new("/h"))),
            Some(PathBuf::from("/h/.config/donguru/donguru.toml"))
        );
        assert_eq!(
            path_of(Layer::System, &sub, None, None),
            Some(PathBuf::from("/etc/donguru/donguru.toml"))
        );
        assert!(load(&sub.join("none.toml")).unwrap().is_none());
    }
}
