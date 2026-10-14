//! USB hub U301 (USB251xB) bring-up and brown-out recovery (FW-18), from `examples/usb_serial.rs` (FW-5). The hub
//! is strapped for SMBus configuration (hw_b153a.md §6): out of reset it waits for its config over I2C and an
//! attach command, and the MCU's own USB sits behind it on hub port 1, so nothing enumerates on the host until
//! this has run. Config and attach go through the `usb251xb` crate (typed registers, port swap, VID/PID
//! readback, FW-6) instead of the raw bytes written in the FW-5 bring-up.
//!
//! The hub and the MCU share `+3V3_DCDC` (hw_b153a.md §3). A hard turn-on load on the header's switched VBUS
//! (Q102, FW-14) sags that rail enough to reset the hub but not the MCU (P2534#12oct-1819): the hub comes back
//! powered but unconfigured and un-attached, and only redoing this bring-up gets it back. [`watchdog_task`]
//! notices the loss two ways: `src/ww.rs` calls [`request_recovery`] when the composite USB device (on hub port
//! 1) drops out of the configured state after having reached it once — the hub's own reset bounces that port
//! along with the rest of the chip, so this fires within a USB frame or two of the brown-out — and a periodic
//! I2C probe backstops it for a brown-out that, for some reason, does not bounce the MCU's own port.

use crate::HubResources;
use defmt::{info, warn};
use embassy_futures::select::{Either, select};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::i2c::{I2c, Master};
use embassy_stm32::mode::Blocking;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use usb251xb::register::{ConfigByte3, REG_VENDOR_ID_LSB};
use usb251xb::{Config, CurrentSensing, Milliamps, Port, PowerSwitching, Usb251xb, Variant};

/// Hub chip fitted on this board: Main has the 2-port USB2512B (PID 0x2512); Plus's 4-port USB2514B is a
/// different board variant, not wired up here.
const VARIANT: Variant = Variant::Usb2512b;

/// Config byte 3, register 0x08, verbatim from the FW-5 bring-up (0x02 = bit 1 set). The datasheet marks this
/// bit reserved and the `usb251xb` crate gives it no setter; nobody has found a reason to clear it, so it stays
/// until someone does.
const CONFIG_BYTE_3: ConfigByte3 = ConfigByte3::from_bytes([0x02]);

/// Hub controller's own current draw while self-powered, register units of 2 mA (FW-5 bring-up value; the
/// `usb251xb` crate's own default is the bus-powered value, 100 mA, so this needs setting explicitly).
const HUB_CURRENT_SELF: Milliamps = Milliamps(2);

/// Ports needing their D+/D- polarity swapped: port 2 (plug, U701) and port 4 (SBU USB, U901, Plus only) both
/// cross D+/D- between the hub and the far end (hw_b153a.md §6). Unswapped, a full/high-speed device's D+
/// pull-up shows up on the hub's D-, which the hub reads as low-speed (FW-6: the reMarkable 2 result,
/// `error -32`). Port 4 doesn't exist on the 2-port [`VARIANT`] fitted here; setting its bit is a no-op.
const SWAPPED_PORTS: [Port; 2] = [Port::Port2, Port::Port4];

/// Builds the hub's configuration. VID (0x0424), PID (0x2512 for [`VARIANT`]), DID (0x0BB3), and every register
/// not mentioned here (max power, hub current in bus-powered mode, power-on time, ...) already match the FW-5
/// bring-up as the crate's own datasheet defaults; only [`CONFIG_BYTE_3`], [`HUB_CURRENT_SELF`] and the port
/// swap differ from them and need setting below. Unlike the FW-5 bring-up (which stopped writing registers
/// after power-on time, 0x10), this also writes the registers up to the port swap/map (0xFC) — mostly explicit
/// zeros (no battery charging, no signal boost override) where the FW-5 bring-up relied on the hub's own
/// power-on defaults.
fn hub_config() -> Config {
    let mut config = Config::builder(VARIANT)
        .self_powered(true)
        .mtt(true)
        .high_speed(true)
        .power_switching(PowerSwitching::Individual)
        // OCS_N1..4 are unconnected on this board; the datasheet gives them internal pull-ups (DS00001692,
        // "the over-current sense inputs ... have internal pull-up resistors"), which read as "no overcurrent",
        // so per-port sensing is safe to keep on this self-powered hub (hw_b153a.md §6, HUB-10).
        .current_sensing(CurrentSensing::Individual)
        .hub_current_self(HUB_CURRENT_SELF)
        .port_swap(&SWAPPED_PORTS)
        .into_config();
    config.config3 = CONFIG_BYTE_3;
    config
}

/// How often [`watchdog_task`] probes the hub over I2C when nothing has told it to recover sooner.
const POLL_INTERVAL_MS: u64 = 200;

static RECOVERY_REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Tells [`watchdog_task`] the hub needs its bring-up redone, without waiting for the next I2C poll. Called from
/// `src/ww.rs`'s USB handler when the composite device loses the configured state after having reached it, which
/// happens whenever the hub itself gets reset (it carries the MCU's own USB on port 1).
pub fn request_recovery() {
    RECOVERY_REQUEST.signal(());
}

/// Owns the hub's reset pin and the `usb251xb` driver (which owns the I2C bus) for the life of the firmware, so
/// [`watchdog_task`] can redo the bring-up without re-touching PA4 (the SDA pull-up supply, driven high for good
/// by [`bring_up`]).
pub struct Hub {
    nrst: Output<'static>,
    dev: Usb251xb<I2c<'static, Blocking, Master>>,
}

impl Hub {
    fn configure(&mut self) {
        if let Err(e) = self.dev.configure(&hub_config()) {
            warn!("hub config: {:?}", e);
        }
        self.log_vid_pid();
        match self.dev.attach() {
            Ok(()) => info!("hub attached"),
            Err(e) => warn!("hub attach: {:?}", e),
        }
    }

    /// Reads back VID/PID right after the config write (HUB-5: earlier attempts using a plain register read,
    /// without the SMBus block-read framing the `usb251xb` crate uses, got 0 back regardless of the hub).
    fn log_vid_pid(&mut self) {
        let mut buf = [0u8; 1 + 4];
        match self.dev.read_register(REG_VENDOR_ID_LSB, &mut buf) {
            Ok(_) => info!(
                "hub VID/PID readback: {:x}/{:x}",
                u16::from_le_bytes([buf[1], buf[2]]),
                u16::from_le_bytes([buf[3], buf[4]])
            ),
            Err(e) => warn!("hub VID/PID readback: {:?}", e),
        }
    }

    /// The bring-up sequence again: reset pulse, config, attach.
    async fn recover(&mut self) {
        warn!("hub: lost after brown-out, redoing bring-up");
        self.nrst.set_low();
        Timer::after_millis(1).await;
        self.nrst.set_high();
        Timer::after_millis(1).await;
        self.configure();
    }

    /// Minimal register read (1 byte: just the SMBus block-read count, no data): a cheap presence/health probe.
    /// The hub's address phase NAKs or times out the same way whether the transaction is a read or a write, so
    /// this catches a hub mid brown-out exactly as the bare zero-length write it replaces did — but by reusing
    /// [`Usb251xb::read_register`] instead of reaching around the driver for a raw write, there is only one
    /// I2C path through this module instead of two.
    fn probe_lost(&mut self) -> bool {
        let mut buf = [0u8; 1];
        self.dev.read_register(REG_VENDOR_ID_LSB, &mut buf).is_err()
    }
}

/// Releases the hub from reset, writes its config and attaches it upstream.
pub async fn bring_up(r: HubResources) -> Hub {
    let mut nrst = Output::new(r.pc13, Level::Low, Speed::Low);
    // SDA pull-up R301 goes to PA4 on B153A, not to 3.3 V
    let i2c_en = Output::new(r.pa4, Level::High, Speed::Low);
    Timer::after_millis(1).await;
    nrst.set_high();

    let i2c = I2c::new_blocking(r.i2c2, r.pb10, r.pb14, Default::default());
    let mut hub = Hub {
        nrst,
        dev: Usb251xb::new(i2c),
    };
    hub.configure();
    // Kept for the life of the firmware: dropping it would put PA4 back to analog (hub's SDA pull-up gone)
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
    loop {
        match select(
            Timer::after_millis(POLL_INTERVAL_MS),
            RECOVERY_REQUEST.wait(),
        )
        .await
        {
            Either::First(()) => {
                if hub.probe_lost() {
                    hub.recover().await;
                }
            }
            Either::Second(()) => hub.recover().await,
        }
    }
}
