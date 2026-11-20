//! `donguru.mock.MockDevice`: donguru-core's in-process mock (CORE-3) from Python, so scripts and their tests run
//! without a dongle. It answers with the firmware's rules (VBUS guard, settings, ...); the meter shows what the
//! test sets.

use std::sync::atomic::{AtomicUsize, Ordering};

use donguru_core::api::{DataPath, MeterReading, MeterTotals};
use donguru_core::mock;
use pyo3::prelude::*;
use ww_si::{Ampere, Coulomb, Joule, Prefix, Second, Volt};

use crate::values::{PyVbusMode, milli};
use crate::{PyDongle, block_on};

/// A mock dongle in this process, running until it and the dongles opened on it are garbage-collected. `open()`
/// gives a `Dongle` on it.
#[pyclass(
    name = "MockDevice",
    module = "donguru.mock",
    frozen,
    skip_from_py_object
)]
pub(crate) struct MockDevice {
    inner: mock::MockDevice,
}

impl MockDevice {
    fn with<R: Send + 'static>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&mut mock::MockState) -> R + Send + 'static,
    ) -> PyResult<R> {
        block_on(py, async { Ok(self.inner.with(f).await) })
    }
}

#[pymethods]
impl MockDevice {
    /// Starts a mock dongle in its state at boot; `path` names it (each mock needs its own, one is made up).
    #[new]
    #[pyo3(signature = (path=None))]
    fn py_new(path: Option<String>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = path.unwrap_or_else(|| format!("py/{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        MockDevice {
            inner: mock::MockDevice::start(&path),
        }
    }

    /// A `Dongle` on this mock; it keeps the mock running while it is open.
    fn open(slf: &Bound<'_, Self>) -> PyResult<PyDongle> {
        let dongle = block_on(slf.py(), slf.get().inner.open())?;
        Ok(PyDongle::new(dongle, Some(slf.clone().into_any().unbind())))
    }

    /// What the meter reads from now on: VBUS_IN in V, current in A.
    fn set_reading(&self, py: Python<'_>, voltage: f64, current: f64) -> PyResult<()> {
        let reading = MeterReading {
            vbus_in: Volt::milli(milli(voltage, "voltage")?),
            current: Ampere::milli(milli(current, "current")?),
        };
        self.with(py, move |s| s.reading = reading)
    }

    /// The meter's totals: charge in C, energy in J, peak current in A, over `since` s.
    fn set_totals(
        &self,
        py: Python<'_>,
        charge: f64,
        energy: f64,
        peak_current: f64,
        since: f64,
    ) -> PyResult<()> {
        let totals = MeterTotals {
            charge: Coulomb::new(Prefix::Milli, milli(charge, "charge")?),
            energy: Joule::new(Prefix::Milli, milli(energy, "energy")?),
            peak_current: Ampere::milli(milli(peak_current, "peak_current")?),
            since: Second::milli(milli(since, "since")?),
        };
        self.with(py, move |s| s.totals = totals)
    }

    /// Makes the next settings write (or `reset`) fail as the real store does: code 1 flash failure, 2 a newer
    /// firmware's record, 3 a record too big; `None` clears it.
    #[pyo3(signature = (code=None))]
    fn set_settings_fault(&self, py: Python<'_>, code: Option<u32>) -> PyResult<()> {
        self.with(py, move |s| s.settings_fault = code)
    }

    /// The plug VBUS mode the mock holds.
    #[getter]
    fn plug_vbus_mode(&self, py: Python<'_>) -> PyResult<PyVbusMode> {
        self.with(py, |s| PyVbusMode(s.plug_vbus_mode.clone()))
    }

    /// Plug VBUS switched off and on by `vbus_cycle` so far.
    #[getter]
    fn vbus_cycles(&self, py: Python<'_>) -> PyResult<u32> {
        self.with(py, |s| s.vbus_cycles)
    }

    #[getter]
    fn data_connected(&self, py: Python<'_>) -> PyResult<bool> {
        self.with(py, |s| matches!(s.plug_data, DataPath::Connected))
    }

    #[getter]
    fn relay(&self, py: Python<'_>) -> PyResult<bool> {
        self.with(py, |s| s.relay)
    }

    /// `leds.identify` calls so far.
    #[getter]
    fn identified(&self, py: Python<'_>) -> PyResult<u32> {
        self.with(py, |s| s.identified)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<MockDevice>()
}
