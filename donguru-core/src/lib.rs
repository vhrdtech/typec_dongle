//! Host library for Donguru dongles: enumeration and selection ([device]), the generated `donguru_api` client
//! ([client]) and a typed facade per level of the API tree ([levels]) for the CLI, the GUI and the Python
//! bindings. With the `mock` feature, [mock] serves `DonguruApi` in the same process, as the test double.

pub mod client;
pub mod counters;
pub mod device;
pub mod error;
pub mod levels;
pub mod log;
#[cfg(feature = "mock")]
pub mod mock;
pub mod nets;
pub mod swd_memtest;
pub mod update;
pub mod vbus;

pub use device::{DeviceEntry, Dongle, Selector};
pub use donguru_api as api;
pub use error::{DeviceError, DeviceErrorKind, Error, Result};

/// Power into the DUT of a meter reading, in W: `vbus_in` x `current` (the device doesn't send it, it is exactly
/// this product; the U601 drop is not measured).
pub fn reading_power_w(r: &api::MeterReading) -> f64 {
    r.vbus_in.to_f64() * r.current.to_f64()
}
