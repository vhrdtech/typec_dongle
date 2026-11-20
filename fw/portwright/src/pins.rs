//! MCU pins for the `debug` level (API-18): the GPIO registers as the reset left them ([capture_boot]), the pin
//! claim table ([CLAIMS], `donguru_board::pins::Claims`) and the pin changes a host asks for ([set]), checked by
//! `donguru_board::pins::check` (refused pins, pins a running function holds, pin states that fight the board).
//!
//! The writes go to the GPIO registers directly, in one critical section with the check (the claim table's lock): the pins `debug` may
//! change are the ones no driver holds, and embassy's own GPIO writes take a critical section too, so a
//! read-modify-write here never races one of them.

use core::cell::{Cell, RefCell};

use donguru_api::{GpioPort, McuPin, PinState, PortRegs};
use donguru_board::pins::{self as rules, Change, Claims, Denied, Holder, PORTS, Port, Ports};
use embassy_stm32::pac;
use embassy_stm32::pac::gpio::Gpio;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

/// Who holds which pin now: what runs from boot, and an SWD session or a UCPD role while it runs.
static CLAIMS: Mutex<CriticalSectionRawMutex, RefCell<Claims>> =
    Mutex::new(RefCell::new(Claims::boot()));

/// GPIOA..F as [capture_boot] found them.
static BOOT: Mutex<CriticalSectionRawMutex, Cell<Option<[PortRegs; 6]>>> =
    Mutex::new(Cell::new(None));

const fn regs(port: GpioPort) -> Gpio {
    match port {
        GpioPort::A => pac::GPIOA,
        GpioPort::B => pac::GPIOB,
        GpioPort::C => pac::GPIOC,
        GpioPort::D => pac::GPIOD,
        GpioPort::E => pac::GPIOE,
        GpioPort::F => pac::GPIOF,
    }
}

fn read_port(port: GpioPort) -> PortRegs {
    let r = regs(port);
    PortRegs {
        port,
        moder: r.moder().read().0,
        otyper: r.otyper().read().0,
        ospeedr: r.ospeedr().read().0,
        pupdr: r.pupdr().read().0,
        idr: r.idr().read().0,
        odr: r.odr().read().0,
        afrl: r.afr(0).read().0,
        afrh: r.afr(1).read().0,
    }
}

/// GPIOA..F registers now. The GPIO clocks are on from `embassy_stm32::init` on (all six ports).
pub fn dump() -> [PortRegs; 6] {
    PORTS.map(read_port)
}

/// Reads GPIOA..F before anything sets a pin up (verify_b153a SYS-1): first thing in `main`. Turns the port clocks
/// on for the read (GPIOENR bit 4, GPIOE, has no field in the PAC) and back to what they were.
pub fn capture_boot() {
    let rcc = pac::RCC;
    let was = rcc.gpioenr().read().0;
    rcc.gpioenr().modify(|w| w.0 |= 0x3F);
    // the clock reaches the port two AHB cycles after the enable (RM0444 §5.2.17): the read back covers them
    let _ = rcc.gpioenr().read();
    let ports = dump();
    rcc.gpioenr().write(|w| w.0 = was);
    BOOT.lock(|b| b.set(Some(ports)));
}

/// GPIOA..F as the reset left them, `None` before [capture_boot] ran.
pub fn boot() -> Option<[PortRegs; 6]> {
    BOOT.lock(|b| b.get())
}

fn rule_ports(dump: &[PortRegs; 6]) -> Ports {
    dump.map(|r| Port {
        moder: r.moder,
        otyper: r.otyper,
        ospeedr: r.ospeedr,
        pupdr: r.pupdr,
        odr: r.odr,
        afrl: r.afrl,
        afrh: r.afrh,
    })
}

fn pin_state(r: &PortRegs, pin: u8) -> PinState {
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

/// A pin's registers; `InvalidArgument` for one not on the LQFP48.
pub fn get(p: McuPin) -> Result<PinState, Denied> {
    if p.pin > 15 || !rules::bonded(p) {
        return Err(Denied::Invalid {
            arg: 0,
            reason: "not a pin of the LQFP48",
        });
    }
    Ok(pin_state(&read_port(p.port), p.pin))
}

/// Makes `change` on `p` when the rules allow it, and answers the pin's registers after it.
pub fn set(p: McuPin, change: Change) -> Result<PinState, Denied> {
    CLAIMS.lock(|claims| {
        let now = dump();
        let write = rules::check(p, change, &claims.borrow(), &rule_ports(&now))?;
        let r = regs(p.port);
        let pin = p.pin as usize;
        // in the order of PinWrite's doc: ODR, OTYPER, PUPDR, AFR, MODER
        if let Some(high) = write.output {
            r.bsrr().write(|w| {
                if high {
                    w.set_bs(pin, true)
                } else {
                    w.set_br(pin, true)
                }
            });
        }
        r.otyper()
            .modify(|w| w.0 = (w.0 & !(1 << pin)) | ((write.open_drain as u32) << pin));
        r.pupdr()
            .modify(|w| w.0 = (w.0 & !(3 << (2 * pin))) | ((write.pull as u32 & 3) << (2 * pin)));
        if let Some(af) = write.af {
            let shift = 4 * (pin % 8);
            r.afr(pin / 8)
                .modify(|w| w.0 = (w.0 & !(0xF << shift)) | ((af as u32 & 0xF) << shift));
        }
        r.moder()
            .modify(|w| w.0 = (w.0 & !(3 << (2 * pin))) | ((write.mode as u32 & 3) << (2 * pin)));
        Ok(pin_state(&read_port(p.port), p.pin))
    })
}

/// `holder` takes its pins, or names the function that holds one of them.
pub fn claim(holder: Holder) -> Result<(), Holder> {
    CLAIMS.lock(|c| c.borrow_mut().claim(holder))
}

/// `holder` lets go of its pins.
pub fn release(holder: Holder) {
    CLAIMS.lock(|c| c.borrow_mut().release(holder))
}

/// Whether `holder` holds its pins now.
pub fn holds(holder: Holder) -> bool {
    CLAIMS.lock(|c| {
        holder
            .pins()
            .first()
            .is_some_and(|p| c.borrow().holder(*p) == Some(holder))
    })
}

/// Puts `pins` in analog mode (a UCPD role's CC pins: the CC PHY takes them over).
pub fn set_analog(pins: &[McuPin]) {
    CLAIMS.lock(|_| {
        for p in pins {
            regs(p.port)
                .moder()
                .modify(|w| w.0 |= 3 << (2 * (p.pin as u32 & 15)));
        }
    })
}
