//! SWD host over the plug SBU lines (FW-15), for probe-rs over CMSIS-DAP v2 (`src/cmsis_dap.rs` carries the USB
//! side) and for the Hi-Z [`release`] used by bench power-cut tests (first use: P2625).
//!
//! Wiring (`hw/b153_typec_serial/doc/hw_b153a.md` §8, "SWD as host"): R211/R212, the 0 Ω bypass of the TXS0102
//! level shifter, are not fitted on B153A (checked on the bench 12 Oct 2026: PB8/PB6 don't reach PA1/PA0), so
//! the driver works on the SBU side of the shifter, the only MCU pins wired straight to the plug:
//! * SBU1 net (PA1): SWCLK in [`SwdOrientation::Normal`], SWDIO in [`SwdOrientation::Swapped`].
//! * SBU2 net (PA0): SWDIO in `Normal`, SWCLK in `Swapped`.
//! In this default bit-bang mode the `_VT` pins (PB6..PB11) stay inputs, so it also works on a board with the bypass
//! fitted. The SPI mode (FW-21, [`SwdMode::Spi`], "SPI-driven SWD" below) is the other way round: SPI2 on the `_VT`
//! side, through the shifter with `SBU_OE` high and PA0/PA1 Hi-Z. ⚠️ Not for a board with R211/R212 fitted: OE
//! must never be high there while the MCU drives SBU (hw_b153a.md §8).
//!
//! V_SBU is set to 3.3 V (`VSBU_SEL` PD3 high) since PA0/PA1 drive 3.3 V on the shifter's A side.
//! `SBU_OE` (PB2, the level shifter's output enable) is low except while the SPI mode is engaged, and goes low first
//! in every release (hw_b153a.md §8: its 10K pull-ups on both sides are on only while OE is high). `SBU_PU_EN`
//! (PC15, the switched 3K3 pull-ups) is always driven low; the bit-bang driver supplies its own weak pull during
//! SWD turnaround and goes fully floating on [`release`].
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
//! The bit-bang kernels and how their timing is calibrated: see "Bit-bang kernels" below (FW-20). This crate has no
//! test target (`fw/donguru/Cargo.toml`, like the rest of the firmware): the timing arithmetic is unit-tested in
//! `donguru_dap::swclk`, the real clock can only be checked with a scope or logic analyzer on hardware.
//!
//! **Plug CC drive, stopgap (FW-17, P2534#12oct-1338b)**: a downstream board such as B129A gates its SBU->SWD
//! mux on CC voltage (comparators on both CC lines, switching a 74LVC1G08 `SWD_SEL`), so without a driven CC it
//! never routes the plug SBU lines to its own MCU. [`SwdHost::drive_cc`] / [`SwdHost::release`] drive PD0/PD2
//! (UCPD2_CC1/CC2, reaching the plug CC directly since B153A has no DPO2036, hw_b153a.md §0) push-pull high
//! only while a CMSIS-DAP session is connected, Hi-Z together with SWCLK/SWDIO everywhere else: driven CC
//! back-feeds an unpowered DUT through the comparator inputs otherwise. Hardcoded to the CMSIS-DAP v2 command
//! bytes in `src/cmsis_dap.rs` rather than a real connect/disconnect callback through `dap_rs`; see the tpm task
//! for the follow-up (UCPD2 driving CC properly, with Rp/Rd and fault handling).

use core::arch::asm;
use core::ptr::write_volatile;
use core::sync::atomic::{AtomicBool, Ordering};

use cnt::cnt;
use cortex_m::asm::delay as cycle_delay;
use dap_rs::dap::{Dap, DapLeds, HostStatus};
use dap_rs::swd::{self, APnDP, DPRegister, RnW};
use dap_rs::swj::{self, Dependencies};
use dap_rs::{jtag, swo};
use donguru_api::{SwdMode, SwdOrientation};
use donguru_dap::swclk::{Halves, KernelCost, spi_prescaler};
use embassy_stm32::Peri;
use embassy_stm32::gpio::{Flex, Level, Output, Pull, Speed};
use embassy_stm32::pac;
use embassy_stm32::pac::gpio::{Gpio, vals};
use embassy_stm32::pac::spi::vals as spi_vals;
use embassy_stm32::peripherals::SPI2;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Delay;

use crate::SwdResources;

const SYSCLK_HZ: u32 = 64_000_000;
/// Floor on SWCLK: slow enough that bit-bang jitter (task scheduling, interrupts) cannot matter.
const MIN_FREQUENCY_HZ: u32 = 10_000;
/// Ceiling on the requested SWCLK: about what the kernels do at their fastest with both clock halves equal
/// (measured 13 Oct 2026: about 3 MHz). The clock actually reached is lower when the kernels can't go that fast
/// (see [`Calibration`]); `DAP_SWJ_Clock` still answers OK then, and the boot log says what the top speed is.
const MAX_FREQUENCY_HZ: u32 = 3_000_000;
/// Default SWCLK before the host sends `DAP_SWJ_Clock`.
const DEFAULT_FREQUENCY_HZ: u32 = 400_000;
/// Idle cycles driven after each SWD register transaction, SWDIO low: more than the one bit the spec requires,
/// to give the DUT margin without a measured minimum.
const IDLE_CYCLES: u32 = 8;

/// GPIOA pin of the SBU1 net (PA1).
const PIN_A: u32 = 1;
/// GPIOA pin of the SBU2 net (PA0).
const PIN_B: u32 = 0;

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

// ---- Bit-bang kernels (FW-20) ----
//
// Two small loops do all the clocking, in inline assembly on GPIOA's BSRR / BRR / IDR: one store per edge, SWCLK
// falling and the next SWDIO bit in the same BSRR write. Assembly rather than Rust because the compiler spilled the
// loop's values to the stack (about 24 cycles per bit at the least, 2.7 MHz) and its cycle count changes with the
// build profile; the assembly loops are 11-12 cycles plus 3 per spin iteration by the instruction count (about 21
// at the fastest on the scope, so about 3 MHz), the same in every build, with both clock halves the same length. They take the pin masks as arguments, so the orientation is resolved once per transfer
// ([`SwdPins::engage`]), not per edge. Each clock half is padded with a countdown loop (`subs` + `bne`, a fixed
// cost per iteration, at least 1 iteration: 0 would spin 2^32 times); how many iterations give the requested
// frequency comes from [`Calibration`], which times the very same kernels with SysTick at boot, so the numbers
// hold even where a GPIO store or a flash fetch takes a cycle more than counted. The kernels are
// `#[inline(never)]`: one copy of the code, so what the calibration measured is what runs.

/// Clocks out `nbits` (0..=32) bits of `word`, LSB first: per bit SWCLK falls and SWDIO takes the bit (one BSRR
/// write), `lo` spins, SWCLK rises (the target samples), `hi` spins; SWCLK low at the end. With `dio == 0` it only
/// clocks (turnaround, idle cycles), SWDIO untouched. Branch-free per bit, so every bit costs the same. The next
/// bit's BSRR word is worked out in the low half and the loop counter runs in the high one, so with `lo == hi`
/// the two halves take the same time give or take a cycle or two (by the instruction count: 3 lo + 5 low,
/// 3 hi + 5 high, 12 + 3 * (lo + hi) per bit with single-cycle GPIO stores).
#[inline(never)]
fn out_bits(bus: Bus, dio: u32, h: Halves, word: u32, nbits: u32) {
    let (bsrr, clk) = (bus.bsrr(), bus.clk);
    let clk_low = clk << 16;
    if nbits > 0 {
        // BSRR word for a 1 bit (SWDIO set), and what turns it into the one for a 0 bit (SWDIO reset)
        let one = clk_low | dio;
        let flip = one ^ (clk_low | (dio << 16));
        let high = [h.lo, h.hi, clk];
        // SAFETY: only writes the port's BSRR, a set/reset register: just the SWCLK/SWDIO bits in the masks change.
        // `h.lo`, `h.hi` >= 1 (`Halves`), `nbits` >= 1.
        unsafe {
            asm!(
                // Only r0-r7 can be operands in Thumb-1: the spin counts, SWCLK's mask and the bit counter go to
                // r8, r10, r11, r12; `t` starts as the pointer to the first three, `v` as the bit count.
                "mov r12, {v}",
                "ldr {v}, [{t}]",
                "mov r8, {v}",
                "ldr {v}, [{t}, #4]",
                "mov r10, {v}",
                "ldr {v}, [{t}, #8]",
                "mov r11, {v}",
                // v = BSRR word of the first bit
                "lsrs {word}, {word}, #1", // C = bit
                "sbcs {v}, {v}",           // 0 for a 1 bit, all ones for a 0 bit
                "ands {v}, {flip}",
                "eors {v}, {one}",
                "2:",
                "str {v}, [{bsrr}]",       // SWCLK falls, SWDIO takes the bit
                "lsrs {word}, {word}, #1", // the next bit's word (unused after the last)
                "sbcs {v}, {v}",
                "ands {v}, {flip}",
                "eors {v}, {one}",
                "mov {t}, r8",
                "3:",
                "subs {t}, #1",
                "bne 3b",
                "mov {t}, r11",
                "str {t}, [{bsrr}]",       // SWCLK rises
                "mov {t}, r10",
                "4:",
                "subs {t}, #1",
                "bne 4b",
                "mov {t}, r12",
                "subs {t}, #1",
                "mov r12, {t}",
                "bne 2b",
                word = inout(reg) word => _,
                v = inout(reg) nbits => _,
                t = inout(reg) high.as_ptr() => _,
                flip = in(reg) flip,
                one = in(reg) one,
                bsrr = in(reg) bsrr,
                out("r8") _,
                out("r10") _,
                out("r11") _,
                out("r12") _,
                options(nostack),
            );
        }
    }
    // SAFETY: the port's BSRR, as above.
    unsafe { write_volatile(bsrr, clk_low) };
}

/// Clocks in `nbits` (0..=32) bits, LSB first: per bit SWCLK falls, `lo` spins, SWDIO (pin `dio_pin`) is sampled
/// just before SWCLK rises, as the CMSIS-DAP reference does (`SW_READ_BIT`: the target changes SWDIO on the rising
/// edge, so a sample just after it is a race), `hi` spins; SWCLK low at the end. Each sample is shifted in during
/// the next low half and the loop counter runs in the high one, so with `lo == hi` the halves take the same time
/// give or take a cycle or two (by the instruction count: 3 lo + 4 low, 3 hi + 5 high, 11 + 3 * (lo + hi) per bit
/// with single-cycle GPIO access).
#[inline(never)]
fn in_bits(bus: Bus, h: Halves, nbits: u32) -> u32 {
    let (idr, clk, dio_pin) = (bus.idr(), bus.clk, bus.dio_pin);
    let mut v = 0u32;
    if nbits > 0 {
        let high = [h.lo, h.hi, nbits];
        // shifting the IDR value right by this leaves SWDIO in the carry
        let shift = dio_pin + 1;
        // SAFETY: reads the port's IDR and writes BSRR (IDR + 8) / BRR (IDR + 0x18), set/reset registers: only SWCLK
        // changes. `h.lo`, `h.hi` >= 1 (`Halves`), `nbits` >= 1.
        unsafe {
            asm!(
                // The spin counts and the bit counter go to r8, r10, r11 (see `out_bits`).
                "ldr {b}, [{t}]",
                "mov r8, {b}",
                "ldr {b}, [{t}, #4]",
                "mov r10, {b}",
                "ldr {b}, [{t}, #8]",
                "mov r11, {b}",
                "movs {b}, #0",              // the first round shifts in a 0 before any sample
                "2:",
                "str {clk}, [{idr}, #0x18]", // BRR: SWCLK falls
                "lsrs {b}, {shift}",         // C = previous sample's SWDIO
                "adcs {v}, {v}",             // first bit ends up highest
                "mov {t}, r8",
                "3:",
                "subs {t}, #1",
                "bne 3b",
                "ldr {b}, [{idr}]",          // sample SWDIO
                "str {clk}, [{idr}, #8]",    // BSRR: SWCLK rises
                "mov {t}, r10",
                "4:",
                "subs {t}, #1",
                "bne 4b",
                "mov {t}, r11",
                "subs {t}, #1",
                "mov r11, {t}",
                "bne 2b",
                "lsrs {b}, {shift}",         // the last sample
                "adcs {v}, {v}",
                v = inout(reg) v,
                t = inout(reg) high.as_ptr() => _,
                b = out(reg) _,
                shift = in(reg) shift,
                clk = in(reg) clk,
                idr = in(reg) idr,
                out("r8") _,
                out("r10") _,
                out("r11") _,
                options(nostack),
            );
        }
    }
    // SAFETY: the port's BSRR.
    unsafe { write_volatile(bus.bsrr(), clk << 16) };
    // nbits + 1 bits shifted in, the leading 0 first: it falls off the top (nbits = 32) or above the result. A
    // shift by 32 (nbits = 0) wraps to 0, v is 0 anyway.
    v.reverse_bits().wrapping_shr(32u32.wrapping_sub(nbits))
}

/// What the two kernels cost on this chip and build, measured once at boot with SysTick (free: embassy's time
/// driver runs on TIM4). Done with SWCLK/SWDIO as inputs: the kernels then only change GPIOA's output data bits
/// for PA0/PA1, which reach no pin while the mode is input, so the plug sees nothing. Interrupts are off for the
/// few microseconds it takes. Per kernel, bits are timed in two batch sizes (the difference drops the call and
/// SysTick read overhead) with two spin counts per half (the difference is the spin cost); the arithmetic is
/// `donguru_dap::swclk`, unit-tested on the host.
#[derive(Clone, Copy)]
struct Calibration {
    out: KernelCost,
    inp: KernelCost,
}

impl Calibration {
    fn measure() -> Self {
        cortex_m::interrupt::free(|_| {
            let syst = unsafe { &*cortex_m::peripheral::SYST::PTR };
            // SAFETY: SysTick is unused in this firmware; started here from the core clock and stopped again.
            unsafe {
                syst.rvr.write(0x00FF_FFFF);
                syst.cvr.write(0);
                syst.csr.write(0b101); // ENABLE, CLKSOURCE = processor clock, no interrupt
            }
            let time = |f: &dyn Fn()| {
                let t0 = syst.cvr.read();
                f();
                let t1 = syst.cvr.read();
                t0.wrapping_sub(t1) & 0x00FF_FFFF
            };
            let bus = Bus {
                port: pac::GPIOA,
                clk: 1 << PIN_A,
                dio: 1 << PIN_B,
                dio_pin: PIN_B,
            };
            let cost = |kernel: &dyn Fn(Halves, u32)| {
                let (fast, slow) = (1, 1 + KernelCost::EXTRA);
                let sample = |spins: u32, bits: u32| time(&|| kernel(Halves { lo: spins, hi: spins }, bits));
                let n = KernelCost::BATCH;
                KernelCost::from_samples(
                    sample(fast, n),
                    sample(fast, 2 * n),
                    sample(slow, n),
                    sample(slow, 2 * n),
                )
            };
            let out = cost(&|h, n| out_bits(bus, bus.dio, h, 0x5555_5555, n));
            let inp = cost(&|h, n| {
                core::hint::black_box(in_bits(bus, h, n));
            });
            // SAFETY: back to SysTick's reset state.
            unsafe { syst.csr.write(0) };
            Calibration { out, inp }
        })
    }

    fn timing(&self, frequency_hz: u32) -> Timing {
        let hz = frequency_hz.clamp(MIN_FREQUENCY_HZ, MAX_FREQUENCY_HZ);
        let period = SYSCLK_HZ.div_ceil(hz);
        let (out, out_period) = self.out.halves(period);
        let (inp, _) = self.inp.halves(period);
        Timing {
            out,
            inp,
            real_hz: SYSCLK_HZ / out_period,
        }
    }
}

/// Spin counts for one SWCLK frequency.
#[derive(Clone, Copy)]
struct Timing {
    out: Halves,
    inp: Halves,
    /// SWCLK the write kernel actually runs at (the read kernel is within a cycle or two of it).
    real_hz: u32,
}

/// The pins of one transfer, on one GPIO port: resolved once from the orientation and the mode by
/// [`SwdPins::engage`].
#[derive(Clone, Copy)]
struct Bus {
    port: Gpio,
    /// SWCLK's mask.
    clk: u32,
    /// SWDIO's mask.
    dio: u32,
    /// SWDIO's pin number.
    dio_pin: u32,
}

impl Bus {
    fn bsrr(&self) -> *mut u32 {
        self.port.bsrr().as_ptr() as *mut u32
    }

    fn idr(&self) -> *mut u32 {
        self.port.idr().as_ptr() as *mut u32
    }

    /// SWDIO driven by the host, starting at `level`: output data set before the mode, so no glitch.
    fn dio_drive(&self, level: bool) {
        let d = self.dio_pin as usize;
        cortex_m::interrupt::free(|_| {
            self.port
                .bsrr()
                .write(|w| if level { w.set_bs(d, true) } else { w.set_br(d, true) });
            self.port.moder().modify(|w| w.set_moder(d, vals::Moder::OUTPUT));
        });
    }

    /// SWDIO released to the target, a pull-up holding it high meanwhile: the MCU's own on the direct pins (see
    /// [`SwdPins::engage`]), the level shifter's 10K ones on the shifter side.
    fn dio_release(&self) {
        let d = self.dio_pin as usize;
        cortex_m::interrupt::free(|_| self.port.moder().modify(|w| w.set_moder(d, vals::Moder::INPUT)));
    }
}

// ---- SPI-driven SWD (FW-21) ----
//
// SPI2 through the TXS0102 level shifter, the only way its pins reach the plug on B153A (R211/R212 not fitted,
// hw_b153a.md §8): SBU_OE (PB2) high, V_SBU at 3.3 V (PD3, set at boot), PA0/PA1 Hi-Z since they sit on the
// shifter's A side. SWCLK = SPI2_SCK on PB8 (SBU1_VT); SWDIO = SPI2_MOSI on PB11 and SPI2_MISO on PB6, both on
// SBU2_VT, full duplex: MOSI is switched to input (MODER) while the target drives, so MISO hears the target. Normal
// orientation only: SBU1_VT has no MISO pin, a swapped cable would need BIDIMODE on PB7 (not done; swapped falls
// back to the direct bit-bang).
//
// Every frame is CPOL 0 / CPHA 0, LSB first: MOSI changes on the falling edge and the target samples on the rising
// one, as SWD wants; MISO is sampled on the rising edge, which reads the bit the target put out on the previous
// rising edge: the same point as the bit-bang's sample just before the edge, with the propagation through the
// shifter, the cable and the target's output delay as the hold margin. Frames are 4..16 bits, so a transaction is
// cut as: request 8 | turnaround + ack 4 | then for a read data 16 + 14 + (2 data + parity + turnaround) 4, idle 8
// with MOSI back on; for a write (turnaround + 15 data) 16 + 16 + (1 data + parity + idle 8) 10.
//
// Who drives SWDIO when, so the host never fights the target: MOSI goes back on only where the target is either
// done (after a read's last turnaround) or driving the same level. For a write, the frame after the ack starts with
// the turnaround bit while the target may still be driving its last ack bit; that bit is 0 for OK, and MOSI then
// outputs 0 too (the SPI keeps the last bit sent, 0 from the receive frames). A WAIT or FAULT ack is followed by a
// bit-banged turnaround and idle on the same pins (`out_bits` on GPIOB), released exactly like the direct path.
// Sequences (line reset, JTAG-to-SWD, any bit count) and clocks under PCLK / 256 bit-bang on the B pins the same way.

/// Ceiling on the SPI-driven SWCLK: 4 MHz worked on B153A + B129A (13 Oct 2026), 8 MHz failed there (protocol errors:
/// weak highs through the shifter against the DUT's network, see FEATURES.md FW-21).
const SPI_MAX_HZ: u32 = 4_000_000;
/// SPI2's kernel clock: PCLK, APB undivided (`src/main.rs`).
const PCLK_HZ: u32 = SYSCLK_HZ;
/// Polls of SPI2's status per frame before giving up: a 16-bit frame at 250 kHz takes about 4100 CPU cycles.
const SPI_POLL_LIMIT: u32 = 20_000;
/// SBU1_VT (PB8): SPI2_SCK, AF1.
const PB_SCK: usize = 8;
/// SBU2_VT (PB11): SPI2_MOSI, AF0.
const PB_MOSI: usize = 11;
/// SBU2_VT (PB6): SPI2_MISO, AF4.
const PB_MISO: usize = 6;

/// `false` = [`SwdMode::BitBang`], `true` = [`SwdMode::Spi`]. Global like the orientation; the `swd-spi` build
/// feature makes SPI the default.
static MODE_SPI: AtomicBool = AtomicBool::new(cfg!(feature = "swd-spi"));

pub fn set_mode(mode: SwdMode) {
    MODE_SPI.store(mode == SwdMode::Spi, Ordering::Relaxed);
}

pub fn mode() -> SwdMode {
    if MODE_SPI.load(Ordering::Relaxed) {
        SwdMode::Spi
    } else {
        SwdMode::BitBang
    }
}

/// One SPI2 frame of `bits` (4..=16), LSB first, full duplex: sends `tx`, returns what MISO saw at each rising
/// edge, once the frame is completely over (BSY clear: the last SCK edge is out). Only called while idle between
/// frames, which is when the data size may change. `AckProtocol` if SPI2 does not finish in time, which only a
/// misconfiguration could cause (counted).
fn spi_frame(bits: u32, tx: u32) -> swd::Result<u32> {
    let spi = pac::SPI2;
    let small = bits <= 8;
    spi.cr2().modify(|w| {
        w.set_ds(spi_vals::Ds::from_bits(bits as u8 - 1));
        w.set_frxth(if small {
            spi_vals::Frxth::QUARTER
        } else {
            spi_vals::Frxth::HALF
        });
    });
    // A data-register access as wide as the frame: a 16-bit write of a frame of 8 bits or less sends two.
    if small {
        spi.dr8().write_value(tx as u8);
    } else {
        spi.dr16().write_value(tx as u16);
    }
    let wait = |done: &dyn Fn() -> bool| {
        for _ in 0..SPI_POLL_LIMIT {
            if done() {
                return Ok(());
            }
        }
        cnt!(swd_spi_timeouts: u32);
        Err(swd::Error::AckProtocol)
    };
    wait(&|| spi.sr().read().rxne())?;
    let rx = if small {
        spi.dr8().read() as u32
    } else {
        spi.dr16().read() as u32
    };
    wait(&|| !spi.sr().read().bsy())?;
    Ok(rx & ((1 << bits) - 1))
}

/// SPI2 to SCK (`sck`) and MOSI (`mosi`) on their pins, or the GPIO modes: SCK a plain output (driven low: its
/// output data stays low, and SPI2 idles SCK low too, so the switch is glitch-free), MOSI an input.
fn shifter_modes(sck: bool, mosi: bool) {
    cortex_m::interrupt::free(|_| {
        pac::GPIOB.moder().modify(|w| {
            w.set_moder(
                PB_SCK,
                if sck {
                    vals::Moder::ALTERNATE
                } else {
                    vals::Moder::OUTPUT
                },
            );
            w.set_moder(
                PB_MOSI,
                if mosi {
                    vals::Moder::ALTERNATE
                } else {
                    vals::Moder::INPUT
                },
            );
        })
    });
}

/// The level-shifter side (FW-21): SPI2 and its pins, and the shifter's output enable.
pub struct ShifterPins {
    /// SBU1_VT (PB8), SPI2_SCK.
    pub sck: Flex<'static>,
    /// SBU2_VT (PB11), SPI2_MOSI.
    pub mosi: Flex<'static>,
    /// SBU2_VT (PB6), SPI2_MISO.
    pub miso: Flex<'static>,
    /// SBU_OE (PB2), TXS0102 output enable, R213 pull-down. High only while the shifter side is engaged.
    pub oe: Output<'static>,
    /// Held so nothing else can take SPI2: driven through its registers here.
    pub _spi: Peri<'static, SPI2>,
}

/// SWD pins: implements `dap_rs`'s `swj::Dependencies`, `swd::Swd` and `jtag::Jtag` all on one type (DEPS = SWD =
/// JTAG), relying on the standard library's blanket `impl<T> From<T> for T` to satisfy the `From` bounds those
/// traits ask for.
pub struct SwdPins {
    /// SBU1 net (PA1): SWCLK in Normal orientation, SWDIO in Swapped.
    a: Flex<'static>,
    /// SBU2 net (PA0): SWDIO in Normal orientation, SWCLK in Swapped.
    b: Flex<'static>,
    shifter: ShifterPins,
    /// The shifter side is engaged: OE high, SPI2 on, PA0/PA1 Hi-Z.
    shifted: bool,
    /// SPI2 prescaler for the current clock, `None` under PCLK / 256 (bit-bang on the shifter side then).
    spi_br: Option<u8>,
    cal: Calibration,
    timing: Timing,
}

impl SwdPins {
    fn new(a: Flex<'static>, b: Flex<'static>, shifter: ShifterPins) -> Self {
        let mut pins = SwdPins {
            a,
            b,
            shifter,
            shifted: false,
            spi_br: spi_prescaler(PCLK_HZ, DEFAULT_FREQUENCY_HZ.min(SPI_MAX_HZ)),
            cal: Calibration {
                out: KernelCost { base: 0, per_spin: 1 },
                inp: KernelCost { base: 0, per_spin: 1 },
            },
            timing: Timing {
                out: Halves { lo: 1, hi: 1 },
                inp: Halves { lo: 1, hi: 1 },
                real_hz: 0,
            },
        };
        // Inputs first: the calibration toggles the output data bits, which must not reach the plug.
        pins.high_impedance_mode();
        pins.cal = Calibration::measure();
        pins.timing = pins.cal.timing(DEFAULT_FREQUENCY_HZ);
        let top = pins.cal.timing(MAX_FREQUENCY_HZ).real_hz;
        let Calibration { out, inp } = pins.cal;
        defmt::info!(
            "swd: cycles per bit out {}+{}/spin, in {}+{}/spin, top SWCLK {} Hz",
            out.base,
            out.per_spin,
            inp.base,
            inp.per_spin,
            top
        );
        pins
    }

    fn swapped(&self) -> bool {
        ORIENTATION_SWAPPED.load(Ordering::Relaxed)
    }

    /// The shifter side carries this transfer: SPI mode asked for, and a cable it works with.
    fn use_shifter(&self) -> bool {
        MODE_SPI.load(Ordering::Relaxed) && !self.swapped()
    }

    fn clk(&mut self) -> &mut Flex<'static> {
        if self.use_shifter() {
            &mut self.shifter.sck
        } else if self.swapped() {
            &mut self.b
        } else {
            &mut self.a
        }
    }

    fn dio(&mut self) -> &mut Flex<'static> {
        if self.use_shifter() {
            &mut self.shifter.mosi
        } else if self.swapped() {
            &mut self.a
        } else {
            &mut self.b
        }
    }

    /// Resolves the orientation and the mode for one transfer, engages that side (releasing the other) and puts
    /// SWCLK out as a GPIO, push-pull and low. Every clocking path calls it: after reset or a release the pins are
    /// inputs, and probe-rs starts with SWJ_Sequence, not SWJ_Pins. SWCLK is always push-pull: whichever net
    /// carries it, this driver is the only one on the plug SBU lines (hw_b153a.md §8, no other SBU-mode module
    /// runs at the same time). SWDIO's direction is left as it is.
    fn engage(&mut self) -> Bus {
        if self.use_shifter() {
            self.engage_shifter()
        } else {
            if self.shifted {
                self.release_shifter();
            }
            self.engage_direct()
        }
    }

    /// PA1/PA0 straight on the SBU lines, shifter off. SWDIO gets the weak pull-up it keeps for the session, so
    /// the line idles high while released (turnaround), matching the bus convention (the board's own pull-ups
    /// are switched, off by default, see `SBU_PU_EN` in the module doc).
    fn engage_direct(&self) -> Bus {
        let (clk_pin, dio_pin) = if self.swapped() { (PIN_B, PIN_A) } else { (PIN_A, PIN_B) };
        let (c, d) = (clk_pin as usize, dio_pin as usize);
        let gpio = pac::GPIOA;
        cortex_m::interrupt::free(|_| {
            gpio.bsrr().write(|w| w.set_br(c, true));
            gpio.otyper().modify(|w| {
                w.set_ot(c, vals::Ot::PUSH_PULL);
                w.set_ot(d, vals::Ot::PUSH_PULL);
            });
            gpio.ospeedr().modify(|w| {
                w.set_ospeedr(c, vals::Ospeedr::MEDIUM_SPEED);
                w.set_ospeedr(d, vals::Ospeedr::MEDIUM_SPEED);
            });
            gpio.pupdr().modify(|w| {
                w.set_pupdr(c, vals::Pupdr::FLOATING);
                w.set_pupdr(d, vals::Pupdr::PULL_UP);
            });
            gpio.moder().modify(|w| w.set_moder(c, vals::Moder::OUTPUT));
        });
        Bus {
            port: gpio,
            clk: 1 << clk_pin,
            dio: 1 << dio_pin,
            dio_pin,
        }
    }

    /// The shifter side (FW-21, see "SPI-driven SWD" above). On first use: PA0/PA1 Hi-Z, SPI2 on, the B pins set
    /// up with SCK and SWDIO driven low, then OE high, so the shifter only ever passes driven levels to the plug.
    fn engage_shifter(&mut self) -> Bus {
        let gpio = pac::GPIOB;
        if !self.shifted {
            self.a.set_as_input(Pull::None);
            self.b.set_as_input(Pull::None);
            let spi = pac::SPI2;
            spi.cr1().write(|w| {
                w.set_mstr(spi_vals::Mstr::MASTER);
                w.set_ssm(true);
                w.set_ssi(true);
                w.set_lsbfirst(spi_vals::Lsbfirst::LSBFIRST);
                w.set_br(spi_vals::Br::from_bits(self.spi_br.unwrap_or(7)));
            });
            spi.cr2().write(|w| {
                w.set_ds(spi_vals::Ds::BITS8);
                w.set_frxth(spi_vals::Frxth::QUARTER);
            });
            spi.cr1().modify(|w| w.set_spe(true));
            cortex_m::interrupt::free(|_| {
                gpio.bsrr().write(|w| {
                    w.set_br(PB_SCK, true);
                    w.set_br(PB_MOSI, true);
                });
                gpio.otyper().modify(|w| {
                    w.set_ot(PB_SCK, vals::Ot::PUSH_PULL);
                    w.set_ot(PB_MOSI, vals::Ot::PUSH_PULL);
                });
                gpio.ospeedr().modify(|w| {
                    w.set_ospeedr(PB_SCK, vals::Ospeedr::HIGH_SPEED);
                    w.set_ospeedr(PB_MOSI, vals::Ospeedr::HIGH_SPEED);
                });
                // the shifter's own 10K pull-ups hold the lines while OE is high
                gpio.pupdr().modify(|w| {
                    w.set_pupdr(PB_SCK, vals::Pupdr::FLOATING);
                    w.set_pupdr(PB_MOSI, vals::Pupdr::FLOATING);
                    w.set_pupdr(PB_MISO, vals::Pupdr::FLOATING);
                });
                gpio.afr(PB_SCK / 8).modify(|w| w.set_afr(PB_SCK % 8, 1));
                gpio.afr(PB_MOSI / 8).modify(|w| w.set_afr(PB_MOSI % 8, 0));
                gpio.afr(PB_MISO / 8).modify(|w| w.set_afr(PB_MISO % 8, 4));
                gpio.moder().modify(|w| {
                    w.set_moder(PB_SCK, vals::Moder::OUTPUT);
                    w.set_moder(PB_MOSI, vals::Moder::OUTPUT);
                    w.set_moder(PB_MISO, vals::Moder::ALTERNATE);
                });
            });
            self.shifter.oe.set_high();
            self.shifted = true;
        } else {
            cortex_m::interrupt::free(|_| {
                gpio.bsrr().write(|w| w.set_br(PB_SCK, true));
                gpio.moder().modify(|w| w.set_moder(PB_SCK, vals::Moder::OUTPUT));
            });
        }
        Bus {
            port: gpio,
            clk: 1 << PB_SCK,
            dio: 1 << PB_MOSI,
            dio_pin: PB_MOSI as u32,
        }
    }

    /// Shifter side off. OE first: the shifter cuts both sides (and its pull-ups) before the B pins let go, so
    /// the plug never sees SWCLK pulled up while SPI2 stops. Idempotent; harmless before SPI2 was ever used.
    fn release_shifter(&mut self) {
        self.shifter.oe.set_low();
        self.shifter.sck.set_as_input(Pull::None);
        self.shifter.mosi.set_as_input(Pull::None);
        self.shifter.miso.set_as_input(Pull::None);
        pac::SPI2.cr1().modify(|w| w.set_spe(false));
        self.shifted = false;
    }

    fn write_bits(&mut self, nbits: usize, data: &[u8]) {
        let bus = self.engage();
        bus.dio_drive(data.first().is_some_and(|b| b & 1 != 0));
        for (i, chunk) in data.chunks(4).enumerate() {
            let n = nbits.saturating_sub(i * 32).min(32) as u32;
            if n == 0 {
                break;
            }
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            out_bits(bus, bus.dio, self.timing.out, u32::from_le_bytes(word), n);
        }
    }

    fn read_bits(&mut self, nbits: usize, data: &mut [u8]) {
        let bus = self.engage();
        bus.dio_release();
        let len = nbits.div_ceil(8).min(data.len());
        for (i, chunk) in data[..len].chunks_mut(4).enumerate() {
            let n = nbits.saturating_sub(i * 32).min(32) as u32;
            let word = in_bits(bus, self.timing.inp, n).to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }

    /// One ARM ADIv5 SWD register transaction: 8-bit request, turnaround, 3-bit ack, then (on ack OK) a 32-bit
    /// data phase with parity, in the direction given by `rnw`. On a read, returns the data; `write_value` is
    /// ignored. On a write, `write_value` is sent; the return value is unused.
    fn transaction(&mut self, apndp: APnDP, rnw: RnW, a: DPRegister, write_value: u32) -> swd::Result<u32> {
        let bus = self.engage();
        if self.shifted && self.spi_br.is_some() {
            return self.spi_transaction(bus, apndp, rnw, a, write_value);
        }
        let Timing { out, inp, .. } = self.timing;
        let request = swd::make_request(apndp, rnw, a);
        // A request starts with its start bit, 1.
        bus.dio_drive(true);
        out_bits(bus, bus.dio, out, request as u32, 8);

        // Turnaround: host releases SWDIO so the target can drive the ack.
        bus.dio_release();
        out_bits(bus, 0, out, 0, 1);

        let ack = in_bits(bus, inp, 3) as u8;
        let ack_result = swd::Ack::try_ok(ack);

        // Exactly one more turnaround happens in this transaction, at the one point where the direction
        // changes again: back to the target->host direction for a read's data phase (none needed, same
        // direction as the ack just read), or back to host->target before a write's data phase or before the
        // idle cycles below.
        let result = match (ack_result, rnw) {
            (Ok(()), RnW::R) => {
                let data = in_bits(bus, inp, 32);
                let parity = in_bits(bus, inp, 1) != 0;
                out_bits(bus, 0, out, 0, 1);
                if parity != (data.count_ones() % 2 == 1) {
                    Err(swd::Error::BadParity)
                } else {
                    Ok(data)
                }
            }
            (Ok(()), RnW::W) => {
                out_bits(bus, 0, out, 0, 1);
                bus.dio_drive(write_value & 1 != 0);
                out_bits(bus, bus.dio, out, write_value, 32);
                out_bits(bus, bus.dio, out, write_value.count_ones() & 1, 1);
                Ok(0)
            }
            (Err(e), _) => {
                // No data phase on WAIT/FAULT.
                out_bits(bus, 0, out, 0, 1);
                Err(e)
            }
        };

        bus.dio_drive(false);
        out_bits(bus, bus.dio, out, 0, IDLE_CYCLES);
        result
    }

    /// [`Self::transaction`] through SPI2 (FW-21): the frame cut and who drives when are in "SPI-driven SWD" above.
    /// `bus` is the shifter side, SCK a GPIO output (from [`Self::engage`]).
    fn spi_transaction(
        &mut self,
        bus: Bus,
        apndp: APnDP,
        rnw: RnW,
        a: DPRegister,
        write_value: u32,
    ) -> swd::Result<u32> {
        shifter_modes(true, true);
        spi_frame(8, swd::make_request(apndp, rnw, a) as u32)?;
        shifter_modes(true, false);
        // turnaround (junk: nobody drives yet), then the three ack bits
        let ack = ((spi_frame(4, 0)? >> 1) & 0b111) as u8;
        match (swd::Ack::try_ok(ack), rnw) {
            (Ok(()), RnW::R) => {
                let low = spi_frame(16, 0)?;
                let mid = spi_frame(14, 0)?;
                // the last two data bits, parity, turnaround (the target lets go)
                let tail = spi_frame(4, 0)?;
                shifter_modes(true, true);
                spi_frame(IDLE_CYCLES, 0)?;
                let data = low | (mid << 16) | ((tail & 0b11) << 30);
                let parity = (tail >> 2) & 1 != 0;
                if parity != (data.count_ones() % 2 == 1) {
                    Err(swd::Error::BadParity)
                } else {
                    Ok(data)
                }
            }
            (Ok(()), RnW::W) => {
                // MOSI drives 0 from here, the level of the OK ack's last bit the target may still be driving
                shifter_modes(true, true);
                let parity = write_value.count_ones() & 1;
                spi_frame(16, (write_value << 1) & 0xFFFF)?;
                spi_frame(16, (write_value >> 15) & 0xFFFF)?;
                spi_frame(2 + IDLE_CYCLES, (write_value >> 31) | (parity << 1))?;
                Ok(0)
            }
            (Err(e), _) => {
                // No data phase. Turnaround and idle bit-banged: released for the turnaround, then driven low.
                shifter_modes(false, false);
                out_bits(bus, 0, self.timing.out, 0, 1);
                bus.dio_drive(false);
                out_bits(bus, bus.dio, self.timing.out, 0, IDLE_CYCLES);
                Err(e)
            }
        }
    }
}

impl swj::Dependencies<Self, Self> for SwdPins {
    fn high_impedance_mode(&mut self) {
        self.release_shifter();
        self.a.set_as_input(Pull::None);
        self.b.set_as_input(Pull::None);
    }

    fn process_swj_pins(&mut self, output: swj::Pins, mask: swj::Pins, wait_us: u32) -> swj::Pins {
        // On the shifter side the pins only reach the plug once it is engaged (OE high).
        if self.use_shifter() {
            self.engage();
        }
        // No NRESET/NTRST/TDI/TDO wires on this SBU-only SWD path (hw_b153a.md §8): only SWCLK/SWDIO are real.
        // Level first, then the mode: no glitch to the old output data. Not timing-critical, so through `Flex`.
        if mask.contains(swj::Pins::SWCLK) {
            self.clk().set_level(Level::from(output.contains(swj::Pins::SWCLK)));
            self.clk().set_as_output(Speed::Medium);
        }
        if mask.contains(swj::Pins::SWDIO) {
            self.dio().set_level(Level::from(output.contains(swj::Pins::SWDIO)));
            self.dio().set_as_output(Speed::Medium);
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
        self.timing = self.cal.timing(max_frequency);
        self.spi_br = spi_prescaler(PCLK_HZ, max_frequency.min(SPI_MAX_HZ));
        if self.shifted {
            // between transfers, SPI2 idle
            if let Some(br) = self.spi_br {
                pac::SPI2.cr1().modify(|w| w.set_br(spi_vals::Br::from_bits(br)));
            }
        }
        defmt::debug!(
            "swd: SWCLK {} Hz asked, {} Hz bit-bang, {} Hz SPI",
            max_frequency,
            self.timing.real_hz,
            self.spi_br.map_or(0, |br| PCLK_HZ >> (br + 1))
        );
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
    pu_en: Output<'static>,
    /// PD0 (UCPD2_CC1, plug), stopgap CC drive: see the module doc.
    cc1: Flex<'static>,
    /// PD2 (UCPD2_CC2, plug), stopgap CC drive: see the module doc.
    cc2: Flex<'static>,
}

impl SwdHost {
    pub fn new(r: SwdResources) -> Self {
        // V_SBU to 3.3 V before PA0/PA1 ever drive: the shifter's A side sits on the same nets
        let vsbu_sel = Output::new(r.pd3, Level::High, Speed::Low);
        core::mem::forget(vsbu_sel);
        let a = Flex::new(r.pa1);
        let b = Flex::new(r.pa0);
        // Off until SPI mode engages the shifter side (FW-21), and off again with every release.
        let oe = Output::new(r.pb2, Level::Low, Speed::Low);
        embassy_stm32::rcc::enable_and_reset::<SPI2>();
        let shifter = ShifterPins {
            sck: Flex::new(r.pb8),
            mosi: Flex::new(r.pb11),
            miso: Flex::new(r.pb6),
            oe,
            _spi: r.spi2,
        };
        let pins = SwdPins::new(a, b, shifter);
        // Never enabled: the bit-bang driver supplies its own weak pull during SWD turnaround (see `dio_release`),
        // the level shifter its 10K ones in SPI mode.
        let pu_en = Output::new(r.pc15, Level::Low, Speed::Low);
        let mut cc1 = Flex::new(r.pd0);
        let mut cc2 = Flex::new(r.pd2);
        cc1.set_as_input(Pull::None);
        cc2.set_as_input(Pull::None);
        SwdHost {
            dap: Dap::new(pins, NoLeds, Delay, None, "donguru-swd 0.1"),
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
        // `suspend` ends in `high_impedance_mode`: SBU_OE low first, then all SWD pins Hi-Z
        self.dap.suspend();
        self.pu_en.set_low();
        self.cc1.set_as_input(Pull::None);
        self.cc2.set_as_input(Pull::None);
    }
}
