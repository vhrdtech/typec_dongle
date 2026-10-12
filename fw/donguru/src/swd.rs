//! SWD host over the plug SBU lines (FW-15), for probe-rs over CMSIS-DAP v2 (`src/cmsis_dap.rs` carries the USB
//! side) and for the Hi-Z [`release`] used by bench power-cut tests (first use: P2625).
//!
//! Wiring (`hw/b153_typec_serial/doc/hw_b153a.md` §8, "SWD as host"), after R211/R212 bypass the TXS0102 level
//! shifter (fitted on B153A, electrically shorting each `_VT` pin to its SBU pin):
//! * SBU1_VT net (PB8, PB7 tied together): SWCLK in [`SwdOrientation::Normal`], SWDIO in
//!   [`SwdOrientation::Swapped`]. Only PB8 is driven; PB7 is the same net and stays unconfigured.
//! * SBU2 net (PA0, PB9, PB6, PB11 tied together): SWDIO in `Normal`, SWCLK in `Swapped`. Only PB6 is driven.
//!
//! `SBU_OE` (PB2, the level shifter's output enable) is driven low and never touched again: with R211/R212
//! fitted, enabling it or selecting 1.8 V on V_SBU while the MCU drives SBU at 3.3 V is explicitly unsafe
//! (hw_b153a.md §8). `SBU_PU_EN` (PC15, the switched 3K3 pull-ups) is likewise always driven low; the bit-bang
//! driver supplies its own weak pull during SWD turnaround and goes fully floating on [`release`].
//!
//! Orientation is a plain setting (no auto-detect yet: that would need UCPD2 CC polarity on the plug, not wired
//! up by any module today). It is a global, not part of the pin driver's state, so [`set_orientation`] can take
//! effect without reaching into the `dap_rs::Dap` state machine, which owns the pins while a CMSIS-DAP host is
//! connected.
//!
//! `dap_rs` is used for CMSIS-DAP command processing (request parsing, transfer retries, ack/parity handling) but
//! not its `usb` module: that targets the separate `usb-device` crate, incompatible with the `embassy-usb` stack
//! used here. [`crate::cmsis_dap`] feeds it raw packets instead.
//!
//! No host-side unit tests: this crate has no test target (`fw/donguru/Cargo.toml`, like the rest of the
//! firmware), and the bit-bang timing can only really be checked with a scope on hardware (hardware checklist,
//! FEATURES.md FW-15).
//!
//! **Plug CC drive, stopgap (FW-17, P2534#12oct-1338b)**: a downstream board such as B129A gates its SBU->SWD
//! mux on CC voltage (comparators on both CC lines, switching a 74LVC1G08 `SWD_SEL`), so without a driven CC it
//! never routes the plug SBU lines to its own MCU. [`SwdHost::drive_cc`] / [`SwdHost::release`] drive PD0/PD2
//! (UCPD2_CC1/CC2, reaching the plug CC directly since B153A has no DPO2036, hw_b153a.md §0) push-pull high
//! only while a CMSIS-DAP session is connected, Hi-Z together with SWCLK/SWDIO everywhere else: driven CC
//! back-feeds an unpowered DUT through the comparator inputs otherwise. Hardcoded to the CMSIS-DAP v2 command
//! bytes in `src/cmsis_dap.rs` rather than a real connect/disconnect callback through `dap_rs`; see the tpm task
//! for the follow-up (UCPD2 driving CC properly, with Rp/Rd and fault handling).

use core::sync::atomic::{AtomicBool, Ordering};

use cortex_m::asm::delay as cycle_delay;
use dap_rs::dap::{Dap, DapLeds, HostStatus};
use dap_rs::swd::{self, APnDP, DPRegister, RnW};
use dap_rs::swj::{self, Dependencies};
use dap_rs::{jtag, swo};
use donguru_api::SwdOrientation;
use embassy_stm32::gpio::{Flex, Level, Output, Pull, Speed};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Delay;

use crate::SwdResources;

const SYSCLK_HZ: u32 = 64_000_000;
/// Floor on SWCLK: slow enough that bit-bang jitter (task scheduling, interrupts) cannot matter.
const MIN_FREQUENCY_HZ: u32 = 10_000;
/// Ceiling: a conservative starting point for a GPIO bit-bang loop on a 64 MHz Cortex-M0+, not yet verified with
/// a scope. Raise once measured (hardware checklist, FW-15).
const MAX_FREQUENCY_HZ: u32 = 1_000_000;
/// Default SWCLK before the host sends `DAP_SWJ_Clock`.
const DEFAULT_FREQUENCY_HZ: u32 = 400_000;
/// Idle cycles driven after each SWD register transaction, SWDIO low: more than the one bit the spec requires,
/// to give the DUT margin without a measured minimum.
const IDLE_CYCLES: usize = 8;

fn half_period_cycles(frequency_hz: u32) -> u32 {
    let hz = frequency_hz.clamp(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ);
    (SYSCLK_HZ / hz / 2).max(1)
}

/// `false` = [`SwdOrientation::Normal`], `true` = [`SwdOrientation::Swapped`]. A global rather than part of the
/// pin driver's state: see the module doc.
static ORIENTATION_SWAPPED: AtomicBool = AtomicBool::new(false);

pub fn set_orientation(orientation: SwdOrientation) {
    ORIENTATION_SWAPPED.store(orientation == SwdOrientation::Swapped, Ordering::Relaxed);
}

pub fn orientation() -> SwdOrientation {
    if ORIENTATION_SWAPPED.load(Ordering::Relaxed) {
        SwdOrientation::Swapped
    } else {
        SwdOrientation::Normal
    }
}

static RELEASE_REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static RELEASE_DONE: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Release the SWD-host pins to Hi-Z, independent of any CMSIS-DAP session (`DonguruApi::swd_release`, FW-15).
/// Waits for `src/cmsis_dap.rs`'s task to have actually done it, so a caller about to cut the DUT's power can
/// rely on it.
pub async fn release() {
    RELEASE_REQUEST.signal(());
    RELEASE_DONE.wait().await;
}

/// Waits for a pending [`release`] request. For `src/cmsis_dap.rs`'s task, which owns the [`SwdHost`] and must
/// service requests even while otherwise waiting for a USB packet.
pub(crate) async fn wait_release_request() {
    RELEASE_REQUEST.wait().await;
}

/// Answers a [`release`] call once the pins have actually been released.
pub(crate) fn signal_release_done() {
    RELEASE_DONE.signal(());
}

/// GPIO bit-bang SWD pins: implements `dap_rs`'s `swj::Dependencies`, `swd::Swd` and `jtag::Jtag` all on one
/// type (DEPS = SWD = JTAG), relying on the standard library's blanket `impl<T> From<T> for T` to satisfy the
/// `From` bounds those traits ask for.
pub struct SwdPins {
    /// SBU1_VT net (PB8): SWCLK in Normal orientation, SWDIO in Swapped.
    a: Flex<'static>,
    /// SBU2 net (PB6): SWDIO in Normal orientation, SWCLK in Swapped.
    b: Flex<'static>,
    half_period: u32,
}

impl SwdPins {
    fn new(a: Flex<'static>, b: Flex<'static>) -> Self {
        let mut pins = SwdPins {
            a,
            b,
            half_period: half_period_cycles(DEFAULT_FREQUENCY_HZ),
        };
        pins.high_impedance_mode();
        pins
    }

    fn swapped(&self) -> bool {
        ORIENTATION_SWAPPED.load(Ordering::Relaxed)
    }

    fn clk(&mut self) -> &mut Flex<'static> {
        if self.swapped() { &mut self.b } else { &mut self.a }
    }

    fn dio(&mut self) -> &mut Flex<'static> {
        if self.swapped() { &mut self.a } else { &mut self.b }
    }

    fn half_delay(&self) {
        cycle_delay(self.half_period);
    }

    /// SWCLK is always push-pull: whichever net carries it in the current orientation, this driver is the only
    /// one on the plug SBU lines (hw_b153a.md §8, no other SBU-mode module runs at the same time).
    fn drive_clk_low(&mut self) {
        self.clk().set_as_output(Speed::Medium);
        self.clk().set_low();
    }

    /// SWCLK as a push-pull output, keeping its level (idles low). Every clocking path calls it: after reset or
    /// a release the pins are inputs, and probe-rs starts with SWJ_Sequence, not SWJ_Pins.
    fn clk_drive(&mut self) {
        self.clk().set_as_output(Speed::Medium);
    }

    fn dio_drive(&mut self) {
        self.dio().set_as_output(Speed::Medium);
    }

    /// SWDIO released: weak pull-up so the line idles high during turnaround, matching the bus convention (the
    /// board's own pull-ups are switched, off by default, see `SBU_PU_EN` in the module doc).
    fn dio_release(&mut self) {
        self.dio().set_as_input(Pull::Up);
    }

    fn clock_bit_out(&mut self, bit: bool) {
        self.dio().set_level(Level::from(bit));
        self.half_delay();
        self.clk().set_high();
        self.half_delay();
        self.clk().set_low();
    }

    /// Samples SWDIO at the end of the low half, before the rising edge, as the CMSIS-DAP reference does
    /// (`SW_READ_BIT`): the target changes SWDIO on the rising edge, so a sample just after it is a race.
    fn clock_bit_in(&mut self) -> bool {
        self.half_delay();
        let bit = self.dio().is_high();
        self.clk().set_high();
        self.half_delay();
        self.clk().set_low();
        bit
    }

    fn clock_cycles(&mut self, n: usize) {
        self.clk_drive();
        for _ in 0..n {
            self.half_delay();
            self.clk().set_high();
            self.half_delay();
            self.clk().set_low();
        }
    }

    fn write_bits(&mut self, nbits: usize, data: &[u8]) {
        self.clk_drive();
        self.dio_drive();
        for i in 0..nbits {
            let bit = (data[i / 8] >> (i % 8)) & 1 != 0;
            self.clock_bit_out(bit);
        }
    }

    fn read_bits(&mut self, nbits: usize, data: &mut [u8]) {
        self.clk_drive();
        self.dio_release();
        data[..nbits.div_ceil(8)].fill(0);
        for i in 0..nbits {
            if self.clock_bit_in() {
                data[i / 8] |= 1 << (i % 8);
            }
        }
    }

    fn read_bits_u32(&mut self, nbits: usize) -> u32 {
        self.clk_drive();
        self.dio_release();
        let mut v = 0u32;
        for i in 0..nbits {
            if self.clock_bit_in() {
                v |= 1 << i;
            }
        }
        v
    }

    /// One ARM ADIv5 SWD register transaction: 8-bit request, turnaround, 3-bit ack, then (on ack OK) a 32-bit
    /// data phase with parity, in the direction given by `rnw`. On a read, returns the data; `write_value` is
    /// ignored. On a write, `write_value` is sent; the return value is unused.
    fn transaction(&mut self, apndp: APnDP, rnw: RnW, a: DPRegister, write_value: u32) -> swd::Result<u32> {
        self.write_bits(8, &[swd::make_request(apndp, rnw, a)]);

        // Turnaround: host releases SWDIO so the target can drive the ack.
        self.dio_release();
        self.clock_cycles(1);

        let ack = self.read_bits_u32(3) as u8;
        let ack_result = swd::Ack::try_ok(ack);

        // Exactly one more turnaround happens in this transaction, at the one point where the direction
        // changes again: back to the target->host direction for a read's data phase (none needed, same
        // direction as the ack just read), or back to host->target before a write's data phase or before the
        // idle cycles below. `dio_drive()` after this match performs the actual GPIO direction switch.
        let result = match (ack_result, rnw) {
            (Ok(()), RnW::R) => {
                let data = self.read_bits_u32(32);
                let parity = self.read_bits_u32(1) != 0;
                self.clock_cycles(1);
                if parity != (data.count_ones() % 2 == 1) {
                    Err(swd::Error::BadParity)
                } else {
                    Ok(data)
                }
            }
            (Ok(()), RnW::W) => {
                self.clock_cycles(1);
                let parity = write_value.count_ones() % 2 == 1;
                let mut buf = [0u8; 5];
                buf[..4].copy_from_slice(&write_value.to_le_bytes());
                buf[4] = parity as u8;
                self.write_bits(33, &buf);
                Ok(0)
            }
            (Err(e), _) => {
                // No data phase on WAIT/FAULT.
                self.clock_cycles(1);
                Err(e)
            }
        };

        self.dio_drive();
        self.dio().set_low();
        self.clock_cycles(IDLE_CYCLES);
        result
    }
}

impl swj::Dependencies<Self, Self> for SwdPins {
    fn high_impedance_mode(&mut self) {
        self.a.set_as_input(Pull::None);
        self.b.set_as_input(Pull::None);
    }

    fn process_swj_pins(&mut self, output: swj::Pins, mask: swj::Pins, wait_us: u32) -> swj::Pins {
        // No NRESET/NTRST/TDI/TDO wires on this SBU-only SWD path (hw_b153a.md §8): only SWCLK/SWDIO are real.
        if mask.contains(swj::Pins::SWCLK) {
            self.drive_clk_low();
            self.clk().set_level(Level::from(output.contains(swj::Pins::SWCLK)));
        }
        if mask.contains(swj::Pins::SWDIO) {
            self.dio_drive();
            self.dio().set_level(Level::from(output.contains(swj::Pins::SWDIO)));
        }
        if wait_us > 0 {
            cycle_delay(wait_us.saturating_mul(SYSCLK_HZ / 1_000_000));
        }
        let mut result = swj::Pins::NRESET; // not wired: report as never asserted
        if self.clk().is_high() {
            result |= swj::Pins::SWCLK;
        }
        if self.dio().is_high() {
            result |= swj::Pins::SWDIO;
        }
        result
    }

    fn process_swj_sequence(&mut self, data: &[u8], nbits: usize) {
        self.write_bits(nbits, data);
    }

    fn process_swj_clock(&mut self, max_frequency: u32) -> bool {
        self.half_period = half_period_cycles(max_frequency);
        true
    }
}

impl swd::Swd<Self> for SwdPins {
    const AVAILABLE: bool = true;

    fn read_inner(&mut self, apndp: APnDP, a: DPRegister) -> swd::Result<u32> {
        self.transaction(apndp, RnW::R, a, 0)
    }

    fn write_inner(&mut self, apndp: APnDP, a: DPRegister, data: u32) -> swd::Result<()> {
        self.transaction(apndp, RnW::W, a, data).map(|_| ())
    }

    fn set_clock(&mut self, max_frequency: u32) -> bool {
        self.process_swj_clock(max_frequency)
    }

    fn write_sequence(&mut self, num_bits: usize, data: &[u8]) -> swd::Result<()> {
        self.write_bits(num_bits, data);
        Ok(())
    }

    fn read_sequence(&mut self, num_bits: usize, data: &mut [u8]) -> swd::Result<()> {
        self.read_bits(num_bits, data);
        Ok(())
    }
}

/// JTAG is not wired on this SBU path (only SWCLK/SWDIO, see the module doc): `AVAILABLE = false` means
/// `dap_rs` never calls into these, `DAP_Connect(JTAG)` fails cleanly instead.
impl jtag::Jtag<Self> for SwdPins {
    const AVAILABLE: bool = false;

    fn sequences(&mut self, _data: &[u8], _rxbuf: &mut [u8]) -> u32 {
        0
    }

    fn set_clock(&mut self, max_frequency: u32) -> bool {
        self.process_swj_clock(max_frequency)
    }
}

/// SWO (trace) is not wired on this SBU path either: `dap_rs` still needs a concrete type for the generic
/// parameter even unused, since `Dap::new` is called with `None::<NoSwo>`.
pub(crate) struct NoSwo;

impl swo::Swo for NoSwo {
    fn set_transport(&mut self, _transport: swo::SwoTransport) {}
    fn set_mode(&mut self, _mode: swo::SwoMode) {}
    fn set_baudrate(&mut self, _baudrate: u32) -> u32 {
        0
    }
    fn set_control(&mut self, _control: swo::SwoControl) {}
    fn polling_data(&mut self, _buf: &mut [u8]) -> u32 {
        0
    }
    fn streaming_data(&mut self) {}
    fn is_active(&self) -> bool {
        false
    }
    fn bytes_available(&self) -> u32 {
        0
    }
    fn buffer_size(&self) -> u32 {
        0
    }
    fn support(&self) -> swo::SwoSupport {
        swo::SwoSupport {
            uart: false,
            manchester: false,
        }
    }
    fn status(&mut self) -> swo::SwoStatus {
        swo::SwoStatus {
            active: false,
            trace_error: false,
            trace_overrun: false,
            bytes_available: 0,
        }
    }
}

/// The donguru status LED already carries device heartbeat / USB state (`src/led.rs`); not repurposing it here
/// without a design decision, so host-connect / host-running status from CMSIS-DAP is only logged.
pub(crate) struct NoLeds;

impl DapLeds for NoLeds {
    fn react_to_host_status(&mut self, host_status: HostStatus) {
        match host_status {
            HostStatus::Connected(true) => defmt::debug!("swd: probe-rs session connected"),
            HostStatus::Connected(false) => defmt::debug!("swd: probe-rs session disconnected"),
            HostStatus::Running(running) => defmt::trace!("swd: target running: {}", running),
        }
    }
}

pub type DapHandler = Dap<'static, SwdPins, NoLeds, Delay, SwdPins, SwdPins, NoSwo>;

/// Builds the `dap_rs` handler and the always-low OE / pull-up-enable outputs, from the resources in
/// `SwdResources`. Used by `src/cmsis_dap.rs`, which owns the result, drives [`DapHandler::process_command`]
/// from the CMSIS-DAP v2 USB endpoints, and services [`release`] requests via [`SwdHost::release`].
pub struct SwdHost {
    pub dap: DapHandler,
    oe: Output<'static>,
    pu_en: Output<'static>,
    /// PD0 (UCPD2_CC1, plug), stopgap CC drive: see the module doc.
    cc1: Flex<'static>,
    /// PD2 (UCPD2_CC2, plug), stopgap CC drive: see the module doc.
    cc2: Flex<'static>,
}

impl SwdHost {
    pub fn new(r: SwdResources) -> Self {
        let a = Flex::new(r.pb8);
        let b = Flex::new(r.pb6);
        let pins = SwdPins::new(a, b);
        // Never enabled: R211/R212 already bypass the level shifter, and hw_b153a.md §8 warns never to enable
        // OE while the MCU drives SBU at 3.3 V.
        let oe = Output::new(r.pb2, Level::Low, Speed::Low);
        // Never enabled: the bit-bang driver supplies its own weak pull during SWD turnaround (see `dio_release`).
        let pu_en = Output::new(r.pc15, Level::Low, Speed::Low);
        let mut cc1 = Flex::new(r.pd0);
        let mut cc2 = Flex::new(r.pd2);
        cc1.set_as_input(Pull::None);
        cc2.set_as_input(Pull::None);
        SwdHost {
            dap: Dap::new(pins, NoLeds, Delay, None, "donguru-swd 0.1"),
            oe,
            pu_en,
            cc1,
            cc2,
        }
    }

    /// Drive both plug CC lines push-pull high: stopgap, see the module doc. Called from `src/cmsis_dap.rs` on a
    /// successful `DAP_Connect`.
    pub fn drive_cc(&mut self) {
        self.cc1.set_as_output(Speed::Low);
        self.cc1.set_high();
        self.cc2.set_as_output(Speed::Low);
        self.cc2.set_high();
    }

    /// Hi-Z the SWD-host pins and the plug CC lines, independent of any CMSIS-DAP session. Idempotent.
    pub fn release(&mut self) {
        self.dap.suspend();
        self.oe.set_low();
        self.pu_en.set_low();
        self.cc1.set_as_input(Pull::None);
        self.cc2.set_as_input(Pull::None);
    }
}
