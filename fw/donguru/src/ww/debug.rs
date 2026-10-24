//! `debug`: bring-up and production test (`Bringup`, API-18). Behind the `debug-api` cargo feature (on by default,
//! decision D2): without it every handler answers `Unimplemented` and the API stays the same.
//!
//! Pins go through `src/pins.rs` (refused list, claim table, electrical rules from `donguru_board::pins`), the hub's
//! SMBus through the hub watchdog task that owns the bus (`src/hub.rs`), UCPD through `src/ucpd.rs`. No memory
//! writes: `reg_read` reads peripheral space only.

use super::ServerState;
use crate::{hub, led, pins, ucpd};
use cnt::cnt;
use donguru_api::{DebugError, LedMode, PinDump, PinState, PortRegs, UcpdState};
use donguru_board::pins::{Change, Denied, HUB_REG_STATUS_COMMAND, check_reg_read};
use wire_weaver::prelude::*;

/// Answer buffers of the debug level: the handlers answer borrowed slices of these.
pub(super) struct DebugState {
    ports: [PortRegs; 6],
    bytes: [u8; hub::SMBUS_BLOCK_MAX],
    words: [u32; donguru_board::pins::REG_READ_MAX_WORDS as usize],
    /// Number of the last SMBus request, to tell its answer from a late one.
    seq: u8,
}

impl DebugState {
    pub(super) fn new() -> Self {
        DebugState {
            ports: pins::dump(),
            bytes: [0; hub::SMBUS_BLOCK_MAX],
            words: [0; donguru_board::pins::REG_READ_MAX_WORDS as usize],
            seq: 0,
        }
    }
}

/// The API error of a refused change.
fn denied(d: Denied) -> DebugError<'static> {
    DebugError::Common(match d {
        Denied::Invalid { arg, reason } => CommonError::InvalidArgument {
            arg,
            reason: MaxLen::new_unchecked(reason),
        },
        Denied::Refused(reason) => CommonError::Refused {
            reason: MaxLen::new_unchecked(reason),
        },
        Denied::InUse(holder) => CommonError::InUse {
            by: MaxLen::new_unchecked(holder.name()),
        },
    })
}

fn smbus_error(e: hub::SmbusError) -> DebugError<'static> {
    match e {
        hub::SmbusError::Nack => DebugError::Nack,
        hub::SmbusError::Timeout => DebugError::Common(CommonError::Timeout),
        // arbitration or a bus error: the hub or the watchdog's probe was on the bus
        hub::SmbusError::Bus => DebugError::Common(CommonError::Busy),
    }
}

fn invalid(arg: u8, reason: &'static str) -> DebugError<'static> {
    denied(Denied::Invalid { arg, reason })
}

impl ServerState {
    // donguru_api::Bringup at debug

    pub(super) fn debug_pin_set(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        pin: donguru_api::McuPin,
        mode: donguru_api::PinMode,
        level: Option<ww_gpio::Level>,
    ) -> RpcResult<Result<PinState, DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        let level = match level {
            None => None,
            Some(ww_gpio::Level::Low) => Some(false),
            Some(ww_gpio::Level::High) => Some(true),
            #[allow(unreachable_patterns)] // a level of a newer host
            Some(_) => return Ready(Err(invalid(2, "unknown level"))),
        };
        cnt!(debug_pin_set_calls: u32);
        Ready(pins::set(pin, Change::Mode { mode, level }).map_err(denied))
    }

    pub(super) fn debug_pin_get(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        pin: donguru_api::McuPin,
    ) -> RpcResult<Result<PinState, DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        Ready(pins::get(pin).map_err(denied))
    }

    pub(super) fn debug_pin_af(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        pin: donguru_api::McuPin,
        af: u8,
    ) -> RpcResult<Result<PinState, DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        cnt!(debug_pin_set_calls: u32);
        Ready(pins::set(pin, Change::Af(af)).map_err(denied))
    }

    pub(super) fn debug_pins(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<PinDump<'_>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        self.debug.ports = pins::dump();
        Ready(PinDump {
            ports: MaxLen::new_unchecked(RefVec::Slice {
                slice: &self.debug.ports,
            }),
        })
    }

    pub(super) fn debug_boot_pins(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<PinDump<'_>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        // captured first thing in main: always there by the time a host asks
        let slice: &[PortRegs] = match pins::boot() {
            Some(ports) => {
                self.debug.ports = ports;
                &self.debug.ports
            }
            None => &[],
        };
        Ready(PinDump {
            ports: MaxLen::new_unchecked(RefVec::Slice { slice }),
        })
    }

    pub(super) async fn debug_hub_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        reg: u8,
        len: u8,
    ) -> RpcResult<Result<MaxLen<RefVec<'_, u8>, 32>, DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        if len == 0 || len as usize > hub::SMBUS_BLOCK_MAX {
            return Ready(Err(invalid(1, "len 1..=32")));
        }
        self.debug.seq = self.debug.seq.wrapping_add(1);
        let request = hub::SmbusRequest {
            seq: self.debug.seq,
            write: false,
            reg,
            len,
            data: [0; hub::SMBUS_BLOCK_MAX],
        };
        match hub::smbus(request).await {
            Ok(data) => {
                self.debug.bytes = data;
                Ready(Ok(MaxLen::new_unchecked(RefVec::new_bytes(
                    &self.debug.bytes[..len as usize],
                ))))
            }
            Err(e) => Ready(Err(smbus_error(e))),
        }
    }

    pub(super) async fn debug_hub_write(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        reg: u8,
        data: MaxLen<RefVec<'_, u8>, 32>,
    ) -> RpcResult<Result<(), DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        if reg == HUB_REG_STATUS_COMMAND {
            return Ready(Err(denied(Denied::Refused(
                "STATUS/COMMAND drops our own USB: use hub.recover",
            ))));
        }
        let bytes = data.as_slice();
        if bytes.len() > hub::SMBUS_BLOCK_MAX {
            return Ready(Err(invalid(1, "at most 32 bytes")));
        }
        self.debug.seq = self.debug.seq.wrapping_add(1);
        let mut request = hub::SmbusRequest {
            seq: self.debug.seq,
            write: true,
            reg,
            len: bytes.len() as u8,
            data: [0; hub::SMBUS_BLOCK_MAX],
        };
        request.data[..bytes.len()].copy_from_slice(bytes);
        cnt!(debug_hub_writes: u32);
        Ready(hub::smbus(request).await.map(|_| ()).map_err(smbus_error))
    }

    pub(super) fn debug_ucpd(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        port: donguru_api::UcpdPort,
    ) -> RpcResult<UcpdState> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        Ready(ucpd::state(port))
    }

    pub(super) fn debug_ucpd_role(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        port: donguru_api::UcpdPort,
        role: donguru_api::CcRole,
    ) -> RpcResult<Result<UcpdState, DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        cnt!(debug_ucpd_role_calls: u32);
        Ready(ucpd::set_role(port, role).map_err(|d| match d {
            // the CC pins of a running function: an SWD session's stopgap CC drive (FW-17)
            Denied::InUse(holder) => denied(Denied::Refused(match holder {
                donguru_board::pins::Holder::SwdCc => "CC pins in use by an SWD session",
                _ => "CC pins in use",
            })),
            d => denied(d),
        }))
    }

    pub(super) fn debug_led_set(
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

    pub(super) fn debug_reg_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        addr: u32,
        words: u8,
    ) -> RpcResult<Result<MaxLen<RefVec<'_, u32>, 32>, DebugError<'_>>> {
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        if let Err(d) = check_reg_read(addr, words) {
            return Ready(Err(denied(d)));
        }
        for (i, w) in self.debug.words[..words as usize].iter_mut().enumerate() {
            // SAFETY: check_reg_read: word aligned, inside a peripheral's block of the memory map (a read of a
            // reserved address would fault); reads only, side effects of reading a data register are the host's
            *w = unsafe { core::ptr::read_volatile((addr + 4 * i as u32) as *const u32) };
        }
        Ready(Ok(MaxLen::new_unchecked(RefVec::Slice {
            slice: &self.debug.words[..words as usize],
        })))
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

    pub(super) fn debug_uart_replay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        capture: donguru_api::ReplayCapture,
    ) -> RpcResult<Result<u32, donguru_api::DebugError<'_>>> {
        use donguru_api::DebugError;
        if !cfg!(feature = "debug-api") {
            return RpcResult::Unimplemented;
        }
        #[cfg(feature = "uart-replay")]
        {
            let Some(data) = crate::uart::capture(capture) else {
                // a capture of a newer API
                return Ready(Err(DebugError::Common(CommonError::InvalidArgument {
                    arg: 0,
                    reason: MaxLen::new_unchecked("unknown capture"),
                })));
            };
            crate::uart::replay(data);
            Ready(Ok(data.len() as u32))
        }
        #[cfg(not(feature = "uart-replay"))]
        {
            let _ = capture;
            Ready(Err(DebugError::Common(CommonError::Unsupported)))
        }
    }
}
