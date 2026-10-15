//! `gpio[]`: GPIO banks, 0 = plug SBU1, SBU2, A2, B2; 1 = header IO7..IO10 (`ww_gpio::Bank`, API-15).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // Both banks of the plan are there, with 4 pins each; their items answer Unimplemented until API-15.
    pub(super) fn valid_indices_root_gpio(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..2)
    }

    // ww_gpio::Bank at gpio[]

    pub(super) fn valid_indices_root_gpio_pin(
        &mut self,
        _index_chain: [UNib32; 1],
    ) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..4)
    }

    pub(super) async fn gpio_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_gpio::BankCapabilities<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_gpio::BankConfig,
    ) -> RpcResult<Result<ww_gpio::BankConfig, ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<&'_ str> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_read_inputs(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _pins: wire_weaver::shrink_wrap::MaxLen<
            wire_weaver::shrink_wrap::RefVec<'_, wire_weaver::shrink_wrap::UNib32>,
            64,
        >,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<
                wire_weaver::shrink_wrap::RefVec<'_, ww_gpio::Level>,
                64,
            >,
            ww_gpio::GpioError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    pub(super) async fn gpio_write_outputs(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _levels: wire_weaver::shrink_wrap::MaxLen<
            wire_weaver::shrink_wrap::RefVec<'_, ww_gpio::PinLevel>,
            64,
        >,
    ) -> RpcResult<Result<(), ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    // ww_gpio::Pin at gpio[].pin[]

    pub(super) async fn gpio_pin_set_output_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _level: ww_gpio::Level,
    ) -> RpcResult<()> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_pin_toggle(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<()> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_pin_output_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::Level> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_pin_input_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::Level> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_pin_event(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _enabled: ww_gpio::IoPinEnabledEvents<'_>,
    ) -> RpcResult<Result<(), ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn close_gpio_pin_event(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) {
    }

    pub(super) async fn stream_gpio_pin_event(
        &mut self,
        _index_chain: [UNib32; 2],
    ) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    pub(super) async fn gpio_pin_set_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _mode: ww_gpio::Mode,
        _initial: Option<ww_gpio::Level>,
    ) -> RpcResult<Result<(), ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_pin_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::Mode> {
        Unimplemented.into()
    }

    pub(super) async fn set_gpio_pin_pull(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _value: ww_gpio::Pull,
    ) -> SetResult<ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_gpio_pin_pull(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> GetResult<ww_gpio::Pull, ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_gpio_pin_speed(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _value: ww_gpio::Speed,
    ) -> SetResult<ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_gpio_pin_speed(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> GetResult<ww_gpio::Speed, ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn gpio_pin_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::PinCapabilities<'_>> {
        Unimplemented.into()
    }
}
