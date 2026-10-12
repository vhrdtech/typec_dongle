//! USB hub U301 (USB251xB) bring-up and brown-out recovery (FW-18), from `examples/usb_serial.rs` (FW-5). The hub
//! is strapped for SMBus configuration (hw_b153a.md §6): out of reset it waits for its config over I2C and an
//! attach command, and the MCU's own USB sits behind it on hub port 1, so nothing enumerates on the host until
//! this has run.
//!
//! The hub and the MCU share `+3V3_DCDC` (hw_b153a.md §3). A hard turn-on load on the header's switched VBUS
//! (Q102, FW-14) sags that rail enough to reset the hub but not the MCU (P2534#12oct-1819): the hub comes back
//! powered but unconfigured and un-attached, and only redoing this bring-up gets it back. [`watchdog_task`]
//! notices the loss two ways: `src/ww.rs` calls [`request_recovery`] when the composite USB device (on hub port
//! 1) drops out of the configured state after having reached it once — the hub's own reset bounces that port
//! along with the rest of the chip, so this fires within a USB frame or two of the brown-out — and a periodic
//! I2C probe backstops it for a brown-out that, for some reason, does not bounce the MCU's own port.
//!
//! Port polarity swap for ports 2 and 4 is still open (FW-6).

use crate::HubResources;
use defmt::{info, warn};
use embassy_futures::select::{Either, select};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::i2c::{I2c, Master};
use embassy_stm32::mode::Blocking;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;

const ADDR: u8 = 0b010_1100;

/// Config registers from address 0, as written in the FW-5 bring-up.
const CONFIG: [u8; 19] = [
    0, 17, 0x24, 0x04, 0x12, 0x25, 0xB3, 0x0B, 0x9B, 0x20, 0x02, 0, 0, 0, 0x01, 0x32, 0x01, 0x32,
    0x32,
];
const ATTACH: [u8; 3] = [0xFF, 1, 0b0000_0001];

/// How often [`watchdog_task`] probes the hub over I2C when nothing has told it to recover sooner.
const POLL_INTERVAL_MS: u64 = 200;

static RECOVERY_REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Tells [`watchdog_task`] the hub needs its bring-up redone, without waiting for the next I2C poll. Called from
/// `src/ww.rs`'s USB handler when the composite device loses the configured state after having reached it, which
/// happens whenever the hub itself gets reset (it carries the MCU's own USB on port 1).
pub fn request_recovery() {
    RECOVERY_REQUEST.signal(());
}

/// Owns the hub's reset pin and I2C bus for the life of the firmware, so [`watchdog_task`] can redo the bring-up
/// without re-touching PA4 (the SDA pull-up supply, driven high for good by [`bring_up`]).
pub struct Hub {
    nrst: Output<'static>,
    i2c: I2c<'static, Blocking, Master>,
}

impl Hub {
    fn configure(&mut self) {
        if let Err(e) = self.i2c.blocking_write(ADDR, &CONFIG) {
            warn!("hub config write: {:?}", e);
        }
        match self.i2c.blocking_write(ADDR, &ATTACH) {
            Ok(()) => info!("hub attached"),
            Err(e) => warn!("hub attach: {:?}", e),
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

    /// Zero-length write: a cheap presence probe (address byte + ACK/NAK only, no data phase, no register
    /// touched). Deliberately does not try to read anything back: the B153A bring-up notes found the hub's
    /// register read-back unreliable (`hw/b153_typec_serial/report/assy_b153a.md`), and ACK/NAK on the address is
    /// all the detection needs — a hub mid brown-out NAKs or times out here.
    fn probe_lost(&mut self) -> bool {
        self.i2c.blocking_write(ADDR, &[]).is_err()
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
    let mut hub = Hub { nrst, i2c };
    hub.configure();
    // Kept for the life of the firmware: dropping it would put PA4 back to analog (hub's SDA pull-up gone)
    core::mem::forget(i2c_en);
    hub
}

/// Watches for the hub having lost its config (module doc) and redoes [`Hub::recover`] when it has. Runs on
/// I2C2, a bus of its own that shares no pins with the SWD-host (`src/swd.rs`, `src/cmsis_dap.rs`): this task
/// cannot contend with the CMSIS-DAP path, and each poll or recovery is a handful of short blocking I2C
/// transactions, not a busy-loop, so it only ever delays other tasks by that long.
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
