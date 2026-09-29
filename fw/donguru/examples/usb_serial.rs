#![no_std]
#![no_main]

use defmt::{panic, *};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::i2c::I2c;
use embassy_stm32::usb::{Driver, Instance};
use embassy_stm32::{Config, bind_interrupts, peripherals, usb};
use embassy_time::Timer;
use embassy_usb::Builder;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use panic_probe as _;

bind_interrupts!(struct Irqs {
    USB_UCPD1_2 => usb::InterruptHandler<peripherals::USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi48 = Some(Hsi48Config {
            sync_from_usb: true,
        });
        config.rcc.mux.usbsel = mux::Usbsel::HSI48;
    }
    let p = embassy_stm32::init(config);

    info!("Hello World!");

    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);
    let mut led = Output::new(p.PF1, Level::Low, Speed::Low);
    let mut hub_nrst = Output::new(p.PC13, Level::Low, Speed::Low);
    let _hub_i2c_en = Output::new(p.PA4, Level::High, Speed::Low);
    Timer::after_millis(1).await;
    hub_nrst.set_high();

    let mut data = [0u8; 32];
    let mut i2c = I2c::new_blocking(p.I2C2, p.PB10, p.PB14, Default::default());
    const ADDR: u8 = 0b010_1100;
    let r = i2c.blocking_write(
        ADDR,
        &[
            0, 17, 0x24, 0x04, 0x12, 0x25, 0xB3, 0x0B, 0x9B, 0x20, 0x02, 0, 0, 0, 0x01, 0x32, 0x01,
            0x32, 0x32,
        ],
    );
    info!("write: {:?}", r);
    let r = i2c.blocking_write_read(ADDR, &[0], &mut data);
    info!("r: {:?} data: {:02x}", r, data);
    let r = i2c.blocking_write(ADDR, &[0xFF, 1, 0b0000_0001]);
    info!("up: {:?}", r);

    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.product = Some("USB Serial");
    //config.max_packet_size_0 = 64;

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no msos descriptors
        &mut control_buf,
    );

    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();
    let usb_fut = usb.run();

    let echo_fut = async {
        loop {
            class.wait_connection().await;
            info!("Connected");
            let _ = echo(&mut class).await;
            info!("Disconnected");
        }
    };

    led.set_high();
    join(usb_fut, echo_fut).await;
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn echo<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];
        info!("data: {:x}", data);
        class.write_packet(data).await?;
    }
}
