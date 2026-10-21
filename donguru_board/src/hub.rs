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
//! [check] tells whether a hub that stayed up through an MCU reset (FW-46) still holds what [hub_config] would
//! write and is attached: the boot then keeps it, so a DUT behind it keeps its USB link, and resets and
//! reconfigures it otherwise.
//!
//! No embassy and no GPIO/I2C in here — just the mapping and the `usb251xb` crate's pure config builder — so
//! this is unit-tested on the host (`fw/donguru/src/hub.rs` only reads the strap and calls through here), the
//! same way `donguru_dap` keeps the CMSIS-DAP framing apart from the firmware's USB plumbing.

use donguru_api::BoardVariant;
use usb251xb::register::{ConfigByte3, REG_STATUS_COMMAND, StatusCommand};
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

/// What [check] found in a hub that may have stayed up through an MCU reset (FW-46).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Check {
    /// Every register this firmware writes reads back as written and the hub is attached: keep it.
    Kept,
    /// Register `register` reads `read` instead of `expected` (another firmware's config, for one): reset and
    /// reconfigure.
    Differs {
        register: u8,
        expected: u8,
        read: u8,
    },
    /// The ID registers read all-zero: a hub out of reset waiting for its config, or one that attached less than
    /// ~0.4 s ago (HUB-5): ask again a bit later before resetting it.
    Unconfigured,
    /// Configured as this firmware would, but not attached upstream.
    NotAttached,
    /// No answer on I2C: busy with the host, or not powered.
    NoAnswer,
}

/// Reads back what `config` ([hub_config]) writes, one SMBus block per register chunk of
/// `usb251xb::Config::to_register_chunks`, and the attach bit of the status register. `read(register, data)` fills
/// `data` with the bytes from `register` on (the SMBus count byte left out).
pub fn check<E>(config: &Config, mut read: impl FnMut(u8, &mut [u8]) -> Result<(), E>) -> Check {
    let mut buf = [0u8; 32];
    for (first, (register, expected, len)) in config.to_register_chunks().into_iter().enumerate() {
        let data = &mut buf[..len as usize];
        if data.is_empty() {
            continue;
        }
        if read(register, data).is_err() {
            return Check::NoAnswer;
        }
        // chunk 0 starts with VID/PID, all-zero only before a config
        if first == 0 && data[..4] == [0; 4] {
            return Check::Unconfigured;
        }
        let differs = expected.iter().zip(data.iter()).position(|(e, r)| e != r);
        if let Some(i) = differs {
            return Check::Differs {
                register: register.wrapping_add(i as u8),
                expected: expected[i],
                read: data[i],
            };
        }
    }
    let mut status = [0u8; 1];
    if read(REG_STATUS_COMMAND, &mut status).is_err() {
        return Check::NoAnswer;
    }
    if StatusCommand::from_bytes(status).usb_attach() {
        Check::Kept
    } else {
        Check::NotAttached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hub's register file holding `config` and the status byte `status`.
    fn registers(config: &Config, status: u8) -> [u8; 256] {
        let mut regs = [0u8; 256];
        for (register, data, len) in config.to_register_chunks() {
            let start = register as usize;
            regs[start..start + len as usize].copy_from_slice(&data[..len as usize]);
        }
        regs[REG_STATUS_COMMAND as usize] = status;
        regs
    }

    fn read_from(regs: &[u8; 256]) -> impl FnMut(u8, &mut [u8]) -> Result<(), ()> + '_ {
        |register, data| {
            let start = register as usize;
            data.copy_from_slice(&regs[start..start + data.len()]);
            Ok(())
        }
    }

    #[test]
    fn configured_and_attached_hub_is_kept() {
        let config = hub_config(BoardVariant::Main);
        let regs = registers(&config, 0x01);
        assert_eq!(check(&config, read_from(&regs)), Check::Kept);
    }

    #[test]
    fn hub_not_attached() {
        let config = hub_config(BoardVariant::Main);
        let regs = registers(&config, 0x00);
        assert_eq!(check(&config, read_from(&regs)), Check::NotAttached);
    }

    #[test]
    fn all_zero_hub_is_unconfigured() {
        let config = hub_config(BoardVariant::Main);
        assert_eq!(check(&config, read_from(&[0; 256])), Check::Unconfigured);
    }

    #[test]
    fn another_variants_config_differs() {
        let main = hub_config(BoardVariant::Main);
        let regs = registers(&hub_config(BoardVariant::Plus), 0x01);
        assert_eq!(
            check(&main, read_from(&regs)),
            Check::Differs {
                register: 0x02,
                expected: 0x12,
                read: 0x14
            }
        );
    }

    #[test]
    fn a_changed_register_past_the_ids_differs() {
        let config = hub_config(BoardVariant::Main);
        let mut regs = registers(&config, 0x01);
        regs[0xFA] ^= 0x04; // port swap
        assert_eq!(
            check(&config, read_from(&regs)),
            Check::Differs {
                register: 0xFA,
                expected: regs[0xFA] ^ 0x04,
                read: regs[0xFA]
            }
        );
    }

    #[test]
    fn no_answer() {
        let config = hub_config(BoardVariant::Main);
        assert_eq!(check(&config, |_, _| Err(())), Check::NoAnswer);
    }

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
