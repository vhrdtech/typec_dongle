//! MCU pins for the `debug` level (`Bringup`, API-18): which pins a host may change, who holds a pin now, and the
//! register writes a change takes.
//!
//! Three rules decide a `pin_set` / `pin_af` before the firmware touches a register ([`check`]):
//!
//! 1. **Refused, always** ([`refused`]): NRST, the pins tied to a supply, our own USB and SWD, `CC_DA_EN` (rev A
//!    bug), the VBUS enables (`plug` / `header` with their guard) and the hub's reset and I2C (`hub_read` /
//!    `hub_write`), from hw_b153a.md §14. Pins not on the LQFP48 are an invalid argument.
//! 2. **In use** ([`Claims`]): a pin a running function holds, the MCU pin -> holder table. The firmware fills it at
//!    boot ([`Claims::boot`]: LEDs, meter, button, header UART, V_SBU select) and as functions start and stop (an
//!    SWD session, a UCPD role). The SBU owner (API-14) builds on the same table.
//! 3. **Electrical** ([`Violations`]): no change may leave the pins in a state that fights itself on this board:
//!    two MCU pins on one net both driving (PB1 / PB3, PB6 / PB9 / PB11, PB7 / PB8), the level shifter's OE high
//!    while PA0 / PA1 drive the same SBU lines, PA0 / PA1 push-pull while V_SBU isn't selected to 3.3 V (PD3), or
//!    `~PWR_FLT` (U601's open-drain FLTB) driven push-pull. Only new violations refuse a change: one already there
//!    doesn't block fixing it.
//!
//! No embassy and no GPIO in here: the firmware reads the registers into [`Ports`], asks [`check`], and writes the
//! [`PinWrite`] it gets; the mock in `donguru-core` does the same on its register model.

use donguru_api::{GpioPort, McuPin, PinMode};

/// GPIOA..F.
pub const PORTS: [GpioPort; 6] = [
    GpioPort::A,
    GpioPort::B,
    GpioPort::C,
    GpioPort::D,
    GpioPort::E,
    GpioPort::F,
];

/// Index of `port` in [Ports].
pub const fn port_index(port: GpioPort) -> usize {
    match port {
        GpioPort::A => 0,
        GpioPort::B => 1,
        GpioPort::C => 2,
        GpioPort::D => 3,
        GpioPort::E => 4,
        GpioPort::F => 5,
    }
}

const fn pin(port: GpioPort, pin: u8) -> McuPin {
    McuPin { port, pin }
}

/// The registers of one GPIO port that the rules look at (IDR is left out: it is what the pins read, not what
/// the MCU asks of them).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Port {
    pub moder: u32,
    pub otyper: u32,
    pub ospeedr: u32,
    pub pupdr: u32,
    pub odr: u32,
    pub afrl: u32,
    pub afrh: u32,
}

/// MODER values.
pub const MODE_INPUT: u8 = 0;
pub const MODE_OUTPUT: u8 = 1;
pub const MODE_ALTERNATE: u8 = 2;
pub const MODE_ANALOG: u8 = 3;

impl Port {
    pub const fn mode(&self, pin: u8) -> u8 {
        ((self.moder >> (2 * pin)) & 3) as u8
    }

    pub const fn open_drain(&self, pin: u8) -> bool {
        (self.otyper >> pin) & 1 != 0
    }

    pub const fn speed(&self, pin: u8) -> u8 {
        ((self.ospeedr >> (2 * pin)) & 3) as u8
    }

    pub const fn pull(&self, pin: u8) -> u8 {
        ((self.pupdr >> (2 * pin)) & 3) as u8
    }

    pub const fn output(&self, pin: u8) -> bool {
        (self.odr >> pin) & 1 != 0
    }

    pub const fn af(&self, pin: u8) -> u8 {
        let afr = if pin < 8 { self.afrl } else { self.afrh };
        ((afr >> (4 * (pin % 8))) & 0xF) as u8
    }

    /// Output or alternate function: a peripheral's AF may drive, so it counts as driving.
    const fn drives(&self, pin: u8) -> bool {
        matches!(self.mode(pin), MODE_OUTPUT | MODE_ALTERNATE)
    }

    /// Driven high, or on an AF that may drive it high.
    const fn drives_high(&self, pin: u8) -> bool {
        match self.mode(pin) {
            MODE_OUTPUT => self.output(pin),
            MODE_ALTERNATE => true,
            _ => false,
        }
    }

    /// Push-pull output or AF: drives both levels.
    const fn push_pull(&self, pin: u8) -> bool {
        match self.mode(pin) {
            MODE_OUTPUT => !self.open_drain(pin),
            MODE_ALTERNATE => true,
            _ => false,
        }
    }

    /// Output high, push-pull or open drain (the pin itself, not what a pull makes of it).
    const fn output_high(&self, pin: u8) -> bool {
        self.mode(pin) == MODE_OUTPUT && self.output(pin)
    }
}

/// GPIOA..F, indexed by [port_index].
pub type Ports = [Port; 6];

/// The GPIO registers out of reset (RM0444 §7.4): every pin analog, but PA13 (SWDIO: AF0, pull-up, very high
/// speed) and PA14 (SWCLK / BOOT0: AF0, pull-down). What `boot_pins` should show on a healthy board (verify_b153a
/// SYS-1), as the firmware captures it before anything is set up.
pub const fn reset_state() -> Ports {
    let other = Port {
        moder: 0xFFFF_FFFF,
        otyper: 0,
        ospeedr: 0,
        pupdr: 0,
        odr: 0,
        afrl: 0,
        afrh: 0,
    };
    let a = Port {
        moder: 0xEBFF_FFFF,
        ospeedr: 0x0C00_0000,
        pupdr: 0x2400_0000,
        ..other
    };
    [a, other, other, other, other, other]
}

/// A pin bonded out on the LQFP48 of the B153 (U202): PA0..15, PB0..15, PC6, PC7, PC13..15, PD0..3, PF0..2.
pub const fn bonded(p: McuPin) -> bool {
    match p.port {
        GpioPort::A | GpioPort::B => p.pin <= 15,
        GpioPort::C => matches!(p.pin, 6 | 7 | 13..=15),
        GpioPort::D => p.pin <= 3,
        GpioPort::E => false,
        GpioPort::F => p.pin <= 2,
    }
}

/// Why a pin is never changed by `debug`, `None` for a pin it may change (subject to [Claims] and [Violations]).
pub const fn refused(p: McuPin) -> Option<&'static str> {
    use GpioPort::*;
    Some(match (p.port, p.pin) {
        (F, 2) => "NRST",
        (C, 6) => "tied to GND",
        (C, 7) => "tied to 3.3 V",
        (A, 11) | (A, 12) => "our own USB",
        (A, 13) | (A, 14) => "our own SWD and BOOT0",
        (A, 15) => "CC_DA_EN: Rp on the computer's CC on rev A",
        (A, 6) => "PWR_EN: use plug.vbus_mode, it has the guard",
        (D, 1) => "5V_EN: use header.vbus_sw_mode, it has the guard",
        (C, 13) => "hub reset: use hub.recover",
        (A, 4) | (B, 10) | (B, 14) => "hub I2C: use hub_read / hub_write",
        _ => return None,
    })
}

/// A function of the firmware that holds pins, as `InUse { by }` names it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Holder {
    /// PF0 / PF1 on TIM14 / TIM15: `debug.led_set` or `leds`.
    Leds = 1,
    /// VBUS_IN (PA3) and the current (PB0) on the ADC.
    Meter,
    /// The button and switch divider (PA7) on the ADC.
    Button,
    /// `uart[1]`: USART5 on IO7 / IO8 (PB3 / PB4), and PB1, which is tied to IO7.
    HeaderUart,
    /// V_SBU select (PD3), held at 3.3 V before PA0 / PA1 ever drive.
    VSbu,
    /// A CMSIS-DAP session: the SBU pins on both sides of the level shifter, its OE, the pull-ups, the plug CC
    /// stopgap drive.
    Swd,
    /// UCPD1 with a CC role: the receptacle CC pins.
    Ucpd1,
    /// UCPD2 with a CC role: the plug CC pins.
    Ucpd2,
}

impl Holder {
    const ALL: [Holder; 8] = [
        Holder::Leds,
        Holder::Meter,
        Holder::Button,
        Holder::HeaderUart,
        Holder::VSbu,
        Holder::Swd,
        Holder::Ucpd1,
        Holder::Ucpd2,
    ];

    /// The name in `InUse { by }` (32 bytes at most).
    pub const fn name(self) -> &'static str {
        match self {
            Holder::Leds => "the LEDs (use led_set)",
            Holder::Meter => "the power meter",
            Holder::Button => "the button",
            Holder::HeaderUart => "the header UART (uart[1])",
            Holder::VSbu => "V_SBU select",
            Holder::Swd => "an SWD session",
            Holder::Ucpd1 => "UCPD1 (ucpd_role)",
            Holder::Ucpd2 => "UCPD2 (ucpd_role)",
        }
    }

    /// The pins this function holds while it runs.
    pub const fn pins(self) -> &'static [McuPin] {
        use GpioPort::*;
        const LEDS: &[McuPin] = &[pin(F, 0), pin(F, 1)];
        const METER: &[McuPin] = &[pin(A, 3), pin(B, 0)];
        const BUTTON: &[McuPin] = &[pin(A, 7)];
        const HEADER_UART: &[McuPin] = &[pin(B, 3), pin(B, 4), pin(B, 1)];
        const VSBU: &[McuPin] = &[pin(D, 3)];
        const SWD: &[McuPin] = &[
            pin(A, 0),
            pin(A, 1),
            pin(B, 2),
            pin(B, 6),
            pin(B, 8),
            pin(B, 11),
            pin(C, 15),
            pin(D, 0),
            pin(D, 2),
        ];
        const UCPD1: &[McuPin] = &[pin(A, 8), pin(B, 15)];
        const UCPD2: &[McuPin] = &[pin(D, 0), pin(D, 2)];
        match self {
            Holder::Leds => LEDS,
            Holder::Meter => METER,
            Holder::Button => BUTTON,
            Holder::HeaderUart => HEADER_UART,
            Holder::VSbu => VSBU,
            Holder::Swd => SWD,
            Holder::Ucpd1 => UCPD1,
            Holder::Ucpd2 => UCPD2,
        }
    }

    const fn from_u8(v: u8) -> Option<Holder> {
        let mut i = 0;
        while i < Holder::ALL.len() {
            if Holder::ALL[i] as u8 == v {
                return Some(Holder::ALL[i]);
            }
            i += 1;
        }
        None
    }
}

/// The MCU pin -> holding function table: one byte per pin of GPIOA..F, 0 for a free pin.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Claims {
    held: [[u8; 16]; 6],
}

impl Default for Claims {
    fn default() -> Self {
        Self::new()
    }
}

impl Claims {
    /// Every pin free.
    pub const fn new() -> Self {
        Claims { held: [[0; 16]; 6] }
    }

    /// What runs from boot on and holds its pins for good: the LEDs, the meter and button ADC pins, the header
    /// UART, the V_SBU select.
    pub const fn boot() -> Self {
        let mut c = Claims::new();
        let boot = [
            Holder::Leds,
            Holder::Meter,
            Holder::Button,
            Holder::HeaderUart,
            Holder::VSbu,
        ];
        let mut i = 0;
        while i < boot.len() {
            let pins = boot[i].pins();
            let mut j = 0;
            while j < pins.len() {
                c.held[port_index(pins[j].port)][pins[j].pin as usize & 15] = boot[i] as u8;
                j += 1;
            }
            i += 1;
        }
        c
    }

    /// Who holds `p`, `None` when it is free.
    pub fn holder(&self, p: McuPin) -> Option<Holder> {
        Holder::from_u8(*self.held.get(port_index(p.port))?.get(p.pin as usize)?)
    }

    /// `holder` takes all of its [pins](Holder::pins), or none of them when another function holds one: that one's
    /// holder is the error. Taking pins it already holds is fine.
    pub fn claim(&mut self, holder: Holder) -> Result<(), Holder> {
        for p in holder.pins() {
            match self.holder(*p) {
                Some(other) if other != holder => return Err(other),
                _ => {}
            }
        }
        for p in holder.pins() {
            self.held[port_index(p.port)][p.pin as usize & 15] = holder as u8;
        }
        Ok(())
    }

    /// `holder` lets go of every pin it holds.
    pub fn release(&mut self, holder: Holder) {
        for port in self.held.iter_mut() {
            for h in port.iter_mut() {
                if *h == holder as u8 {
                    *h = 0;
                }
            }
        }
    }
}

/// Bits of [Violations]: a pin state that fights itself on this board.
pub mod violation {
    /// PB1 and PB3 (IO7) both drive: NT201 ties them.
    pub const TIED_IO7: u8 = 1 << 0;
    /// Two of PB6, PB9, PB11 drive: all three are SBU2_VT.
    pub const TIED_SBU2_VT: u8 = 1 << 1;
    /// PB7 and PB8 both drive: both are SBU1_VT.
    pub const TIED_SBU1_VT: u8 = 1 << 2;
    /// The level shifter's OE (PB2) high while PA0 / PA1 drive the SBU lines it drives too.
    pub const OE_WITH_DIRECT: u8 = 1 << 3;
    /// PA0 / PA1 push-pull while V_SBU isn't selected to 3.3 V (PD3 low): they would back-feed the shifter's A side.
    pub const DIRECT_WITHOUT_VSBU: u8 = 1 << 4;
    /// `~PWR_FLT` (PC14) push-pull: U601's FLTB is open drain and pulls it low on a fault.
    pub const PWR_FLT_PUSH_PULL: u8 = 1 << 5;
}

/// A set of [violation] bits.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Violations(pub u8);

impl Violations {
    /// Every rule `ports` breaks.
    pub fn of(ports: &Ports) -> Violations {
        let a = &ports[port_index(GpioPort::A)];
        let b = &ports[port_index(GpioPort::B)];
        let c = &ports[port_index(GpioPort::C)];
        let d = &ports[port_index(GpioPort::D)];
        let drivers = |port: &Port, pins: &[u8]| pins.iter().filter(|p| port.drives(**p)).count();
        let mut v = 0;
        if drivers(b, &[1, 3]) > 1 {
            v |= violation::TIED_IO7;
        }
        if drivers(b, &[6, 9, 11]) > 1 {
            v |= violation::TIED_SBU2_VT;
        }
        if drivers(b, &[7, 8]) > 1 {
            v |= violation::TIED_SBU1_VT;
        }
        let direct = a.drives(0) || a.drives(1);
        if direct && b.drives_high(2) {
            v |= violation::OE_WITH_DIRECT;
        }
        if (a.push_pull(0) || a.push_pull(1)) && !d.output_high(3) {
            v |= violation::DIRECT_WITHOUT_VSBU;
        }
        if c.push_pull(14) {
            v |= violation::PWR_FLT_PUSH_PULL;
        }
        Violations(v)
    }

    /// The reason of the lowest rule in the set, for `Refused { reason }` (64 bytes at most).
    pub fn reason(self) -> &'static str {
        let v = self.0;
        if v & violation::TIED_IO7 != 0 {
            "PB1 and PB3 (IO7) share a net: only one may drive"
        } else if v & violation::TIED_SBU2_VT != 0 {
            "PB6, PB9 and PB11 share SBU2_VT: only one may drive"
        } else if v & violation::TIED_SBU1_VT != 0 {
            "PB7 and PB8 share SBU1_VT: only one may drive"
        } else if v & violation::OE_WITH_DIRECT != 0 {
            "SBU_OE high while PA0 / PA1 drive the SBU lines"
        } else if v & violation::DIRECT_WITHOUT_VSBU != 0 {
            "PA0 / PA1 push-pull need V_SBU at 3.3 V (PD3 high)"
        } else if v & violation::PWR_FLT_PUSH_PULL != 0 {
            "~PWR_FLT is U601's open-drain FLTB: input or open drain only"
        } else {
            "no violation"
        }
    }
}

/// What a host asks of a pin.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// `pin_set`: a mode and, for an output, the level (`None` keeps ODR as it is).
    Mode { mode: PinMode, level: Option<bool> },
    /// `pin_af`: alternate function 0..=7.
    Af(u8),
}

/// The register fields of one pin after a [Change], written by the firmware in this order: ODR, OTYPER, PUPDR,
/// AFR, MODER last, so an output never shows a stale level or type.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PinWrite {
    /// `Some` sets ODR.
    pub output: Option<bool>,
    pub open_drain: bool,
    pub pull: u8,
    /// `Some` sets AFR.
    pub af: Option<u8>,
    pub mode: u8,
}

impl PinWrite {
    /// `port` with the write applied to `pin`.
    pub fn apply(&self, pin: u8, port: &mut Port) {
        let one = 1u32 << pin;
        let two = 3u32 << (2 * pin);
        if let Some(high) = self.output {
            port.odr = if high {
                port.odr | one
            } else {
                port.odr & !one
            };
        }
        port.otyper = if self.open_drain {
            port.otyper | one
        } else {
            port.otyper & !one
        };
        port.pupdr = (port.pupdr & !two) | ((self.pull as u32 & 3) << (2 * pin));
        if let Some(af) = self.af {
            let shift = 4 * (pin as u32 % 8);
            let afr = if pin < 8 {
                &mut port.afrl
            } else {
                &mut port.afrh
            };
            *afr = (*afr & !(0xF << shift)) | ((af as u32 & 0xF) << shift);
        }
        port.moder = (port.moder & !two) | ((self.mode as u32 & 3) << (2 * pin));
    }
}

/// Why `debug` doesn't make a change.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Denied {
    /// `InvalidArgument { arg, reason }`.
    Invalid { arg: u8, reason: &'static str },
    /// `Refused { reason }`.
    Refused(&'static str),
    /// `InUse { by }`.
    InUse(Holder),
}

/// The write `change` takes on a pin whose registers are `now` (a mode keeps the AF bits, an AF keeps type and
/// pull). A mode from a newer host is an invalid argument.
pub fn plan(change: Change, now: &Port, pin: u8) -> Result<PinWrite, Denied> {
    match change {
        Change::Mode { mode, level } => {
            let (mode, open_drain, pull) = match mode {
                PinMode::Analog => (MODE_ANALOG, false, 0),
                PinMode::Input => (MODE_INPUT, false, 0),
                PinMode::InputPullUp => (MODE_INPUT, false, 1),
                PinMode::InputPullDown => (MODE_INPUT, false, 2),
                PinMode::PushPull => (MODE_OUTPUT, false, 0),
                PinMode::OpenDrain => (MODE_OUTPUT, true, 0),
                PinMode::OpenDrainPullUp => (MODE_OUTPUT, true, 1),
                #[allow(unreachable_patterns)] // a mode of a newer host
                _ => {
                    return Err(Denied::Invalid {
                        arg: 1,
                        reason: "unknown pin mode",
                    });
                }
            };
            Ok(PinWrite {
                output: level,
                open_drain,
                pull,
                af: None,
                mode,
            })
        }
        Change::Af(af) if af <= 7 => Ok(PinWrite {
            output: None,
            open_drain: now.open_drain(pin),
            pull: now.pull(pin),
            af: Some(af),
            mode: MODE_ALTERNATE,
        }),
        Change::Af(_) => Err(Denied::Invalid {
            arg: 1,
            reason: "AF 0..=7",
        }),
    }
}

/// Whether `change` of pin `p` may be made with the pins as `now` and the functions holding what `claims` says,
/// and the write it takes. See the module doc for the rules.
pub fn check(p: McuPin, change: Change, claims: &Claims, now: &Ports) -> Result<PinWrite, Denied> {
    if p.pin > 15 {
        return Err(Denied::Invalid {
            arg: 0,
            reason: "pin 0..=15",
        });
    }
    if !bonded(p) {
        return Err(Denied::Invalid {
            arg: 0,
            reason: "not a pin of the LQFP48",
        });
    }
    if let Some(reason) = refused(p) {
        return Err(Denied::Refused(reason));
    }
    if let Some(holder) = claims.holder(p) {
        return Err(Denied::InUse(holder));
    }
    let port = port_index(p.port);
    let write = plan(change, &now[port], p.pin)?;
    let mut after = *now;
    write.apply(p.pin, &mut after[port]);
    let before = Violations::of(now).0;
    let new = Violations::of(&after).0 & !before;
    if new != 0 {
        return Err(Denied::Refused(Violations(new).reason()));
    }
    Ok(write)
}

/// The register on the hub (U301) that `hub_write` refuses: STATUS/COMMAND (0xFF), whose reset and attach bits
/// take our own USB link down. `hub.recover` does that on purpose.
pub const HUB_REG_STATUS_COMMAND: u8 = 0xFF;

/// The 1 KiB blocks of the G0B1's peripheral space that hold a peripheral (base addresses from the STM32G0B1
/// memory map, as in stm32-metapac): `reg_read` reads only inside these, so it never hits a reserved address.
const PERIPHERAL_BLOCKS: &[u32] = &[
    // APB
    0x4000_0000, // TIM2
    0x4000_0400, // TIM3
    0x4000_0800, // TIM4
    0x4000_1000, // TIM6
    0x4000_1400, // TIM7
    0x4000_2000, // TIM14
    0x4000_2800, // RTC
    0x4000_2C00, // WWDG
    0x4000_3000, // IWDG
    0x4000_3800, // SPI2
    0x4000_3C00, // SPI3
    0x4000_4400, // USART2
    0x4000_4800, // USART3
    0x4000_4C00, // USART4
    0x4000_5000, // USART5
    0x4000_5400, // I2C1
    0x4000_5800, // I2C2
    0x4000_5C00, // USB
    0x4000_6400, // FDCAN1
    0x4000_6800, // FDCAN2
    0x4000_6C00, // CRS
    0x4000_7000, // PWR
    0x4000_7400, // DAC1
    0x4000_7800, // CEC
    0x4000_7C00, // LPTIM1
    0x4000_8000, // LPUART1
    0x4000_8400, // LPUART2
    0x4000_8800, // I2C3
    0x4000_9400, // LPTIM2
    0x4000_9800, // USB RAM (2 KiB)
    0x4000_9C00,
    0x4000_A000, // UCPD1
    0x4000_A400, // UCPD2
    0x4000_B000, // TAMP
    0x4000_B400, // FDCAN RAM
    0x4001_0000, // SYSCFG, VREFBUF, COMP
    0x4001_2400, // ADC1
    0x4001_2C00, // TIM1
    0x4001_3000, // SPI1
    0x4001_3800, // USART1
    0x4001_3C00, // USART6
    0x4001_4000, // TIM15
    0x4001_4400, // TIM16
    0x4001_4800, // TIM17
    0x4001_5800, // DBGMCU
    // AHB
    0x4002_0000, // DMA1
    0x4002_0400, // DMA2
    0x4002_0800, // DMAMUX
    0x4002_1000, // RCC
    0x4002_1800, // EXTI
    0x4002_2000, // FLASH
    0x4002_3000, // CRC
    // IOPORT
    0x5000_0000, // GPIOA
    0x5000_0400, // GPIOB
    0x5000_0800, // GPIOC
    0x5000_0C00, // GPIOD
    0x5000_1000, // GPIOE
    0x5000_1400, // GPIOF
];

/// Most words one `reg_read` answers.
pub const REG_READ_MAX_WORDS: u8 = 32;

/// Whether `reg_read(addr, words)` may read: word aligned, 1..=32 words, every word inside a block of
/// [PERIPHERAL_BLOCKS] (peripheral space only, never memory). A read of a data register can take what it reads
/// (a UART's RDR, an SPI FIFO): that is up to the host.
pub fn check_reg_read(addr: u32, words: u8) -> Result<(), Denied> {
    if words == 0 || words > REG_READ_MAX_WORDS {
        return Err(Denied::Invalid {
            arg: 1,
            reason: "words 1..=32",
        });
    }
    if !addr.is_multiple_of(4) {
        return Err(Denied::Invalid {
            arg: 0,
            reason: "not word aligned",
        });
    }
    for i in 0..words as u32 {
        let Some(word) = addr.checked_add(4 * i) else {
            return Err(Denied::Refused("not peripheral space"));
        };
        if !PERIPHERAL_BLOCKS.contains(&(word & !0x3FF)) {
            return Err(Denied::Refused("no peripheral at this address"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use GpioPort::*;

    fn set(mode: PinMode, level: Option<bool>) -> Change {
        Change::Mode { mode, level }
    }

    /// Applies a change as the firmware would, after [check] allowed it.
    fn apply(ports: &mut Ports, p: McuPin, change: Change) -> Result<(), Denied> {
        let write = check(p, change, &Claims::new(), ports)?;
        write.apply(p.pin, &mut ports[port_index(p.port)]);
        Ok(())
    }

    #[test]
    fn refused_pins_are_refused_whatever_the_mode() {
        let ports = reset_state();
        for p in [
            pin(F, 2),
            pin(C, 6),
            pin(C, 7),
            pin(A, 11),
            pin(A, 12),
            pin(A, 13),
            pin(A, 14),
            pin(A, 15),
            pin(A, 6),
            pin(D, 1),
            pin(C, 13),
            pin(A, 4),
            pin(B, 10),
            pin(B, 14),
        ] {
            for change in [
                set(PinMode::Analog, None),
                set(PinMode::PushPull, Some(true)),
                Change::Af(0),
            ] {
                assert!(
                    matches!(
                        check(p, change, &Claims::new(), &ports),
                        Err(Denied::Refused(_))
                    ),
                    "{p:?} {change:?}"
                );
            }
        }
    }

    #[test]
    fn free_pins_are_set() {
        let mut ports = reset_state();
        // the relay: PA5 push-pull high
        apply(&mut ports, pin(A, 5), set(PinMode::PushPull, Some(true))).unwrap();
        let a = &ports[0];
        assert_eq!(
            (a.mode(5), a.output(5), a.open_drain(5), a.pull(5)),
            (MODE_OUTPUT, true, false, 0)
        );
        apply(&mut ports, pin(A, 5), set(PinMode::InputPullDown, None)).unwrap();
        assert_eq!(
            (ports[0].mode(5), ports[0].pull(5), ports[0].output(5)),
            (MODE_INPUT, 2, true)
        );
        apply(&mut ports, pin(B, 13), Change::Af(4)).unwrap();
        assert_eq!((ports[1].mode(13), ports[1].af(13)), (MODE_ALTERNATE, 4));
        apply(&mut ports, pin(B, 5), Change::Af(3)).unwrap();
        assert_eq!(ports[1].af(5), 3);
        assert_eq!(ports[1].afrl, 3 << 20);
        assert_eq!(ports[1].afrh, 4 << 20);
        // PA13 / PA14 untouched by all of it
        assert_eq!(
            ports[0].moder & 0x3C00_0000,
            reset_state()[0].moder & 0x3C00_0000
        );
    }

    #[test]
    fn invalid_arguments() {
        let ports = reset_state();
        let c = Claims::new();
        let invalid = |r: Result<PinWrite, Denied>| matches!(r, Err(Denied::Invalid { .. }));
        assert!(invalid(check(
            pin(A, 16),
            set(PinMode::Input, None),
            &c,
            &ports
        )));
        assert!(invalid(check(
            pin(E, 3),
            set(PinMode::Input, None),
            &c,
            &ports
        )));
        assert!(invalid(check(
            pin(C, 0),
            set(PinMode::Input, None),
            &c,
            &ports
        )));
        assert!(invalid(check(
            pin(D, 4),
            set(PinMode::Input, None),
            &c,
            &ports
        )));
        assert!(invalid(check(pin(A, 5), Change::Af(8), &c, &ports)));
        assert!(check(pin(A, 5), Change::Af(7), &c, &ports).is_ok());
    }

    #[test]
    fn bonded_pins_are_the_44_of_the_package() {
        let n = PORTS
            .iter()
            .flat_map(|&port| (0..16).map(move |p| pin(port, p)))
            .filter(|p| bonded(*p))
            .count();
        assert_eq!(n, 44);
    }

    #[test]
    fn claims_at_boot() {
        let c = Claims::boot();
        assert_eq!(c.holder(pin(F, 0)), Some(Holder::Leds));
        assert_eq!(c.holder(pin(F, 1)), Some(Holder::Leds));
        assert_eq!(c.holder(pin(A, 3)), Some(Holder::Meter));
        assert_eq!(c.holder(pin(B, 0)), Some(Holder::Meter));
        assert_eq!(c.holder(pin(A, 7)), Some(Holder::Button));
        assert_eq!(c.holder(pin(B, 3)), Some(Holder::HeaderUart));
        assert_eq!(c.holder(pin(B, 4)), Some(Holder::HeaderUart));
        assert_eq!(c.holder(pin(B, 1)), Some(Holder::HeaderUart));
        assert_eq!(c.holder(pin(D, 3)), Some(Holder::VSbu));
        assert_eq!(c.holder(pin(A, 5)), None);
        assert_eq!(
            c.holder(pin(A, 0)),
            None,
            "SBU is free until a session takes it"
        );
        assert_eq!(
            check(
                pin(F, 1),
                set(PinMode::PushPull, Some(true)),
                &c,
                &reset_state()
            ),
            Err(Denied::InUse(Holder::Leds))
        );
        assert_eq!(
            check(pin(B, 1), set(PinMode::Input, None), &c, &reset_state()),
            Err(Denied::InUse(Holder::HeaderUart))
        );
        for h in Holder::ALL {
            assert!(h.name().len() <= 32, "{h:?}");
            assert_eq!(Holder::from_u8(h as u8), Some(h));
            for p in h.pins() {
                assert!(bonded(*p) && refused(*p).is_none(), "{h:?} {p:?}");
            }
        }
    }

    #[test]
    fn claims_come_and_go() {
        let mut c = Claims::boot();
        c.claim(Holder::Swd).unwrap();
        assert_eq!(c.holder(pin(A, 0)), Some(Holder::Swd));
        assert_eq!(c.holder(pin(D, 2)), Some(Holder::Swd));
        c.claim(Holder::Swd).unwrap();
        // the plug CC pins are the session's: UCPD2 can't have them, and takes none of them
        assert_eq!(c.claim(Holder::Ucpd2), Err(Holder::Swd));
        c.release(Holder::Swd);
        assert_eq!(c.holder(pin(A, 0)), None);
        assert_eq!(
            c.holder(pin(D, 3)),
            Some(Holder::VSbu),
            "other holders keep theirs"
        );
        c.claim(Holder::Ucpd2).unwrap();
        assert_eq!(c.claim(Holder::Swd), Err(Holder::Ucpd2));
        assert_eq!(c.holder(pin(A, 0)), None, "all or nothing");
        c.claim(Holder::Ucpd1).unwrap();
        assert_eq!(c.holder(pin(B, 15)), Some(Holder::Ucpd1));
    }

    #[test]
    fn tied_pins_never_both_drive() {
        let mut ports = reset_state();
        let mut claims = Claims::boot();
        claims.release(Holder::HeaderUart); // to see the rule rather than the claim
        let try_set = |ports: &mut Ports, p: McuPin, change: Change| {
            let w = check(p, change, &claims, ports)?;
            w.apply(p.pin, &mut ports[port_index(p.port)]);
            Ok::<_, Denied>(())
        };
        try_set(&mut ports, pin(B, 3), set(PinMode::PushPull, Some(false))).unwrap();
        assert!(matches!(
            try_set(&mut ports, pin(B, 1), set(PinMode::PushPull, Some(true))),
            Err(Denied::Refused(r)) if r.contains("PB1")
        ));
        assert!(matches!(
            try_set(&mut ports, pin(B, 1), Change::Af(0)),
            Err(Denied::Refused(_))
        ));
        try_set(&mut ports, pin(B, 1), set(PinMode::InputPullUp, None)).unwrap();
        try_set(&mut ports, pin(B, 9), Change::Af(4)).unwrap();
        for p in [pin(B, 6), pin(B, 11)] {
            assert!(matches!(
                try_set(&mut ports, p, set(PinMode::OpenDrain, Some(false))),
                Err(Denied::Refused(r)) if r.contains("SBU2_VT")
            ));
        }
        try_set(&mut ports, pin(B, 7), set(PinMode::PushPull, Some(true))).unwrap();
        assert!(matches!(
            try_set(&mut ports, pin(B, 8), set(PinMode::PushPull, Some(true))),
            Err(Denied::Refused(r)) if r.contains("SBU1_VT")
        ));
    }

    #[test]
    fn sbu_shifter_and_direct_pins_take_turns() {
        // V_SBU at 3.3 V as the firmware holds it
        let mut ports = reset_state();
        apply_raw(&mut ports, pin(D, 3), set(PinMode::PushPull, Some(true)));
        apply(&mut ports, pin(A, 1), set(PinMode::PushPull, Some(true))).unwrap();
        // OE high now would let the shifter drive against PA1
        assert!(matches!(
            apply(&mut ports, pin(B, 2), set(PinMode::PushPull, Some(true))),
            Err(Denied::Refused(r)) if r.contains("SBU_OE")
        ));
        // OE as an output, low, is fine
        apply(&mut ports, pin(B, 2), set(PinMode::PushPull, Some(false))).unwrap();
        apply(&mut ports, pin(A, 1), set(PinMode::Input, None)).unwrap();
        apply(&mut ports, pin(B, 2), set(PinMode::PushPull, Some(true))).unwrap();
        assert!(matches!(
            apply(&mut ports, pin(A, 0), set(PinMode::OpenDrain, Some(false))),
            Err(Denied::Refused(_))
        ));
        assert!(matches!(
            apply(&mut ports, pin(A, 0), Change::Af(0)),
            Err(Denied::Refused(_))
        ));
        apply(&mut ports, pin(A, 0), set(PinMode::InputPullUp, None)).unwrap();
    }

    #[test]
    fn direct_push_pull_needs_vsbu() {
        let mut ports = reset_state();
        assert!(matches!(
            apply(&mut ports, pin(A, 0), set(PinMode::PushPull, Some(false))),
            Err(Denied::Refused(r)) if r.contains("PD3")
        ));
        // open drain is fine: it never drives V_SBU's side high
        apply(&mut ports, pin(A, 0), set(PinMode::OpenDrain, Some(false))).unwrap();
        apply_raw(&mut ports, pin(D, 3), set(PinMode::PushPull, Some(true)));
        apply(&mut ports, pin(A, 0), set(PinMode::PushPull, Some(true))).unwrap();
    }

    #[test]
    fn pwr_flt_is_open_drain_only() {
        let mut ports = reset_state();
        apply(&mut ports, pin(C, 14), set(PinMode::InputPullUp, None)).unwrap();
        apply(&mut ports, pin(C, 14), set(PinMode::OpenDrain, Some(false))).unwrap();
        assert!(matches!(
            apply(&mut ports, pin(C, 14), set(PinMode::PushPull, Some(true))),
            Err(Denied::Refused(r)) if r.contains("PWR_FLT")
        ));
    }

    #[test]
    fn an_old_violation_does_not_block_other_pins() {
        let mut ports = reset_state();
        // as if something outside debug had left PB7 and PB8 both driving
        apply_raw(&mut ports, pin(B, 7), set(PinMode::PushPull, Some(true)));
        apply_raw(&mut ports, pin(B, 8), set(PinMode::PushPull, Some(false)));
        apply(&mut ports, pin(A, 5), set(PinMode::PushPull, Some(true))).unwrap();
        // and fixing it is allowed
        apply(&mut ports, pin(B, 8), set(PinMode::Input, None)).unwrap();
        assert_eq!(Violations::of(&ports), Violations(0));
    }

    /// A write past every rule, as a running function does it.
    fn apply_raw(ports: &mut Ports, p: McuPin, change: Change) {
        let port = &mut ports[port_index(p.port)];
        plan(change, port, p.pin).unwrap().apply(p.pin, port);
    }

    #[test]
    fn reasons_fit_the_wire() {
        for bit in 0..6 {
            assert!(Violations(1 << bit).reason().len() <= 64);
        }
        for port in PORTS {
            for p in 0..16 {
                if let Some(r) = refused(pin(port, p)) {
                    assert!(r.len() <= 64, "{r}");
                }
            }
        }
    }

    #[test]
    fn reset_state_is_rm0444s() {
        let r = reset_state();
        let a = &r[0];
        for p in 0..16 {
            match p {
                13 => assert_eq!(
                    (a.mode(p), a.pull(p), a.speed(p), a.af(p)),
                    (MODE_ALTERNATE, 1, 3, 0)
                ),
                14 => assert_eq!((a.mode(p), a.pull(p), a.speed(p)), (MODE_ALTERNATE, 2, 0)),
                _ => assert_eq!((a.mode(p), a.pull(p)), (MODE_ANALOG, 0)),
            }
        }
        assert!(
            r[1..]
                .iter()
                .all(|p| p.moder == 0xFFFF_FFFF && p.pupdr == 0)
        );
    }

    #[test]
    fn reg_read_only_in_peripherals() {
        assert!(check_reg_read(0x4000_A000, 8).is_ok()); // UCPD1
        assert!(check_reg_read(0x5000_0000, 10).is_ok()); // GPIOA
        assert!(check_reg_read(0x4000_9800, 32).is_ok()); // USB RAM, 2 KiB
        assert!(check_reg_read(0x4000_3FFC, 1).is_ok()); // last word of SPI3
        assert!(matches!(
            check_reg_read(0x4000_3FFC, 2),
            Err(Denied::Refused(_))
        )); // into the gap after SPI3
        assert!(matches!(
            check_reg_read(0x2000_0000, 1),
            Err(Denied::Refused(_))
        )); // SRAM
        assert!(matches!(
            check_reg_read(0x0800_0000, 1),
            Err(Denied::Refused(_))
        )); // flash
        assert!(matches!(
            check_reg_read(0x1FFF_7590, 1),
            Err(Denied::Refused(_))
        )); // UID: not a peripheral
        assert!(matches!(
            check_reg_read(0x5000_1800, 1),
            Err(Denied::Refused(_))
        )); // past GPIOF
        assert!(matches!(
            check_reg_read(0xFFFF_FFFC, 2),
            Err(Denied::Refused(_))
        ));
        assert!(matches!(
            check_reg_read(0x4000_A002, 1),
            Err(Denied::Invalid { .. })
        ));
        assert!(matches!(
            check_reg_read(0x4000_A000, 0),
            Err(Denied::Invalid { .. })
        ));
        assert!(matches!(
            check_reg_read(0x4000_A000, 33),
            Err(Denied::Invalid { .. })
        ));
    }
}
