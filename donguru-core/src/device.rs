//! Device enumeration and selection.
//!
//! A *selector* may be a serial number, an index or a USB path.
//! Selection resolves to exactly one device or fails with [`Error::NoDevice`].

use crate::Result;

/// Details of a specific dongle.
#[derive(Debug, Clone)]
struct DeviceInfo {}

/// High level dongle/donguru api.
pub struct Dongle {
    _dev_info: DeviceInfo,
}

/// A select identified a specific device.
/// This usually is done either by serial, index or usb path.
#[derive(Debug, Clone)]
pub struct Selector(pub String);

/// Find/Enumerate all connected dongles.
pub fn list() -> Result<Vec<Dongle>> {
    todo!("device enumeration is not implemented yet")
}

/// Get a specific device based on a selector.
///
/// <div class="warning">
/// auto-select when exactly one dongle is present!
/// </div>
pub fn select(_selector: Option<&Selector>) -> Result<Dongle> {
    todo!("device selection is not implemented yet")
}
