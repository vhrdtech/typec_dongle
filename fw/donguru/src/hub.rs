//! USB hub U301 (USB251xB) bring-up and brown-out recovery (FW-18), from `examples/usb_serial.rs` (FW-5). The hub
//! is strapped for SMBus configuration (hw_b153a.md §6): out of reset it waits for its config over I2C and an
//! attach command, and the MCU's own USB sits behind it on hub port 1, so nothing enumerates on the host until
//! this has run. Config and attach go through the `usb251xb` crate (typed registers, port swap, VID/PID
//! readback, FW-6) instead of the raw bytes written in the FW-5 bring-up.
//!
//! Two board variants (Main, 2-port USB2512B; Plus, 4-port USB2514B, P2534 #12, not decided/built yet) share this
//! bring-up. Reading which one is fitted from the hub's own power-on-default PID register (HUB-5) does not work
//! on this hub's SMBus-slave strap — [`Hub::read_power_on_defaults`]'s doc has the detail — so the signal used
//! instead is a board strap on PA10 (SBU_TO_USB, U901's S): [`read_board_variant_strap`], R902 fitted only on
//! Plus (hw_b153a.md §9/§11). [`bring_up`] reads it once, before anything else drives PA10, and keeps the
//! result in `Hub::variant` for every [`configure`](Hub::configure)/[`recover`](Hub::recover) afterward; it is
//! also kept in [`BOARD_VARIANT`] and exposed read-only over WireWeaver (`device.variant`,
//! FW-11 hw_info).
//!
//! The hub and the MCU share `+3V3_DCDC` (hw_b153a.md §3). A hard turn-on load on the header's switched VBUS
//! (Q102, FW-14) sags that rail enough to reset the hub but not the MCU (P2534#12oct-1819): the hub comes back
//! powered but unconfigured and un-attached, and only redoing this bring-up gets it back. [`watchdog_task`]
//! notices the loss two ways: `src/ww/mod.rs` calls [`request_recovery`] when the composite USB device (on hub port
//! 1) drops out of the configured state after having reached it once — the hub's own reset bounces that port
//! along with the rest of the chip, so this fires within a USB frame or two of the brown-out — and a periodic
//! I2C probe backstops it for a brown-out that, for some reason, does not bounce the MCU's own port.

use core::sync::atomic::{AtomicU8, Ordering};

use crate::HubResources;
use defmt::{info, warn};
use donguru_api::BoardVariant;
use donguru_board::hub::{board_variant_from_strap, hub_config};
use embassy_futures::select::{Either, select};
use embassy_stm32::Peri;
use embassy_stm32::gpio::{Flex, Level, Output, Pull, Speed};
use embassy_stm32::i2c::{I2c, Master};
use embassy_stm32::mode::Blocking;
use embassy_stm32::pac;
use embassy_stm32::peripherals::{PA10, PC13};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use usb251xb::Usb251xb;
use usb251xb::register::{REG_CONFIG_BYTE_1, REG_VENDOR_ID_LSB};

/// How long to wait after releasing the hub from reset before talking to it over I2C. 0: tried 0/5/100/1000 ms
/// on a B153A (14 Oct 2026) and all four read [`Hub::read_power_on_defaults`] back as all-zero, so there is no
/// settle time that makes a difference here — see that function's doc for what the all-zero read means. The
/// config write itself (register block_write) has always landed with no delay at all, pre- and post-HUB-5.
const HUB_RESET_RELEASE_SETTLE_MS: u64 = 0;

/// How long to wait after enabling PA10's pull-up before sampling the board-variant strap
/// ([`read_board_variant_strap`]): 1 ms, the first value tried, generous for a plain resistor divider (R902 vs
/// the MCU's own pull-up) with nothing deliberately filtering it.
const BOARD_VARIANT_STRAP_SETTLE_MS: u64 = 1;

/// Reads the board-variant strap on PA10 (SBU_TO_USB, U901's S, hw_b153a.md §9/§11): R902, a 10K pull-down to
/// GND, is fitted only on Plus, so the net reads low there against the MCU's own ~40k pull-up winning on Main,
/// where nothing else pulls on it. [`board_variant_from_strap`] does that mapping; this function
/// only does the GPIO/register side of reading it.
///
/// PA10 doubles as UCPD1's DBCC2 dead-battery pin, which otherwise holds an internal pull-down on it (active
/// out of any reset, independent of whether firmware ever touches UCPD1) that would read as Plus on both
/// variants — `SYSCFG.cfgr1().ucpd1_strobe` has to be set first to release it. Leaves PA10 driven low on
/// return, its normal state (SBU routed to the MCU) until a future SBU mode switch (FW-7/FW-8) changes it.
async fn read_board_variant_strap(pa10: Peri<'static, PA10>) -> BoardVariant {
    pac::SYSCFG.cfgr1().modify(|w| w.set_ucpd1_strobe(true));
    let mut pa10 = Flex::new(pa10);
    pa10.set_as_input(Pull::Up);
    Timer::after_millis(BOARD_VARIANT_STRAP_SETTLE_MS).await;
    let high = pa10.is_high();
    let variant = board_variant_from_strap(high);
    info!(
        "board variant strap (PA10): {} -> {}",
        if high { "high" } else { "low" },
        if high { "Main" } else { "Plus" }
    );
    pa10.set_as_output(Speed::Low);
    pa10.set_low();
    // Kept for the life of the firmware: dropping it would put PA10 back to analog.
    core::mem::forget(pa10);
    variant
}

/// Board variant [`bring_up`] reads from the PA10 strap (module doc), shared with `src/ww/device.rs` for the
/// read-only `device.variant` WireWeaver property (FW-11 hw_info). A plain atomic rather than a
/// [`Signal`]/[`embassy_sync::mutex::Mutex`]: readers only ever want the latest value, never to wait for a
/// change. Holds [`VARIANT_UNKNOWN`], [`VARIANT_MAIN`] or [`VARIANT_PLUS`] ([`BoardVariant`] has a field in its
/// future-version variant, so no `as u8`).
static BOARD_VARIANT: AtomicU8 = AtomicU8::new(VARIANT_UNKNOWN);
const VARIANT_UNKNOWN: u8 = 0;
const VARIANT_MAIN: u8 = 1;
const VARIANT_PLUS: u8 = 2;

fn set_board_variant(variant: BoardVariant) {
    let v = match variant {
        BoardVariant::Main => VARIANT_MAIN,
        BoardVariant::Plus => VARIANT_PLUS,
        BoardVariant::Unknown | BoardVariant::Other(_) => VARIANT_UNKNOWN,
    };
    BOARD_VARIANT.store(v, Ordering::Relaxed);
}

/// Board variant [`bring_up`] read from the PA10 strap, [`BoardVariant::Unknown`] before it has run.
pub fn board_variant() -> BoardVariant {
    match BOARD_VARIANT.load(Ordering::Relaxed) {
        VARIANT_MAIN => BoardVariant::Main,
        VARIANT_PLUS => BoardVariant::Plus,
        _ => BoardVariant::Unknown,
    }
}

/// How often [`watchdog_task`] probes the hub over I2C when nothing has told it to recover sooner.
const POLL_INTERVAL_MS: u64 = 200;

static RECOVERY_REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Tells [`watchdog_task`] the hub needs its bring-up redone, without waiting for the next I2C poll. Called from
/// `src/ww/mod.rs`'s USB handler when the composite device loses the configured state after having reached it, which
/// happens whenever the hub itself gets reset (it carries the MCU's own USB on port 1).
pub fn request_recovery() {
    RECOVERY_REQUEST.signal(());
}

/// The hub's RESET_N on PC13 (B153A rework, hw_b153a.md §6; rev B's schematic has it there too).
///
/// Rev A (feature `rev_a`): R304 pulls it low, so every MCU reset would hold the hub in reset, and the ROM
/// bootloader (FW-12) would sit on a dead USB path. It is driven from RTC_OUT1 instead of the GPIO: RTC_CR (in the
/// backup domain) keeps the level through a system reset, the hub stays attached through the reset into the ROM and
/// the reset after the ROM's leave (`donguru_board::boot::rtc_cr_hub`, values tested on B153A 21 Oct 2026). A power
/// cycle resets the backup domain: PC13 back to the pull-down, as before. Rev B: a pull-up instead of R304
/// (change_b153b.md), the GPIO drives it.
struct ResetPin {
    #[cfg(not(feature = "rev_a"))]
    pin: Output<'static>,
}

impl ResetPin {
    /// Holds the hub in reset.
    fn new(pc13: Peri<'static, PC13>) -> Self {
        #[cfg(feature = "rev_a")]
        {
            // RTC_OUT1 overrides the GPIO while RTC_CR.OSEL selects it: the pin stays in its reset state
            let _ = pc13;
            let mut pin = ResetPin {};
            pin.set(false);
            pin
        }
        #[cfg(not(feature = "rev_a"))]
        ResetPin {
            pin: Output::new(pc13, Level::Low, Speed::Low),
        }
    }

    /// Low (hub in reset) or high (`run`).
    fn set(&mut self, run: bool) {
        #[cfg(feature = "rev_a")]
        {
            // RTC and backup domain clocked and writable since init::init
            let rtc = pac::RTC;
            rtc.wpr().write_value(pac::rtc::regs::Wpr(0xCA));
            rtc.wpr().write_value(pac::rtc::regs::Wpr(0x53));
            rtc.cr()
                .modify(|w| w.0 = donguru_board::boot::rtc_cr_hub(w.0, run));
            // any wrong key locks again
            rtc.wpr().write_value(pac::rtc::regs::Wpr(0xFF));
        }
        #[cfg(not(feature = "rev_a"))]
        self.pin
            .set_level(if run { Level::High } else { Level::Low });
    }
}

/// Owns the hub's reset pin and I2C bus for the life of the firmware, so [`watchdog_task`] can redo the
/// bring-up without re-touching PA4 (the SDA pull-up supply, driven high for good by [`bring_up`]). The bus
/// itself stays a plain [`I2c`]: [`dev`](Hub::dev) wraps `&mut` it in a fresh [`Usb251xb`] per call instead of
/// the driver owning it outright.
pub struct Hub {
    nrst: ResetPin,
    i2c: I2c<'static, Blocking, Master>,
    /// Read once from the PA10 strap in [`bring_up`], reused by every [`configure`](Hub::configure) /
    /// [`recover`](Hub::recover): the fitted hub does not change at runtime, so there is nothing to re-read.
    variant: BoardVariant,
}

impl Hub {
    fn dev(&mut self) -> Usb251xb<&mut I2c<'static, Blocking, Master>> {
        Usb251xb::new(&mut self.i2c)
    }

    /// Reads VID/PID/DID (registers 0x00-0x05) and CFG1-3 (0x06-0x08) before anything has been written to the
    /// hub, meant to catch the values straight out of reset (HUB-5). In practice (B153A, 14 Oct 2026, every
    /// delay in [`HUB_RESET_RELEASE_SETTLE_MS`] from 0 to 1000 ms) this reads all-zero every time, unlike
    /// [`log_vid_pid_after_config`](Hub::log_vid_pid_after_config)'s readback right after the config write,
    /// which correctly echoes what was just written. So in this hub's SMBus-slave strap (hw_b153a.md §6),
    /// these registers hold whatever was last written over SMBus, not a factory/silicon default — there is no
    /// PID signal here to read before configuring, matching hw_b153a.md §9's existing caution ("the hub ID
    /// read over SMBus is not reliable, defaults read 0 on rev A"). Board variant instead comes from
    /// [`read_board_variant_strap`]; this read stays only as a logged diagnostic (so the all-zero result is
    /// visible rather than silently assumed), not as a detection signal.
    fn read_power_on_defaults(&mut self) {
        let mut id_buf = [0u8; 1 + 6];
        let mut cfg_buf = [0u8; 1 + 3];
        let id = self.dev().read_register(REG_VENDOR_ID_LSB, &mut id_buf);
        let cfg = self.dev().read_register(REG_CONFIG_BYTE_1, &mut cfg_buf);
        match (id, cfg) {
            (Ok(_), Ok(_)) => info!(
                "hub power-on defaults: VID {:x} PID {:x} DID {:x} CFG {:x} {:x} {:x}",
                u16::from_le_bytes([id_buf[1], id_buf[2]]),
                u16::from_le_bytes([id_buf[3], id_buf[4]]),
                u16::from_le_bytes([id_buf[5], id_buf[6]]),
                cfg_buf[1],
                cfg_buf[2],
                cfg_buf[3]
            ),
            (id, cfg) => warn!(
                "hub power-on defaults: VID/PID/DID {:?}, CFG1-3 {:?}",
                id, cfg
            ),
        }
    }

    /// Reads back VID/PID right after the config write, labelled accordingly: unlike
    /// [`read_power_on_defaults`](Hub::read_power_on_defaults), this only ever echoes the value just written
    /// (HUB-5's original readback, kept as a sanity check that the write landed).
    fn log_vid_pid_after_config(&mut self) {
        let mut buf = [0u8; 1 + 4];
        match self.dev().read_register(REG_VENDOR_ID_LSB, &mut buf) {
            Ok(_) => info!(
                "hub VID/PID readback (after config write): {:x}/{:x}",
                u16::from_le_bytes([buf[1], buf[2]]),
                u16::from_le_bytes([buf[3], buf[4]])
            ),
            Err(e) => warn!("hub VID/PID readback (after config write): {:?}", e),
        }
    }

    /// Configures the hub for `self.variant` and attaches it upstream.
    fn configure(&mut self) {
        self.read_power_on_defaults();
        let variant = self.variant;
        if let Err(e) = self.dev().configure(&hub_config(variant)) {
            warn!("hub config: {:?}", e);
        }
        self.log_vid_pid_after_config();
        match self.dev().attach() {
            Ok(()) => info!("hub attached"),
            Err(e) => warn!("hub attach: {:?}", e),
        }
    }

    /// The bring-up sequence again: reset pulse, config, attach. The board variant does not need re-reading
    /// (module doc): `self.variant` carries over from [`bring_up`].
    async fn recover(&mut self, why: &str) {
        warn!("hub: lost ({=str}), redoing bring-up", why);
        self.nrst.set(false);
        Timer::after_millis(1).await;
        self.nrst.set(true);
        Timer::after_millis(HUB_RESET_RELEASE_SETTLE_MS).await;
        self.configure();
    }

    /// Reads VID/PID back to tell a hub that lost its config from one that is just busy (FW-41). A hub that
    /// browned out comes back waiting for its config over SMBus, like out of reset: it answers, and its ID
    /// registers read all-zero ([`read_power_on_defaults`](Hub::read_power_on_defaults)). A configured hub answers
    /// with the IDs written by [`configure`](Hub::configure). While the hub's own microcontroller is busy with the
    /// host (attach, enumeration) it NAKs its address for a while: that is no loss. Counting a NAK as one (the
    /// zero-length write used before) reset the hub once or twice after every boot, and every 0.6 s for good while
    /// the MCU's own device was off the bus (B153A, 19-20 Oct 2026).
    fn probe(&mut self) -> Probe {
        let mut buf = [0u8; 1 + 4];
        match self.dev().read_register(REG_VENDOR_ID_LSB, &mut buf) {
            Ok(_) if buf[1..] == [0; 4] => Probe::Unconfigured,
            Ok(_) => Probe::Configured,
            Err(_) => Probe::Busy,
        }
    }
}

/// What [`Hub::probe`] found.
#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
enum Probe {
    Configured,
    /// ID registers all-zero: the hub reset and waits for its config.
    Unconfigured,
    /// No answer on I2C: the hub is busy with the host, or gone with its supply.
    Busy,
}

/// After an attach the hub reads its ID registers as all-zero for up to ~0.4 s, as if it had lost its config, and
/// NAKs while the host enumerates it (B153A, 20 Oct 2026): [`watchdog_task`] doesn't probe it in this time.
const ATTACH_GRACE: Duration = Duration::from_secs(1);

/// A recovery resets the hub, which drops the MCU's own device out of the configured state some 0.6 s later: a
/// request in this window after an attach is that echo, not a new loss.
const OWN_RESET_ECHO: Duration = Duration::from_secs(2);

/// Consecutive [`Probe::Unconfigured`] reads before [`watchdog_task`] redoes the bring-up: one stray read does not
/// reset a hub the host is using.
const UNCONFIGURED_READS: u8 = 2;

/// Reads the board-variant strap, releases the hub from reset, writes its config and attaches it upstream.
pub async fn bring_up(r: HubResources) -> Hub {
    let variant = read_board_variant_strap(r.pa10).await;
    set_board_variant(variant);

    let mut nrst = ResetPin::new(r.pc13);
    // SDA pull-up R301 goes to PA4 on B153A, not to 3.3 V; rev B has it on 3.3 V and PA4 free
    #[cfg(feature = "rev_a")]
    let i2c_en = Output::new(r.pa4, Level::High, Speed::Low);
    #[cfg(not(feature = "rev_a"))]
    let _ = r.pa4;
    Timer::after_millis(1).await;
    nrst.set(true);
    Timer::after_millis(HUB_RESET_RELEASE_SETTLE_MS).await;

    let i2c = I2c::new_blocking(r.i2c2, r.pb10, r.pb14, Default::default());
    let mut hub = Hub { nrst, i2c, variant };
    hub.configure();
    // Kept for the life of the firmware: dropping it would put PA4 back to analog (hub's SDA pull-up gone)
    #[cfg(feature = "rev_a")]
    core::mem::forget(i2c_en);
    hub
}

/// Watches for the hub having lost its config (module doc) and redoes [`Hub::recover`] when it has. Runs on
/// I2C2, a bus of its own that shares no pins with the SWD-host (`src/swd.rs`, `src/cmsis_dap.rs`): this task
/// cannot contend with the CMSIS-DAP path, and each poll or recovery is a handful of short blocking I2C
/// transactions, not a busy-loop, so it only ever delays other tasks by that long. Blocking I2C (rather than
/// embassy's async I2C2, which would need its own DMA channels and interrupt binding) keeps that true: nothing
/// here contends for a shared executor task while waiting on the bus, so there is nothing async buys.
#[embassy_executor::task]
pub async fn watchdog_task(mut hub: Hub) {
    let mut last = Probe::Configured;
    let mut unconfigured = 0u8;
    let mut attached_at = Instant::now(); // bring_up has just attached it
    loop {
        let why = match select(
            Timer::after_millis(POLL_INTERVAL_MS),
            RECOVERY_REQUEST.wait(),
        )
        .await
        {
            Either::First(()) if attached_at.elapsed() < ATTACH_GRACE => continue,
            Either::First(()) => {
                let probe = hub.probe();
                if probe != last {
                    info!("hub probe: {}", probe);
                    last = probe;
                }
                unconfigured = if probe == Probe::Unconfigured {
                    unconfigured + 1
                } else {
                    0
                };
                if unconfigured < UNCONFIGURED_READS {
                    continue;
                }
                "ID registers read all-zero"
            }
            // our own hub reset bounced the MCU's port as well
            Either::Second(()) if attached_at.elapsed() < OWN_RESET_ECHO => continue,
            Either::Second(()) => "USB device left the configured state",
        };
        hub.recover(why).await;
        unconfigured = 0;
        attached_at = Instant::now();
    }
}
