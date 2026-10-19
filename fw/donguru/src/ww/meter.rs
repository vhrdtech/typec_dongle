//! `meter`: the power meter in SI units (`PowerMeter`, API-12), from the ADC task's windowed readings
//! (`src/adc.rs`). Values are integers: V and A in milli, W in milli, J and C in milli, °C in milli.

use super::ServerState;
use crate::adc::{self, Readings};
use donguru_api::{MeterCalibration, MeterDeadband, MeterReading, MeterTotals};
use embassy_time::{Duration, Instant, Timer};
use portable_atomic::{AtomicU32, Ordering};
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use ww_si::ww_numeric::NumericValue;
use ww_si::{Ampere, Coulomb, DegreeCelsius, Joule, Prefix, Second, Volt};

/// `reading` is not marked changed more often than the ADC updates it; `totals` change all the time, so they are
/// marked this often while observed.
const TOTALS_PERIOD: Duration = Duration::from_secs(1);
/// Die temperature moves slowly: a change of this much (0.01 °C) counts.
const TEMPERATURE_DEADBAND_CDEG: i32 = 50;
/// How long a read waits for the first ADC update after boot or a channel set switch.
const FIRST_READING_WAIT: Duration = Duration::from_millis(100);

/// `meter.deadband` in mV and mA: what counts as a change of `reading`, read by the ADC task ([MeterMarks]).
static DEADBAND_MV: AtomicU32 = AtomicU32::new(0);
static DEADBAND_MA: AtomicU32 = AtomicU32::new(0);

/// Starts with `deadband` (`device.settings.meter_deadband`), the default when it isn't a valid one.
pub(super) fn init_deadband(deadband: &MeterDeadband) {
    let (mv, ma) = deadband_of(deadband)
        .or_else(|| deadband_of(&crate::settings::default_deadband()))
        .unwrap_or((0, 0));
    set_deadband(mv, ma);
}

pub(super) fn set_deadband(mv: u32, ma: u32) {
    DEADBAND_MV.store(mv, Ordering::Relaxed);
    DEADBAND_MA.store(ma, Ordering::Relaxed);
}

/// What was last announced to `OnChange` observers of `meter`, kept by the ADC task: each update that moved beyond
/// the deadband marks `reading` changed through the observe signal (SRV-25), which wakes the server task, so an idle
/// DUT costs no wake and no link. `totals` are marked every [`TOTALS_PERIOD`].
pub struct MeterMarks {
    marked_mv: i32,
    marked_ma: i32,
    marked_cdeg: i32,
    marked_totals_at: Instant,
}

impl MeterMarks {
    pub const fn new() -> Self {
        MeterMarks {
            marked_mv: 0,
            marked_ma: 0,
            marked_cdeg: 0,
            marked_totals_at: Instant::MIN,
        }
    }

    /// The ADC task's new readings `r`, taken at `now`.
    pub fn update(&mut self, r: &Readings, now: Instant) {
        let signal = &super::OBSERVE;
        let mv = r.input_voltage_mv as i32;
        let ma = r.input_current_ma as i32;
        if (mv - self.marked_mv).unsigned_abs() >= DEADBAND_MV.load(Ordering::Relaxed).max(1)
            || (ma - self.marked_ma).unsigned_abs() >= DEADBAND_MA.load(Ordering::Relaxed).max(1)
        {
            self.marked_mv = mv;
            self.marked_ma = ma;
            signal.mark_changed_meter_reading();
        }
        let cdeg = r.temp_cdeg as i32;
        if (cdeg - self.marked_cdeg).abs() >= TEMPERATURE_DEADBAND_CDEG {
            self.marked_cdeg = cdeg;
            signal.mark_changed_meter_temperature();
        }
        if now.duration_since(self.marked_totals_at) >= TOTALS_PERIOD {
            self.marked_totals_at = now;
            signal.mark_changed_meter_totals();
        }
    }
}

/// The latest ADC update, waiting for the first one after boot or a channel set switch.
pub(super) async fn readings() -> Option<Readings> {
    let deadline = Instant::now() + FIRST_READING_WAIT;
    loop {
        if let Some(r) = adc::latest() {
            return Some(r);
        }
        if Instant::now() >= deadline {
            return None;
        }
        Timer::after(Duration::from_millis(2)).await;
    }
}

/// A value of any integer encoding in units of `prefix`, `None` for what an integer can't hold.
fn to_prefixed(value: &NumericValue, from: Prefix, to: Prefix) -> Option<i32> {
    let v = i32::try_from(value.to_i128()?).ok()?;
    let (from, to): (i8, i8) = (from.into(), to.into());
    let mut v = v;
    for _ in 0..(from - to).max(0) {
        v = v.checked_mul(10)?;
    }
    for _ in 0..(to - from).max(0) {
        v /= 10;
    }
    Some(v)
}

/// A `meter.window` in µs, `None` outside of what the ADC can average (see FEATURES.md FW-3).
pub(super) fn window_us_of(window: &Second) -> Option<u32> {
    to_prefixed(&window.value, window.prefix, Prefix::Micro)
        .filter(|us| *us >= adc::WINDOW_MIN_US as i32 && *us <= adc::WINDOW_MAX_US as i32)
        .map(|us| us as u32)
}

/// A `meter.deadband` in mV and mA, `None` unless both are non-negative integers there.
pub(super) fn deadband_of(deadband: &MeterDeadband) -> Option<(u32, u32)> {
    let mv = to_prefixed(
        &deadband.voltage.value,
        deadband.voltage.prefix,
        Prefix::Milli,
    )?;
    let ma = to_prefixed(
        &deadband.current.value,
        deadband.current.prefix,
        Prefix::Milli,
    )?;
    (mv >= 0 && ma >= 0).then_some((mv as u32, ma as u32))
}

pub(super) const WINDOW_INVALID: &str = "window out of range, see FEATURES.md FW-3";
pub(super) const DEADBAND_INVALID: &str =
    "deadband must be a non-negative integer number of mV / mA";

pub(super) fn invalid(reason: &'static str) -> CommonError<'static> {
    CommonError::InvalidArgument {
        arg: 0,
        reason: MaxLen::new_unchecked(reason),
    }
}

impl ServerState {
    // donguru_api::PowerMeter at meter

    pub(super) async fn get_meter_reading(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterReading, ()> {
        let Some(r) = readings().await else {
            return GetError(());
        };
        Value(reading_of(&r))
    }

    pub(super) async fn get_meter_temperature(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<DegreeCelsius, ()> {
        let Some(r) = readings().await else {
            return GetError(());
        };
        Value(temperature_of(&r))
    }

    pub(super) fn get_meter_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterTotals, ()> {
        Value(totals_of(&adc::totals()))
    }

    pub(super) fn meter_reset_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        adc::reset_totals();
        self.mark_changed_meter_totals();
        Ready(())
    }

    pub(super) fn set_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: Second,
    ) -> SetResult<CommonError<'_>> {
        let Some(us) = window_us_of(&value) else {
            return SetError(invalid(WINDOW_INVALID));
        };
        adc::set_window_us(us);
        Set
    }

    pub(super) fn get_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<Second, CommonError<'_>> {
        Value(Second::micro(adc::window_us() as i32))
    }

    pub(super) fn set_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: MeterDeadband,
    ) -> SetResult<CommonError<'_>> {
        let Some((mv, ma)) = deadband_of(&value) else {
            return SetError(invalid(DEADBAND_INVALID));
        };
        set_deadband(mv, ma);
        Set
    }

    pub(super) fn get_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterDeadband, CommonError<'_>> {
        Value(MeterDeadband {
            voltage: Volt::milli(DEADBAND_MV.load(Ordering::Relaxed) as i32),
            current: Ampere::milli(DEADBAND_MA.load(Ordering::Relaxed) as i32),
        })
    }

    pub(super) fn get_meter_calibration(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterCalibration, ()> {
        // FW-9 brings factory data: nominal values until then
        Value(MeterCalibration {
            calibrated: false,
            vbus_in_offset: Volt::milli(0),
            vbus_in_gain_ppm: 0,
            current_offset: Ampere::milli(0),
            current_gain_ppm: 0,
        })
    }
}

fn reading_of(r: &Readings) -> MeterReading {
    reading_from(r.input_voltage_mv as i32, r.input_current_ma as i32)
}

fn reading_from(mv: i32, ma: i32) -> MeterReading {
    MeterReading {
        vbus_in: Volt::milli(mv),
        current: Ampere::milli(ma),
    }
}

fn temperature_of(r: &Readings) -> DegreeCelsius {
    // 0.01 °C -> m°C
    DegreeCelsius::milli(r.temp_cdeg as i32 * 10)
}

fn totals_of(t: &adc::Totals) -> MeterTotals {
    let now_ms = Instant::now().as_millis() as u32;
    MeterTotals {
        charge: Coulomb::milli(t.charge_mc),
        energy: Joule::milli(t.energy_mj),
        peak_current: Ampere::milli(t.peak_ma as i32),
        since: Second::milli(now_ms.wrapping_sub(t.since_ms) as i32),
    }
}

/// Feeds `meter.reading` samples (API-39) from the ADC task: one row per ADC sequence into the observe cells
/// `poll_properties` reads (`METER_SAMPLES` in `ww/mod.rs`).
pub struct MeterSamples(
    super::server_impl::RootMeterWriter<'static, { super::METER_SAMPLES_RING }>,
);

impl MeterSamples {
    /// The one writer of the cells, `None` once taken.
    pub fn take() -> Option<Self> {
        super::METER_SAMPLES
            .writer()
            .map(|w| MeterSamples(w.with_wake(super::OBSERVE.as_wake())))
    }

    /// Whether a host observes `meter.reading` in `Samples` mode: without one, rows aren't worth computing.
    pub fn is_sampling(&self) -> bool {
        self.0.is_sampling()
    }

    /// One sequence's VBUS_IN and current, taken at `t_us` (the device's µs clock, wrapping).
    pub fn push(&mut self, t_us: u32, mv: i32, ma: i32) {
        self.0.reading(reading_from(mv, ma));
        self.0.commit(Second::micro(t_us));
    }
}
