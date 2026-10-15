//! Host library for Donguru dongles: enumeration and selection ([device]), the generated `donguru_api` client
//! ([client]) and a typed facade per level of the API tree ([levels]) for the CLI, the GUI and the Python
//! bindings. With the `mock` feature, [mock] serves `DonguruApi` in the same process, as the test double.

pub mod client;
pub mod device;
pub mod error;
pub mod levels;
#[cfg(feature = "mock")]
pub mod mock;
pub mod nets;
pub mod swd_memtest;

pub use device::{DeviceEntry, Dongle, Selector};
pub use donguru_api as api;
pub use error::{DeviceError, DeviceErrorKind, Error, Result};
