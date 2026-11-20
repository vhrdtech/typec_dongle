//! Device enumeration and selection (CORE-1).
//!
//! A Donguru is a USB device with VID:PID `c0de:cafe` whose WireWeaver API id (read from the USB descriptors, no
//! open needed) is `donguru_api`; other WireWeaver devices share the VID:PID. A [Selector] picks one of them by
//! index, serial number (the chip UID) or USB path; without one, the only connected dongle is taken.

use std::fmt;
use std::str::FromStr;

use wire_weaver_client::DeviceInfo;

use crate::client::DonguruClient;
use crate::{Error, Result};

/// USB VID:PID of WireWeaver devices, Donguru among them.
pub const USB_VID: u16 = 0xc0de;
pub const USB_PID: u16 = 0xcafe;
/// API crate name a Donguru reports in its USB descriptors.
pub const API_GID: &str = "donguru_api";

/// A connected dongle as seen on USB, before it is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    /// Position in [list], sorted by USB path: stable while nothing is re-plugged.
    pub index: usize,
    /// USB serial number: the STM32 UID in hex.
    pub serial: String,
    /// USB path, `<bus>-<port>.<port>...` as in Linux sysfs, e.g. `3-1.2`.
    pub path: String,
    /// `donguru_api` version the firmware reports.
    pub api_version: Option<String>,
    /// User label from the descriptors, empty if not set.
    pub label: String,
    /// Where it is, as wire_weaver_client says (`usb 3-1.2 c0de:cafe`, `in_process sim/0`).
    pub location: String,
    pub(crate) connect: ConnectTo,
}

/// How to reach an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConnectTo {
    Usb {
        bus_id: String,
        port_chain: Vec<u8>,
    },
    #[cfg(feature = "mock")]
    InProcess(String),
}

impl DeviceEntry {
    /// An entry for a WireWeaver device, `None` if it isn't a Donguru.
    fn from_info(info: &DeviceInfo) -> Option<Self> {
        let usb = info.usb.as_ref()?;
        let api = info.api.as_ref()?;
        if usb.vid != USB_VID || usb.pid != USB_PID || api.gid != API_GID {
            return None;
        }
        Some(DeviceEntry {
            index: 0,
            serial: info.serials.first().cloned().unwrap_or_default(),
            path: usb_path(&usb.bus_id, &usb.port_chain),
            api_version: Some(api.version.to_string()),
            label: info.user_label.clone(),
            location: info.location.clone(),
            connect: ConnectTo::Usb {
                bus_id: usb.bus_id.clone(),
                port_chain: usb.port_chain.clone(),
            },
        })
    }

    /// An in-process device (the mock), registered under `path`.
    #[cfg(feature = "mock")]
    pub fn in_process(path: &str) -> Self {
        DeviceEntry {
            index: 0,
            serial: String::new(),
            path: path.to_string(),
            api_version: None,
            label: String::new(),
            location: format!("in_process {path}"),
            connect: ConnectTo::InProcess(path.to_string()),
        }
    }
}

/// `5-1.2`: the bus without the leading zeros nusb keeps on Linux (`005`), as sysfs and `lsusb -t` write it.
fn usb_path(bus_id: &str, port_chain: &[u8]) -> String {
    let ports: Vec<String> = port_chain.iter().map(|p| p.to_string()).collect();
    format!("{}-{}", strip_zeros(bus_id), ports.join("."))
}

fn strip_zeros(n: &str) -> &str {
    match n.trim_start_matches('0') {
        "" if !n.is_empty() => "0",
        s => s,
    }
}

/// A path as typed (`005-1.2`, `5-1.2`) in the form of [DeviceEntry::path].
fn normalize_path(p: &str) -> String {
    match p.split_once('-') {
        Some((bus, ports)) => format!("{}-{ports}", strip_zeros(bus)),
        None => p.to_string(),
    }
}

/// Picks one dongle: by index in [list], serial number (or a unique prefix of it, any case), USB path or user
/// label (any case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    Index(usize),
    /// A serial, or a unique prefix of one; when no serial matches, a label equal to it (`bench-1`).
    Serial(String),
    Path(String),
    Label(String),
}

impl FromStr for Selector {
    type Err = Error;

    /// `index:N`, `serial:S`, `path:P`, `label:L`, or without a prefix: a number below 1000 is an index,
    /// `<bus>-<ports>` a path, anything else a serial or, when no serial matches, a label.
    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if let Some((kind, value)) = s.split_once(':') {
            return match kind {
                "index" => value
                    .parse()
                    .map(Selector::Index)
                    .map_err(|_| Error::NoDevice(format!("not an index: {value:?}"))),
                "serial" => Ok(Selector::Serial(value.to_string())),
                "path" => Ok(Selector::Path(value.to_string())),
                "label" => Ok(Selector::Label(value.to_string())),
                _ => Err(Error::NoDevice(format!(
                    "unknown selector kind {kind:?}, expected index:, serial:, path: or label:"
                ))),
            };
        }
        if s.is_empty() {
            return Err(Error::NoDevice("empty device selector".into()));
        }
        if s.len() <= 3
            && let Ok(index) = s.parse()
        {
            return Ok(Selector::Index(index));
        }
        if is_usb_path(s) {
            return Ok(Selector::Path(s.to_string()));
        }
        Ok(Selector::Serial(s.to_string()))
    }
}

/// `<bus>-<port>[.<port>...]`, bus and ports decimal.
fn is_usb_path(s: &str) -> bool {
    let Some((bus, ports)) = s.split_once('-') else {
        return false;
    };
    let decimal = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    decimal(bus) && ports.split('.').all(decimal)
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Selector::Index(i) => write!(f, "index:{i}"),
            Selector::Serial(s) => write!(f, "serial:{s}"),
            Selector::Path(p) => write!(f, "path:{p}"),
            Selector::Label(l) => write!(f, "label:{l}"),
        }
    }
}

/// Connected dongles, sorted by USB path, without opening them.
pub async fn list() -> Result<Vec<DeviceEntry>> {
    let devices = wire_weaver_client::list_usb_devices()
        .await
        .map_err(|e| Error::Link(format!("listing USB devices: {e}")))?;
    Ok(entries(&devices))
}

/// The Donguru entries among `devices`, sorted by path and numbered.
fn entries(devices: &[DeviceInfo]) -> Vec<DeviceEntry> {
    let mut entries: Vec<DeviceEntry> = devices.iter().filter_map(DeviceEntry::from_info).collect();
    entries.sort_by_key(|e| path_key(&e.connect));
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.index = index;
    }
    entries
}

fn path_key(c: &ConnectTo) -> (String, Vec<u8>) {
    match c {
        ConnectTo::Usb { bus_id, port_chain } => (bus_id.clone(), port_chain.clone()),
        #[cfg(feature = "mock")]
        ConnectTo::InProcess(path) => (path.clone(), vec![]),
    }
}

/// The entry `selector` picks from `entries`; without a selector the only one.
pub fn select<'e>(
    entries: &'e [DeviceEntry],
    selector: Option<&Selector>,
) -> Result<&'e DeviceEntry> {
    let Some(selector) = selector else {
        return match entries {
            [] => Err(Error::NoDevice("no Donguru connected".into())),
            [one] => Ok(one),
            many => Err(Error::NoDevice(format!(
                "{} Donguru connected, pick one with --device:{}",
                many.len(),
                describe(many)
            ))),
        };
    };
    let found: Vec<&DeviceEntry> = match selector {
        Selector::Index(i) => entries.iter().filter(|e| e.index == *i).collect(),
        Selector::Path(p) => {
            let p = normalize_path(p);
            entries.iter().filter(|e| e.path == p).collect()
        }
        Selector::Serial(s) => {
            let exact: Vec<_> = entries
                .iter()
                .filter(|e| e.serial.eq_ignore_ascii_case(s))
                .collect();
            if !exact.is_empty() {
                exact
            } else {
                let upper = s.to_ascii_uppercase();
                let prefixed: Vec<_> = entries
                    .iter()
                    .filter(|e| {
                        !upper.is_empty() && e.serial.to_ascii_uppercase().starts_with(&upper)
                    })
                    .collect();
                if prefixed.is_empty() {
                    by_label(entries, s)
                } else {
                    prefixed
                }
            }
        }
        Selector::Label(l) => by_label(entries, l),
    };
    match found.as_slice() {
        [one] => Ok(one),
        [] if entries.is_empty() => {
            Err(Error::NoDevice(format!("{selector}: no Donguru connected")))
        }
        [] => Err(Error::NoDevice(format!(
            "{selector} matches none of the connected Donguru:{}",
            describe(entries)
        ))),
        many => Err(Error::NoDevice(format!(
            "{selector} matches {} Donguru, give more of the serial:{}",
            many.len(),
            describe(&many.iter().map(|e| (*e).clone()).collect::<Vec<_>>())
        ))),
    }
}

/// Entries whose label is `label`, any case; an empty label matches none.
fn by_label<'e>(entries: &'e [DeviceEntry], label: &str) -> Vec<&'e DeviceEntry> {
    entries
        .iter()
        .filter(|e| !label.is_empty() && e.label.eq_ignore_ascii_case(label))
        .collect()
}

fn describe(entries: &[DeviceEntry]) -> String {
    entries
        .iter()
        .map(|e| {
            let label = if e.label.is_empty() {
                String::new()
            } else {
                format!(" label {:?}", e.label)
            };
            format!("\n  {} {} serial {}{label}", e.index, e.path, e.serial)
        })
        .collect()
}

/// A connected dongle: the levels of the API tree through [Dongle::plug], [Dongle::meter], ... (see
/// [crate::levels]), the generated client through [Dongle::client].
pub struct Dongle {
    pub(crate) client: DonguruClient,
    entry: DeviceEntry,
}

impl Dongle {
    /// Lists, selects and opens a dongle.
    pub async fn open(selector: Option<&Selector>) -> Result<Dongle> {
        let entries = list().await?;
        let entry = select(&entries, selector)?;
        Dongle::open_entry(entry.clone()).await
    }

    /// Opens a listed dongle. Checks the firmware's `donguru_api` version: before 1.0 major and minor must match.
    pub async fn open_entry(entry: DeviceEntry) -> Result<Dongle> {
        let connection = match &entry.connect {
            ConnectTo::Usb { bus_id, port_chain } => {
                let (bus_id, port_chain) = (bus_id.clone(), port_chain.clone());
                DonguruClient::config(move |c| c.usb_port_chain(bus_id.clone(), port_chain.clone()))
            }
            #[cfg(feature = "mock")]
            ConnectTo::InProcess(path) => {
                let path = path.clone();
                DonguruClient::config(move |c| c.in_process_path(path.clone()))
            }
        };
        let client = connection
            .connect()
            .await
            .map_err(|e| Error::connect(&entry.location, e))?;
        Ok(Dongle { client, entry })
    }

    /// Where this dongle was found.
    pub fn entry(&self) -> &DeviceEntry {
        &self.entry
    }

    /// The generated client, for what the facade doesn't cover.
    pub fn client(&self) -> &DonguruClient {
        &self.client
    }

    /// Closes the link.
    pub async fn close(self) -> Result<()> {
        self.client
            .disconnect()
            .asynch()
            .await
            .map_err(|e| Error::client("disconnect", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(index: usize, serial: &str, path: &str) -> DeviceEntry {
        DeviceEntry {
            index,
            serial: serial.into(),
            path: path.into(),
            api_version: Some("0.2.0".into()),
            label: String::new(),
            location: format!("usb {path}"),
            connect: ConnectTo::Usb {
                bus_id: "3".into(),
                port_chain: vec![1, index as u8],
            },
        }
    }

    fn two() -> Vec<DeviceEntry> {
        vec![
            entry(0, "0049002A3133510B37363436", "3-1.1"),
            entry(1, "0049002A3133510B99999999", "3-1.2"),
        ]
    }

    #[test]
    fn selector_parsing() {
        assert_eq!("0".parse::<Selector>().unwrap(), Selector::Index(0));
        assert_eq!("12".parse::<Selector>().unwrap(), Selector::Index(12));
        assert_eq!(
            "3-1.2".parse::<Selector>().unwrap(),
            Selector::Path("3-1.2".into())
        );
        assert_eq!(
            "1-4".parse::<Selector>().unwrap(),
            Selector::Path("1-4".into())
        );
        assert_eq!(
            "0049002A3133".parse::<Selector>().unwrap(),
            Selector::Serial("0049002A3133".into())
        );
        assert_eq!(
            "serial:12".parse::<Selector>().unwrap(),
            Selector::Serial("12".into())
        );
        assert_eq!("index:7".parse::<Selector>().unwrap(), Selector::Index(7));
        assert_eq!(
            "path:2-3".parse::<Selector>().unwrap(),
            Selector::Path("2-3".into())
        );
        assert!("index:x".parse::<Selector>().is_err());
        assert!("bogus:x".parse::<Selector>().is_err());
        assert!("".parse::<Selector>().is_err());
        // a long number is a serial, not an index
        assert_eq!(
            "12345".parse::<Selector>().unwrap(),
            Selector::Serial("12345".into())
        );
    }

    #[test]
    fn auto_select_only_when_one() {
        assert!(matches!(select(&[], None), Err(Error::NoDevice(_))));
        let one = vec![entry(0, "AB", "3-1")];
        assert_eq!(select(&one, None).unwrap().serial, "AB");
        let Err(Error::NoDevice(msg)) = select(&two(), None) else {
            panic!("two dongles must not auto-select");
        };
        assert!(msg.contains("--device"), "{msg}");
        assert!(msg.contains("3-1.2"), "{msg}");
    }

    #[test]
    fn select_by_index_path_serial() {
        let e = two();
        assert_eq!(select(&e, Some(&Selector::Index(1))).unwrap().path, "3-1.2");
        assert_eq!(
            select(&e, Some(&Selector::Path("3-1.1".into())))
                .unwrap()
                .index,
            0
        );
        let s = Selector::Serial("0049002a3133510b37363436".into());
        assert_eq!(select(&e, Some(&s)).unwrap().index, 0, "serial in any case");
        let s = Selector::Serial("0049002A3133510B9".into());
        assert_eq!(select(&e, Some(&s)).unwrap().index, 1, "unique prefix");
        let s = Selector::Serial("0049002A".into());
        assert!(matches!(select(&e, Some(&s)), Err(Error::NoDevice(m)) if m.contains("matches 2")));
        assert!(select(&e, Some(&Selector::Index(5))).is_err());
        assert!(select(&e, Some(&Selector::Path("9-9".into()))).is_err());
    }

    #[test]
    fn select_by_label() {
        let mut e = two();
        e[1].label = "bench-1".into();
        assert_eq!(
            "label:bench-1".parse::<Selector>().unwrap(),
            Selector::Label("bench-1".into())
        );
        let l = Selector::Label("Bench-1".into());
        assert_eq!(select(&e, Some(&l)).unwrap().index, 1, "label in any case");
        // without a prefix: a serial first, a label when no serial matches
        let s: Selector = "bench-1".parse().unwrap();
        assert_eq!(select(&e, Some(&s)).unwrap().index, 1);
        e[0].label = "0049002A".into();
        let s = Selector::Serial("0049002A".into());
        assert!(
            matches!(select(&e, Some(&s)), Err(Error::NoDevice(m)) if m.contains("matches 2")),
            "a serial prefix wins over a label"
        );
        assert!(select(&e, Some(&Selector::Label(String::new()))).is_err());
        assert!(select(&e, Some(&Selector::Label("nope".into()))).is_err());
    }

    fn info(vid: u16, gid: &str, serial: &str, ports: Vec<u8>) -> DeviceInfo {
        DeviceInfo {
            location: format!("usb 3-{ports:?}"),
            manufacturer: "vhrd.tech".into(),
            product: "Donguru".into(),
            serials: vec![serial.into()],
            user_label: String::new(),
            api: Some(wire_weaver_client::ApiInfo {
                gid: gid.into(),
                version: "0.2.0".parse().unwrap(),
                signature: wire_weaver_client::ww_version::ApiHashOwned { hash: vec![] },
            }),
            usb: Some(wire_weaver_client::UsbLocation {
                bus_id: "003".into(),
                port_chain: ports,
                vid,
                pid: USB_PID,
            }),
        }
    }

    #[test]
    fn only_donguru_listed_sorted_by_path() {
        let devices = vec![
            info(USB_VID, API_GID, "B", vec![1, 3]),
            info(USB_VID, "report_api", "X", vec![1, 1]),
            info(0x1234, API_GID, "Y", vec![1, 2]),
            info(USB_VID, API_GID, "A", vec![1, 2]),
        ];
        let e = entries(&devices);
        assert_eq!(e.len(), 2);
        assert_eq!(
            (e[0].index, e[0].serial.as_str(), e[0].path.as_str()),
            (0, "A", "3-1.2")
        );
        assert_eq!(
            (e[1].index, e[1].serial.as_str(), e[1].path.as_str()),
            (1, "B", "3-1.3")
        );
        assert_eq!(e[0].api_version.as_deref(), Some("0.2.0"));
    }
}
