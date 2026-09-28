#![no_std]
#![no_main]

mod init;

use defmt::{info, error};
use defmt_rtt as _;
use panic_probe as _;
use cnt::cnt_if; // bkp_cnt_if! for counters in backup registers

use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::Timer;
use cortex_m_rt::exception;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    info!("donguru starting...");
    let mut config = embassy_stm32::Config::default();
    // TODO: configure config.rcc (clock tree) for your board
    let p = embassy_stm32::init(config);
    init::init();

    let mut led = Output::new(p.PF1, Level::Low, Speed::Low);

    info!("init done");
    loop {
        led.toggle();
        cnt_if!(true, led_toggles: u32 += 1);
        Timer::after_millis(1000).await;
    }
}

#[exception]
unsafe fn DefaultHandler(irqn: i16) {
    cnt_if!(true, unhandled_exceptions: u32 += 1);
    cnt::bkp_cnt_if!(true, unhandled_exceptions_total: u32 += 1);
    error!("unhandled exception, IRQn = {}", irqn);
}

#[exception]
unsafe fn HardFault(ef: &cortex_m_rt::ExceptionFrame) -> ! {
    cnt::bkp_cnt_if!(true, hard_faults: u32 += 1);
    error!("HardFault {}", defmt::Debug2Format(ef));
    // TODO: consider cortex_m::peripheral::SCB::sys_reset() in production
    loop {}
}
