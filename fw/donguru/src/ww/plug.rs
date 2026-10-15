//! `plug`: plug VBUS, data switch, orientation, CC (`Plug`, API-11).

use super::ServerState;
use crate::swd;
use defmt::info;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Plug at plug

    pub(super) async fn get_plug_vbus(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusState, ()> {
        Unimplemented.into()
    }

    pub(super) async fn set_plug_vbus_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: donguru_api::VbusMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_plug_vbus_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn plug_vbus_cycle(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _off: ww_si::Second,
        _release_sbu: bool,
    ) -> RpcResult<Result<donguru_api::VbusState, donguru_api::PowerError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn set_plug_data(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: donguru_api::DataPath,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_plug_data(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::DataPath, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_plug_orientation(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        orientation: donguru_api::Orientation,
    ) -> SetResult<()> {
        // only SWD uses it so far (FW-15): from its next DAP_Connect, not for a session already connected
        info!("plug orientation set");
        swd::set_orientation(orientation);
        self.mark_changed_plug_orientation();
        Set
    }

    pub(super) async fn get_plug_orientation(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::Orientation, ()> {
        Value(swd::orientation())
    }

    pub(super) async fn get_plug_cc(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::CcState, ()> {
        Unimplemented.into()
    }
}
