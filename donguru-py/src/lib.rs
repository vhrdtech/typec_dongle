//! Python bindings of donguru-core (CORE-4): the `donguru` package, a blocking API with SI values as floats (V, A,
//! W, s, C, J). Every call runs on one tokio runtime of the process with the GIL released; logic stays in
//! donguru-core, this crate only converts arguments and results.
//!
//! ```python
//! import donguru
//! dg = donguru.open()            # the only dongle, or by label, serial, index or USB path
//! dg.power.on()
//! for r in dg.meter.watch(interval=0.5, count=10):
//!     print(r.voltage, r.current)
//! ```

use std::sync::{OnceLock, RwLock};

use donguru_core::{Dongle, Selector};
use pyo3::prelude::*;

mod errors;
mod levels;
#[cfg(feature = "mock")]
mod mock;
mod values;

use errors::to_py;
use values::{DeviceInfo, Info};

/// The runtime every call blocks on: the WireWeaver client runs its event loop there, so it lives as long as the
/// process.
fn runtime() -> PyResult<&'static tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("donguru")
        .enable_all()
        .build()
        .map_err(|e| errors::LinkError::new_err(format!("starting the async runtime: {e}")))?;
    Ok(RUNTIME.get_or_init(|| rt))
}

/// Runs `f` to completion with the GIL released.
fn block_on<T: Send>(
    py: Python<'_>,
    f: impl Future<Output = donguru_core::Result<T>> + Send,
) -> PyResult<T> {
    let rt = runtime()?;
    py.detach(|| rt.block_on(f)).map_err(to_py)
}

/// An open dongle, shared by the level objects (`dg.power`, `dg.meter`, ...); `None` once closed.
pub(crate) struct Shared {
    dongle: RwLock<Option<Dongle>>,
}

impl Shared {
    /// Runs `f` on the open dongle with the GIL released.
    pub(crate) fn run<T: Send>(
        &self,
        py: Python<'_>,
        f: impl AsyncFnOnce(&Dongle) -> donguru_core::Result<T> + Send,
    ) -> PyResult<T> {
        let rt = runtime()?;
        py.detach(|| {
            let guard = self.dongle.read().unwrap_or_else(|e| e.into_inner());
            let Some(dongle) = guard.as_ref() else {
                return Err(errors::closed());
            };
            rt.block_on(f(dongle)).map_err(to_py)
        })
    }
}

/// Connected dongles, sorted by USB path, without opening them.
#[pyfunction]
fn list(py: Python<'_>) -> PyResult<Vec<DeviceInfo>> {
    let entries = block_on(py, donguru_core::device::list())?;
    Ok(entries.iter().map(DeviceInfo::from).collect())
}

/// What `open(selector)` takes: an index (`int`), or a string as `dg --device` does.
fn selector(sel: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Selector>> {
    let Some(sel) = sel else { return Ok(None) };
    if sel.is_none() {
        return Ok(None);
    }
    if let Ok(index) = sel.extract::<usize>() {
        return Ok(Some(Selector::Index(index)));
    }
    let s: String = sel.extract()?;
    s.parse::<Selector>().map(Some).map_err(to_py)
}

/// Opens a dongle: the only one connected, or the one `selector` picks: an index into `list()`, a serial (or a
/// unique prefix of it), a USB path (`3-1.2`), a label (`"bench-1"`), or a prefixed `index:`, `serial:`, `path:`,
/// `label:` form.
#[pyfunction]
#[pyo3(signature = (selector=None))]
fn open(py: Python<'_>, selector: Option<&Bound<'_, PyAny>>) -> PyResult<PyDongle> {
    let selector = self::selector(selector)?;
    let dongle = block_on(py, async move { Dongle::open(selector.as_ref()).await })?;
    Ok(PyDongle::new(dongle))
}

/// A connected Donguru: the levels as attributes (`power`, `meter`, `usb`, `header`, `uart`, `settings`, `leds`).
/// Closes on `close()`, at the end of a `with` block, or when garbage-collected.
#[pyclass(name = "Dongle", module = "donguru", frozen)]
pub(crate) struct PyDongle {
    shared: std::sync::Arc<Shared>,
    entry: DeviceInfo,
}

impl PyDongle {
    pub(crate) fn new(dongle: Dongle) -> Self {
        let entry = DeviceInfo::from(dongle.entry());
        PyDongle {
            shared: std::sync::Arc::new(Shared {
                dongle: RwLock::new(Some(dongle)),
            }),
            entry,
        }
    }
}

#[pymethods]
impl PyDongle {
    /// Where it was found: serial, USB path, label as listed.
    #[getter]
    fn entry(&self) -> DeviceInfo {
        self.entry.clone()
    }

    /// The USB serial number (the chip UID in hex).
    #[getter]
    fn serial(&self) -> String {
        self.entry.serial.clone()
    }

    /// Variant, board, firmware, UID, label and last reset in one read.
    fn info(&self, py: Python<'_>) -> PyResult<Info> {
        let summary = self
            .shared
            .run(py, async |d| d.device().summary().await)?;
        Ok(Info::from(&summary))
    }

    /// Firmware name and version, e.g. `"donguru 0.20.27"`; `None` when the firmware doesn't report it.
    #[getter]
    fn firmware_version(&self, py: Python<'_>) -> PyResult<Option<String>> {
        Ok(self.info(py)?.firmware)
    }

    /// The user label kept on the dongle (at most 32 bytes); `open("label")` finds it by this.
    #[getter]
    fn label(&self, py: Python<'_>) -> PyResult<String> {
        self.shared.run(py, async |d| d.device().label().await)
    }

    #[setter]
    fn set_label(&self, py: Python<'_>, label: String) -> PyResult<()> {
        self.shared
            .run(py, async move |d| d.device().set_label(&label).await)
    }

    /// Plug VBUS to the DUT (switch U601).
    #[getter]
    fn power(&self) -> levels::Power {
        levels::Power::new(&self.shared)
    }

    /// The power meter on the receptacle VBUS.
    #[getter]
    fn meter(&self) -> levels::Meter {
        levels::Meter::new(&self.shared)
    }

    /// USB data (D+/D-) to the DUT (switch U701).
    #[getter]
    fn usb(&self) -> levels::Usb {
        levels::Usb::new(&self.shared)
    }

    /// Header J101: its switched VBUS_SW and the relay.
    #[getter]
    fn header(&self) -> levels::Header {
        levels::Header::new(&self.shared)
    }

    /// The UARTs: `uart.plug` (SBU) and `uart.header` (IO7/IO8).
    #[getter]
    fn uart(&self) -> levels::Uarts {
        levels::Uarts::new(&self.shared)
    }

    /// Settings kept in flash.
    #[getter]
    fn settings(&self) -> levels::Settings {
        levels::Settings::new(&self.shared)
    }

    /// The LEDs.
    #[getter]
    fn leds(&self) -> levels::Leds {
        levels::Leds::new(&self.shared)
    }

    /// Closes the link; the object can't be used afterwards. Closing twice is fine.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let dongle = {
            let mut guard = self.shared.dongle.write().unwrap_or_else(|e| e.into_inner());
            guard.take()
        };
        match dongle {
            Some(d) => block_on(py, d.close()),
            None => Ok(()),
        }
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (*_args))]
    fn __exit__(&self, py: Python<'_>, _args: &Bound<'_, pyo3::types::PyTuple>) -> PyResult<bool> {
        self.close(py)?;
        Ok(false)
    }

    fn __repr__(&self) -> String {
        format!(
            "Dongle(serial={:?}, path={:?}, label={:?})",
            self.entry.serial, self.entry.path, self.entry.label
        )
    }
}

#[pymodule]
fn _donguru(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(list, m)?)?;
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_class::<PyDongle>()?;
    values::register(m)?;
    levels::register(m)?;
    errors::register(m)?;
    #[cfg(feature = "mock")]
    mock::register(m)?;
    Ok(())
}
