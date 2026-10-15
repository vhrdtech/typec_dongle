//! WireWeaver server: implements `donguru_api::DonguruApi` (defined in `donguru_api/src/lib.rs` in the repo root) for
//! [ServerState] and serves it over USB, on a composite device built by [start] that also carries the CMSIS-DAP
//! v2 interface (`src/cmsis_dap.rs`, FW-15). The USB driver is created in `main.rs` and handed to [start].
//!
//! To add a method, property or stream: declare it in the API trait, then implement it here, `ww_codegen!` below
//! regenerates the dispatch code and the compiler lists what is missing. Extend [ServerState] with the peripherals
//! and data the implementations need. See https://github.com/vhrdtech/wire_weaver (`docs/`, `examples_mcu/`).

use crate::{cmsis_dap, hub, led, swd};
use cnt::cnt;
use defmt::{info, unwrap, warn};
use donguru_api::{BoardVariant, SwdMode, SwdOrientation};
use embassy_executor::Spawner;
use embassy_stm32::{peripherals, usb};
use embassy_usb::{Builder, Config as UsbConfig, Handler};
use static_cell::StaticCell;
use wire_weaver::prelude::*;
use wire_weaver::{Context, EventOut, EventWriter, MessageSink, WireWeaverAsyncApiBackend};
use wire_weaver_usb_embassy::{
    LinkConfig, ServerBuffers, State as WwState, UsbDevice, UsbServer, UsbTimings, WireWeaverClass,
};

/// Longest WireWeaver message the device accepts and can reply with (reported to the host)
const MAX_MESSAGE_LEN: usize = 1024;

/// Everything the API implementation needs: peripherals, drivers, state.
/// LEDs are driven by `led::led_task`, the API only changes their mode. SWD-host orientation and release
/// (FW-15) live in `src/swd.rs`, shared with the CMSIS-DAP v2 task rather than owned here.
pub struct ServerState {}

// Implementations of the DonguruApi methods. `method_model = "_=immediate"`: they return a result right away
// (`RpcResult` can also defer the reply, see wire_weaver::RpcResult).
impl ServerState {
    async fn led_on(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        info!("led on");
        cnt!(led_on_calls: u32);
        led::set(led::Led::Status, led::Mode::On);
        Ready(())
    }

    async fn led_off(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        info!("led off");
        cnt!(led_off_calls: u32);
        led::set(led::Led::Status, led::Mode::Off);
        Ready(())
    }

    async fn get_swd_orientation(&mut self, _cx: &mut Context<'_, impl EventOut>) -> GetResult<SwdOrientation, ()> {
        Value(swd::orientation())
    }

    async fn set_swd_orientation(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        orientation: SwdOrientation,
    ) -> SetResult<()> {
        info!("swd orientation set");
        swd::set_orientation(orientation);
        Set
    }

    async fn get_swd_mode(&mut self, _cx: &mut Context<'_, impl EventOut>) -> GetResult<SwdMode, ()> {
        Value(swd::mode())
    }

    async fn set_swd_mode(&mut self, _cx: &mut Context<'_, impl EventOut>, mode: SwdMode) -> SetResult<()> {
        // a mode from a newer host: refuse it, keep the current one
        if let SwdMode::Unknown(n) = mode {
            warn!("swd mode {} unknown, refused", n);
            cnt!(swd_mode_unknown: u32);
            return SetError(());
        }
        info!("swd mode set");
        swd::set_mode(mode);
        Set
    }

    async fn swd_release(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        info!("swd release");
        cnt!(swd_release_calls: u32);
        swd::release().await;
        Ready(())
    }

    async fn get_board_variant(&mut self, _cx: &mut Context<'_, impl EventOut>) -> GetResult<BoardVariant, ()> {
        Value(hub::board_variant())
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
    type Medium = ();

    async fn process_bytes<'a>(
        &mut self,
        out: &mut EventWriter<'_, impl MessageSink>,
        medium: (),
        data: &[u8],
        scratch: &'a mut [u8],
    ) -> Result<&'a [u8], shrink_wrap::Error> {
        self.process_request_bytes(data, scratch, out, medium).await
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

/// Watches the composite device's own USB state for the hub (U301, port 1 carries the MCU's own USB) bouncing
/// after a brown-out (`src/hub.rs`, FW-18): a reset from `None`/`Default` state is normal on first enumeration,
/// but once the device has reached `Configured` once, any later `reset()` means something upstream bounced the
/// link outside of the host ever issuing one — exactly what the hub's own reset does to its downstream ports.
struct HubWatchHandler {
    was_configured: bool,
}

impl HubWatchHandler {
    const fn new() -> Self {
        HubWatchHandler { was_configured: false }
    }

    fn lost(&mut self) {
        if self.was_configured {
            self.was_configured = false;
            hub::request_recovery();
        }
    }
}

impl Handler for HubWatchHandler {
    fn reset(&mut self) {
        self.lost();
    }

    fn configured(&mut self, configured: bool) {
        if configured {
            self.was_configured = true;
        } else {
            self.lost();
        }
    }
}

pub type UsbDriver = usb::Driver<'static, peripherals::USB>;

const MAX_USB_PACKET_LEN: usize = 64; // Full Speed

/// Descriptor and class buffers for the whole composite device (WireWeaver + CMSIS-DAP v2). Replaces
/// `wire_weaver_usb_embassy::usb_init`'s convenience `UsbBuffers`, which only has room for one function: its own
/// doc comment points at building the `Builder` by hand for "other USB classes alongside".
struct UsbBuffers {
    config_descriptor: [u8; 192],
    bos_descriptor: [u8; 40],
    msos_descriptor: [u8; 330],
    control: [u8; 256],
    ww_state: WwState,
    dap_state: cmsis_dap::State,
    hub_watch: HubWatchHandler,
    server: ServerBuffers<MAX_USB_PACKET_LEN, MAX_MESSAGE_LEN>,
}

impl Default for UsbBuffers {
    fn default() -> Self {
        UsbBuffers {
            config_descriptor: [0; 192],
            bos_descriptor: [0; 40],
            msos_descriptor: [0; 330],
            control: [0; 256],
            ww_state: WwState::new(),
            dap_state: cmsis_dap::State::new(),
            hub_watch: HubWatchHandler::new(),
            server: ServerBuffers::default(),
        }
    }
}

static USB_BUFFERS: StaticCell<UsbBuffers> = StaticCell::new();

/// Builds the composite USB device (WireWeaver + CMSIS-DAP v2, `src/cmsis_dap.rs`) on `driver` and spawns the
/// USB, WireWeaver server and CMSIS-DAP tasks. `swd_host` is handed to the CMSIS-DAP task, which is the sole
/// owner of the SWD-host pins (FW-15).
pub fn start(spawner: Spawner, driver: UsbDriver, state: ServerState, swd_host: swd::SwdHost) {
    let buffers = USB_BUFFERS.init(UsbBuffers::default());

    let mut config = UsbConfig::new(0xc0de, 0xcafe);
    config.manufacturer = Some("donguru");
    config.product = Some("donguru");
    config.serial_number = Some(embassy_stm32::uid::uid_hex());
    // Required for Windows compatibility with more than one function, see wire_weaver_usb_embassy::usb_init.
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;
    config.max_power = 100;
    config.self_powered = false;

    let mut builder = Builder::new(
        driver,
        config,
        &mut buffers.config_descriptor,
        &mut buffers.bos_descriptor,
        &mut buffers.msos_descriptor,
        &mut buffers.control,
    );
    builder.handler(&mut buffers.hub_watch);

    let timings = UsbTimings::fs_higher_speed();
    let link_cfg = link_config();
    // server_impl::API_ID is reported in a USB string descriptor, `ww list` shows it without opening the device.
    // A user label (e.g. loaded from flash) can be added with wire_weaver::api_id::with_label().
    let ww = WireWeaverClass::new(
        &mut builder,
        &mut buffers.ww_state,
        MAX_USB_PACKET_LEN as u16,
        timings.use_bulk_endpoints,
        timings.packet_send_timeout,
        &link_cfg.user_api_version,
        server_impl::API_ID,
    );
    let dap_class = cmsis_dap::CmsisDapClass::new(&mut builder, &mut buffers.dap_state, MAX_USB_PACKET_LEN as u16);

    let usb = builder.build();
    let server = ww.into_server(link_cfg, &timings, &mut buffers.server);

    spawner.spawn(unwrap!(usb_task(usb)));
    spawner.spawn(unwrap!(server_task(server, state)));
    spawner.spawn(unwrap!(cmsis_dap::dap_task(dap_class, swd_host)));
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
