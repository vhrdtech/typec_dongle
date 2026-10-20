//! `hub`: hub state and recovery (`Hub`, API-17).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Hub at hub

    pub(super) fn get_hub_state(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::HubState, ()> {
        Unimplemented.into()
    }

    pub(super) fn hub_recover(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        Unimplemented.into()
    }
}
