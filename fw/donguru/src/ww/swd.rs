//! `swd`: SWD-host mode and CMSIS-DAP session (`Swd`, API-17).

use super::ServerState;
use crate::swd;
use cnt::cnt;
use defmt::{info, warn};
use donguru_api::SwdMode;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Swd at swd

    pub(super) fn set_swd_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        mode: SwdMode,
    ) -> SetResult<CommonError<'_>> {
        // a mode from a newer host: refuse it, keep the current one
        if let SwdMode::Unknown(n) = mode {
            warn!("swd mode {} unknown, refused", n);
            cnt!(swd_mode_unknown: u32);
            return SetError(CommonError::InvalidArgument {
                arg: 0,
                reason: MaxLen::new_unchecked("unknown mode"),
            });
        }
        info!("swd mode set");
        swd::set_mode(mode);
        Set
    }

    pub(super) fn get_swd_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<SwdMode, CommonError<'_>> {
        Value(swd::mode())
    }

    pub(super) fn get_swd_session(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::SwdSession, ()> {
        Unimplemented.into()
    }
}
