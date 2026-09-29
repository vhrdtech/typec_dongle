#![no_std]
#![no_main]

mod adc;
mod init;
mod led;

use cnt::cnt_if;
use defmt::{error, info, unwrap};
use defmt_rtt as _;
use panic_probe as _;

use assign_resources::assign_resources;
use cortex_m_rt::exception;
use embassy_stm32::{
    Peri, bind_interrupts, dma, peripherals,
    rcc::{
        AHBPrescaler, APBPrescaler, Hsi, Hsi48Config, HsiSysDiv, Pll, PllMul, PllPDiv, PllPreDiv,
        PllRDiv, PllSource, Sysclk,
        mux::{Adcsel, Usbsel},
    },
};

assign_resources! {
    leds: LedResources {
        tim14: TIM14,
        /// Power LED, TIM14_CH1
        pf0: PF0,
        tim15: TIM15,
        /// Status LED, TIM15_CH1N
        pf1: PF1,
    }
    adc: AdcResources {
        adc: ADC1,
        /// Must match the DMA interrupt bound in Irqs
        dma: DMA1_CH1,
        pa2: PA2,
        pa3: PA3,
        pa7: PA7,
        pb0: PB0,
        pb1: PB1,
        pb12: PB12,
    }
}

bind_interrupts!(pub struct Irqs {
    DMA1_CHANNEL1 => dma::InterruptHandler<peripherals::DMA1_CH1>;
});

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    info!("donguru starting...");
    let mut config = embassy_stm32::Config::default();
    // SYSCLK 64 MHz (max): HSI16 / 1 * 8 = 128 MHz VCO, / 2. AHB and APB undivided, also 64 MHz.
    config.rcc.hsi = Some(Hsi {
        sys_div: HsiSysDiv::DIV1,
    });
    config.rcc.pll = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL8,
        // ADC 32 MHz: the fastest clock at which the longest sample time still gives Vtemp its 5 us
        divp: Some(PllPDiv::DIV4),
        divq: None,
        divr: Some(PllRDiv::DIV2),
    });
    config.rcc.sys = Sysclk::PLL1_R;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV1;
    // USB from HSI48, trimmed by CRS from USB SOF
    config.rcc.hsi48 = Some(Hsi48Config {
        sync_from_usb: true,
    });
    config.rcc.mux.adcsel = Adcsel::PLL1_P;
    config.rcc.mux.usbsel = Usbsel::HSI48;
    let p = embassy_stm32::init(config);
    init::init();
    let r = split_resources!(p);

    spawner.spawn(unwrap!(led::led_task(r.leds)));
    led::set_brightness(led::Led::Power, led::Brightness::Bright);
    led::set(led::Led::Power, led::Mode::Breath { period_ms: 3000 });
    led::set(
        led::Led::Status,
        led::Mode::Blink {
            on_ms: 300,
            off_ms: 1200,
        },
    );

    spawner.spawn(unwrap!(adc::adc_task(r.adc, adc::ChannelSet::Base)));

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
