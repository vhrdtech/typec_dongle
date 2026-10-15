#![no_std]
#![no_main]

mod adc;
mod button;
mod cmsis_dap;
mod hub;
mod init;
mod led;
mod swd;
mod ww;

use cnt::cnt; // cnt::bkp_cnt! for counters in backup registers
use defmt::{error, info, unwrap};
use defmt_rtt as _;
use panic_probe as _;

use assign_resources::assign_resources;
use cortex_m_rt::exception;
use embassy_stm32::{
    Peri, bind_interrupts, dma,
    gpio::{Level, Output, Pull, Speed},
    peripherals,
    rcc::{
        AHBPrescaler, APBPrescaler, Hsi, Hsi48Config, HsiSysDiv, Pll, PllMul, PllPDiv, PllPreDiv,
        PllRDiv, PllSource, Sysclk,
        mux::{Adcsel, Usbsel},
    },
    usb,
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
    usb: UsbResources {
        usb: USB,
        /// USB D+
        pa12: PA12,
        /// USB D-
        pa11: PA11,
    }
    hub: HubResources {
        /// Hub RESET_N (B153A rework, hw_b153a.md §6), high releases the hub
        pc13: PC13,
        /// HUB_I2C_EN: SDA pull-up R301 on B153A, high for I2C to work
        pa4: PA4,
        i2c2: I2C2,
        /// I2C2_SCL
        pb10: PB10,
        /// I2C2_SDA
        pb14: PB14,
        /// SBU_TO_USB (U901 S): board-variant strap (R902, Plus only, hw_b153a.md §9/§11) read before hub
        /// config, driven low (SBU to MCU, its normal state) afterward
        pa10: PA10,
    }
    pwr: PwrResources {
        /// PWR_EN: U601 AOZ1375 enable, receptacle VBUS -> plug VBUS. R601 pull-down, off at reset. Driven only
        /// with the `plug-vbus-on` feature (stopgap until FW-8), left at its reset state otherwise.
        pa6: PA6,
        /// ~PWR_FLT: U601 FLTB, open drain with no external pull-up (hw_b153a.md §3).
        pc14: PC14,
    }
    swd: SwdResources {
        /// SBU1 net, plug side of the level shifter (hw_b153a.md §8): SWCLK in [`donguru_api::Orientation::Normal`],
        /// SWDIO swapped. R211/R212 (the 0 Ω bypass to PB8/PB6) are not fitted on B153A (checked on the bench 12 Oct
        /// 2026), so this is the only MCU pin wired straight to the plug SBU1. PB8 is the SPI mode's, below.
        pa1: PA1,
        /// SBU2 net: SWDIO in Normal, SWCLK swapped. PB6/PB11 (the `_VT` side) are the SPI mode's, below.
        pa0: PA0,
        /// VSBU_SEL: high selects 3.3 V for V_SBU, the level shifter's A side, so PA0/PA1 driving 3.3 V don't
        /// back-feed it (hw_b153a.md §8).
        pd3: PD3,
        /// SBU_OE: TXS0102 level shifter output enable. Low (shifter off) except while SPI-mode SWD is engaged
        /// (FW-21): the bit-bang SWD-host drives the SBU side directly.
        pb2: PB2,
        /// SBU_PU_EN: switched 3K3 pull-ups to V_SBU on SBU1/SBU2. Always driven low: the bit-bang driver uses
        /// its own weak pull during SWD turnaround and none at all when released to Hi-Z.
        pc15: PC15,
        /// PD2_CC1 (UCPD2 CC1, plug): B153A has no DPO2036 (`hw_b153a.md` §0), so this pin reaches the plug
        /// CC1 directly. Stopgap-driven push-pull high while a CMSIS-DAP session is connected, Hi-Z otherwise
        /// (P2534#12oct-1338b, `src/swd.rs`).
        pd0: PD0,
        /// PD2_CC2 (UCPD2 CC2, plug): same story as PD0 above, for CC2.
        pd2: PD2,
        /// SBU1_VT, B side of the level shifter: SPI2_SCK (AF1) = SWCLK in SPI mode (FW-21, `src/swd.rs`).
        pb8: PB8,
        /// SBU2_VT: SPI2_MOSI (AF0) = SWDIO out in SPI mode.
        pb11: PB11,
        /// SBU2_VT: SPI2_MISO (AF4) = SWDIO in in SPI mode.
        pb6: PB6,
        spi2: SPI2,
    }
}

bind_interrupts!(pub struct Irqs {
    DMA1_CHANNEL1 => dma::InterruptHandler<peripherals::DMA1_CH1>;
    USB_UCPD1_2 => usb::InterruptHandler<peripherals::USB>;
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
    // Flash prefetch: embassy-stm32 0.6 sets the 2 wait states 64 MHz needs on the G0 but leaves PRFTEN off (its
    // L-series code turns it on), so straight-line fetches stall: code runs 8-16% slower (byte loop, CRC) and
    // WireWeaver over USB 317 -> 451 kB/s device -> host (FW-24). ICEN is on from reset, set again for clarity.
    embassy_stm32::pac::FLASH.acr().modify(|w| {
        w.set_prften(true);
        w.set_icen(true);
    });
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

    // Stopgap (P2534#12oct-1338b, until FW-8): plug VBUS on from boot so one cable powers the DUT and carries
    // SWD. Off in normal builds: on the P2625 power-cut bench plug VBUS must stay off (the DUT runs from Q102).
    if cfg!(feature = "plug-vbus-on") {
        let fault = embassy_stm32::gpio::Input::new(r.pwr.pc14, Pull::Up);
        let pwr_en = Output::new(r.pwr.pa6, Level::High, Speed::Low);
        info!("plug VBUS on (plug-vbus-on), fault: {}", fault.is_low());
        // Kept for the life of the firmware: dropping either would put its pin back to analog.
        core::mem::forget(pwr_en);
        core::mem::forget(fault);
    }

    spawner.spawn(unwrap!(adc::adc_task(r.adc, adc::ChannelSet::Base)));
    spawner.spawn(unwrap!(button::button_task()));

    // The MCU's USB is on hub port 1: nothing enumerates until the hub is configured and attached. The watchdog
    // (FW-18) redoes this after a brown-out resets the hub without resetting the MCU (P2534#12oct-1819).
    let hub = hub::bring_up(r.hub).await;
    spawner.spawn(unwrap!(hub::watchdog_task(hub)));

    // Composite USB device (FS, HSI48 clock above): WireWeaver and CMSIS-DAP v2 (FW-15), see src/ww/mod.rs.
    let usb_driver = usb::Driver::new(r.usb.usb, Irqs, r.usb.pa12, r.usb.pa11);
    let swd_host = swd::SwdHost::new(r.swd);
    ww::start(spawner, usb_driver, swd_host);

    info!("init done");
}

#[exception]
unsafe fn DefaultHandler(irqn: i16) {
    cnt!(unhandled_exceptions: u32, warn);
    cnt::bkp_cnt!(unhandled_exceptions_total: u32, warn);
    error!("unhandled exception, IRQn = {}", irqn);
}

#[exception]
unsafe fn HardFault(ef: &cortex_m_rt::ExceptionFrame) -> ! {
    cnt::bkp_cnt!(hard_faults: u32, error);
    error!("HardFault {}", defmt::Debug2Format(ef));
    // TODO: consider cortex_m::peripheral::SCB::sys_reset() in production
    loop {}
}
