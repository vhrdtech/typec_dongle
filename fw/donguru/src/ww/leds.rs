//! `leds`: the power and status LEDs as one `ww_indication::Indication` (API-9), shown through `src/led.rs`.
//! RAM only: mode and brightness are back to `Normal` after a reset until the settings store (API-19).

use super::ServerState;
use crate::led::{self, DEFAULT_PEAK, Led};
use cnt::cnt;
use embassy_time::{Duration, Instant};
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use ww_indication::{Brightness, Mode};
use ww_si::ww_numeric::NumericValue;
use ww_si::{Prefix, Second};

/// `leds.NAMES`, in the order of `crate::led::Led`.
pub(super) const NAMES: RefVec<'static, &'static str> = RefVec::Slice {
    slice: &["power", "status"],
};

/// Longest identify, longer requests are cut to it.
const IDENTIFY_MAX_MS: u32 = 10 * 60 * 1000;

/// What the API last set, `led.rs` only takes it.
pub(super) struct LedsState {
    mode: Mode,
    brightness: Brightness,
    /// End of the running identify, for `Busy`.
    identify_until: Instant,
}

impl LedsState {
    pub(super) const fn new() -> Self {
        LedsState {
            mode: Mode::Normal,
            brightness: Brightness::Normal,
            identify_until: Instant::from_ticks(0),
        }
    }
}

impl ServerState {
    // ww_indication::Indication at leds

    /// `OnlyEssential` keeps the power LED: the status LED is decoration.
    pub(super) async fn set_leds_mode(
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

    pub(super) async fn get_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Mode, ()> {
        Value(self.leds.mode)
    }

    pub(super) async fn set_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_indication::Brightness,
    ) -> SetResult<()> {
        // perceptual peak, see led.rs
        let peak = match value {
            Brightness::Lowest => 8,
            Brightness::Low => 40,
            Brightness::Normal => DEFAULT_PEAK,
            Brightness::High => 200,
            Brightness::Highest => 255,
            _ => {
                cnt!(leds_brightness_unknown: u32);
                DEFAULT_PEAK
            }
        };
        led::set_peak(peak);
        self.leds.brightness = value;
        Set
    }

    pub(super) async fn get_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Brightness, ()> {
        Value(self.leds.brightness)
    }

    /// Both LEDs blink at full brightness, whatever the mode; `Busy` while one runs.
    pub(super) async fn leds_identify(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        duration: ww_si::Second,
    ) -> RpcResult<Result<(), wire_weaver::ww_error::CommonError<'_>>> {
        let now = Instant::now();
        if now < self.leds.identify_until {
            return Ready(Err(CommonError::Busy));
        }
        let Some(ms) = to_ms(&duration) else {
            return Ready(Err(CommonError::InvalidArgument {
                arg: 0,
                reason: MaxLen::new_unchecked("not an integer number of s, ms or us"),
            }));
        };
        let ms = ms.min(IDENTIFY_MAX_MS);
        led::identify(ms);
        self.leds.identify_until = now + Duration::from_millis(ms as u64);
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
