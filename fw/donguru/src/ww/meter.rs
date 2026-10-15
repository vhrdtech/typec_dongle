//! `meter`: the power meter in SI units (`PowerMeter`, API-12).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::PowerMeter at meter

    pub(super) async fn get_meter_reading(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterReading, ()> {
        Unimplemented.into()
    }

    pub(super) async fn get_meter_temperature(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::DegreeCelsius, ()> {
        Unimplemented.into()
    }

    pub(super) async fn get_meter_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterTotals, ()> {
        Unimplemented.into()
    }

    pub(super) async fn meter_reset_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        Unimplemented.into()
    }

    pub(super) async fn set_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: ww_si::Second,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Second, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: donguru_api::MeterDeadband,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterDeadband, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_meter_calibration(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterCalibration, ()> {
        Unimplemented.into()
    }
}
