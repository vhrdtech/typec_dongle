//! UCPD1 (receptacle) and UCPD2 (plug) for the `debug` level (`ucpd`, `ucpd_role`, API-18): the CC PHY only, no PD
//! messages. Which role a port may take is `donguru_board::ucpd`'s; this sets the registers.
//!
//! A role turns the port's clock and UCPD on (CFGR1 as embassy's driver sets it: HSI16 / 2, half-bit / 13, ...) and
//! holds its CC pins in the claim table (`src/pins.rs`), analog for the CC PHY; `Off` turns the CC PHY and UCPD off
//! again and gives the pins back. The SYSCFG strobe that hands the CC pins from the dead-battery pull-down to UCPD_CR is set with the first
//! role and left set: UCPD1's is set at boot already (`src/hub.rs`, the PA10 strap).

use crate::pins;
use donguru_api::{CcRole, UcpdPort, UcpdState};
use donguru_board::pins::Denied;
use donguru_board::ucpd::{holder, role_fields};
use embassy_stm32::pac;
use embassy_stm32::pac::ucpd::Ucpd;

const fn regs(port: UcpdPort) -> Ucpd {
    match port {
        UcpdPort::Ucpd1 => pac::UCPD1,
        UcpdPort::Ucpd2 => pac::UCPD2,
    }
}

fn clock_on(port: UcpdPort) -> bool {
    let r = pac::RCC.apbenr1().read();
    match port {
        UcpdPort::Ucpd1 => r.ucpd1en(),
        UcpdPort::Ucpd2 => r.ucpd2en(),
    }
}

/// CR, SR and the CC line states; all zero while the port's clock is off (never given a role).
pub fn state(port: UcpdPort) -> UcpdState {
    if !clock_on(port) {
        return UcpdState {
            cr: 0,
            sr: 0,
            cc1: 0,
            cc2: 0,
        };
    }
    let r = regs(port);
    let cr = r.cr().read().0;
    let sr = r.sr().read().0;
    UcpdState {
        cr,
        sr,
        cc1: ((sr >> 16) & 3) as u8,
        cc2: ((sr >> 18) & 3) as u8,
    }
}

/// Gives `port` the CC `role`, when this board allows it and nothing else holds its CC pins.
pub fn set_role(port: UcpdPort, role: CcRole) -> Result<UcpdState, Denied> {
    let fields = role_fields(port, role, cfg!(feature = "rev_a"))?;
    let r = regs(port);
    if !fields.enabled {
        if clock_on(port) {
            r.cr().modify(|w| w.0 &= !(3 << 10)); // CCENABLE off
            r.cfgr1().modify(|w| w.set_ucpden(false));
        }
        pins::release(holder(port));
        return Ok(state(port));
    }
    pins::claim(holder(port)).map_err(Denied::InUse)?;
    pins::set_analog(holder(port).pins());
    critical_section::with(|_| {
        pac::RCC.apbenr1().modify(|w| match port {
            UcpdPort::Ucpd1 => w.set_ucpd1en(true),
            UcpdPort::Ucpd2 => w.set_ucpd2en(true),
        })
    });
    if !r.cfgr1().read().ucpden() {
        // CFGR1 takes writes only while UCPDEN is 0
        r.cfgr1().write(|w| {
            w.set_psc_usbpdclk(pac::ucpd::vals::PscUsbpdclk::Div2);
            w.set_hbitclkdiv(13 - 1);
            w.set_transwin(8 - 1);
            w.set_ifrgap(17 - 1);
        });
        r.cfgr1().modify(|w| w.set_ucpden(true));
    }
    r.cr().modify(|w| {
        // ANAMODE (9), ANASUBMODE (8:7), CCENABLE (11:10) both lines
        w.0 = (w.0 & !(0b1111 << 7))
            | ((fields.sink as u32) << 9)
            | ((fields.rp as u32 & 3) << 7)
            | (3 << 10)
    });
    critical_section::with(|_| {
        pac::SYSCFG.cfgr1().modify(|w| match port {
            UcpdPort::Ucpd1 => w.set_ucpd1_strobe(true),
            UcpdPort::Ucpd2 => w.set_ucpd2_strobe(true),
        })
    });
    Ok(state(port))
}
