//! Typed error categories for the core library.
//!
//! Users like the CLI tools, mapping these variants onto stable exit codes (see `donguru-cli/src/exit.rs`),
//! so scripts can branch on the failure category.

use std::any::Any;
use std::fmt;

use wire_weaver::ww_error::{CommonErrorOwned, HasCommon};
use wire_weaver_client::CallError;

/// Convenience alias
pub type Result<T> = std::result::Result<T, Error>;

/// Failure categories reported by the core library.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No dongle matched the selector, or the selection was ambiguous.
    #[error("no matching device: {0}")]
    NoDevice(String),

    /// The dongle answered with an error of the item (`Refused`, `InUse`, `OverVoltage`, ...).
    #[error("{item}: {error}")]
    Device {
        /// API item, e.g. `Plug::vbus_mode`.
        item: String,
        error: DeviceError,
    },

    /// The dongle's firmware doesn't implement the item (yet): it answered `Unimplemented`, or its API doesn't have
    /// it.
    #[error("{item} is not implemented by the device's firmware")]
    Unimplemented { item: String },

    /// An operation did not complete in time.
    #[error("timed out: {0}")]
    Timeout(String),

    /// The firmware speaks an API this host can't talk to (`donguru_api` major.minor differ).
    #[error("incompatible firmware: {0}")]
    Incompatible(String),

    /// The link to the dongle failed: USB, disconnect, protocol.
    #[error("link to the device failed: {0}")]
    Link(String),
}

/// An error the dongle answered with: its category and the typed error of the API.
pub struct DeviceError {
    /// The category, from the `CommonError` part of the item's error.
    pub kind: DeviceErrorKind,
    message: String,
    /// The owned error type of the item (`PowerErrorOwned`, `CommonErrorOwned`, `ww_uart::UartErrorOwned`, ...).
    typed: Box<dyn Any + Send + Sync>,
}

impl DeviceError {
    /// The item's own error type, e.g. `err.typed::<donguru_api::PowerErrorOwned>()` for `OverVoltage { measured }`.
    pub fn typed<E: 'static>(&self) -> Option<&E> {
        self.typed.downcast_ref()
    }
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl fmt::Debug for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceError")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .finish()
    }
}

/// Category of a [DeviceError]: the `CommonError` variants, or [Specific](DeviceErrorKind::Specific) for an item's
/// own variant (`PowerError::OverVoltage`, `DebugError::Nack`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceErrorKind {
    Unsupported,
    InvalidArgument,
    InUse,
    Busy,
    WrongState,
    NoPower,
    Refused,
    Timeout,
    Fault,
    /// A variant of the item's own error type, see [DeviceError::typed].
    Specific,
    /// A common error from a newer firmware this host doesn't know.
    Other,
}

impl DeviceErrorKind {
    /// Stable snake_case name, e.g. `in_use`, `refused`.
    pub const fn name(self) -> &'static str {
        match self {
            DeviceErrorKind::Unsupported => "unsupported",
            DeviceErrorKind::InvalidArgument => "invalid_argument",
            DeviceErrorKind::InUse => "in_use",
            DeviceErrorKind::Busy => "busy",
            DeviceErrorKind::WrongState => "wrong_state",
            DeviceErrorKind::NoPower => "no_power",
            DeviceErrorKind::Refused => "refused",
            DeviceErrorKind::Timeout => "timeout",
            DeviceErrorKind::Fault => "fault",
            DeviceErrorKind::Specific => "specific",
            DeviceErrorKind::Other => "other",
        }
    }

    fn of(common: Option<&CommonErrorOwned>) -> Self {
        match common {
            None => DeviceErrorKind::Specific,
            Some(CommonErrorOwned::Unsupported) => DeviceErrorKind::Unsupported,
            Some(CommonErrorOwned::InvalidArgument { .. }) => DeviceErrorKind::InvalidArgument,
            Some(CommonErrorOwned::InUse { .. }) => DeviceErrorKind::InUse,
            Some(CommonErrorOwned::Busy) => DeviceErrorKind::Busy,
            Some(CommonErrorOwned::WrongState { .. }) => DeviceErrorKind::WrongState,
            Some(CommonErrorOwned::NoPower { .. }) => DeviceErrorKind::NoPower,
            Some(CommonErrorOwned::Refused { .. }) => DeviceErrorKind::Refused,
            Some(CommonErrorOwned::Timeout) => DeviceErrorKind::Timeout,
            Some(CommonErrorOwned::Fault { .. }) => DeviceErrorKind::Fault,
            #[allow(unreachable_patterns)] // future variants of CommonError
            Some(_) => DeviceErrorKind::Other,
        }
    }
}

impl Error {
    /// The firmware has no implementation of the item.
    pub fn is_unimplemented(&self) -> bool {
        matches!(self, Error::Unimplemented { .. })
    }

    /// A plain-language note for a `CommonError::Fault` from the settings store (API-19): the firmware's `code`
    /// says why the write (or `settings.reset`) didn't stick. `None` for any other error, or a code this host
    /// doesn't know the meaning of (still shown via the error's own `Display`, "Hardware fault {code:#x}").
    pub fn fault_note(&self) -> Option<&'static str> {
        let Error::Device { error, .. } = self else {
            return None;
        };
        let CommonErrorOwned::Fault { code } = error.typed::<CommonErrorOwned>()? else {
            return None;
        };
        Some(match *code {
            1 => "the flash write failed; the value was not saved",
            2 => {
                "a newer firmware's settings record is stored in flash; `settings reset` overwrites it"
            }
            3 => "the settings record is too big for the flash store",
            _ => return None,
        })
    }

    /// The device's error category, if the device answered with an error.
    pub fn device_kind(&self) -> Option<DeviceErrorKind> {
        match self {
            Error::Device { error, .. } => Some(error.kind),
            _ => None,
        }
    }

    /// A stable snake_case name of what the dongle answered: the [DeviceErrorKind] of a device error,
    /// `unimplemented` or `incompatible`; `None` for an error that isn't the dongle's answer (no device, timeout,
    /// link). Scripts see it as `kind` in the CLI's json errors.
    pub fn kind_name(&self) -> Option<&'static str> {
        match self {
            Error::Device { error, .. } => Some(error.kind.name()),
            Error::Unimplemented { .. } => Some("unimplemented"),
            Error::Incompatible(_) => Some("incompatible"),
            _ => None,
        }
    }

    /// From an error of the client for `item`, an item without an error type.
    pub(crate) fn client(item: &str, e: wire_weaver_client::Error) -> Self {
        use wire_weaver_client::Error as E;
        if e.is_unimplemented() {
            return Error::Unimplemented { item: item.into() };
        }
        match e {
            E::Timeout | E::LinkSetupTimeout | E::NoPingFromDevice => {
                Error::Timeout(format!("{item}: {e}"))
            }
            E::DeviceNotFound { .. } | E::AmbiguousDeviceChoice(_) => {
                Error::NoDevice(e.to_string())
            }
            E::IncompatibleDeviceProtocol
            | E::OlderProtocol(..)
            | E::IncompatibleResource { .. } => Error::Incompatible(e.to_string()),
            e => Error::Link(format!("{item}: {e}")),
        }
    }

    /// From a failed connect to the device at `location`.
    pub(crate) fn connect(location: &str, e: anyhow::Error) -> Self {
        match e.downcast::<wire_weaver_client::Error>() {
            Ok(e) => Error::client(&format!("connecting to {location}"), e),
            Err(e) => Error::Link(format!("connecting to {location}: {e:#}")),
        }
    }

    /// From a [CallError] of `item`, an item with an error type.
    pub(crate) fn call<E>(item: &str, e: CallError<E>) -> Self
    where
        E: HasCommon<Common = CommonErrorOwned> + fmt::Display + Send + Sync + 'static,
    {
        match e {
            CallError::Device(e) => Error::Device {
                item: item.into(),
                error: DeviceError {
                    kind: DeviceErrorKind::of(e.common()),
                    message: e.to_string(),
                    typed: Box::new(e),
                },
            },
            CallError::Unimplemented { .. } => Error::Unimplemented { item: item.into() },
            CallError::Client(e) => Error::client(item, e),
        }
    }
}
