//! WireWeaver server: implements `donguru_api::DonguruApi` (defined in `donguru_api/src/lib.rs` in the repo root) for
//! [ServerState] and serves it over USB. The USB driver is created in `main.rs` and handed to [start].
//!
//! To add a method, property or stream: declare it in the API trait, then implement it here, `ww_codegen!` below
//! regenerates the dispatch code and the compiler lists what is missing. Extend [ServerState] with the peripherals
//! and data the implementations need. See https://github.com/vhrdtech/wire_weaver (`docs/`, `examples_mcu/`).

use crate::led;
use cnt::cnt;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::{peripherals, usb};
use static_cell::StaticCell;
use wire_weaver::prelude::*;
use wire_weaver::{MessageSink, WireWeaverAsyncApiBackend};
use wire_weaver_usb_embassy::{LinkConfig, UsbBuffers, UsbDevice, UsbServer, UsbTimings, usb_init};

/// Longest WireWeaver message the device accepts and can reply with (reported to the host)
const MAX_MESSAGE_LEN: usize = 1024;

/// Everything the API implementation needs: peripherals, drivers, state.
/// LEDs are driven by `led::led_task`, the API only changes their mode.
pub struct ServerState {}

// Implementations of the DonguruApi methods. `method_model = "_=immediate"`: they return a result right away
// (`RpcResult` can also defer the reply, see wire_weaver::RpcResult).
impl ServerState {
    async fn led_on(&mut self, _msg_tx: &mut impl MessageSink) -> RpcResult<()> {
        info!("led on");
        cnt!(led_on_calls: u32);
        led::set(led::Led::Status, led::Mode::On);
        Ready(())
    }

    async fn led_off(&mut self, _msg_tx: &mut impl MessageSink) -> RpcResult<()> {
        info!("led off");
        cnt!(led_off_calls: u32);
        led::set(led::Led::Status, led::Mode::Off);
        Ready(())
    }
}

mod server_impl {
    // Re-parses the API trait and generates request dispatch + serdes for ServerState.
    // To see the generated code, add: debug_to_file = "./target/ww_server.rs"
    wire_weaver::ww_codegen!(
        donguru_api :: DonguruApi for super::ServerState,
        server = true, no_alloc = true, use_async = true,
        method_model = "_=immediate",
        property_model = "_=get_set",
        introspect = "with_docs",
    );
}

impl WireWeaverAsyncApiBackend for ServerState {
    async fn process_bytes<'a>(
        &mut self,
        msg_tx: &mut impl MessageSink,
        data: &[u8],
        scratch: &'a mut [u8],
    ) -> Result<&'a [u8], shrink_wrap::Error> {
        self.process_request_bytes(data, scratch, msg_tx).await
    }

    fn version(&self) -> FullVersion<'_> {
        donguru_api::DONGURU_API_FULL_GID
    }
}

fn link_config() -> LinkConfig<'static> {
    LinkConfig::new(
        donguru_api::DONGURU_API_FULL_GID,
        server_impl::api_hash(),
        ww_client_server::COMPACT_VERSION,
    )
}

pub type UsbDriver = usb::Driver<'static, peripherals::USB>;

const MAX_USB_PACKET_LEN: usize = 64; // Full Speed
static USB_BUFFERS: StaticCell<UsbBuffers<MAX_USB_PACKET_LEN, MAX_MESSAGE_LEN>> = StaticCell::new();

/// Builds the USB device with the WireWeaver class on `driver` and spawns the USB and server tasks.
pub fn start(spawner: Spawner, driver: UsbDriver, state: ServerState) {
    // server_impl::API_ID is reported in a USB string descriptor, `ww list` shows it without opening the device.
    // A user label (e.g. loaded from flash) can be added with wire_weaver::api_id::with_label().
    let (usb, server) = usb_init(
        driver,
        USB_BUFFERS.init(UsbBuffers::default()),
        UsbTimings::fs_higher_speed(),
        link_config(),
        server_impl::API_ID,
        |config| {
            // VID:PID is c0de:cafe unless set here
            config.manufacturer = Some("donguru");
            config.product = Some("donguru");
            config.serial_number = Some(embassy_stm32::uid::uid_hex());
        },
    );
    spawner.spawn(unwrap!(usb_task(usb)));
    spawner.spawn(unwrap!(server_task(server, state)));
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, UsbDriver>) {
    usb.run().await;
}

#[embassy_executor::task]
async fn server_task(mut server: UsbServer<'static, UsbDriver>, mut state: ServerState) {
    // Nothing else to wait for: the prepared loop. Use server.wait() / server.handle() to also react to other events.
    server.run(&mut state).await;
}
