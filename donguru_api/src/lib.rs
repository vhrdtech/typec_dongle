//! WireWeaver API of the donguru firmware (`donguru_api/` in the repo root), served by `fw/donguru/src/ww.rs`.
//!
//! The crate name and version identify the API on the wire (`DONGURU_API_FULL_GID`), so bump the version with every
//! change and follow the WireWeaver evolution rules to stay compatible with deployed devices and hosts:
//! https://github.com/vhrdtech/wire_weaver/blob/master/docs/evolution/rules.md
//! Data types used in the API go here too, with `#[derive_shrink_wrap]`.
#![no_std]

use wire_weaver::prelude::*;

/// donguru device
#[ww_api_root]
pub trait DonguruApi {
    /// Turn the LED on
    fn led_on();
    /// Turn the LED off
    fn led_off();

    /// Cable orientation to use for the SWD-host pins on the plug SBU lines (FW-15). Takes effect on the next
    /// `DAP_Connect`; does not affect an already connected CMSIS-DAP session.
    #[since = "0.1.1"]
    property!(rw swd_orientation: SwdOrientation);

    /// Release the SWD-host pins to Hi-Z (SWCLK, SWDIO and the SBU level shifter output), independent of any
    /// CMSIS-DAP session. For a test harness to call right before cutting the DUT's power, so nothing on the
    /// plug SBU lines can back-power it (FW-15).
    #[since = "0.1.1"]
    fn swd_release();
}

/// Cable orientation for the SWD-host pins, see [`DonguruApi::swd_orientation`].
#[derive_shrink_wrap(borrowed, derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum SwdOrientation {
    /// SBU1 = SWCLK, SBU2 = SWDIO
    Normal,
    /// SBU1 = SWDIO, SBU2 = SWCLK
    Swapped,
}
