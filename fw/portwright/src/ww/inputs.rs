//! `inputs`: button and slide switch (`Inputs`, API-13).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // portwright_api::Inputs at inputs

    pub(super) fn get_inputs_button(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<portwright_api::ButtonState, CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) fn get_inputs_switch(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<portwright_api::SwitchPosition, CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) fn inputs_events(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        RpcResult::Unimplemented
    }

    pub(super) fn close_inputs_events(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    pub(super) fn stream_inputs_events(&mut self) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    pub(super) fn set_inputs_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: portwright_api::ButtonAction,
    ) -> SetResult<wire_weaver::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) fn get_inputs_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<portwright_api::ButtonAction, wire_weaver::CommonError<'_>> {
        Unimplemented.into()
    }
}
