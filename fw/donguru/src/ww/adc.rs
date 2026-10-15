//! `adc`: ADC1 channels by index with names (`ww_adc::Adc`, API-12).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // ww_adc::Adc at adc

    pub(super) async fn adc_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _channel: u8,
    ) -> RpcResult<Result<ww_adc::Reading, ww_adc::AdcError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn adc_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<ww_adc::AdcCapabilities<'_>> {
        Unimplemented.into()
    }
}
