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

pub use meter::{MeterMarks, MeterSamples};

use crate::{cmsis_dap, hub as hub_ctl, power, settings, swd as swd_host};
use cnt::cnt;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{Either5, select5};
use embassy_stm32::{peripherals, usb};
use embassy_time::{Instant, Timer};
use embassy_usb::driver::{Direction, EndpointAddress};
use embassy_usb::{Builder, Config as UsbConfig, Handler};
use static_cell::StaticCell;
use wire_weaver::prelude::*;
use wire_weaver::{EventWriter, MessageSink, WireWeaverAsyncApiBackend};
use wire_weaver_usb_embassy::ww_device::LinkEvent;
use wire_weaver_usb_embassy::{
    LinkConfig, ServerBuffers, State as WwState, UsbDevice, UsbServer, UsbTimings, WireWeaverClass,
};
use ww_stream::observe::ObserveLimits;
use ww_stream::sink::SinkCredit;
use ww_stream::tx::Counted;
use ww_uid::{UidKind, UidSource, UniqueId};

/// Longest WireWeaver message the device accepts and can reply with (reported to the host)
const MAX_MESSAGE_LEN: usize = 1024;

/// USB endpoint registers, fixed so they don't move when an interface is added or cut by a feature (API-8, endpoint
/// table in the design). WireWeaver's bulk endpoints are double-buffered, each takes a whole register.
const EP_WW_OUT: usize = 1;
const EP_WW_IN: usize = 2;
const EP_CMSIS_DAP: usize = 3;

/// Biggest `PropertyUpdate` event the link carries; a batch of samples is sealed before it outgrows one.
const OBSERVE_BUF_LEN: usize = 512;
/// Rows of `meter.reading` samples the ADC task can commit between two `poll_properties` (API-39): a DMA half
/// buffer brings 24 at once, so 64 rides out a few late turns of the server task.
const METER_SAMPLES_RING: usize = 64;

/// Observe cells of `meter.reading` (API-39, `observe_cells` in `ww_codegen!`): the ADC task writes one row per
/// sequence through [MeterSamples], `poll_properties` encodes them for hosts observing in `Samples` mode.
static METER_SAMPLES: server_impl::RootMeterCells<METER_SAMPLES_RING> =
    server_impl::RootMeterCells::new();
/// Changes to observed properties made outside the server state (`src/power.rs`, the ADC task's meter readings),
/// and the wake of the server task (WireWeaver SRV-25): `OBSERVE.mark_changed_<name>()` from any task or interrupt.
pub(crate) static OBSERVE: server_impl::ObserveSignal = server_impl::ObserveSignal::new();
type ObserveState = server_impl::ObserveState<&'static mut server_impl::ObserveBatches<'static>>;
// built in place: by value they would take several times their size of stack
static OBSERVE_BATCHES: StaticCell<server_impl::ObserveBatches<'static>> = StaticCell::new();
static OBSERVE_STATE: StaticCell<ObserveState> = StaticCell::new();
/// One column of a batch while it is encoded: the biggest is `meter.reading`'s.
const OBSERVE_SCRATCH: usize =
    ww_stream::batch::scratch_bytes::<donguru_api::MeterReading>(server_impl::OBSERVE_BATCH);
static SCRATCH: StaticCell<ww_stream::batch::Scratch<OBSERVE_SCRATCH>> = StaticCell::new();
/// Biggest item of a reliable stream: `device.log.frames`' `DefmtFrames` (512 B of frames and its length).
const STREAM_BUF_LEN: usize = ww_log::MAX_FRAMES_LEN + 16;
/// Bytes of stream items sent per turn of the server task (at least one event), so requests keep being served.
const STREAM_BUDGET: usize = 2048;

/// Everything the API implementation needs that isn't a global of its driver module (LEDs, SWD, hub keep their
/// state in `src/led.rs`, `src/swd.rs`, `src/hub.rs`, shared with their tasks).
pub struct ServerState {
    /// Observed properties (SRV-18) and batches of samples (API-39): what hosts observe and when it is due, see
    /// [server_task].
    observe: &'static mut ObserveState,
    /// Sessions of counted and reliable streams opened so far, see `new_stream_session`.
    stream_sessions: u32,
    /// State handed out for every counted stream not implemented yet: their open answers `Unimplemented`, so it is
    /// never opened.
    unopened: Counted,
    /// `device.log.frames`: the defmt log (API-10, `src/log.rs`).
    log_frames: crate::log::Stream,
    /// `uart[].tx` credit, per index (API-14).
    uart_tx_credit: [SinkCredit; uart::COUNT],
    /// `uart[].rx`, the counted receive streams: `uart[1]`'s compresses for hosts that decode LZ4 (FW-42),
    /// `uart[0]` has none yet.
    uart_rx: [uart::RxState; uart::COUNT],
    /// `device.uid.IDS`: the STM32 96-bit UID.
    uid: [UniqueId<'static>; 1],
    /// `device.last_reset`: reset cause and crash record read at boot (`src/last_reset.rs`).
    last_reset: bedrock_crash::Snapshot,
    /// `device.counters.read`: one page of counter words, copied at once.
    counters_page: [UVlq32; ww_counters::MAX_WORDS],
    /// `device.settings` and `device.label`, kept in flash (API-19, `src/settings.rs`).
    user: &'static mut settings::User,
    store: &'static mut settings::Store,
    /// A record was written: erase ahead in the next turn of the server task, after the answer went out.
    settings_maintain: bool,
    /// `leds`: global mode, brightness and the end of a running identify.
    leds: leds::LedsState,
}

impl ServerState {
    async fn new(
        uid: &'static [u8],
        last_reset: bedrock_crash::Snapshot,
        (store, user): (&'static mut settings::Store, &'static mut settings::User),
    ) -> Self {
        // the boot count in the high half: a session of this boot never takes a host's cursor of an earlier boot
        // for its own (the boot count restarts at a power-on, the log store is lost with it then)
        let stream_sessions = last_reset.view().boot_count << 16 | 1;
        meter::init_deadband(&user.get().meter_deadband);
        let observe = ObserveState::init(
            OBSERVE_STATE.uninit(),
            ObserveLimits::DEFAULT,
            server_impl::ObserveBatches::init(
                OBSERVE_BATCHES.uninit(),
                ww_stream::batch::DEFAULT_POLICY,
                unwrap!(METER_SAMPLES.reader()),
            ),
        );
        observe.with_signal(&OBSERVE);
        ServerState {
            observe,
            stream_sessions,
            unopened: Counted::new(),
            log_frames: crate::log::stream(stream_sessions).await,
            // `uart[0]` has no hardware behind it yet: no credit, nothing is accepted. `uart[1]` accepts what its
            // TX pipe holds, items of up to `UART_TX_MAX`
            uart_tx_credit: [
                SinkCredit::new(0),
                SinkCredit::sized_for(
                    crate::uart::TX_BUF as u32,
                    Some(ww_uart::UART_TX_MAX as u32),
                ),
            ],
            uart_rx: uart::RxState::new(),
            uid: [UniqueId {
                kind: UidKind::ChipUnique,
                source: UidSource::InternalIc,
                value: MaxLen::new_unchecked(RefVec::new_bytes(uid)),
            }],
            last_reset,
            counters_page: [UVlq32(0); ww_counters::MAX_WORDS],
            leds: leds::LedsState::new(),
            user,
            store,
            settings_maintain: false,
        }
    }

    /// Number of a new session of a counted or reliable stream, when a host opens one.
    fn new_stream_session(&mut self) -> u32 {
        // the first one went to the log at boot (`ServerState::new`)
        self.stream_sessions = self.stream_sessions.wrapping_add(1);
        self.stream_sessions
    }

    /// Settings and send state of observed properties, for `Observe` requests and `poll_properties`.
    fn observe_state(&mut self) -> &mut ObserveState {
        self.observe
    }
}

mod server_impl {
    // Re-parses the API trait and generates request dispatch + serdes for ServerState.
    // To see the generated code, add: debug_to_file = "./target/ww_server.rs"
    // `const_model`: the board record and the chip ids are read at boot, the build id and the counter buffers are
    // known at link time, the defmt encoding at run time (`device.rs`). The optional build record (`bedrock_build_info`) is left out.
    wire_weaver::ww_codegen!(
        donguru_api :: DonguruApi for super::ServerState,
        server = true, no_alloc = true, use_async = true,
        // handlers that await (flash writes, ADC readings, the VBUS cycle, the log flood) are async, all others
        // plain `fn`s (SRV-21): an `async fn` that never awaits still costs flash as a state of the dispatcher
        handler_model = "debug.log_flood|device.label.*|device.settings.*|sbu.release|adc.read|plug.vbus_cycle|meter.reading|meter.temperature=async, _=sync",
        method_model = "plug_vbus_cycle=deferred, _=immediate",
        property_model = "_=get_set",
        // rows of samples per batch: ~20 ms of meter.reading sequences
        observe_batch = 32,
        observe_outbox = 1024,
        observe_cells = "root_meter",
        const_model = "BOARD|IDS|INFO|BUILD_ID|DEFMT=fn",
        // no I2C slave mode on Donguru (ww_i2c STD-35): the group answers Unimplemented, clients see it omitted
        omit = ["device.firmware.BUILD_RECORD", "i2c.slave"],
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
/// `server_impl::API_ID` with the user label (API-19), as the WireWeaver interface's string descriptor.
static API_ID_LABELED: StaticCell<[u8; 256]> = StaticCell::new();

/// Builds the composite USB device (WireWeaver + CMSIS-DAP v2, `src/cmsis_dap.rs`) on `driver` and spawns the
/// USB, WireWeaver server and CMSIS-DAP tasks. `swd_host` is handed to the CMSIS-DAP task, which is the sole
/// owner of the SWD-host pins (FW-15). `last_reset` is what `last_reset::boot` read, served as `device.last_reset`.
/// `settings` is the open settings store (`src/settings.rs`): its boot values are applied here.
pub async fn start(
    spawner: Spawner,
    driver: UsbDriver,
    swd_host: swd_host::SwdHost,
    last_reset: bedrock_crash::Snapshot,
    settings: (&'static mut settings::Store, &'static mut settings::User),
) {
    let buffers = USB_BUFFERS.init(UsbBuffers::default());
    // in a static, not on the task's future: the observe tables and stream states are a few hundred bytes
    let state =
        SERVER_STATE.init(ServerState::new(embassy_stm32::uid::uid(), last_reset, settings).await);
    state.apply_boot_settings().await;

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
    // server_impl::API_ID is reported in a USB string descriptor, `ww list` shows it without opening the device,
    // with the label from the settings (as it was at boot: a new label shows after the next one)
    let api_id = wire_weaver::api_id::with_label(
        server_impl::API_ID,
        state.user.get().label.as_str(),
        API_ID_LABELED.init([0; 256]),
    );
    // Double-buffered bulk endpoints: the USB peripheral takes the next packet while the previous one is processed,
    // instead of making the host retry (through the board's hub that costs ~half the throughput, FW-24). Each one
    // uses both directions of a hardware endpoint register.
    let ww = WireWeaverClass::with_endpoints(
        &mut builder,
        &mut buffers.ww_state,
        timings.packet_send_timeout,
        &link_cfg.user_api_version,
        api_id,
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

/// Serves requests; every [PUMP_PERIOD](crate::log::PUMP_PERIOD) moves the defmt log into its store
/// (`src/log.rs`) and lets the settings store tidy up; sends what reliable streams have; sends observed properties
/// (SRV-18) when they are due: the task sleeps until `next_wake` or until something changes outside it
/// ([OBSERVE]: `src/power.rs`, the ADC task's meter readings and samples), no fixed tick (SRV-25). Observing stops
/// when the link goes down: a host that comes back asks again; the log stays in its store for a host that resumes
/// it. Also wakes for the deferred answers of `plug.vbus_cycle` (`src/power.rs`).
#[embassy_executor::task]
async fn server_task(mut server: UsbServer<'static, UsbDriver>, state: &'static mut ServerState) {
    let mut buf = [0u8; OBSERVE_BUF_LEN];
    let scratch = SCRATCH.init(ww_stream::batch::Scratch::new());
    let mut stream_buf = [0u8; STREAM_BUF_LEN];
    let mut next_pump = Instant::now() + crate::log::PUMP_PERIOD;
    loop {
        // nothing is observed while the link is down (reset at LinkEvent::Down)
        let observe_at = state
            .observe
            .next_wake(Instant::now().as_micros())
            .map(wire_weaver_usb_embassy::ww_device::Instant::from_micros);
        match select5(
            server.wait_until(observe_at),
            Timer::at(next_pump),
            power::wait_event(),
            OBSERVE.wait(),
            crate::uart::wait_event(),
        )
        .await
        {
            // a due `Ready` (observe_at came) leaves `handle` nothing to do
            Either5::First(ready) => {
                if let Some(event) = server.handle(ready, state).await {
                    info!("{:?}", event);
                    if matches!(event, LinkEvent::Down(_)) {
                        state.observe.reset();
                        state.log_frames.detach();
                        for rx in &mut state.uart_rx {
                            ww_stream::codec::CountedState::detach(rx);
                        }
                    }
                }
            }
            Either5::Second(()) => {
                next_pump += crate::log::PUMP_PERIOD;
                state.settings_idle().await;
            }
            Either5::Third(()) => {
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
            // a change from outside the server state (taken by next_wake / poll_properties) or a batch of samples
            Either5::Fourth(()) => {}
            Either5::Fifth(()) => state.uart_pump(&mut server).await,
        }
        crate::log::pump(&mut state.log_frames).await;
        if server.is_up() {
            let now_us = Instant::now().as_micros();
            if state
                .observe
                .next_wake(now_us)
                .is_some_and(|at| at <= now_us)
                && state
                    .poll_properties(&mut server.sink(), (), now_us, &mut buf, scratch)
                    .await
                    .is_err()
            {
                cnt!(observe_send_errors: u32);
            }
            if state
                .poll_streams(
                    &mut server.sink(),
                    now_us / 1000,
                    &mut stream_buf,
                    STREAM_BUDGET,
                )
                .await
                .is_err()
            {
                cnt!(stream_send_errors: u32);
            }
        }
    }
}
