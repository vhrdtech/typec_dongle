#![no_std]
#![no_main]

mod init;
mod led;

use cnt::cnt_if;
use defmt::{error, info, unwrap};
use defmt_rtt as _;
use panic_probe as _;

use cortex_m_rt::exception;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    info!("donguru starting...");
    let mut config = embassy_stm32::Config::default();
    // TODO: configure config.rcc (clock tree) for your board
    let p = embassy_stm32::init(config);
    init::init();

    spawner.spawn(unwrap!(led::led_task(p.TIM14, p.PF0, p.TIM15, p.PF1)));
    led::set_brightness(led::Led::Power, led::Brightness::Bright);
    led::set(led::Led::Power, led::Mode::Breath { period_ms: 3000 });
    led::set(
        led::Led::Status,
        led::Mode::Blink {
            on_ms: 300,
            off_ms: 1200,
        },
    );

    info!("init done");
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
