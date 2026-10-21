#![no_std]
#![no_main]

mod adc;
mod boot;
mod button;
mod cmsis_dap;
mod cpu;
mod hub;
mod init;
mod last_reset;
mod led;
mod log;
mod power;
mod settings;
mod swd;
mod sync;
mod uart;
mod ww;

use cnt::cnt; // cnt::bkp_cnt! for counters in backup registers
use defmt::{error, info, unwrap};

use assign_resources::assign_resources;
use cortex_m_rt::exception;
use embassy_stm32::{
    Peri, bind_interrupts, dma, peripherals,
    rcc::{
        AHBPrescaler, APBPrescaler, LsConfig, Hsi, Hsi48Config, HsiSysDiv, Pll, PllMul, PllPDiv, PllPreDiv,
        PllRDiv, PllSource, Sysclk,
        mux::{Adcsel, Usbsel},
    },
    usart, usb,
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
        /// HUB_I2C_EN: SDA pull-up R301 on B153A, high for I2C to work (feature `rev_a`); free on rev B, R301 to 3.3 V
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
        /// PWR_EN: U601 AOZ1375 enable, receptacle VBUS -> plug VBUS. R601 pull-down, off at reset. Driven by
        /// `plug.vbus_mode` through the guard (`src/power.rs`, API-11).
        pa6: PA6,
        /// ~PWR_FLT: U601 FLTB, open drain with no external pull-up (hw_b153a.md §3): the MCU's pull-up.
        pc14: PC14,
        /// 5V_EN: header J101 pin 10 VBUS_SW through Q101/Q102, R109 pull-down (hw_b153a.md §10, FW-14).
        pd1: PD1,
        /// DATA_CUT: U701 S, R702 pull-down, high disconnects the DUT's USB data (hw_b153a.md §7).
        pa9: PA9,
        /// RELAY_EN: U101 photo-MOS LED through R106, high closes RL1-RL2.
        pa5: PA5,
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
    uart: UartResources {
        usart5: USART5,
        /// IO7, header pin 1: USART5_TX (`src/uart.rs`)
        pb3: PB3,
        /// IO8, header pin 2: USART5_RX
        pb4: PB4,
        tx_dma: DMA1_CH2,
        /// Must match the DMA interrupt bound in Irqs
        rx_dma: DMA1_CH3,
    }
    settings: SettingsResources {
        /// The ww_settings store in the last 4 pages of bank 2 (`SETTINGS` in memory.x, API-19).
        flash: FLASH,
    }
}

bind_interrupts!(pub struct Irqs {
    DMA1_CHANNEL1 => dma::InterruptHandler<peripherals::DMA1_CH1>;
    DMA1_CHANNEL2_3 => dma::InterruptHandler<peripherals::DMA1_CH2>, dma::InterruptHandler<peripherals::DMA1_CH3>;
    USART3_4_5_6_LPUART1 => usart::InterruptHandler<peripherals::USART5>;
    USB_UCPD1_2 => usb::InterruptHandler<peripherals::USB>;
});

#[cortex_m_rt::entry]
fn main() -> ! {
    log::init_rtt(); // before the first log line, so RTT has the whole boot log (src/log.rs)
    // a ROM bootloader request or the ROM's leave: before clocks and embassy, from the reset state (FW-12)
    boot::early();
    // embassy's thread-mode executor, run by hand to count the CPU's idle time (FW-43)
    cpu::run(|spawner| spawner.spawn(unwrap!(boot(spawner))))
}

#[embassy_executor::task]
async fn boot(spawner: embassy_executor::Spawner) {
    info!("donguru starting...");
    let last_reset = last_reset::boot(); // reset cause and crash record (src/last_reset.rs), logged
    let mut config = embassy_stm32::Config::default();
    // SYSCLK 64 MHz (max): HSI16 / 1 * 8 = 128 MHz VCO, / 2. AHB and APB undivided, also 64 MHz.
    config.rcc.hsi = Some(Hsi {
        sys_div: HsiSysDiv::Div1,
    });
    config.rcc.pll = Some(Pll {
        source: PllSource::Hsi,
        prediv: PllPreDiv::Div1,
        mul: PllMul::Mul8,
        // ADC 32 MHz: the fastest clock at which the longest sample time still gives Vtemp its 5 us
        divp: Some(PllPDiv::Div4),
        divq: None,
        divr: Some(PllRDiv::Div2),
    });
    config.rcc.sys = Sysclk::Pll1R;
    config.rcc.ahb_pre = AHBPrescaler::Div1;
    config.rcc.apb1_pre = APBPrescaler::Div1;
    // USB from HSI48, trimmed by CRS from USB SOF
    config.rcc.hsi48 = Some(Hsi48Config {
        sync_from_usb: true,
    });
    config.rcc.mux.adcsel = Adcsel::Pll1P;
    config.rcc.mux.usbsel = Usbsel::Hsi48;
    // RTC from LSI, embassy's default, said here: the backup domain (TAMP registers: the boot word, bkp_cnt!; RTC_CR:
    // the rev A hub hold, FW-12) survives a system reset only while this matches what BDCR holds, else embassy resets
    // the domain to change it
    config.rcc.ls = LsConfig::default_lsi();
    let p = embassy_stm32::init(config);
    // Flash prefetch off again: embassy-stm32 main turns it on with the 2 wait states 64 MHz needs (embassy#7276), but
    // on the G0B1 a prefetch can fail on a branch or call into the other flash bank (ES0548 2.2.10, every silicon
    // revision, no workaround but prefetch off), the CPU then runs wrong instructions. This image spans both 256 KiB
    // banks (FW-38).
    embassy_stm32::pac::FLASH
        .acr()
        .modify(|w| w.set_prften(false));
    init::init();
    let r = split_resources!(p);

    spawner.spawn(unwrap!(led::led_task(r.leds)));
    led::set(led::Led::Power, led::Mode::Breath { period_ms: 3000 });
    led::set(
        led::Led::Status,
        led::Mode::Blink {
            on_ms: 300,
            off_ms: 1200,
        },
    );

    // Before the ADC task, which feeds its guard: plug and header VBUS off, data connected, relay open (API-11)
    power::init(r.pwr);
    spawner.spawn(unwrap!(power::power_task()));

    spawner.spawn(unwrap!(adc::adc_task(r.adc, adc::ChannelSet::Base)));
    spawner.spawn(unwrap!(button::button_task()));
    spawner.spawn(unwrap!(boot::reboot_task()));

    // The MCU's USB is on hub port 1: nothing enumerates until the hub is configured and attached. The watchdog
    // (FW-18) redoes this after a brown-out resets the hub without resetting the MCU (P2534#12oct-1819).
    let hub = hub::bring_up(r.hub).await;
    spawner.spawn(unwrap!(hub::watchdog_task(hub)));

    // Composite USB device (FS, HSI48 clock above): WireWeaver and CMSIS-DAP v2 (FW-15), see src/ww/mod.rs.
    let usb_driver = usb::Driver::new(r.usb.usb, r.usb.pa12, r.usb.pa11, Irqs);
    let swd_host = swd::SwdHost::new(r.swd);
    // device.settings and the label (API-19): their boot values are applied by ww::start
    let settings = settings::open(r.settings.flash).await;
    uart::init(spawner, r.uart);
    ww::start(spawner, usb_driver, swd_host, last_reset, settings).await;

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
    let exc_return = bedrock_crash::exc_return!(); // first: lr still holds EXC_RETURN here
    // Saved for the next boot (src/last_reset.rs, served as device.last_reset), then reset instead of looping
    // (FW-13). With `probe-rs run` the debugger stops at the fault before this handler runs.
    unsafe { bedrock_crash::capture_fault(ef, exc_return, last_reset::uptime_ms()) };
    cnt::bkp_cnt!(hard_faults: u32, error);
    error!(
        "HardFault pc={=u32:#010x} lr={=u32:#010x} xpsr={=u32:#010x} r0={=u32:#x} r1={=u32:#x} r2={=u32:#x} r3={=u32:#x} r12={=u32:#x}",
        ef.pc(),
        ef.lr(),
        ef.xpsr(),
        ef.r0(),
        ef.r1(),
        ef.r2(),
        ef.r3(),
        ef.r12()
    );
    bedrock_crash::reset()
}
