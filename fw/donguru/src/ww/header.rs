//! `header`: J101 VBUS_SW switch and the relay (`Header`, API-11).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Header at header

    pub(super) async fn get_header_vbus_sw(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusState, ()> {
        Unimplemented.into()
    }

    pub(super) async fn set_header_vbus_sw_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: donguru_api::VbusMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_header_vbus_sw_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_header_relay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: bool,
    ) -> SetResult<()> {
        Unimplemented.into()
    }

    pub(super) async fn get_header_relay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<bool, ()> {
        Unimplemented.into()
    }
}
