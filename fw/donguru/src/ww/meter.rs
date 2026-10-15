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

/// Default deadband: 20 mV and 10 mA.
const DEFAULT_DEADBAND_MV: u32 = 20;
const DEFAULT_DEADBAND_MA: u32 = 10;
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
    pub(super) fn new() -> Self {
        MeterState {
            deadband_mv: DEFAULT_DEADBAND_MV,
            deadband_ma: DEFAULT_DEADBAND_MA,
            marked_mv: 0,
            marked_ma: 0,
            marked_cdeg: 0,
            marked_totals_at: Instant::MIN,
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

fn milli<T>(make: impl Fn(Prefix, NumericValue) -> T, v: i32) -> T {
    make(Prefix::Milli, NumericValue::I32(v))
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

fn invalid(reason: &'static str) -> CommonError<'static> {
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
        let us = match to_prefixed(&value.value, value.prefix, Prefix::Micro) {
            Some(us) if us >= adc::WINDOW_MIN_US as i32 && us <= adc::WINDOW_MAX_US as i32 => us,
            _ => return SetError(invalid("window out of range, see FEATURES.md FW-3")),
        };
        adc::set_window_us(us as u32);
        Set
    }

    pub(super) async fn get_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<Second, CommonError<'_>> {
        Value(Second {
            prefix: Prefix::Micro,
            value: NumericValue::I32(adc::window_us() as i32),
        })
    }

    pub(super) async fn set_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: MeterDeadband,
    ) -> SetResult<CommonError<'_>> {
        let mv = to_prefixed(&value.voltage.value, value.voltage.prefix, Prefix::Milli);
        let ma = to_prefixed(&value.current.value, value.current.prefix, Prefix::Milli);
        match (mv, ma) {
            (Some(mv), Some(ma)) if mv >= 0 && ma >= 0 => {
                self.meter.deadband_mv = mv as u32;
                self.meter.deadband_ma = ma as u32;
                Set
            }
            _ => SetError(invalid("deadband must be a non-negative integer number of mV / mA")),
        }
    }

    pub(super) async fn get_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterDeadband, CommonError<'_>> {
        Value(MeterDeadband {
            voltage: milli(|prefix, value| Volt { prefix, value }, self.meter.deadband_mv as i32),
            current: milli(|prefix, value| Ampere { prefix, value }, self.meter.deadband_ma as i32),
        })
    }

    pub(super) async fn get_meter_calibration(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<MeterCalibration, ()> {
        // FW-9 brings factory data: nominal values until then
        Value(MeterCalibration {
            calibrated: false,
            vbus_in_offset: milli(|prefix, value| Volt { prefix, value }, 0),
            vbus_in_gain_ppm: 0,
            current_offset: milli(|prefix, value| Ampere { prefix, value }, 0),
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
        vbus_in: milli(|prefix, value| Volt { prefix, value }, mv),
        current: milli(|prefix, value| Ampere { prefix, value }, ma),
        // mV x mA = uW
        power: milli(|prefix, value| Watt { prefix, value }, mv * ma / 1000),
    }
}

fn temperature_of(r: &Readings) -> DegreeCelsius {
    // 0.01 °C -> m°C
    milli(|prefix, value| DegreeCelsius { prefix, value }, r.temp_cdeg as i32 * 10)
}

fn totals_of(t: &adc::Totals) -> MeterTotals {
    let now_ms = Instant::now().as_millis() as u32;
    MeterTotals {
        charge: milli(|prefix, value| Coulomb { prefix, value }, t.charge_mc),
        energy: milli(|prefix, value| Joule { prefix, value }, t.energy_mj),
        peak_current: milli(|prefix, value| Ampere { prefix, value }, t.peak_ma as i32),
        since: Second {
            prefix: Prefix::Milli,
            value: NumericValue::I32(now_ms.wrapping_sub(t.since_ms) as i32),
        },
    }
}

