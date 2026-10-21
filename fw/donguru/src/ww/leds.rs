//! `leds`: the power and status LEDs as one `ww_indication::Indication` (API-9), shown through `src/led.rs`.
//! RAM only: mode and brightness are back to `Normal` after a reset until the settings store (API-19). The peaks and
//! the identify bookkeeping are `bedrock_led::indication`, shared with the other bedrock boards.

use super::ServerState;
use crate::led::{self, DEFAULT_PEAK, Led};
use bedrock_led::indication::{Identify, PEAKS, Peaks};
use cnt::cnt;
use embassy_time::Instant;
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use ww_indication::{Brightness, Mode};
use ww_si::ww_numeric::NumericValue;
use ww_si::{Prefix, Second};

/// `leds.NAMES`, in the order of `crate::led::Led`.
pub(super) const NAMES: RefVec<'static, &'static str> = RefVec::Slice {
    slice: &["power", "status"],
};

/// Perceptual peak per brightness (see led.rs): the bedrock defaults with a dimmer `Low`.
const LED_PEAKS: Peaks = Peaks { low: 40, ..PEAKS };

/// What the API last set, `led.rs` only takes it.
pub(super) struct LedsState {
    mode: Mode,
    brightness: Brightness,
    /// The running identify, for `Busy`.
    identify: Identify<Instant>,
}

impl LedsState {
    pub(super) const fn new() -> Self {
        LedsState {
            mode: Mode::Normal,
            brightness: Brightness::Normal,
            identify: Identify::new(),
        }
    }
}

impl ServerState {
    // ww_indication::Indication at leds

    /// `OnlyEssential` keeps the power LED: the status LED is decoration.
    pub(super) fn set_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_indication::Mode,
    ) -> SetResult<()> {
        let (power, status) = match value {
            Mode::AllOff => (false, false),
            Mode::OnlyEssential => (true, false),
            Mode::Normal => (true, true),
            // a mode from a newer host: what it means is unknown, show everything
            _ => {
                cnt!(leds_mode_unknown: u32);
                (true, true)
            }
        };
        led::set_visible(Led::Power, power);
        led::set_visible(Led::Status, status);
        self.leds.mode = value;
        Set
    }

    pub(super) fn get_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Mode, ()> {
        Value(self.leds.mode)
    }

    pub(super) fn set_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_indication::Brightness,
    ) -> SetResult<()> {
        let peak = LED_PEAKS.peak(value).unwrap_or_else(|| {
            cnt!(leds_brightness_unknown: u32);
            DEFAULT_PEAK
        });
        led::set_peak(peak);
        self.leds.brightness = value;
        Set
    }

    pub(super) fn get_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Brightness, ()> {
        Value(self.leds.brightness)
    }

    /// Both LEDs blink at full brightness, whatever the mode; `Busy` while one runs.
    pub(super) fn leds_identify(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        duration: ww_si::Second,
    ) -> RpcResult<Result<(), wire_weaver::ww_error::CommonError<'_>>> {
        let now = Instant::now();
        if self.leds.identify.busy(now) {
            return Ready(Err(CommonError::Busy));
        }
        let Some(ms) = to_ms(&duration) else {
            return Ready(Err(CommonError::InvalidArgument {
                arg: 0,
                reason: MaxLen::new_unchecked("not an integer number of s, ms or us"),
            }));
        };
        led::identify(self.leds.identify.start(now, ms));
        Ready(Ok(()))
    }
}

/// A duration in milliseconds, for the integer encodings a host sends. No float or 64-bit math (M0+).
fn to_ms(d: &Second) -> Option<u32> {
    let v = match d.value {
        NumericValue::U8(v) => v as u32,
        NumericValue::U32(v) => v,
        NumericValue::I32(v) => u32::try_from(v).ok()?,
        _ => return None,
    };
    match d.prefix {
        Prefix::Unit => Some(v.saturating_mul(1000)),
        Prefix::Milli => Some(v),
        Prefix::Micro => Some(v / 1000),
        _ => None,
    }
}
