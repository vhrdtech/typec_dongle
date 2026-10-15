//! `leds`: the power and status LEDs as one `ww_indication::Indication` (API-9).

use super::ServerState;
use wire_weaver::prelude::*;

/// `leds.NAMES`, in the order of `crate::led::Led`.
pub(super) const NAMES: RefVec<'static, &'static str> = RefVec::Slice {
    slice: &["power", "status"],
};

impl ServerState {
    // ww_indication::Indication at leds

    pub(super) async fn set_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: ww_indication::Mode,
    ) -> SetResult<()> {
        Unimplemented.into()
    }

    pub(super) async fn get_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Mode, ()> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["leds.brightness"]
    pub(super) async fn set_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: ww_indication::Brightness,
    ) -> SetResult<()> {
        Unimplemented.into()
    }

    pub(super) async fn get_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Brightness, ()> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["leds.identify"]
    pub(super) async fn leds_identify(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _duration: ww_si::Second,
    ) -> RpcResult<Result<(), wire_weaver::ww_error::CommonError<'_>>> {
        Unimplemented.into()
    }
}
