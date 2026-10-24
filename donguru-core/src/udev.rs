//! Linux udev access rule for the dongle (CLI-8): the text of the rule, writing and removing the rules file, and
//! whether the connected dongles can be opened by the current user.
//!
//! Without a rule the device node is owned by `root` and every command that talks to the hardware fails with a
//! permission error. The rule covers the dongle (`c0de:cafe`) and the STM32 ROM bootloader it reboots into for
//! `fw update` (`0483:df11`, same USB serial).
//!
//! Nothing here needs root by itself: [install] and [remove] fail with `PermissionDenied` on a path the user can't
//! write, and the caller decides how to get root (the CLI re-runs itself through `pkexec`). The filesystem roots of
//! [status] are parameters so it is tested against a fake sysfs.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::device::{USB_PID, USB_VID};

/// Where the rules file goes unless told otherwise. `60-` keeps it ahead of the systemd defaults that would
/// otherwise claim the device first (the `uaccess` tag is handled by `73-seat-late.rules`, group and mode must be set
/// before that).
pub const DEFAULT_RULES_PATH: &str = "/etc/udev/rules.d/60-donguru.rules";

/// Directories a package may own the same rules file in (`status` looks there too).
pub const VENDOR_RULES_DIRS: [&str; 2] = ["/usr/lib/udev/rules.d", "/lib/udev/rules.d"];

/// Group for [Access::Group] unless told otherwise.
pub const DEFAULT_GROUP: &str = "plugdev";

const DFU_VID: u16 = 0x0483;
const DFU_PID: u16 = 0xdf11;

/// How the rule hands out access to the device node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    /// `TAG+="uaccess"`: the user logged in at the seat (systemd-logind).
    Uaccess,
    /// `GROUP="<group>", MODE="0660"`: members of a group.
    Group(String),
    /// `MODE="0666"`: every local user.
    All,
}

impl Access {
    /// Short name for messages and JSON: `uaccess`, `group` or `all`.
    pub fn name(&self) -> &'static str {
        match self {
            Access::Uaccess => "uaccess",
            Access::Group(_) => "group",
            Access::All => "all",
        }
    }
}

/// The rules file for `access`.
pub fn generate(access: &Access) -> String {
    let action = match access {
        Access::Uaccess => "TAG+=\"uaccess\"".to_string(),
        Access::Group(group) => format!("GROUP=\"{group}\", MODE=\"0660\""),
        Access::All => "MODE=\"0666\"".to_string(),
    };
    let line = |vid: u16, pid: u16| {
        format!(
            "SUBSYSTEM==\"usb\", ENV{{DEVTYPE}}==\"usb_device\", ATTR{{idVendor}}==\"{vid:04x}\", ATTR{{idProduct}}==\"{pid:04x}\", {action}\n"
        )
    };
    format!(
        "# Donguru USB-C dongle ({USB_VID:04x}:{USB_PID:04x}) and the STM32 ROM bootloader it reboots into for `donguru fw update` ({DFU_VID:04x}:{DFU_PID:04x}).\n\
         # Written by `donguru udev install` (access: {name}).\n{dongle}{dfu}",
        name = access.name(),
        dongle = line(USB_VID, USB_PID),
        dfu = line(DFU_VID, DFU_PID),
    )
}

/// Which access policy a rules file's text grants, `None` when it isn't a rule this module writes.
pub fn parse_access(text: &str) -> Option<Access> {
    let rule = text.lines().find(|l| l.starts_with("SUBSYSTEM=="))?;
    if rule.contains("TAG+=\"uaccess\"") {
        return Some(Access::Uaccess);
    }
    if let Some(rest) = rule.split("GROUP=\"").nth(1) {
        return Some(Access::Group(rest.split('"').next()?.to_string()));
    }
    rule.contains("MODE=\"0666\"").then_some(Access::All)
}

/// What [install] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    Created,
    Replaced,
    /// The file already has exactly this content.
    Unchanged,
}

/// Writes `rules` to `path`, through a temporary name beside it so udev never reads half a file. An existing file
/// with other content is an `AlreadyExists` error unless `force`.
pub fn install(path: &Path, rules: &str, force: bool) -> io::Result<Installed> {
    let existing = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if existing.as_deref() == Some(rules) {
        return Ok(Installed::Unchanged);
    }
    if existing.is_some() && !force {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} exists with other content", path.display()),
        ));
    }
    let tmp = {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".tmp");
        path.with_file_name(name)
    };
    let mut file = fs::File::create(&tmp)?;
    file.write_all(rules.as_bytes())?;
    file.sync_all()?;
    drop(file);
    set_world_readable(&tmp)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(if existing.is_some() {
        Installed::Replaced
    } else {
        Installed::Created
    })
}

#[cfg(unix)]
fn set_world_readable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn set_world_readable(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Removes the rules file; `false` when there was none.
pub fn remove(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// A rules file found by [status].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleFile {
    pub path: PathBuf,
    /// The policy it grants, `None` for a file this module didn't write (hand-edited).
    pub access: Option<Access>,
    /// The content is exactly what [generate] writes for `access`.
    pub current: bool,
}

/// A connected dongle's device node and who may use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// USB path as in sysfs, `3-1.2`.
    pub usb_path: String,
    /// `/dev/bus/usb/BBB/DDD`.
    pub node: PathBuf,
    /// Permission bits, `0o660`; `None` where the platform has none.
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    /// The current user can open the node for reading and writing (an ACL from `uaccess` counts).
    pub accessible: bool,
}

/// The answer of [status].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub rules: Vec<RuleFile>,
    pub nodes: Vec<Node>,
}

impl Status {
    /// Fine as far as access goes: every connected dongle can be opened, and with none connected, a rule is there.
    pub fn ok(&self) -> bool {
        if self.nodes.is_empty() {
            !self.rules.is_empty()
        } else {
            self.nodes.iter().all(|n| n.accessible)
        }
    }
}

fn rule_file(path: &Path) -> Option<RuleFile> {
    let text = fs::read_to_string(path).ok()?;
    let access = parse_access(&text);
    let current = access.as_ref().is_some_and(|a| generate(a) == text);
    Some(RuleFile {
        path: path.to_path_buf(),
        access,
        current,
    })
}

/// The installed rules (`path`, then the same file name in the vendor directories under `vendor_dirs`) and the
/// connected dongles' nodes, read from `sysfs` (`/sys/bus/usb/devices`) and `dev_root` (`/dev/bus/usb`).
pub fn status(path: &Path, vendor_dirs: &[PathBuf], sysfs: &Path, dev_root: &Path) -> Status {
    let mut rules: Vec<RuleFile> = Vec::new();
    rules.extend(rule_file(path));
    if let Some(name) = path.file_name() {
        for dir in vendor_dirs {
            rules.extend(rule_file(&dir.join(name)));
        }
    }
    Status {
        rules,
        nodes: nodes(sysfs, dev_root),
    }
}

/// [status] on this machine's standard locations.
pub fn status_here(path: &Path) -> Status {
    let vendor: Vec<PathBuf> = VENDOR_RULES_DIRS.iter().map(PathBuf::from).collect();
    status(
        path,
        &vendor,
        Path::new("/sys/bus/usb/devices"),
        Path::new("/dev/bus/usb"),
    )
}

fn read_hex(path: PathBuf) -> Option<u16> {
    u16::from_str_radix(fs::read_to_string(path).ok()?.trim(), 16).ok()
}

fn read_num(path: PathBuf) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn nodes(sysfs: &Path, dev_root: &Path) -> Vec<Node> {
    let Ok(dir) = fs::read_dir(sysfs) else {
        return Vec::new();
    };
    let mut nodes: Vec<Node> = dir
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if read_hex(p.join("idVendor"))? != USB_VID || read_hex(p.join("idProduct"))? != USB_PID
            {
                return None;
            }
            let (bus, dev) = (read_num(p.join("busnum"))?, read_num(p.join("devnum"))?);
            let node = dev_root.join(format!("{bus:03}")).join(format!("{dev:03}"));
            let (mode, uid, gid) = owner(&node);
            let accessible = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&node)
                .is_ok();
            Some(Node {
                usb_path: e.file_name().to_string_lossy().into_owned(),
                node,
                mode,
                uid,
                gid,
                accessible,
            })
        })
        .collect();
    nodes.sort_by(|a, b| a.usb_path.cmp(&b.usb_path));
    nodes
}

#[cfg(unix)]
fn owner(node: &Path) -> (Option<u32>, Option<u32>, Option<u32>) {
    use std::os::unix::fs::MetadataExt;
    match fs::metadata(node) {
        Ok(m) => (Some(m.mode() & 0o7777), Some(m.uid()), Some(m.gid())),
        Err(_) => (None, None, None),
    }
}

#[cfg(not(unix))]
fn owner(_node: &Path) -> (Option<u32>, Option<u32>, Option<u32>) {
    (None, None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_text_per_policy() {
        let uaccess = generate(&Access::Uaccess);
        assert!(
            uaccess
                .contains("ATTR{idVendor}==\"c0de\", ATTR{idProduct}==\"cafe\", TAG+=\"uaccess\"")
        );
        assert!(uaccess.contains("ATTR{idVendor}==\"0483\", ATTR{idProduct}==\"df11\""));
        assert!(
            generate(&Access::Group("dialout".into())).contains("GROUP=\"dialout\", MODE=\"0660\"")
        );
        assert!(generate(&Access::All).contains("MODE=\"0666\""));
        assert!(!generate(&Access::All).contains("uaccess\""));
    }

    #[test]
    fn parse_what_generate_wrote() {
        for access in [
            Access::Uaccess,
            Access::Group("plugdev".into()),
            Access::All,
        ] {
            assert_eq!(parse_access(&generate(&access)), Some(access));
        }
        assert_eq!(parse_access("# nothing\n"), None);
    }

    #[test]
    fn install_replace_remove() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("60-donguru.rules");
        let rules = generate(&Access::Uaccess);
        assert_eq!(install(&path, &rules, false).unwrap(), Installed::Created);
        assert_eq!(fs::read_to_string(&path).unwrap(), rules);
        assert_eq!(install(&path, &rules, false).unwrap(), Installed::Unchanged);

        // other content: refused without force, and the file is untouched
        let all = generate(&Access::All);
        let e = install(&path, &all, false).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), rules);
        assert_eq!(install(&path, &all, true).unwrap(), Installed::Replaced);
        assert_eq!(fs::read_to_string(&path).unwrap(), all);
        // no temporary file is left behind
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);

        assert!(remove(&path).unwrap());
        assert!(!remove(&path).unwrap());
    }

    #[test]
    fn install_into_a_missing_directory_fails_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope/60-donguru.rules");
        assert_eq!(
            install(&path, "x", false).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    fn fake_device(sysfs: &Path, dev: &Path, name: &str, vid: &str, bus: u32, devnum: u32) {
        let d = sysfs.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("idVendor"), format!("{vid}\n")).unwrap();
        fs::write(d.join("idProduct"), "cafe\n").unwrap();
        fs::write(d.join("busnum"), format!("{bus}\n")).unwrap();
        fs::write(d.join("devnum"), format!("{devnum}\n")).unwrap();
        let n = dev.join(format!("{bus:03}"));
        fs::create_dir_all(&n).unwrap();
        fs::write(n.join(format!("{devnum:03}")), "").unwrap();
    }

    #[test]
    fn status_finds_rules_and_nodes() {
        let root = tempfile::tempdir().unwrap();
        let (sysfs, dev) = (root.path().join("sys"), root.path().join("dev"));
        fake_device(&sysfs, &dev, "3-1.2", "c0de", 3, 7);
        fake_device(&sysfs, &dev, "1-4", "1234", 1, 2); // somebody else's device
        let vendor = root.path().join("vendor");
        fs::create_dir_all(&vendor).unwrap();
        let etc = root.path().join("60-donguru.rules");

        let s = status(&etc, std::slice::from_ref(&vendor), &sysfs, &dev);
        assert!(s.rules.is_empty());
        assert_eq!(s.nodes.len(), 1);
        assert_eq!(s.nodes[0].usb_path, "3-1.2");
        assert_eq!(s.nodes[0].node, dev.join("003/007"));
        assert!(s.nodes[0].accessible);
        assert!(s.ok());

        fs::write(&etc, generate(&Access::Group("plugdev".into()))).unwrap();
        fs::write(vendor.join("60-donguru.rules"), "# edited by hand\n").unwrap();
        let s = status(&etc, &[vendor], &sysfs, &dev);
        assert_eq!(s.rules.len(), 2);
        assert_eq!(s.rules[0].access, Some(Access::Group("plugdev".into())));
        assert!(s.rules[0].current);
        assert_eq!(s.rules[1].access, None);
        assert!(!s.rules[1].current);
    }

    #[test]
    fn status_ok_rules() {
        let none = Status {
            rules: vec![],
            nodes: vec![],
        };
        assert!(!none.ok());
        let rule = RuleFile {
            path: "x".into(),
            access: Some(Access::Uaccess),
            current: true,
        };
        assert!(
            Status {
                rules: vec![rule],
                nodes: vec![]
            }
            .ok()
        );
        let node = Node {
            usb_path: "1-1".into(),
            node: "n".into(),
            mode: Some(0o644),
            uid: Some(0),
            gid: Some(0),
            accessible: false,
        };
        assert!(
            !Status {
                rules: vec![],
                nodes: vec![node]
            }
            .ok()
        );
    }
}
