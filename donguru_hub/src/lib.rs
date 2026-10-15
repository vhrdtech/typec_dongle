//! USB hub (U301) per-variant configuration for the donguru firmware (`fw/donguru/src/hub.rs`, FW-6).
//!
//! Two board variants share this hub bring-up: Main (2-port USB2512B) and Plus (4-port USB2514B, P2534 #12,
//! not decided/built yet). Which one is fitted was first going to be read from the hub's own power-on-default
//! PID register (HUB-5), but on the one board tested that register reads back as zero regardless of delay (see
//! `fw/donguru/src/hub.rs`'s `Hub::read_power_on_defaults` doc) — in this hub's SMBus-slave strap, the register
//! holds whatever was last written over SMBus, not a factory default. The board-variant signal that does exist
//! is a strap on PA10 (SBU_TO_USB, U901's S): R902, a 10K pull-down, is fitted only on Plus (hw_b153a.md §9),
//! so the net reads low there and high (the MCU's own pull-up) on Main. [`board_variant_from_strap`] is that
//! mapping; the firmware does the PA10 read (dead-battery strobe, pull-up, settle, sample) and calls through
//! here with the result.
//!
//! [`hub_config`] builds the `usb251xb::Config` for a given [`BoardVariant`]: port count and the VID/PID
//! written back come from the `usb251xb::Variant` passed to its builder, and the D+/D- port swap mask
//! (hw_b153a.md §6/§9) only sets the bits for ports that exist on that variant.
//!
//! No embassy and no GPIO/I2C in here — just the mapping and the `usb251xb` crate's pure config builder — so
//! this is unit-tested on the host (`fw/donguru/src/hub.rs` only reads the strap and calls through here), the
//! same way `donguru_dap` keeps the CMSIS-DAP framing apart from the firmware's USB plumbing.
#![no_std]

use donguru_api::BoardVariant;
use usb251xb::register::ConfigByte3;
use usb251xb::{Config, CurrentSensing, Milliamps, Port, PowerSwitching, Variant};

/// Maps the PA10 board-variant strap read (hw_b153a.md §9/§11) to the board variant it identifies: R902 (10K
/// pull-down to GND) is fitted only on Plus, so the net reads low there against the MCU's own ~40k pull-up
/// winning on Main, where nothing else is on the net.
pub fn board_variant_from_strap(pa10_high: bool) -> BoardVariant {
    if pa10_high {
        BoardVariant::Main
    } else {
        BoardVariant::Plus
    }
}

/// `usb251xb` variant to build the config for. [`BoardVariant::Unknown`] and [`BoardVariant::Other`] (a newer
/// board, as a host reads it) are not reachable results of [`board_variant_from_strap`] (the strap always reads
/// high or low), but [`hub_config`] still needs a definite answer for them: falls back to the smaller Main part,
/// same as a genuine detection failure would.
fn usb251xb_variant(variant: BoardVariant) -> Variant {
    match variant {
        BoardVariant::Main | BoardVariant::Unknown | BoardVariant::Other(_) => Variant::Usb2512b,
        BoardVariant::Plus => Variant::Usb2514b,
    }
}

/// Config byte 3, register 0x08, verbatim from the FW-5 bring-up (0x02 = bit 1 set) on both variants. The
/// datasheet marks this bit reserved and the `usb251xb` crate gives it no setter; nobody has found a reason to
/// clear it, so it stays until someone does.
const CONFIG_BYTE_3: ConfigByte3 = ConfigByte3::from_bytes([0x02]);

/// Hub controller's own current draw while self-powered, register units of 2 mA (FW-5 bring-up value, same on
/// both variants; the `usb251xb` crate's own default is the bus-powered value, 100 mA, so this needs setting
/// explicitly).
const HUB_CURRENT_SELF: Milliamps = Milliamps(2);

/// Ports needing their D+/D- polarity swapped, for the ports that exist on `variant`: port 2 (plug, U701) on
/// both board variants, plus port 4 (SBU USB, U901) on Plus only — both cross D+/D- between the hub and the far
/// end (hw_b153a.md §6). Unswapped, a full/high-speed device's D+ pull-up shows up on the hub's D-, which the
/// hub reads as low-speed (FW-6: the reMarkable 2 result, `error -32`).
fn swapped_ports(variant: BoardVariant) -> &'static [Port] {
    match variant {
        BoardVariant::Plus => &[Port::Port2, Port::Port4],
        BoardVariant::Main | BoardVariant::Unknown | BoardVariant::Other(_) => &[Port::Port2],
    }
}

/// Builds the hub's configuration for `variant`. VID, PID (from `variant`'s `usb251xb::Variant`) and DID
/// already match the FW-5 bring-up as the crate's own datasheet defaults; only [`CONFIG_BYTE_3`],
/// [`HUB_CURRENT_SELF`] and the port swap ([`swapped_ports`]) differ from them and need setting below. Unlike
/// the FW-5 bring-up (which stopped writing registers after power-on time, 0x10), this also writes the
/// registers up to the port swap/map (0xFC) — mostly explicit zeros (no battery charging, no signal boost
/// override) where the FW-5 bring-up relied on the hub's own power-on defaults.
pub fn hub_config(variant: BoardVariant) -> Config {
    let mut config = Config::builder(usb251xb_variant(variant))
        .self_powered(true)
        .mtt(true)
        .high_speed(true)
        .power_switching(PowerSwitching::Individual)
        // OCS_N1..4 are unconnected on this board; the datasheet gives them internal pull-ups (DS00001692,
        // "the over-current sense inputs ... have internal pull-up resistors"), which read as "no overcurrent",
        // so per-port sensing is safe to keep on this self-powered hub (hw_b153a.md §6, HUB-10).
        .current_sensing(CurrentSensing::Individual)
        .hub_current_self(HUB_CURRENT_SELF)
        .port_swap(swapped_ports(variant))
        .into_config();
    config.config3 = CONFIG_BYTE_3;
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strap_high_is_main() {
        assert_eq!(board_variant_from_strap(true), BoardVariant::Main);
    }

    #[test]
    fn strap_low_is_plus() {
        assert_eq!(board_variant_from_strap(false), BoardVariant::Plus);
    }

    #[test]
    fn main_config_has_usb2512b_identity_and_only_port2_swapped() {
        let config = hub_config(BoardVariant::Main);
        assert_eq!(config.product_id, 0x2512);
        assert!(config.port_swap.port2());
        assert!(!config.port_swap.port4());
    }

    #[test]
    fn plus_config_has_usb2514b_identity_and_port2_and_port4_swapped() {
        let config = hub_config(BoardVariant::Plus);
        assert_eq!(config.product_id, 0x2514);
        assert!(config.port_swap.port2());
        assert!(config.port_swap.port4());
    }

    #[test]
    fn unknown_falls_back_to_main_config() {
        let config = hub_config(BoardVariant::Unknown);
        assert_eq!(config.product_id, 0x2512);
        assert!(config.port_swap.port2());
        assert!(!config.port_swap.port4());
    }
}
