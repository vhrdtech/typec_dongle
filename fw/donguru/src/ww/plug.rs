//! `plug`: plug VBUS, data switch, orientation, CC (`Plug`, API-11).

use super::ServerState;
use crate::power::{self, Which};
use crate::swd;
use defmt::info;
use donguru_api::{DataPath, PowerError};
use donguru_power::{CycleError, ModeError};
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
        let seq = cx.reply_to().map(|r| r.seq).unwrap_or(0);
        match power::start_cycle(&off, release_sbu, seq).await {
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
            measured: donguru_power::volt(measured_mv),
        },
        e => PowerError::Common(mode_error(e)),
    })
}
