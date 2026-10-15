//! WireWeaver server: implements `donguru_api::DonguruApi` (defined in `donguru_api/src/lib.rs` in the repo root) for
//! [ServerState] and serves it over USB, on a composite device built by [start] that also carries the CMSIS-DAP
//! v2 interface (`src/cmsis_dap.rs`, FW-15). The USB driver is created in `main.rs` and handed to [start].
//!
//! The handlers live in one module per level of the API tree (`device.rs`, `plug.rs`, ...), each an
//! `impl ServerState` block: a piece of work on one level touches one file. To add an item: declare it in the API
//! trait, then implement it in its level's module; `ww_codegen!` below regenerates the dispatch code and the compiler
//! lists what is missing (`ww api scaffold donguru_api --use-async --server ServerState` prints every handler). Items
//! not implemented yet answer `Unimplemented`. See https://github.com/vhrdtech/wire_weaver (`docs/`).

mod adc;
mod debug;
mod device;
mod gpio;
mod header;
mod hub;
mod i2c;
mod inputs;
mod leds;
mod meter;
mod plug;
mod sbu;
mod swd;
mod uart;

use crate::{cmsis_dap, hub as hub_ctl, power, swd as swd_host};
use cnt::cnt;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{Either3, select3};
use embassy_stm32::{peripherals, usb};
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::driver::{Direction, EndpointAddress};
use embassy_usb::{Builder, Config as UsbConfig, Handler};
use static_cell::StaticCell;
use wire_weaver::prelude::*;
use wire_weaver::{EventWriter, MessageSink, WireWeaverAsyncApiBackend};
use wire_weaver_usb_embassy::ww_device::LinkEvent;
use wire_weaver_usb_embassy::{
    LinkConfig, ServerBuffers, State as WwState, UsbDevice, UsbServer, UsbTimings, WireWeaverClass,
};
use ww_client_server::credit::SinkCredit;
use ww_stream::observe::ObserveLimits;
use ww_stream::store::RamRing;
use ww_stream::tx::{Counted, Reliable};
use ww_uid::{UidKind, UidSource, UniqueId};

/// Longest WireWeaver message the device accepts and can reply with (reported to the host)
const MAX_MESSAGE_LEN: usize = 1024;

/// USB endpoint registers, fixed so they don't move when an interface is added or cut by a feature (API-8, endpoint
/// table in the design). WireWeaver's bulk endpoints are double-buffered, each takes a whole register.
const EP_WW_OUT: usize = 1;
const EP_WW_IN: usize = 2;
const EP_CMSIS_DAP: usize = 3;

/// Bytes `uart[]`'s TX sink accepts ahead (its credit), the TX buffer of API-14. Nothing is accepted until then.
const UART_TX_CREDIT: u32 = 0;

/// Placeholder store of `device.log.frames` until API-10 puts the defmt log there.
type LogStore = RamRing<64>;

/// How often observed properties are checked while a host observes any (`poll_properties`).
const OBSERVE_TICK: Duration = Duration::from_millis(1);
/// Biggest `PropertyUpdate` event the link carries.
const OBSERVE_BUF_LEN: usize = 256;

/// Everything the API implementation needs that isn't a global of its driver module (LEDs, SWD, hub keep their
/// state in `src/led.rs`, `src/swd.rs`, `src/hub.rs`, shared with their tasks).
pub struct ServerState {
    /// Observed properties (SRV-18): what hosts observe and when it is due, see [server_task].
    observe: server_impl::ObserveState,
    /// Sessions of counted and reliable streams opened so far, see `new_stream_session`.
    stream_sessions: u32,
    /// State handed out for every counted stream not implemented yet: their open answers `Unimplemented`, so it is
    /// never opened.
    unopened: Counted,
    /// `device.log.frames` (API-10).
    log_frames: Reliable<LogStore>,
    /// `uart[].tx` credit, per index (API-14).
    uart_tx_credit: [SinkCredit; uart::COUNT],
    /// `device.uid.IDS`: the STM32 96-bit UID.
    uid: [UniqueId<'static>; 1],
    /// `device.last_reset`: reset cause and crash record read at boot (`src/last_reset.rs`).
    last_reset: bedrock_crash::Snapshot,
    /// `device.counters.read`: one page of counter words, copied at once.
    counters_page: [UVlq32; ww_counters::MAX_WORDS],
    /// `device.label`, in RAM until the settings store (API-19).
    label: device::Label,
    /// `leds`: global mode, brightness and the end of a running identify.
    leds: leds::LedsState,
}

impl ServerState {
    fn new(uid: &'static [u8], last_reset: bedrock_crash::Snapshot) -> Self {
        ServerState {
            observe: server_impl::ObserveState::new(ObserveLimits::DEFAULT),
            stream_sessions: 0,
            unopened: Counted::new(),
            log_frames: Reliable::new(RamRing::new()),
            uart_tx_credit: [
                SinkCredit::new(UART_TX_CREDIT),
                SinkCredit::new(UART_TX_CREDIT),
            ],
            uid: [UniqueId {
                kind: UidKind::ChipUnique,
                source: UidSource::InternalIc,
                value: MaxLen::new_unchecked(RefVec::new_bytes(uid)),
            }],
            last_reset,
            counters_page: [UVlq32(0); ww_counters::MAX_WORDS],
            label: device::Label::new(),
            leds: leds::LedsState::new(),
        }
    }

    /// Number of a new session of a counted or reliable stream, when a host opens one.
    fn new_stream_session(&mut self) -> u32 {
        // TODO(API-10): a boot counter in the high bits, so a cursor from before a reboot never matches a new session
        self.stream_sessions = self.stream_sessions.wrapping_add(1);
        self.stream_sessions
    }

    /// Settings and send state of observed properties, for `Observe` requests and `poll_properties`.
    fn observe_state(&mut self) -> &mut server_impl::ObserveState {
        &mut self.observe
    }
}

mod server_impl {
    // Re-parses the API trait and generates request dispatch + serdes for ServerState.
    // To see the generated code, add: debug_to_file = "./target/ww_server.rs"
    // `const_model`: the board record and the chip ids are read at boot, the build id and the counter buffers are
    // known at link time (`device.rs`). The optional build record (`bedrock_build_info`) is left out.
    wire_weaver::ww_codegen!(
        donguru_api :: DonguruApi for super::ServerState,
        server = true, no_alloc = true, use_async = true,
        method_model = "plug_vbus_cycle=deferred, _=immediate",
        property_model = "_=get_set",
        const_model = "BOARD|IDS|INFO|BUILD_ID=fn",
        omit = ["device.firmware.BUILD_RECORD"],
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
        HubWatchHandler {
            was_configured: false,
        }
    }

    fn lost(&mut self) {
        if self.was_configured {
            self.was_configured = false;
            hub_ctl::request_recovery();
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
static SERVER_STATE: StaticCell<ServerState> = StaticCell::new();
static UID: StaticCell<[u8; 12]> = StaticCell::new();

/// Builds the composite USB device (WireWeaver + CMSIS-DAP v2, `src/cmsis_dap.rs`) on `driver` and spawns the
/// USB, WireWeaver server and CMSIS-DAP tasks. `swd_host` is handed to the CMSIS-DAP task, which is the sole
/// owner of the SWD-host pins (FW-15). `last_reset` is what `last_reset::boot` read, served as `device.last_reset`.
pub fn start(
    spawner: Spawner,
    driver: UsbDriver,
    swd_host: swd_host::SwdHost,
    last_reset: bedrock_crash::Snapshot,
) {
    let buffers = USB_BUFFERS.init(UsbBuffers::default());
    // in a static, not on the task's future: the observe tables and stream states are a few hundred bytes
    let state = SERVER_STATE.init(ServerState::new(
        UID.init(embassy_stm32::uid::uid()),
        last_reset,
    ));

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
    // Double-buffered bulk endpoints: the USB peripheral takes the next packet while the previous one is processed,
    // instead of making the host retry (through the board's hub that costs ~half the throughput, FW-24). Each one
    // uses both directions of a hardware endpoint register.
    let ww = WireWeaverClass::with_endpoints(
        &mut builder,
        &mut buffers.ww_state,
        timings.packet_send_timeout,
        &link_cfg.user_api_version,
        server_impl::API_ID,
        |alt| {
            (
                alt.endpoint_bulk_out_double_buffered(
                    Some(EndpointAddress::from_parts(EP_WW_OUT, Direction::Out)),
                    MAX_USB_PACKET_LEN as u16,
                ),
                alt.endpoint_bulk_in_double_buffered(
                    Some(EndpointAddress::from_parts(EP_WW_IN, Direction::In)),
                    MAX_USB_PACKET_LEN as u16,
                ),
            )
        },
    );
    let dap_class = cmsis_dap::CmsisDapClass::new(
        &mut builder,
        &mut buffers.dap_state,
        EP_CMSIS_DAP as u8,
        MAX_USB_PACKET_LEN as u16,
    );

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

/// Serves requests and, while a host observes anything, sends the observed properties that are due every
/// [OBSERVE_TICK] (SRV-18). Observing stops when the link goes down: a host that comes back asks again. Also wakes
/// for `src/power.rs`: properties the guard changed and the deferred answers of `plug.vbus_cycle`.
#[embassy_executor::task]
async fn server_task(mut server: UsbServer<'static, UsbDriver>, state: &'static mut ServerState) {
    let mut buf = [0u8; OBSERVE_BUF_LEN];
    loop {
        let observing = state.observe.budget_in_use() > 0;
        let tick = async {
            if observing {
                Timer::after(OBSERVE_TICK).await
            } else {
                core::future::pending().await
            }
        };
        match select3(server.wait(), tick, power::wait_event()).await {
            Either3::First(ready) => {
                if let Some(event) = server.handle(ready, state).await {
                    info!("{:?}", event);
                    if matches!(event, LinkEvent::Down(_)) {
                        state.observe.reset();
                    }
                }
            }
            Either3::Second(()) => {}
            Either3::Third(()) => {
                state.power_changed();
                while let Some(done) = power::take_cycle_done() {
                    if done.seq != 0
                        && server.is_up()
                        && ServerState::plug_vbus_cycle_send_return(
                            &mut server.sink(),
                            done.seq,
                            plug::cycle_result(done.result),
                        )
                        .await
                        .is_err()
                    {
                        cnt!(vbus_cycle_answer_errors: u32, warn);
                    }
                }
            }
        }
        if observing && server.is_up() {
            let now_us = Instant::now().as_micros();
            if state
                .poll_properties(&mut server.sink(), (), now_us, &mut buf)
                .await
                .is_err()
            {
                cnt!(observe_send_errors: u32);
            }
        }
    }
}

impl ServerState {
    /// Marks what `src/power.rs` changed on its own (a trip, a fault, the end of a cycle).
    fn power_changed(&mut self) {
        let changes = power::take_changes();
        if changes & power::PLUG_VBUS != 0 {
            self.mark_changed_plug_vbus();
        }
        if changes & power::HEADER_VBUS_SW != 0 {
            self.mark_changed_header_vbus_sw();
        }
    }
}
