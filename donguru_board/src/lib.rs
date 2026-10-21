//! Board logic of the donguru firmware (B153, STM32G0B1): the parts of `fw/donguru` that need no hardware, kept
//! apart so they are unit-tested on the host, as `donguru_dap` does for the CMSIS-DAP framing. The firmware owns the
//! pins and peripherals and calls through here with what it read.
//!
//! - [`boot`]: the early boot: ROM bootloader request, boot-loop count, start after a jump, the hub hold on RTC_OUT1 (rev A), the
//!   USB serial number the ROM reports.
//! - [`hub`]: the USB hub U301, board variant from its strap and the per-variant hub configuration.
//! - [`power`]: the VBUS switches, their modes and the guard.
#![no_std]

pub mod boot;
pub mod hub;
pub mod power;
