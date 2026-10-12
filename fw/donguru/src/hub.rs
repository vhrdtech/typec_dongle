//! USB hub U301 (USB251xB) bring-up, from `examples/usb_serial.rs` (FW-5). The hub is strapped for SMBus
//! configuration (hw_b153a.md §6): out of reset it waits for its config over I2C and an attach command, and the
//! MCU's own USB sits behind it on port 1, so nothing enumerates on the host until this has run.
//! Port polarity swap for ports 2 and 4 is still open (FW-6).

use crate::HubResources;
use defmt::{info, warn};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::i2c::I2c;
use embassy_time::Timer;

const ADDR: u8 = 0b010_1100;

/// Config registers from address 0, as written in the FW-5 bring-up.
const CONFIG: [u8; 19] = [
    0, 17, 0x24, 0x04, 0x12, 0x25, 0xB3, 0x0B, 0x9B, 0x20, 0x02, 0, 0, 0, 0x01, 0x32, 0x01, 0x32,
    0x32,
];

/// Releases the hub from reset, writes its config and attaches it upstream.
pub async fn bring_up(r: HubResources) {
    let mut nrst = Output::new(r.pc13, Level::Low, Speed::Low);
    // SDA pull-up R301 goes to PA4 on B153A, not to 3.3 V
    let i2c_en = Output::new(r.pa4, Level::High, Speed::Low);
    Timer::after_millis(1).await;
    nrst.set_high();

    let mut i2c = I2c::new_blocking(r.i2c2, r.pb10, r.pb14, Default::default());
    if let Err(e) = i2c.blocking_write(ADDR, &CONFIG) {
        warn!("hub config write: {:?}", e);
    }
    match i2c.blocking_write(ADDR, &[0xFF, 1, 0b0000_0001]) {
        Ok(()) => info!("hub attached"),
        Err(e) => warn!("hub attach: {:?}", e),
    }
    // Kept for the life of the firmware: dropping them would put the pins back to analog (hub back in reset)
    core::mem::forget(nrst);
    core::mem::forget(i2c_en);
}
