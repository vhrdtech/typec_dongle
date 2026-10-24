//! Host side of the `debug` level (API-18, CLI-23): pin registers in words, a pin dump compared with the reset
//! state, the UCPD registers decoded. The calls are on [crate::levels::Debug], net names in [crate::nets]; the
//! rules the firmware applies (refused pins, the claim table, UCPD roles) are `donguru_board`'s, shared with it.

use donguru_api::{CcRole, McuPin, PinState, PortRegs, UcpdState};
use serde::Serialize;

pub use donguru_board::pins::{REG_READ_MAX_WORDS, bonded, check_reg_read, refused, reset_state};
pub use donguru_board::ucpd::role_of;

use crate::nets;

/// MODER values.
const INPUT: u8 = 0;
const OUTPUT: u8 = 1;
const ALTERNATE: u8 = 2;

/// One pin's fields out of its port's registers.
pub fn pin_state(r: &PortRegs, pin: u8) -> PinState {
    let two = 2 * pin as u32;
    let afr = if pin < 8 { r.afrl } else { r.afrh };
    PinState {
        moder: ((r.moder >> two) & 3) as u8,
        open_drain: (r.otyper >> pin) & 1 != 0,
        speed: ((r.ospeedr >> two) & 3) as u8,
        pull: ((r.pupdr >> two) & 3) as u8,
        af: ((afr >> (4 * (pin as u32 % 8))) & 0xF) as u8,
        input: (r.idr >> pin) & 1 != 0,
        output: (r.odr >> pin) & 1 != 0,
    }
}

/// What a pin is set up as, in words: `analog`, `input pull-up`, `output high open-drain`, `AF4 pull-up`. The
/// level an output drives is part of it; the level a pin reads is not ([PinState::input]).
pub fn mode_text(s: &PinState) -> String {
    let pull = match s.pull {
        1 => " pull-up",
        2 => " pull-down",
        _ => "",
    };
    let od = if s.open_drain { " open-drain" } else { "" };
    match s.moder {
        INPUT => format!("input{pull}"),
        OUTPUT => format!("output {}{od}{pull}", if s.output { "high" } else { "low" }),
        ALTERNATE => format!("AF{}{od}{pull}", s.af),
        _ => "analog".to_string(),
    }
}

/// A pin of a dump, with the net on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub pin: McuPin,
    pub net: Option<&'static nets::Net>,
    pub state: PinState,
}

impl Pin {
    /// `PA5`.
    pub fn name(&self) -> String {
        nets::port_pin_name(self.pin)
    }
}

/// The bonded pins of a dump (`pins` / `boot_pins`), in port order; ports a dump doesn't carry are left out.
pub fn pins(ports: &[PortRegs]) -> Vec<Pin> {
    ports
        .iter()
        .flat_map(|r| {
            (0..16u8).filter_map(move |n| {
                let pin = McuPin {
                    port: r.port,
                    pin: n,
                };
                bonded(pin).then(|| Pin {
                    pin,
                    net: nets::net_of(pin),
                    state: pin_state(r, n),
                })
            })
        })
        .collect()
}

/// A pin not as the reset leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinDiff {
    pub pin: Pin,
    /// The pin out of reset (IDR 0).
    pub reset: PinState,
}

/// The bonded pins of `ports` whose configuration (MODER, OTYPER, OSPEEDR, PUPDR, ODR, AFR; not IDR, which is what
/// the pin reads) differs from the reset state ([reset_state], RM0444 §7.4). Empty for a healthy `boot_pins`
/// (verify_b153a SYS-1).
pub fn diff_reset(ports: &[PortRegs]) -> Vec<PinDiff> {
    let reset = reset_state();
    pins(ports)
        .into_iter()
        .filter_map(|pin| {
            let r = &reset[donguru_board::pins::port_index(pin.pin.port)];
            let mut reset = PinState {
                moder: r.mode(pin.pin.pin),
                open_drain: r.open_drain(pin.pin.pin),
                speed: r.speed(pin.pin.pin),
                pull: r.pull(pin.pin.pin),
                af: r.af(pin.pin.pin),
                input: false,
                output: r.output(pin.pin.pin),
            };
            reset.input = pin.state.input;
            (reset != pin.state).then_some(PinDiff { pin, reset })
        })
        .collect()
}

/// `rd`, `rp1a5`, ... as `dg debug ucpd --role` takes them.
pub fn role_name(role: CcRole) -> &'static str {
    match role {
        CcRole::Off => "off",
        CcRole::Rd => "rd",
        CcRole::Rp0A9 => "rp0a9",
        CcRole::Rp1A5 => "rp1a5",
        CcRole::Rp3A0 => "rp3a0",
        #[allow(unreachable_patterns)] // a role of a newer API
        _ => "unknown",
    }
}

/// A UCPD port's registers decoded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Ucpd {
    /// The CC role UCPD_CR holds, `None` for a combination the firmware never writes.
    #[serde(serialize_with = "ser_role")]
    pub role: Option<CcRole>,
    /// TYPEC_VSTATE of CC1 and CC2 in words (`vRd-1.5`, `vOPEN`), `off` without a role.
    pub cc1: &'static str,
    pub cc2: &'static str,
    pub cr: u32,
    pub sr: u32,
}

fn ser_role<S: serde::Serializer>(role: &Option<CcRole>, s: S) -> Result<S::Ok, S::Error> {
    match role {
        Some(r) => s.serialize_str(role_name(*r)),
        None => s.serialize_none(),
    }
}

/// TYPEC_VSTATE_CCx (RM0444 §34.7.3) in words: what a sink sees of the source's Rp, or what a source sees of the
/// sink's Rd.
pub fn vstate_text(vstate: u8, role: Option<CcRole>) -> &'static str {
    match role {
        None | Some(CcRole::Off) => "off",
        Some(CcRole::Rd) => match vstate & 3 {
            0 => "vRa",
            1 => "vRd-USB",
            2 => "vRd-1.5",
            _ => "vRd-3.0",
        },
        Some(_) => match vstate & 3 {
            0 => "vRa",
            1 => "vRd",
            2 => "vOPEN",
            _ => "reserved",
        },
    }
}

/// `ucpd` / `ucpd_role`'s answer decoded.
pub fn ucpd(state: &UcpdState) -> Ucpd {
    let role = role_of(state.cr);
    Ucpd {
        role,
        cc1: vstate_text(state.cc1, role),
        cc2: vstate_text(state.cc2, role),
        cr: state.cr,
        sr: state.sr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_api::GpioPort;

    fn dump(ports: donguru_board::pins::Ports) -> Vec<PortRegs> {
        donguru_board::pins::PORTS
            .iter()
            .zip(ports)
            .map(|(&port, r)| PortRegs {
                port,
                moder: r.moder,
                otyper: r.otyper,
                ospeedr: r.ospeedr,
                pupdr: r.pupdr,
                idr: 0,
                odr: r.odr,
                afrl: r.afrl,
                afrh: r.afrh,
            })
            .collect()
    }

    #[test]
    fn reset_state_has_no_diff_and_idr_is_ignored() {
        let mut ports = dump(reset_state());
        ports[1].idr = 0xFFFF;
        assert!(diff_reset(&ports).is_empty());
        ports[0].moder &= !(3 << 10); // PA5 input
        let diff = diff_reset(&ports);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].pin.name(), "PA5");
        assert_eq!(diff[0].pin.net.map(|n| n.name), Some("RELAY_EN"));
        assert_eq!(mode_text(&diff[0].reset), "analog");
        assert_eq!(mode_text(&diff[0].pin.state), "input");
    }

    #[test]
    fn only_bonded_pins() {
        let ports = dump(reset_state());
        let pins = pins(&ports);
        // PA0..15, PB0..15, PC6, PC7, PC13..15, PD0..3, PF0..2
        assert_eq!(pins.len(), 16 + 16 + 5 + 4 + 3);
        assert!(pins.iter().all(|p| p.pin.port != GpioPort::E));
    }

    #[test]
    fn mode_texts() {
        let mut s = PinState {
            moder: OUTPUT,
            open_drain: true,
            speed: 0,
            pull: 1,
            af: 0,
            input: false,
            output: true,
        };
        assert_eq!(mode_text(&s), "output high open-drain pull-up");
        s.moder = ALTERNATE;
        s.af = 4;
        assert_eq!(mode_text(&s), "AF4 open-drain pull-up");
        s.moder = INPUT;
        s.pull = 2;
        assert_eq!(mode_text(&s), "input pull-down");
    }

    #[test]
    fn ucpd_decoded() {
        let sink = UcpdState {
            cr: 3 << 10 | 1 << 9,
            sr: 0,
            cc1: 2,
            cc2: 0,
        };
        let u = ucpd(&sink);
        assert_eq!(u.role, Some(CcRole::Rd));
        assert_eq!((u.cc1, u.cc2), ("vRd-1.5", "vRa"));
        let off = ucpd(&UcpdState {
            cr: 0,
            sr: 0,
            cc1: 0,
            cc2: 0,
        });
        assert_eq!((off.role, off.cc1), (Some(CcRole::Off), "off"));
    }
}
