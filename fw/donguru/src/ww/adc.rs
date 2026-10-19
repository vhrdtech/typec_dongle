//! `adc`: ADC1 channels by index with names (`ww_adc::Adc`, API-12), from the same sequence as the meter
//! (`src/adc.rs`). Calibrated values are integers in the channel's prefix (mV, mA, m°C).
//!
//! The channels of the extended set (`io7`, `io9`, `io10`) are not sampled while the base set runs: a read of one
//! switches the ADC task to the extended set (a few ms gap in `meter.reading`) and it stays there.
//! `sbu1` / `sbu2` (PA1 / PA0) answer `Busy` until the SBU level (API-14) says who owns the pins.

use super::ServerState;
use super::meter::readings;
use crate::adc::{self, ChannelSet, Readings};
use embassy_time::{Duration, Instant, Timer};
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use ww_adc::{AdcCapabilities, AdcError, ChannelInfo, Reading};
use ww_si::ww_numeric::NumericValue;
use ww_si::{Prefix, SIExpr, Second, units};

/// How long a read of an extended channel waits for the set switch.
const SWITCH_WAIT: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ch {
    VbusIn,
    Current,
    Vdda,
    Temperature,
    Button,
    Io9,
    Io7,
    Io10,
    Sbu1,
    Sbu2,
}

/// In API order: `adc.read(n)` is `CHANNELS[n]`.
const CHANNELS: [(Ch, &str, Unit); 10] = [
    (Ch::VbusIn, "vbus_in", Unit::Volt),
    (Ch::Current, "current", Unit::Ampere),
    (Ch::Vdda, "vdda", Unit::Volt),
    (Ch::Temperature, "temperature", Unit::Celsius),
    (Ch::Button, "button", Unit::Volt),
    (Ch::Io9, "io9", Unit::Volt),
    (Ch::Io7, "io7", Unit::Volt),
    (Ch::Io10, "io10", Unit::Volt),
    (Ch::Sbu1, "sbu1", Unit::Volt),
    (Ch::Sbu2, "sbu2", Unit::Volt),
];

#[derive(Clone, Copy)]
enum Unit {
    Volt,
    Ampere,
    Celsius,
}

impl Unit {
    const fn expr(self) -> SIExpr<'static> {
        match self {
            Unit::Volt => SIExpr::named(<units::Volt as ww_si::Unit>::EXPR),
            Unit::Ampere => SIExpr::named(<units::Ampere as ww_si::Unit>::EXPR),
            Unit::Celsius => SIExpr::named(<units::DegreeCelsius as ww_si::Unit>::EXPR),
        }
    }
}

/// `Some((value in the prefix, raw code))`, `None` for a channel the running set doesn't sample.
fn value_of(ch: Ch, r: &Readings) -> Option<(i32, u16)> {
    Some(match ch {
        Ch::VbusIn => (r.input_voltage_mv as i32, r.raw.input_voltage),
        Ch::Current => (r.input_current_ma as i32, r.raw.input_current),
        Ch::Vdda => (r.vdda_mv as i32, r.raw.vref),
        Ch::Temperature => (r.temp_cdeg as i32 * 10, r.raw.temp),
        Ch::Button => (r.button_switch_mv as i32, r.raw.button_switch),
        Ch::Io9 => (r.p2_mv? as i32, r.raw.p2),
        Ch::Io7 => (r.p0_mv? as i32, r.raw.p0),
        Ch::Io10 => (r.p3_mv? as i32, r.raw.p3),
        Ch::Sbu1 | Ch::Sbu2 => return None,
    })
}

/// 12 bit converter, values in milli units: V (VDDA up to 3.6 V, VBUS_IN through the PA3 divider up to 25.8 V),
/// A (+-7.2 A), m°C.
const fn info(index: usize) -> ChannelInfo<'static> {
    let (_, name, unit) = CHANNELS[index];
    let (min, max) = match unit {
        Unit::Volt if index == 0 => (0, 25_800),
        Unit::Volt => (0, 3_600),
        Unit::Ampere => (-7_200, 7_200),
        Unit::Celsius => (-40_000, 125_000),
    };
    ChannelInfo {
        name: MaxLen::new_unchecked(name),
        unit: unit.expr(),
        prefix: Prefix::Milli,
        min: NumericValue::I32(min),
        max: NumericValue::I32(max),
        bits: 12,
        // nominal until FW-9
        calibrated: false,
        // streams and captures don't exist yet (AdcStream, later)
        raw_to_value: None,
    }
}

static CHANNEL_INFO: [ChannelInfo<'static>; 10] = [
    info(0),
    info(1),
    info(2),
    info(3),
    info(4),
    info(5),
    info(6),
    info(7),
    info(8),
    info(9),
];

impl ServerState {
    // ww_adc::Adc at adc

    pub(super) async fn adc_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        channel: u8,
    ) -> RpcResult<Result<Reading, AdcError<'_>>> {
        let Some(&(ch, _, _)) = CHANNELS.get(channel as usize) else {
            return Ready(Err(AdcError::NoSuchChannel));
        };
        if matches!(ch, Ch::Sbu1 | Ch::Sbu2) {
            return Ready(Err(AdcError::Common(CommonError::Busy)));
        }
        let mut r = readings().await;
        if matches!(ch, Ch::Io9 | Ch::Io7 | Ch::Io10)
            && r.is_none_or(|r| value_of(ch, &r).is_none())
        {
            adc::set_channels(ChannelSet::Extended);
            let deadline = Instant::now() + SWITCH_WAIT;
            r = loop {
                if let Some(r) = adc::latest()
                    && value_of(ch, &r).is_some()
                {
                    break Some(r);
                }
                if Instant::now() >= deadline {
                    break None;
                }
                Timer::after(Duration::from_millis(2)).await;
            };
        }
        let Some(r) = r else {
            return Ready(Err(AdcError::Common(CommonError::Timeout)));
        };
        let Some((value, raw)) = value_of(ch, &r) else {
            return Ready(Err(AdcError::Common(CommonError::Timeout)));
        };
        let t_ms = r.t_ms;
        Ready(Ok(Reading {
            value: NumericValue::I32(value),
            raw: raw as u32,
            t: Second::milli(t_ms as i32),
        }))
    }

    pub(super) fn adc_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<AdcCapabilities<'_>> {
        Ready(AdcCapabilities {
            channels: MaxLen::new_unchecked(RefVec::Slice {
                slice: &CHANNEL_INFO,
            }),
        })
    }
}
