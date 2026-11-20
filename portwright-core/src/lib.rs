//! Host library for Portwright dongles: enumeration and selection ([device]), the generated `portwright_api` client
//! ([client]) and a typed facade per level of the API tree ([levels]) for the CLI, the GUI and the Python
//! bindings. With the `mock` feature, [mock] serves `PortwrightApi` in the same process, as the test double.

pub mod client;
pub mod config;
pub mod counters;
pub mod crash;
pub mod debug;
pub mod device;
pub mod error;
pub mod levels;
pub mod log;
#[cfg(feature = "mock")]
pub mod mock;
pub mod nets;
pub mod pd;
pub mod swd_memtest;
pub mod udev;
pub mod update;
pub mod vbus;

pub use device::{DeviceEntry, Dongle, Selector};
pub use error::{DeviceError, DeviceErrorKind, Error, Result};
pub use portwright_api as api;
pub use ww_indication;

/// Power into the DUT of a meter reading, in W: `vbus_in` x `current` (the device doesn't send it, it is exactly
/// this product; the U601 drop is not measured).
pub fn reading_power_w(r: &api::MeterReading) -> f64 {
    r.vbus_in.to_f64() * r.current.to_f64()
}
