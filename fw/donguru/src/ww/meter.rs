//! `meter`: the power meter in SI units (`PowerMeter`, API-12), from the ADC task's windowed readings
//! (`src/adc.rs`). Values are integers: V and A in milli, W in milli, J and C in milli, °C in milli.

use super::ServerState;
use crate::adc::{self, Readings};
use donguru_api::{MeterCalibration, MeterDeadband, MeterReading, MeterTotals};
use embassy_time::{Duration, Instant, Timer};
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use ww_si::ww_numeric::NumericValue;
use ww_si::{Ampere, Coulomb, DegreeCelsius, Joule, Prefix, Second, Volt, Watt};

/// `reading` is not marked changed more often than the ADC updates it; `totals` change all the time, so they are
/// marked this often while observed.
const TOTALS_PERIOD: Duration = Duration::from_secs(1);
/// Die temperature moves slowly: a change of this much (0.01 °C) counts.
const TEMPERATURE_DEADBAND_CDEG: i32 = 50;
/// How long a read waits for the first ADC update after boot or a channel set switch.
const FIRST_READING_WAIT: Duration = Duration::from_millis(100);

/// Deadband and what was last announced to `OnChange` observers.
pub(super) struct MeterState {
    deadband_mv: u32,
    deadband_ma: u32,
    marked_mv: i32,
    marked_ma: i32,
    marked_cdeg: i32,
    marked_totals_at: Instant,
}

impl MeterState {
    /// Starts with `deadband` (`device.settings.meter_deadband`), the default when it isn't a valid one.
    pub(super) fn new(deadband: &MeterDeadband) -> Self {
        let (deadband_mv, deadband_ma) = deadband_of(deadband)
            .or_else(|| deadband_of(&crate::settings::default_deadband()))
            .unwrap_or((0, 0));
        MeterState {
            deadband_mv,
            deadband_ma,
            marked_mv: 0,
            marked_ma: 0,
            marked_cdeg: 0,
            marked_totals_at: Instant::MIN,
        }
    }
}

impl MeterState {
    pub(super) fn set_deadband(&mut self, mv: u32, ma: u32) {
        self.deadband_mv = mv;
        self.deadband_ma = ma;
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

    pub(super) async fn get_meter_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterTotals, ()> {
        Value(totals_of(&adc::totals()))
    }

    pub(super) async fn meter_reset_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        adc::reset_totals();
        self.mark_changed_meter_totals();
        Ready(())
    }

    pub(super) async fn set_meter_window(
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

    pub(super) async fn get_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<Second, CommonError<'_>> {
        Value(Second::micro(adc::window_us() as i32))
    }

    pub(super) async fn set_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: MeterDeadband,
    ) -> SetResult<CommonError<'_>> {
        let Some((mv, ma)) = deadband_of(&value) else {
            return SetError(invalid(DEADBAND_INVALID));
        };
        self.meter.set_deadband(mv, ma);
        Set
    }

    pub(super) async fn get_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterDeadband, CommonError<'_>> {
        Value(MeterDeadband {
            voltage: Volt::milli(self.meter.deadband_mv as i32),
            current: Ampere::milli(self.meter.deadband_ma as i32),
        })
    }

    pub(super) async fn get_meter_calibration(
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

    /// Called by the server task while a host observes anything: marks `reading` changed only when it moved beyond
    /// the deadband since it was last marked, so an idle DUT costs no link. `totals` every [`TOTALS_PERIOD`].
    pub(super) fn meter_poll(&mut self, now: Instant) {
        let Some(r) = adc::latest() else {
            return;
        };
        let mv = r.input_voltage_mv as i32;
        let ma = r.input_current_ma as i32;
        if (mv - self.meter.marked_mv).unsigned_abs() >= self.meter.deadband_mv.max(1)
            || (ma - self.meter.marked_ma).unsigned_abs() >= self.meter.deadband_ma.max(1)
        {
            self.meter.marked_mv = mv;
            self.meter.marked_ma = ma;
            self.mark_changed_meter_reading();
        }
        let cdeg = r.temp_cdeg as i32;
        if (cdeg - self.meter.marked_cdeg).abs() >= TEMPERATURE_DEADBAND_CDEG {
            self.meter.marked_cdeg = cdeg;
            self.mark_changed_meter_temperature();
        }
        if now.duration_since(self.meter.marked_totals_at) >= TOTALS_PERIOD {
            self.meter.marked_totals_at = now;
            self.mark_changed_meter_totals();
        }
    }
}

fn reading_of(r: &Readings) -> MeterReading {
    let mv = r.input_voltage_mv as i32;
    let ma = r.input_current_ma as i32;
    MeterReading {
        vbus_in: Volt::milli(mv),
        current: Ampere::milli(ma),
        // mV x mA = uW
        power: Watt::milli(mv * ma / 1000),
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
