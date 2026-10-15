//! `uart[]`: 0 = plug SBU, 1 = header IO7/IO8 (`ww_uart::Uart`, API-14).

use super::ServerState;
use wire_weaver::prelude::*;

/// Indices of `uart[]`: 0 = plug SBU, 1 = header.
pub(super) const COUNT: usize = 2;

impl ServerState {
    // Both indices of the plan are there, their items answer Unimplemented until API-14.
    pub(super) fn valid_indices_root_uart(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..COUNT as u32)
    }

    // ww_uart::Uart at uart[]

    pub(super) async fn uart_rx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<()> {
        RpcResult::Unimplemented
    }

    pub(super) async fn close_uart_rx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) {
    }

    pub(super) async fn stream_uart_rx(
        &mut self,
        _index_chain: [UNib32; 1],
    ) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    pub(super) fn credit_uart_tx(
        &mut self,
        index_chain: [UNib32; 1],
    ) -> &mut ww_client_server::credit::SinkCredit {
        // the generated code checked the index against `valid_indices_root_uart`
        let i = (index_chain[0].0 as usize).min(COUNT - 1);
        &mut self.uart_tx_credit[i]
    }

    pub(super) async fn sideband_uart_tx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _sideband: ww_client_server::StreamSideband,
    ) -> Option<ww_client_server::StreamSideband> {
        // TODO: None sends no reply
        None
    }

    pub(super) async fn write_uart_tx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _data: &wire_weaver::shrink_wrap::tail_bytes::TailBytes<'_>,
    ) {
    }

    pub(super) async fn uart_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_uart::Capabilities<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn uart_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_uart::UartConfig,
    ) -> RpcResult<Result<ww_uart::UartConfig, ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn uart_set_pin_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _pin: ww_uart::Pin,
        _is_high: bool,
    ) -> RpcResult<Result<(), ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn uart_send_break(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _duration: ww_si::Second,
    ) -> RpcResult<Result<(), ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn uart_tx_pending(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<u32> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["uart.monitor"]
    pub(super) async fn uart_monitor(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_uart::MonitorConfig,
    ) -> RpcResult<Result<ww_uart::MonitorConfig, ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn close_uart_monitor(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) {
    }

    pub(super) async fn stream_uart_monitor(
        &mut self,
        _index_chain: [UNib32; 1],
    ) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }
}
