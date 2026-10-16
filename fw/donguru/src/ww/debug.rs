//! `debug`: bring-up and production test (`Bringup`, API-18). Behind the `debug-api` cargo feature (on by default,
//! decision D2): without it every handler answers `Unimplemented` and the API stays the same.

use super::ServerState;
use crate::led;
use cnt::cnt;
use donguru_api::LedMode;
use wire_weaver::prelude::*;

impl ServerState {
    // donguru_api::Bringup at debug

    pub(super) async fn debug_pin_set(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _pin: donguru_api::McuPin,
        _mode: donguru_api::PinMode,
        _level: Option<ww_gpio::Level>,
    ) -> RpcResult<Result<donguru_api::PinState, donguru_api::DebugError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_pin_get(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _pin: donguru_api::McuPin,
    ) -> RpcResult<Result<donguru_api::PinState, donguru_api::DebugError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_pin_af(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _pin: donguru_api::McuPin,
        _af: u8,
    ) -> RpcResult<Result<donguru_api::PinState, donguru_api::DebugError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_pins(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<donguru_api::PinDump<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_boot_pins(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<donguru_api::PinDump<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_hub_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _reg: u8,
        _len: u8,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 32>,
            donguru_api::DebugError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    pub(super) async fn debug_hub_write(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _reg: u8,
        _data: wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 32>,
    ) -> RpcResult<Result<(), donguru_api::DebugError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_ucpd(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _port: donguru_api::UcpdPort,
    ) -> RpcResult<donguru_api::UcpdState> {
        Unimplemented.into()
    }

    pub(super) async fn debug_ucpd_role(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _port: donguru_api::UcpdPort,
        _role: donguru_api::CcRole,
    ) -> RpcResult<Result<donguru_api::UcpdState, donguru_api::DebugError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn debug_led_set(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        led: donguru_api::Led,
        mode: donguru_api::LedMode,
    ) -> RpcResult<()> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        let led = match led {
            donguru_api::Led::Power => led::Led::Power,
            donguru_api::Led::Status => led::Led::Status,
            donguru_api::Led::Unknown(_) => return RpcResult::Unimplemented,
        };
        let mode = match mode {
            LedMode::Off => led::Mode::Off,
            LedMode::On => led::Mode::On,
            LedMode::Blink { on_ms, off_ms } => led::Mode::Blink {
                on_ms: on_ms.into(),
                off_ms: off_ms.into(),
            },
            LedMode::Breath { period_ms } => led::Mode::Breath {
                period_ms: period_ms.into(),
            },
            LedMode::Unknown(_) => return RpcResult::Unimplemented,
        };
        cnt!(debug_led_set_calls: u32);
        led::set(led, mode);
        Ready(())
    }

    pub(super) async fn debug_reg_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _addr: u32,
        _words: u8,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u32>, 32>,
            donguru_api::DebugError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    pub(super) async fn debug_log_flood(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        count: u32,
    ) -> RpcResult<()> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        for i in 0..count {
            defmt::info!("flood {=u32} of {=u32}", i + 1, count);
            // other tasks (the ADC) run between the lines; the log isn't pumped before the end (same task), so the
            // flood still overruns the logger's ring as meant
            embassy_futures::yield_now().await;
        }
        Ready(())
    }
}
