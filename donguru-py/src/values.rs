//! What calls answer and take, as small Python classes with SI floats (V, A, W, s, C, J) and plain strings for the
//! API's enums.

use donguru_core::api::{GuardTrip, MeterDeadband, MeterReading, MeterTotals, VbusMode, VbusState};
use donguru_core::levels::DeviceSummary;
use donguru_core::{DeviceEntry, vbus};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use ww_si::{Ampere, Baud, Prefix, Volt};
use ww_uart::{DataBits, Parity, StopBits};

/// A value in thousandths as the firmware's `i32` quantities take it (it has no floats, `donguru_core::vbus`).
pub(crate) fn milli(v: f64, what: &str) -> PyResult<i32> {
    let m = (v * 1000.0).round();
    if !m.is_finite() || m < i32::MIN as f64 || m > i32::MAX as f64 {
        return Err(PyValueError::new_err(format!("{what} out of range: {v}")));
    }
    Ok(m as i32)
}

/// A dongle as `list()` sees it on USB, before it is opened.
#[pyclass(module = "donguru", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct DeviceInfo {
    /// Position in `list()`, sorted by USB path.
    pub index: usize,
    /// USB serial number: the chip UID in hex.
    pub serial: String,
    /// USB path, `3-1.2`.
    pub path: String,
    /// User label, `""` when not set.
    pub label: String,
    /// `donguru_api` version the firmware reports.
    pub api_version: Option<String>,
    /// Where it is (`usb 3-1.2 c0de:cafe`).
    pub location: String,
}

impl From<&DeviceEntry> for DeviceInfo {
    fn from(e: &DeviceEntry) -> Self {
        DeviceInfo {
            index: e.index,
            serial: e.serial.clone(),
            path: e.path.clone(),
            label: e.label.clone(),
            api_version: e.api_version.clone(),
            location: e.location.clone(),
        }
    }
}

#[pymethods]
impl DeviceInfo {
    fn __repr__(&self) -> String {
        format!(
            "DeviceInfo(index={}, serial={:?}, path={:?}, label={:?})",
            self.index, self.serial, self.path, self.label
        )
    }
}

/// `Dongle.info()`: the device contract in one read. What the firmware doesn't report yet is `None`.
#[pyclass(module = "donguru", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct Info {
    /// Board variant from the hub: `"Main"`, `"Plus"` or `"Unknown"`.
    pub variant: String,
    /// `"donguru 0.20.27"`.
    pub firmware: Option<String>,
    /// `"Donguru B153 rev A"`.
    pub board: Option<String>,
    pub uid: Option<String>,
    pub label: Option<String>,
    /// Cause of the last reset, `"PowerOn"`, `"Watchdog"`, ...
    pub last_reset: Option<String>,
}

impl From<&DeviceSummary> for Info {
    fn from(s: &DeviceSummary) -> Self {
        Info {
            variant: format!("{:?}", s.variant),
            firmware: s.firmware.as_ref().map(|f| {
                let v = &f.version;
                format!("{} {}.{}.{}", f.name.as_str(), v.major, v.minor, v.patch)
            }),
            board: s
                .board
                .as_ref()
                .map(|b| format!("{} rev {}", b.name.as_str(), b.revision.as_str())),
            uid: s.uid.clone(),
            label: s.label.clone(),
            last_reset: s.last_reset.as_ref().map(|r| format!("{:?}", r.cause)),
        }
    }
}

#[pymethods]
impl Info {
    fn __repr__(&self) -> String {
        format!(
            "Info(variant={:?}, firmware={}, board={}, uid={}, label={}, last_reset={})",
            self.variant,
            py_opt(&self.firmware),
            py_opt(&self.board),
            py_opt(&self.uid),
            py_opt(&self.label),
            py_opt(&self.last_reset)
        )
    }
}

/// An optional string as Python writes it: `'x'` quoted, or `None`.
fn py_opt(s: &Option<String>) -> String {
    match s {
        Some(s) => format!("{s:?}"),
        None => "None".to_string(),
    }
}

/// One meter reading: `voltage` (VBUS_IN, V), `current` (A, positive towards the DUT), `power` (W). `t` is the
/// device time in s of an observed reading when the device sends one, else `None`.
#[pyclass(module = "donguru", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct Reading {
    pub voltage: f64,
    pub current: f64,
    pub power: f64,
    pub t: Option<f64>,
}

impl Reading {
    pub(crate) fn new(r: &MeterReading, t: Option<f64>) -> Self {
        Reading {
            voltage: r.vbus_in.to_f64(),
            current: r.current.to_f64(),
            power: donguru_core::reading_power_w(r),
            t,
        }
    }
}

#[pymethods]
impl Reading {
    fn __repr__(&self) -> String {
        format!(
            "Reading(voltage={:.3}, current={:.4}, power={:.3})",
            self.voltage, self.current, self.power
        )
    }
}

/// Charge (C), energy (J) and peak current (A) since boot or `meter.reset_totals()`, over `since` seconds;
/// `charge_mah` and `energy_wh` in the units chargers print.
#[pyclass(module = "donguru", frozen, get_all, skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct Totals {
    pub charge: f64,
    pub energy: f64,
    pub peak_current: f64,
    pub since: f64,
}

impl From<&MeterTotals> for Totals {
    fn from(t: &MeterTotals) -> Self {
        Totals {
            charge: t.charge.to_f64(),
            energy: t.energy.to_f64(),
            peak_current: t.peak_current.to_f64(),
            since: t.since.to_f64(),
        }
    }
}

#[pymethods]
impl Totals {
    #[getter]
    fn charge_mah(&self) -> f64 {
        self.charge / 3.6
    }

    #[getter]
    fn energy_wh(&self) -> f64 {
        self.energy / 3600.0
    }

    fn __repr__(&self) -> String {
        format!(
            "Totals(charge {:.2} mAh, energy {:.4} Wh, peak {:.1} mA, since {:.0} s)",
            self.charge_mah(),
            self.energy_wh(),
            self.peak_current * 1000.0,
            self.since
        )
    }
}

/// State of a VBUS switch: `state` is `"off"`, `"on"`, `"fault"` (latched by the switch; switching on again
/// re-arms it), `"tripped"` (cut by the firmware, `trip_reason` says why) or `"unknown"` (a newer firmware's).
#[pyclass(module = "donguru", frozen, get_all, eq, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub(crate) struct VbusStatus {
    pub state: &'static str,
    /// `"over_voltage"`, `"over_current"`, `"over_power"` when tripped.
    pub trip_reason: Option<&'static str>,
}

impl From<VbusState> for VbusStatus {
    fn from(s: VbusState) -> Self {
        let (state, trip_reason) = match s {
            VbusState::Off => ("off", None),
            VbusState::On => ("on", None),
            VbusState::Fault => ("fault", None),
            VbusState::Tripped(reason) => ("tripped", Some(trip_name(reason))),
            _ => ("unknown", None),
        };
        VbusStatus { state, trip_reason }
    }
}

fn trip_name(reason: GuardTrip) -> &'static str {
    match reason {
        GuardTrip::OverVoltage => "over_voltage",
        GuardTrip::OverCurrent => "over_current",
        GuardTrip::OverPower => "over_power",
        _ => "unknown",
    }
}

#[pymethods]
impl VbusStatus {
    /// VBUS is switched on.
    #[getter]
    fn on(&self) -> bool {
        self.state == "on"
    }

    /// The firmware cut it (a limit of the mode was exceeded).
    #[getter]
    fn tripped(&self) -> bool {
        self.state == "tripped"
    }

    fn __str__(&self) -> String {
        match self.trip_reason {
            Some(reason) => format!("tripped ({reason})"),
            None => self.state.to_string(),
        }
    }

    fn __repr__(&self) -> String {
        format!("VbusStatus({:?})", self.__str__())
    }
}

/// How the firmware drives a VBUS switch. Build one with `VbusMode.off()`, `VbusMode.on(max_voltage=5.5)` or
/// `VbusMode.auto(max_voltage, max_current)`; `mode` is `"off"`, `"on"`, `"auto"` or `"unknown"`, the limits in V,
/// A, W where the mode has them.
#[pyclass(name = "VbusMode", module = "donguru", frozen, eq, from_py_object)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyVbusMode(pub(crate) VbusMode);

#[pymethods]
impl PyVbusMode {
    #[staticmethod]
    fn off() -> Self {
        PyVbusMode(VbusMode::ForceOff)
    }

    /// On while VBUS stays under `max_voltage`.
    #[staticmethod]
    #[pyo3(signature = (max_voltage=vbus::DEFAULT_MAX_VOLTAGE_V))]
    fn on(max_voltage: f64) -> PyResult<Self> {
        milli(max_voltage, "max_voltage")?;
        Ok(PyVbusMode(vbus::force_on(max_voltage)))
    }

    /// On, cut when VBUS goes over `max_voltage` or the current over `max_current` (A).
    #[staticmethod]
    #[pyo3(signature = (max_current, max_voltage=vbus::DEFAULT_MAX_VOLTAGE_V))]
    fn auto(max_current: f64, max_voltage: f64) -> PyResult<Self> {
        milli(max_voltage, "max_voltage")?;
        milli(max_current, "max_current")?;
        milli(max_voltage * max_current, "max_voltage x max_current")?;
        Ok(PyVbusMode(vbus::auto(max_voltage, max_current)))
    }

    #[getter]
    fn mode(&self) -> &'static str {
        match &self.0 {
            VbusMode::ForceOff => "off",
            VbusMode::ForceOn { .. } => "on",
            VbusMode::Auto { .. } => "auto",
            _ => "unknown",
        }
    }

    #[getter]
    fn max_voltage(&self) -> Option<f64> {
        match &self.0 {
            VbusMode::ForceOn { max_voltage } | VbusMode::Auto { max_voltage, .. } => {
                Some(max_voltage.to_f64())
            }
            _ => None,
        }
    }

    #[getter]
    fn max_current(&self) -> Option<f64> {
        match &self.0 {
            VbusMode::Auto { max_current, .. } => Some(max_current.to_f64()),
            _ => None,
        }
    }

    #[getter]
    fn max_power(&self) -> Option<f64> {
        match &self.0 {
            VbusMode::Auto { max_power, .. } => Some(max_power.to_f64()),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match &self.0 {
            VbusMode::ForceOff => "VbusMode.off()".to_string(),
            VbusMode::ForceOn { max_voltage } => {
                format!("VbusMode.on(max_voltage={})", max_voltage.to_f64())
            }
            VbusMode::Auto {
                max_voltage,
                max_current,
                ..
            } => format!(
                "VbusMode.auto(max_current={}, max_voltage={})",
                max_current.to_f64(),
                max_voltage.to_f64()
            ),
            _ => "VbusMode(unknown)".to_string(),
        }
    }
}

/// The meter's change threshold for `watch(on_change=True)`: `voltage` (V) and `current` (A).
#[pyclass(module = "donguru", frozen, get_all, eq, from_py_object)]
#[derive(Clone, PartialEq)]
pub(crate) struct Deadband {
    pub voltage: f64,
    pub current: f64,
}

#[pymethods]
impl Deadband {
    #[new]
    fn py_new(voltage: f64, current: f64) -> Self {
        Deadband { voltage, current }
    }

    fn __repr__(&self) -> String {
        format!(
            "Deadband(voltage={}, current={})",
            self.voltage, self.current
        )
    }
}

impl Deadband {
    pub(crate) fn to_api(&self) -> PyResult<MeterDeadband> {
        Ok(MeterDeadband {
            voltage: Volt::milli(milli(self.voltage, "voltage")?),
            current: Ampere::milli(milli(self.current, "current")?),
        })
    }
}

impl From<&MeterDeadband> for Deadband {
    fn from(d: &MeterDeadband) -> Self {
        Deadband {
            voltage: d.voltage.to_f64(),
            current: d.current.to_f64(),
        }
    }
}

/// Line settings of a UART: `baud`, `data_bits` (5..9), `parity` (`"none"`, `"odd"`, `"even"`), `stop_bits` (1, 2),
/// `flow` (`"none"`, `"hardware"`, `"software"`, `"high_z"`, `"test"`), `prevent_back_feed`, `io_level` (V, `None`:
/// as it is) and `idle_chars` (RX idle time that ends a chunk, in character times).
#[pyclass(module = "donguru", frozen, get_all, eq, from_py_object)]
#[derive(Clone, PartialEq)]
pub(crate) struct UartConfig {
    pub baud: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub flow: String,
    pub prevent_back_feed: bool,
    pub io_level: Option<f64>,
    pub idle_chars: u8,
}

#[pymethods]
impl UartConfig {
    #[new]
    #[pyo3(signature = (baud=115_200, data_bits=8, parity="none".to_string(), stop_bits=1, flow="none".to_string(),
        prevent_back_feed=false, io_level=None, idle_chars=ww_uart::IDLE_CHARS_DEFAULT))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn py_new(
        baud: u32,
        data_bits: u8,
        parity: String,
        stop_bits: u8,
        flow: String,
        prevent_back_feed: bool,
        io_level: Option<f64>,
        idle_chars: u8,
    ) -> PyResult<Self> {
        let config = UartConfig {
            baud,
            data_bits,
            parity,
            stop_bits,
            flow,
            prevent_back_feed,
            io_level,
            idle_chars,
        };
        config.to_api()?;
        Ok(config)
    }

    fn __repr__(&self) -> String {
        format!(
            "UartConfig(baud={}, data_bits={}, parity={:?}, stop_bits={}, flow={:?}, prevent_back_feed={}, \
             io_level={:?}, idle_chars={})",
            self.baud,
            self.data_bits,
            self.parity,
            self.stop_bits,
            self.flow,
            if self.prevent_back_feed {
                "True"
            } else {
                "False"
            },
            self.io_level,
            self.idle_chars
        )
    }
}

impl UartConfig {
    pub(crate) fn to_api(&self) -> PyResult<ww_uart::UartConfig> {
        let data_bits = match self.data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            8 => DataBits::Eight,
            9 => DataBits::Nine,
            n => return Err(PyValueError::new_err(format!("data_bits 5..9, not {n}"))),
        };
        let parity = match self.parity.to_ascii_lowercase().as_str() {
            "none" | "n" => Parity::None,
            "odd" | "o" => Parity::Odd,
            "even" | "e" => Parity::Even,
            p => {
                return Err(PyValueError::new_err(format!(
                    "parity \"none\", \"odd\" or \"even\", not {p:?}"
                )));
            }
        };
        let stop_bits = match self.stop_bits {
            1 => StopBits::Stop1,
            2 => StopBits::Stop2,
            n => return Err(PyValueError::new_err(format!("stop_bits 1 or 2, not {n}"))),
        };
        let mode = match self.flow.as_str() {
            "none" => ww_uart::Mode::Asynchronous,
            "hardware" => ww_uart::Mode::HardwareFlowControl,
            "software" => ww_uart::Mode::SoftwareFlowControl,
            "high_z" => ww_uart::Mode::HighZ,
            "test" => ww_uart::Mode::Test,
            f => {
                return Err(PyValueError::new_err(format!(
                    "flow \"none\", \"hardware\", \"software\", \"high_z\" or \"test\", not {f:?}"
                )));
            }
        };
        let io_level = match self.io_level {
            Some(v) => Some(Volt::milli(milli(v, "io_level")?)),
            None => None,
        };
        Ok(ww_uart::UartConfig {
            baud_rate: Baud::new(Prefix::Unit, self.baud),
            data_bits,
            parity,
            stop_bits,
            mode,
            prevent_back_feed: self.prevent_back_feed,
            io_level,
            idle_chars: self.idle_chars,
        })
    }
}

impl From<&ww_uart::UartConfig> for UartConfig {
    fn from(c: &ww_uart::UartConfig) -> Self {
        let data_bits = match c.data_bits {
            DataBits::Five => 5,
            DataBits::Six => 6,
            DataBits::Seven => 7,
            DataBits::Nine => 9,
            _ => 8,
        };
        let parity = match c.parity {
            Parity::Odd => "odd",
            Parity::Even => "even",
            _ => "none",
        };
        let stop_bits = match c.stop_bits {
            StopBits::Stop2 => 2,
            _ => 1,
        };
        let flow = match c.mode {
            ww_uart::Mode::Asynchronous => "none",
            ww_uart::Mode::HardwareFlowControl => "hardware",
            ww_uart::Mode::SoftwareFlowControl => "software",
            ww_uart::Mode::HighZ => "high_z",
            ww_uart::Mode::Test => "test",
            #[allow(unreachable_patterns)] // a mode of a newer firmware
            _ => "unknown",
        };
        UartConfig {
            baud: c.baud_rate.to_f64().round() as u32,
            data_bits,
            parity: parity.to_string(),
            stop_bits,
            flow: flow.to_string(),
            prevent_back_feed: c.prevent_back_feed,
            io_level: c.io_level.as_ref().map(|v| v.to_f64()),
            idle_chars: c.idle_chars,
        }
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<DeviceInfo>()?;
    m.add_class::<Info>()?;
    m.add_class::<Reading>()?;
    m.add_class::<Totals>()?;
    m.add_class::<VbusStatus>()?;
    m.add_class::<PyVbusMode>()?;
    m.add_class::<Deadband>()?;
    m.add_class::<UartConfig>()?;
    Ok(())
}
