//! The levels of a dongle as Python objects (`dg.power`, `dg.meter`, ...), each a thin layer over the typed facade
//! of `donguru_core::levels`: arguments in SI floats, results as the classes of [crate::values].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use donguru_core::api::{ButtonAction, DataPath, VbusMode};
use donguru_core::client::observe::{
    BatchedOwned, ObserveEvent, Observer, PowerMeterObserve, PowerMeterUpdate, on_change, periodic,
};
use donguru_core::levels::Side;
use donguru_core::vbus;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use ww_si::Second;

use crate::errors::LinkError;
use crate::values::{Deadband, PyVbusMode, Reading, Totals, UartConfig, VbusStatus, milli};
use crate::{Shared, runtime};

/// A duration in s as a [Duration], refused when negative or not finite.
fn duration(s: f64, what: &str) -> PyResult<Duration> {
    Duration::try_from_secs_f64(s)
        .map_err(|_| PyValueError::new_err(format!("{what} must be a duration in s >= 0, not {s}")))
}

/// A duration in s as the firmware's integer-millisecond [Second].
fn seconds(s: f64, what: &str) -> PyResult<Second> {
    duration(s, what)?;
    Ok(Second::milli(milli(s, what)?))
}

/// The mode `on(max_voltage, trip)` asks for: `ForceOn`, or `Auto` with a current limit.
fn on_mode(max_voltage: f64, trip: Option<f64>) -> PyResult<VbusMode> {
    milli(max_voltage, "max_voltage")?;
    Ok(match trip {
        None => vbus::force_on(max_voltage),
        Some(max_current) => {
            milli(max_current, "trip")?;
            milli(max_voltage * max_current, "max_voltage x trip")?;
            vbus::auto(max_voltage, max_current)
        }
    })
}

macro_rules! level {
    ($(#[$doc:meta])* $name:ident, $py:literal) => {
        $(#[$doc])*
        #[pyclass(name = $py, module = "donguru", frozen, skip_from_py_object)]
        pub(crate) struct $name {
            shared: Arc<Shared>,
        }

        impl $name {
            pub(crate) fn new(shared: &Arc<Shared>) -> Self {
                Self { shared: shared.clone() }
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// Plug VBUS to the DUT (switch U601), as `dg power` drives it. `on`, `off` and `cycle` answer the state the switch
    /// ended up in: check `.on` (a trip or a fault is not an exception).
    Power,
    "Power"
);

#[pymethods]
impl Power {
    /// Switches VBUS on: `ForceOn` up to `max_voltage` (V), or with `trip` (A) `Auto`, cut above that current.
    #[pyo3(signature = (max_voltage=vbus::DEFAULT_MAX_VOLTAGE_V, trip=None))]
    fn on(&self, py: Python<'_>, max_voltage: f64, trip: Option<f64>) -> PyResult<VbusStatus> {
        let mode = on_mode(max_voltage, trip)?;
        self.shared.run(py, async move |d| {
            d.plug().set_vbus_mode(mode).await?;
            d.plug().vbus().await.map(VbusStatus::from)
        })
    }

    fn off(&self, py: Python<'_>) -> PyResult<VbusStatus> {
        self.shared.run(py, async |d| {
            d.plug().set_vbus_mode(VbusMode::ForceOff).await?;
            d.plug().vbus().await.map(VbusStatus::from)
        })
    }

    /// Off for `off` s, then back to the mode it was in; `release_sbu` takes the SBU lines to Hi-Z first so nothing
    /// on them back-powers the DUT. Answers once VBUS is back.
    #[pyo3(signature = (off=0.5, release_sbu=false))]
    fn cycle(&self, py: Python<'_>, off: f64, release_sbu: bool) -> PyResult<VbusStatus> {
        let off = seconds(off, "off")?;
        self.shared.run(py, async move |d| {
            d.plug()
                .vbus_cycle(off, release_sbu)
                .await
                .map(VbusStatus::from)
        })
    }

    /// The switch's state now.
    #[getter]
    fn state(&self, py: Python<'_>) -> PyResult<VbusStatus> {
        self.shared
            .run(py, async |d| d.plug().vbus().await.map(VbusStatus::from))
    }

    /// The mode the firmware drives it in.
    #[getter]
    fn mode(&self, py: Python<'_>) -> PyResult<PyVbusMode> {
        self.shared
            .run(py, async |d| d.plug().vbus_mode().await.map(PyVbusMode))
    }

    #[setter]
    fn set_mode(&self, py: Python<'_>, mode: PyVbusMode) -> PyResult<()> {
        self.shared
            .run(py, async move |d| d.plug().set_vbus_mode(mode.0).await)
    }
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// The power meter on the receptacle VBUS: voltage, current to the DUT, power, totals.
    Meter,
    "Meter"
);

#[pymethods]
impl Meter {
    /// One reading, averaged over the meter window.
    fn read(&self, py: Python<'_>) -> PyResult<Reading> {
        self.shared.run(py, async |d| {
            d.meter().reading().await.map(|r| Reading::new(&r, None))
        })
    }

    /// Readings as an iterator: one every `interval` s, or with `on_change` only when the reading moves beyond the
    /// deadband (at most one per `interval`), or with `samples` every ADC sample (~1.6 kHz, in batches at most
    /// `interval` old, each with its device time `t`). Stops after `count` readings, or never; `close()` it (or use
    /// `with`) to stop early. Ctrl-C works while it waits.
    #[pyo3(signature = (interval=0.5, count=None, on_change=false, samples=false))]
    fn watch(
        &self,
        py: Python<'_>,
        interval: f64,
        count: Option<u64>,
        on_change: bool,
        samples: bool,
    ) -> PyResult<MeterWatch> {
        let interval = duration(interval, "interval")?;
        if on_change && samples {
            return Err(PyValueError::new_err(
                "on_change and samples don't go together",
            ));
        }
        let observer = self.shared.run(py, async move |d| {
            if samples {
                d.meter().observe_samples(interval).await
            } else {
                let mode = if on_change {
                    self::on_change(interval)
                } else {
                    periodic(interval)
                };
                d.meter()
                    .observe(PowerMeterObserve::default().reading(mode))
                    .await
            }
        })?;
        Ok(MeterWatch {
            _shared: self.shared.clone(),
            state: Mutex::new(WatchState {
                observer: Some(observer),
                remaining: count,
                pending: VecDeque::new(),
            }),
        })
    }

    /// Charge, energy and peak current since boot or `reset_totals()`.
    fn totals(&self, py: Python<'_>) -> PyResult<Totals> {
        self.shared.run(py, async |d| {
            d.meter().totals().await.map(|t| Totals::from(&t))
        })
    }

    fn reset_totals(&self, py: Python<'_>) -> PyResult<()> {
        self.shared
            .run(py, async |d| d.meter().reset_totals().await)
    }

    /// Board temperature in °C.
    fn temperature(&self, py: Python<'_>) -> PyResult<f64> {
        self.shared.run(py, async |d| {
            d.meter().temperature().await.map(|t| t.to_f64())
        })
    }

    /// Averaging window of a reading, in s (1 ms to 1 s); not kept over a reset, `settings.meter_window` is.
    #[getter]
    fn window(&self, py: Python<'_>) -> PyResult<f64> {
        self.shared
            .run(py, async |d| d.meter().window().await.map(|w| w.to_f64()))
    }

    #[setter]
    fn set_window(&self, py: Python<'_>, window: f64) -> PyResult<()> {
        let window = seconds(window, "window")?;
        self.shared
            .run(py, async move |d| d.meter().set_window(window).await)
    }

    /// What counts as a change for `watch(on_change=True)`.
    #[getter]
    fn deadband(&self, py: Python<'_>) -> PyResult<Deadband> {
        self.shared.run(py, async |d| {
            d.meter().deadband().await.map(|b| Deadband::from(&b))
        })
    }

    #[setter]
    fn set_deadband(&self, py: Python<'_>, deadband: Deadband) -> PyResult<()> {
        let deadband = deadband.to_api()?;
        self.shared
            .run(py, async move |d| d.meter().set_deadband(deadband).await)
    }
}

struct WatchState {
    /// `None` once closed or done.
    observer: Option<Observer<PowerMeterUpdate>>,
    remaining: Option<u64>,
    /// Readings of a batch not handed out yet.
    pending: VecDeque<Reading>,
}

/// How long one wait for an update blocks before Python gets to handle signals (Ctrl-C).
const SIGNAL_CHECK: Duration = Duration::from_millis(100);

/// The iterator of `meter.watch()`.
#[pyclass(module = "donguru", frozen, skip_from_py_object)]
pub(crate) struct MeterWatch {
    /// Keeps the dongle's link alive while observing.
    _shared: Arc<Shared>,
    state: Mutex<WatchState>,
}

impl WatchState {
    /// Waits up to [SIGNAL_CHECK] for the next update and queues its readings. `Ok(false)` when the observer ended.
    fn poll(&mut self, rt: &tokio::runtime::Runtime) -> PyResult<bool> {
        let Some(observer) = self.observer.as_mut() else {
            return Ok(false);
        };
        let event = match rt.block_on(tokio::time::timeout(SIGNAL_CHECK, observer.recv_event())) {
            Err(_elapsed) => return Ok(true),
            Ok(event) => event,
        };
        match event {
            Ok(ObserveEvent::Update(u)) => match u.value.reading {
                Some(BatchedOwned::One(r)) => {
                    self.pending
                        .push_back(Reading::new(&r, u.t.as_ref().map(Second::to_f64)));
                }
                Some(series @ BatchedOwned::Samples(_)) => {
                    let rows = series
                        .into_samples(u.t, u.sample_times.as_deref())
                        .map_err(|e| LinkError::new_err(format!("malformed meter samples: {e}")))?;
                    for (r, t) in rows {
                        self.pending
                            .push_back(Reading::new(&r, t.as_ref().map(Second::to_f64)));
                    }
                }
                _ => {}
            },
            // the link comes back by itself; missed readings are just not there
            Ok(_) => {}
            Err(e) => {
                self.observer = None;
                return Err(LinkError::new_err(format!("meter watch: {e}")));
            }
        }
        Ok(true)
    }
}

#[pymethods]
impl MeterWatch {
    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Reading>> {
        let rt = runtime()?;
        loop {
            let next = py.detach(|| {
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if state.remaining == Some(0) {
                    state.observer = None;
                    return Ok(Some(None));
                }
                if let Some(r) = state.pending.pop_front() {
                    if let Some(n) = state.remaining.as_mut() {
                        *n -= 1;
                    }
                    return Ok(Some(Some(r)));
                }
                if state.poll(rt)? {
                    Ok(None)
                } else {
                    Ok(Some(None))
                }
            });
            match next {
                Ok(Some(reading)) => return Ok(reading),
                Ok(None) => py.check_signals()?,
                Err(e) => return Err(e),
            }
        }
    }

    /// Stops observing.
    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.observer = None;
        state.pending.clear();
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (*_args))]
    fn __exit__(&self, _args: &Bound<'_, pyo3::types::PyTuple>) -> bool {
        self.close();
        false
    }
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// USB data (D+/D-) to the DUT through switch U701, as `dg usb` drives it; VBUS is `power`.
    Usb,
    "Usb"
);

impl Usb {
    fn set(&self, py: Python<'_>, data: DataPath) -> PyResult<bool> {
        self.shared.run(py, async move |d| {
            d.plug().set_data(data).await?;
            Ok(matches!(d.plug().data().await?, DataPath::Connected))
        })
    }
}

#[pymethods]
impl Usb {
    /// USB data reaches the DUT.
    #[getter]
    fn connected(&self, py: Python<'_>) -> PyResult<bool> {
        self.shared.run(py, async |d| {
            Ok(matches!(d.plug().data().await?, DataPath::Connected))
        })
    }

    #[setter]
    fn set_connected(&self, py: Python<'_>, connected: bool) -> PyResult<()> {
        let data = if connected {
            DataPath::Connected
        } else {
            DataPath::Cut
        };
        self.set(py, data).map(|_| ())
    }

    /// Connects USB data; answers `connected` as read back.
    fn connect(&self, py: Python<'_>) -> PyResult<bool> {
        self.set(py, DataPath::Connected)
    }

    /// Cuts USB data without touching VBUS; answers `connected` as read back.
    fn disconnect(&self, py: Python<'_>) -> PyResult<bool> {
        self.set(py, DataPath::Cut)
    }
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// Header J101: its switched VBUS_SW and relay U101, as `dg header` drives them.
    Header,
    "Header"
);

#[pymethods]
impl Header {
    /// Relay U101 closed.
    #[getter]
    fn relay(&self, py: Python<'_>) -> PyResult<bool> {
        self.shared.run(py, async |d| d.header().relay().await)
    }

    #[setter]
    fn set_relay(&self, py: Python<'_>, closed: bool) -> PyResult<()> {
        self.shared
            .run(py, async move |d| d.header().set_relay(closed).await)
    }

    /// VBUS_SW's state now.
    #[getter]
    fn vbus(&self, py: Python<'_>) -> PyResult<VbusStatus> {
        self.shared.run(py, async |d| {
            d.header().vbus_sw().await.map(VbusStatus::from)
        })
    }

    #[getter]
    fn vbus_mode(&self, py: Python<'_>) -> PyResult<PyVbusMode> {
        self.shared.run(py, async |d| {
            d.header().vbus_sw_mode().await.map(PyVbusMode)
        })
    }

    #[setter]
    fn set_vbus_mode(&self, py: Python<'_>, mode: PyVbusMode) -> PyResult<()> {
        self.shared
            .run(py, async move |d| d.header().set_vbus_sw_mode(mode.0).await)
    }

    /// Switches VBUS_SW on, as `power.on` does the plug's.
    #[pyo3(signature = (max_voltage=vbus::DEFAULT_MAX_VOLTAGE_V, trip=None))]
    fn vbus_on(&self, py: Python<'_>, max_voltage: f64, trip: Option<f64>) -> PyResult<VbusStatus> {
        let mode = on_mode(max_voltage, trip)?;
        self.shared.run(py, async move |d| {
            d.header().set_vbus_sw_mode(mode).await?;
            d.header().vbus_sw().await.map(VbusStatus::from)
        })
    }

    fn vbus_off(&self, py: Python<'_>) -> PyResult<VbusStatus> {
        self.shared.run(py, async |d| {
            d.header().set_vbus_sw_mode(VbusMode::ForceOff).await?;
            d.header().vbus_sw().await.map(VbusStatus::from)
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// The UARTs: `plug` on the SBU lines (TX/RX flip by orientation) and `header` on IO7/IO8.
    Uarts,
    "Uarts"
);

#[pymethods]
impl Uarts {
    #[getter]
    fn plug(&self) -> UartPort {
        UartPort {
            shared: self.shared.clone(),
            side: Side::Plug,
        }
    }

    #[getter]
    fn header(&self) -> UartPort {
        UartPort {
            shared: self.shared.clone(),
            side: Side::Header,
        }
    }
}

/// One UART.
#[pyclass(module = "donguru", frozen, skip_from_py_object)]
pub(crate) struct UartPort {
    shared: Arc<Shared>,
    side: Side,
}

#[pymethods]
impl UartPort {
    /// Sets the line up and answers what was set: the baud rate the divider settled on, the idle time, the IO level.
    #[pyo3(signature = (baud=115_200, data_bits=8, parity="none".to_string(), stop_bits=1, flow="none".to_string(),
        prevent_back_feed=false, io_level=None, idle_chars=ww_uart::IDLE_CHARS_DEFAULT))]
    #[allow(clippy::too_many_arguments)]
    fn configure(
        &self,
        py: Python<'_>,
        baud: u32,
        data_bits: u8,
        parity: String,
        stop_bits: u8,
        flow: String,
        prevent_back_feed: bool,
        io_level: Option<f64>,
        idle_chars: u8,
    ) -> PyResult<UartConfig> {
        let config = UartConfig::py_new(
            baud,
            data_bits,
            parity,
            stop_bits,
            flow,
            prevent_back_feed,
            io_level,
            idle_chars,
        )?;
        self.apply(py, config)
    }

    /// Like `configure`, with a `UartConfig` (e.g. `settings.uart_plug`).
    fn apply(&self, py: Python<'_>, config: UartConfig) -> PyResult<UartConfig> {
        let config = config.to_api()?;
        let side = self.side;
        self.shared.run(py, async move |d| {
            d.uart(side)
                .configure(config)
                .await
                .map(|c| UartConfig::from(&c))
        })
    }

    fn __repr__(&self) -> String {
        format!("UartPort({:?})", self.side)
    }
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// Settings kept in flash; a write applies at once where the setting has a live counterpart and persists it.
    /// `reset()` takes every setting and the label back to defaults.
    Settings,
    "Settings"
);

fn action_name(a: ButtonAction) -> &'static str {
    match a {
        ButtonAction::Off => "off",
        ButtonAction::ToggleVbus => "toggle_vbus",
        ButtonAction::CycleVbus => "cycle_vbus",
        ButtonAction::ToggleData => "toggle_data",
        _ => "unknown",
    }
}

fn action(name: &str) -> PyResult<ButtonAction> {
    Ok(match name {
        "off" => ButtonAction::Off,
        "toggle_vbus" => ButtonAction::ToggleVbus,
        "cycle_vbus" => ButtonAction::CycleVbus,
        "toggle_data" => ButtonAction::ToggleData,
        _ => {
            return Err(PyValueError::new_err(format!(
                "button action \"off\", \"toggle_vbus\", \"cycle_vbus\" or \"toggle_data\", not {name:?}"
            )));
        }
    })
}

#[pymethods]
impl Settings {
    /// What plug VBUS does at power-on.
    #[getter]
    fn plug_vbus_at_boot(&self, py: Python<'_>) -> PyResult<PyVbusMode> {
        self.shared.run(py, async |d| {
            d.device()
                .settings()
                .plug_vbus_at_boot()
                .await
                .map(PyVbusMode)
        })
    }

    #[setter]
    fn set_plug_vbus_at_boot(&self, py: Python<'_>, mode: PyVbusMode) -> PyResult<()> {
        self.shared.run(py, async move |d| {
            d.device().settings().set_plug_vbus_at_boot(mode.0).await
        })
    }

    /// What the button does: `"off"`, `"toggle_vbus"`, `"cycle_vbus"`, `"toggle_data"`.
    #[getter]
    fn button_action(&self, py: Python<'_>) -> PyResult<&'static str> {
        self.shared.run(py, async |d| {
            d.device().settings().button_action().await.map(action_name)
        })
    }

    #[setter]
    fn set_button_action(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        let a = action(name)?;
        self.shared.run(py, async move |d| {
            d.device().settings().set_button_action(a).await
        })
    }

    /// The meter's averaging window in s.
    #[getter]
    fn meter_window(&self, py: Python<'_>) -> PyResult<f64> {
        self.shared.run(py, async |d| {
            d.device()
                .settings()
                .meter_window()
                .await
                .map(|w| w.to_f64())
        })
    }

    #[setter]
    fn set_meter_window(&self, py: Python<'_>, window: f64) -> PyResult<()> {
        let window = seconds(window, "meter_window")?;
        self.shared.run(py, async move |d| {
            d.device().settings().set_meter_window(window).await
        })
    }

    #[getter]
    fn meter_deadband(&self, py: Python<'_>) -> PyResult<Deadband> {
        self.shared.run(py, async |d| {
            d.device()
                .settings()
                .meter_deadband()
                .await
                .map(|b| Deadband::from(&b))
        })
    }

    #[setter]
    fn set_meter_deadband(&self, py: Python<'_>, deadband: Deadband) -> PyResult<()> {
        let deadband = deadband.to_api()?;
        self.shared.run(py, async move |d| {
            d.device().settings().set_meter_deadband(deadband).await
        })
    }

    /// The plug UART's line settings at boot.
    #[getter]
    fn uart_plug(&self, py: Python<'_>) -> PyResult<UartConfig> {
        self.shared.run(py, async |d| {
            d.device()
                .settings()
                .uart_plug()
                .await
                .map(|c| UartConfig::from(&c))
        })
    }

    #[setter]
    fn set_uart_plug(&self, py: Python<'_>, config: UartConfig) -> PyResult<()> {
        let config = config.to_api()?;
        self.shared.run(py, async move |d| {
            d.device().settings().set_uart_plug(config).await
        })
    }

    /// Every setting and the label back to defaults; factory data stays.
    fn reset(&self, py: Python<'_>) -> PyResult<()> {
        self.shared
            .run(py, async |d| d.device().settings().reset().await)
    }
}

// ---------------------------------------------------------------------------------------------------------------

level!(
    /// The power and status LEDs.
    Leds,
    "Leds"
);

#[pymethods]
impl Leds {
    /// Blinks the LEDs for `seconds` so the dongle can be found on a bench.
    #[pyo3(signature = (seconds=5.0))]
    fn identify(&self, py: Python<'_>, seconds: f64) -> PyResult<()> {
        let duration = self::seconds(seconds, "seconds")?;
        self.shared
            .run(py, async move |d| d.leds().identify(duration).await)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Power>()?;
    m.add_class::<Meter>()?;
    m.add_class::<MeterWatch>()?;
    m.add_class::<Usb>()?;
    m.add_class::<Header>()?;
    m.add_class::<Uarts>()?;
    m.add_class::<UartPort>()?;
    m.add_class::<Settings>()?;
    m.add_class::<Leds>()?;
    Ok(())
}
