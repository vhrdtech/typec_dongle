//! This crate is responsbile for interacting with a donguru dongle.
//! E.g.: device enumeration/selection, transport abstraction (WireWeaver RPC over USB)
//!       and typed operations for each subsystem (GPIO, USB power, etc.).

pub mod device;
pub mod error;

pub use error::{Error, Result};
