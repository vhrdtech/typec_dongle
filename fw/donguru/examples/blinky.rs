#![no_std]
#![no_main]

use cnt::cnt_if;
use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::Timer;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    info!("donguru starting...");
    let config = embassy_stm32::Config::default();
    let p = embassy_stm32::init(config);

    let mut led = Output::new(p.PF1, Level::Low, Speed::Low);

    info!("init done");
    loop {
        led.toggle();
        info!("led toggle");
        cnt_if!(true, led_toggles: u32 += 1);
        Timer::after_millis(1000).await;
    }
}
