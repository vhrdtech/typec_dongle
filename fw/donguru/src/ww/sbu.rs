//! `sbu`: owner of the plug SBU lines, V_SBU, pull-ups, release (`Sbu`, API-14).

use super::ServerState;
use crate::swd;
use cnt::cnt;
use defmt::info;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Sbu at sbu

    pub(super) fn get_sbu_function(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::SbuFunction, CommonError<'_>> {
        Unimplemented.into()
    }

    // SWD-host pins only so far (FW-15): SWCLK / SWDIO Hi-Z, level shifter off, CC drive off.
    pub(super) async fn sbu_release(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        info!("sbu release");
        cnt!(sbu_release_calls: u32);
        swd::release().await;
        Ready(())
    }

    pub(super) fn get_sbu_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Volt, CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) fn set_sbu_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: bool,
    ) -> SetResult<wire_weaver::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) fn get_sbu_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<bool, wire_weaver::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) fn sbu_route_usb(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _enabled: bool,
    ) -> RpcResult<Result<(), CommonError<'_>>> {
        Unimplemented.into()
    }
}
