//! `donguru.Error` and its subclasses, one per [donguru_core::Error] category, so scripts catch what they can handle
//! (`except donguru.Timeout:`) the way the CLI maps the same categories to exit codes.

use donguru_core::{DeviceErrorKind, Error};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(donguru, DonguruError, PyException, "Any error of a Donguru call.");
create_exception!(
    donguru,
    NoDevice,
    DonguruError,
    "No dongle matched the selector, or the selection was ambiguous."
);
create_exception!(
    donguru,
    DeviceError,
    DonguruError,
    "The dongle refused the call: `.kind` is the category (\"refused\", \"in_use\", \"wrong_state\", ...), `.item` the API item."
);
create_exception!(
    donguru,
    Unimplemented,
    DonguruError,
    "The dongle's firmware doesn't implement this (yet): update the firmware."
);
create_exception!(
    donguru,
    Timeout,
    DonguruError,
    "The dongle didn't answer in time."
);
create_exception!(
    donguru,
    Incompatible,
    DonguruError,
    "The firmware speaks an API this package can't talk to: update the firmware or the package."
);
create_exception!(
    donguru,
    LinkError,
    DonguruError,
    "The link to the dongle failed: USB, disconnect, protocol."
);

/// `.kind` of a [DeviceError], snake_case of the core's category.
fn kind_name(kind: DeviceErrorKind) -> &'static str {
    match kind {
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
        _ => "other",
    }
}

pub(crate) fn to_py(e: Error) -> PyErr {
    let message = e.to_string();
    match &e {
        Error::NoDevice(_) => NoDevice::new_err(message),
        Error::Device { item, error } => {
            let message = match e.fault_note() {
                Some(note) => format!("{message} ({note})"),
                None => message,
            };
            let err = DeviceError::new_err(message);
            let (item, kind) = (item.clone(), kind_name(error.kind));
            Python::attach(|py| {
                let value = err.value(py);
                // attributes of a fresh exception object: setting them can't fail in a way worth reporting
                _ = value.setattr("item", item);
                _ = value.setattr("kind", kind);
            });
            err
        }
        Error::Unimplemented { .. } => Unimplemented::new_err(message),
        Error::Timeout(_) => Timeout::new_err(message),
        Error::Incompatible(_) => Incompatible::new_err(message),
        _ => LinkError::new_err(message),
    }
}

/// A call on a dongle after `close()`.
pub(crate) fn closed() -> PyErr {
    LinkError::new_err("the dongle was closed")
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("Error", py.get_type::<DonguruError>())?;
    m.add("NoDevice", py.get_type::<NoDevice>())?;
    m.add("DeviceError", py.get_type::<DeviceError>())?;
    m.add("Unimplemented", py.get_type::<Unimplemented>())?;
    m.add("Timeout", py.get_type::<Timeout>())?;
    m.add("Incompatible", py.get_type::<Incompatible>())?;
    m.add("LinkError", py.get_type::<LinkError>())?;
    Ok(())
}
