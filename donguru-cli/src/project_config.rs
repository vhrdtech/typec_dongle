//! The project config file, `./donguru.toml` (see [docs/design/cli](../../../docs/design/cli/index.md)).
//!
//! Full layering (defaults, system, user, project, env, flags) is CLI-9, still a stub. This piece only persists
//! the `device` key `device select` writes (CLI-7), read back as the lowest-priority fallback for `--device`.

use std::fs;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "donguru.toml";

/// Walks up from `start` looking for `donguru.toml`, the way git finds `.git`.
fn find_from(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

fn device_selector_from(start: &Path) -> Option<String> {
    let path = find_from(start)?;
    let text = fs::read_to_string(path).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    doc.get("device")?.as_str().map(str::to_string)
}

/// The `device` key of the nearest `donguru.toml` above the current directory, if there is one.
pub fn device_selector() -> Option<String> {
    device_selector_from(&std::env::current_dir().ok()?)
}

fn set_device_selector_in(dir: &Path, selector: &str) -> anyhow::Result<PathBuf> {
    let path = dir.join(FILE_NAME);
    let mut doc: toml::Table = match fs::read_to_string(&path) {
        Ok(text) => text.parse()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => toml::Table::new(),
        Err(e) => return Err(e.into()),
    };
    doc.insert(
        "device".to_string(),
        toml::Value::String(selector.to_string()),
    );
    fs::write(&path, toml::to_string_pretty(&doc)?)?;
    Ok(path)
}

/// Writes (or updates) the `device` key of `./donguru.toml`, creating the file if it doesn't exist yet. Always the
/// current directory, not a file found by walking up: `device select` pins a device for the project rooted here.
pub fn set_device_selector(selector: &str) -> anyhow::Result<PathBuf> {
    set_device_selector_in(&std::env::current_dir()?, selector)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_then_reads_back_in_a_temp_project() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();

        let written = set_device_selector_in(dir.path(), "serial:ABC123").unwrap();
        assert_eq!(written, dir.path().join(FILE_NAME));
        assert_eq!(
            device_selector_from(dir.path()),
            Some("serial:ABC123".to_string())
        );
        // found by walking up from a subdirectory too
        assert_eq!(
            device_selector_from(&sub),
            Some("serial:ABC123".to_string())
        );

        // a second select overwrites the key without disturbing the rest of the file
        fs::write(
            dir.path().join(FILE_NAME),
            "answer = 42\ndevice = \"index:0\"\n",
        )
        .unwrap();
        set_device_selector_in(dir.path(), "path:3-1.2").unwrap();
        let text = fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(text.contains("answer = 42"), "{text}");
        assert_eq!(
            device_selector_from(dir.path()),
            Some("path:3-1.2".to_string())
        );
    }

    #[test]
    fn no_file_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(device_selector_from(dir.path()), None);
    }
}
