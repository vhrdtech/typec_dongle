//! `plug`: plug VBUS, data switch, orientation, CC (`Plug`, API-11), and the PD source `plug.pd` (API-30, `src/pd.rs`).

use super::ServerState;
use crate::power::{self, Which};
use crate::{pd, swd};
use defmt::info;
use donguru_api::{DataPath, PowerError};
use donguru_board::pd::OfferError;
use donguru_board::power::{CycleError, ModeError};
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Plug at plug

    pub(super) fn get_plug_vbus(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusState, ()> {
        Value(power::state(Which::Plug))
    }

    pub(super) fn set_plug_vbus_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::VbusMode,
    ) -> SetResult<CommonError<'_>> {
        if pd::holds_vbus() {
            return SetError(PD_HOLDS_VBUS);
        }
        let res = power::set_mode(Which::Plug, value);
        // a refused write changes nothing; a write that went through may have re-armed a tripped switch
        if res.is_ok() {
            self.mark_changed_plug_vbus_mode();
            self.mark_changed_plug_vbus();
        }
        match res {
            Ok(()) => Set,
            Err(e) => SetError(mode_error(e)),
        }
    }

    pub(super) fn get_plug_vbus_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, CommonError<'_>> {
        Value(power::mode(Which::Plug))
    }

    // deferred (`method_model` in mod.rs): answered by server_task with the result from power::take_cycle_done
    pub(super) async fn plug_vbus_cycle(
        &mut self,
        cx: &mut Context<'_, impl EventOut>,
        off: ww_si::Second,
        release_sbu: bool,
    ) -> RpcResult<Result<donguru_api::VbusState, PowerError<'_>>> {
        if pd::holds_vbus() {
            return Ready(Err(PowerError::Common(PD_HOLDS_VBUS)));
        }
        match power::start_cycle(&off, release_sbu, cx.reply_to()).await {
            Ok(()) => {
                self.mark_changed_plug_vbus();
                Deferred
            }
            Err(e) => Ready(Err(PowerError::Common(match e {
                CycleError::WrongState => CommonError::WrongState {
                    reason: MaxLen::new_unchecked("vbus_mode is ForceOff"),
                },
                CycleError::Busy => CommonError::Refused {
                    reason: MaxLen::new_unchecked("a cycle is running"),
                },
                CycleError::Invalid => CommonError::InvalidArgument {
                    arg: 0,
                    reason: MaxLen::new_unchecked("off: 0..=60 s"),
                },
            }))),
        }
    }

    pub(super) fn set_plug_data(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: DataPath,
    ) -> SetResult<CommonError<'_>> {
        power::set_data(value);
        self.mark_changed_plug_data();
        Set
    }

    pub(super) fn get_plug_data(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<DataPath, CommonError<'_>> {
        Value(power::data())
    }

    pub(super) fn set_plug_orientation(
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

    pub(super) fn get_plug_orientation(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::Orientation, ()> {
        Value(swd::orientation())
    }

    pub(super) fn get_plug_cc(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::CcState, ()> {
        Unimplemented.into()
    }
}

/// `plug.vbus_mode` and `vbus_cycle` while the PD source runs.
const PD_HOLDS_VBUS: CommonError<'static> = CommonError::Refused {
    reason: MaxLen::new_unchecked("the PD source drives plug VBUS (plug.pd)"),
};

impl ServerState {
    // donguru_api::PlugPd at plug.pd

    pub(super) fn set_plug_pd_source(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::PdSource,
    ) -> SetResult<CommonError<'_>> {
        match pd::set(&value) {
            Ok(()) => Set,
            Err(pd::SetError::InUse(holder)) => SetError(CommonError::InUse {
                by: MaxLen::new_unchecked(holder.name()),
            }),
            Err(pd::SetError::Invalid(e)) => SetError(CommonError::InvalidArgument {
                arg: 0,
                reason: MaxLen::new_unchecked(match e {
                    OfferError::Current => "max_current: 100 mA..=3 A",
                    OfferError::SenderResponse => "sender_response: 24 ms..=1 s",
                    OfferError::Unknown => "unknown source setting",
                }),
            }),
        }
    }

    pub(super) fn get_plug_pd_source(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::PdSource, CommonError<'_>> {
        Value(pd::setting())
    }

    pub(super) fn get_plug_pd_state(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::PdState, ()> {
        Value(pd::phase().to_api())
    }

    pub(super) fn plug_pd_messages(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        Ready(())
    }

    pub(super) fn close_plug_pd_messages(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    pub(super) fn stream_plug_pd_messages(&mut self) -> &mut ww_stream::tx::Counted {
        &mut self.pd_messages
    }

    /// Sends the records the PD task logged, a few per turn; with no host they count as lost, so a host that opens
    /// the stream later sees how many it missed.
    pub(super) async fn pd_pump(&mut self, server: &mut super::UsbServer) {
        let mut budget = PD_RECORDS_PER_TURN;
        while let Some((record, lost)) = pd::take_record() {
            if lost > 0 {
                self.pd_messages.lost(u64::from(lost));
            }
            let item = record.to_api();
            let t = item.time;
            let mut out = server.sink();
            if super::server_impl::stream_data_ser()
                .plug()
                .pd()
                .messages_send(&mut self.pd_messages, &item, 1, Some(t), &mut out)
                .await
                .is_err()
            {
                cnt::cnt!(pd_send_errors: u32);
            }
            budget -= 1;
            if budget == 0 {
                // more may be waiting: come back after the requests had their turn
                crate::ww::OBSERVE.wake();
                break;
            }
        }
    }
}

/// Records of `plug.pd.messages` sent per turn of the server task.
const PD_RECORDS_PER_TURN: usize = 8;

/// A refused mode write (`plug.vbus_mode`, `header.vbus_sw_mode`) as the property's error.
pub(super) fn mode_error(e: ModeError) -> CommonError<'static> {
    match e {
        ModeError::Invalid => CommonError::InvalidArgument {
            arg: 0,
            reason: MaxLen::new_unchecked("limits must be positive mV / mA / mW, known mode"),
        },
        ModeError::OverVoltage { .. } => CommonError::Refused {
            reason: MaxLen::new_unchecked("VBUS is over max_voltage"),
        },
        ModeError::NoMeasurement => CommonError::Refused {
            reason: MaxLen::new_unchecked("no VBUS measurement yet"),
        },
        ModeError::CurrentNotMeasured => CommonError::InvalidArgument {
            arg: 0,
            reason: MaxLen::new_unchecked("Auto: this switch's current isn't measured"),
        },
    }
}

/// The answer of a deferred `plug.vbus_cycle`.
pub(super) fn cycle_result(
    result: Result<donguru_api::VbusState, ModeError>,
) -> Result<donguru_api::VbusState, PowerError<'static>> {
    result.map_err(|e| match e {
        ModeError::OverVoltage { measured_mv } => PowerError::OverVoltage {
            measured: donguru_board::power::volt(measured_mv),
        },
        e => PowerError::Common(mode_error(e)),
    })
}
