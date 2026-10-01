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
}
