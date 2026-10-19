//! `header`: J101 VBUS_SW switch and the relay (`Header`, API-11).

use super::ServerState;
use crate::power::{self, Which};
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Header at header

    pub(super) fn get_header_vbus_sw(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusState, ()> {
        Value(power::state(Which::Header))
    }

    pub(super) fn set_header_vbus_sw_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::VbusMode,
    ) -> SetResult<CommonError<'_>> {
        match power::set_mode(Which::Header, value) {
            Ok(()) => {
                self.mark_changed_header_vbus_sw_mode();
                self.mark_changed_header_vbus_sw();
                Set
            }
            Err(e) => SetError(super::plug::mode_error(e)),
        }
    }

    pub(super) fn get_header_vbus_sw_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, CommonError<'_>> {
        Value(power::mode(Which::Header))
    }

    pub(super) fn set_header_relay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: bool,
    ) -> SetResult<()> {
        power::set_relay(value);
        self.mark_changed_header_relay();
        Set
    }

    pub(super) fn get_header_relay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<bool, ()> {
        Value(power::relay())
    }
}
