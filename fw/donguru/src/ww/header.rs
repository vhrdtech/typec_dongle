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

    pub(super) async fn header_vbus_sw_on(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _guard: donguru_api::VbusGuard,
    ) -> RpcResult<Result<donguru_api::VbusState, donguru_api::PowerError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn header_vbus_sw_off(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<donguru_api::VbusState> {
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
