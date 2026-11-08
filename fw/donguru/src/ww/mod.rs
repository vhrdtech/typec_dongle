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
mod boot;
mod clock;
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

#[cfg(feature = "swd-host")]
use crate::cmsis_dap;
use crate::{hub as hub_ctl, power, settings, swd as swd_host};
use bedrock_usb::Usb;
use bedrock_usb::buffer::Double;
use bedrock_usb::control::Handler;
use bedrock_usb::desc::{Device, Function, STRING_FIRST_INTERFACE, STRING_SERIAL, Str};
use bedrock_usb::v4::{BulkIn, BulkOut, Mmio};
use cnt::cnt;
use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_futures::select::{Either5, select5};
use embassy_time::{Duration, Instant, Timer};
use static_cell::{ConstStaticCell, StaticCell};
use wire_weaver::prelude::*;
use wire_weaver::{EventWriter, MessageSink, WireWeaverAsyncApiBackend};
use ww_device::{EmbassyClock, FramedRx, FramedTx, LinkConfig, LinkEvent, RxBuffer};
use ww_stream::observe::ObserveLimits;
use ww_stream::sink::SinkCredit;
use ww_stream::tx::Counted;
use ww_uid::{UidKind, UidSource, UniqueId};

/// Longest WireWeaver message the device accepts and can reply with (reported to the host)
const MAX_MESSAGE_LEN: usize = 1024;

/// USB endpoint registers, fixed so they don't move when an interface is added or cut by a feature (API-8, endpoint
/// table in the design). WireWeaver's bulk endpoints are double-buffered, each takes a whole register.
const EP_WW_OUT: u8 = 1;
const EP_WW_IN: u8 = 2;
#[cfg(feature = "swd-host")]
pub(crate) const EP_CMSIS_DAP: u8 = 3;

/// Biggest `PropertyUpdate` event the link carries; a batch of samples is sealed before it outgrows one.
const OBSERVE_BUF_LEN: usize = 512;
/// Rows of `meter.reading` samples the ADC task can commit between two `poll_properties` (API-39): a DMA half
/// buffer brings 24 at once, so 64 rides out a few late turns of the server task.
#[cfg(feature = "meter-samples")]
const METER_SAMPLES_RING: usize = 64;

/// Observe cells of `meter.reading` (API-39, `observe_cells` in `ww_codegen!`): the ADC task writes one row per
/// sequence through [MeterSamples], `poll_properties` encodes them for hosts observing in `Samples` mode.
#[cfg(feature = "meter-samples")]
static METER_SAMPLES: server_impl::RootMeterCells<METER_SAMPLES_RING> =
    server_impl::RootMeterCells::new();
/// Changes to observed properties made outside the server state (`src/power.rs`, the ADC task's meter readings),
/// and the wake of the server task (WireWeaver SRV-25): `OBSERVE.mark_changed_<name>()` from any task or interrupt.
pub(crate) static OBSERVE: server_impl::ObserveSignal = server_impl::ObserveSignal::new();
#[cfg(feature = "meter-samples")]
type Batches = server_impl::ObserveBatches<'static>;
#[cfg(feature = "meter-samples")]
type ObserveState = server_impl::ObserveState<&'static mut Batches>;
#[cfg(not(feature = "meter-samples"))]
type ObserveState = server_impl::ObserveState;
// built in place: by value they would take several times their size of stack
#[cfg(feature = "meter-samples")]
static OBSERVE_BATCHES: StaticCell<Batches> = StaticCell::new();
static OBSERVE_STATE: StaticCell<ObserveState> = StaticCell::new();
/// One column of a batch while it is encoded: the biggest is `meter.reading`'s.
#[cfg(feature = "meter-samples")]
const OBSERVE_SCRATCH: usize =
    ww_stream::batch::scratch_bytes::<donguru_api::MeterReading>(server_impl::OBSERVE_BATCH);
#[cfg(feature = "meter-samples")]
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
    /// `plug.pd.messages`: the PD source's log (API-30, `src/pd.rs`).
    pd_messages: Counted,
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
    /// `clock`: the sync line's configuration (always `Off`, no line) and whether `time_map` is subscribed
    /// (API-57, FW-44). The shared clock itself is `crate::sync::SYNC`, read by the LED task too.
    clock: clock::ClockState,
    /// `debug`: answer buffers and the SMBus request number (API-18).
    debug: debug::DebugState,
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
            #[cfg(feature = "meter-samples")]
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
            pd_messages: Counted::new(),
            uid: [UniqueId {
                kind: UidKind::ChipUnique,
                source: UidSource::InternalIc,
                value: MaxLen::new_unchecked(RefVec::new_bytes(uid)),
            }],
            last_reset,
            counters_page: [UVlq32(0); ww_counters::MAX_WORDS],
            leds: leds::LedsState::new(),
            clock: clock::ClockState::new(),
            debug: debug::DebugState::new(),
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
    // one invocation: the introspection with or without the API's doc comments (feature `api-docs`, 13.4 KiB),
    macro_rules! server {
        ($introspect:tt, $($cells:tt)*) => {
            wire_weaver::ww_codegen!(
                donguru_api :: DonguruApi for super::ServerState,
                server = true, no_alloc = true, use_async = true,
                // handlers that await (flash writes, ADC readings, the VBUS cycle, the log flood, clock.steer's time_map
                // push) are async, all others plain `fn`s (SRV-21): an `async fn` that never awaits still costs flash as a
                // state of the dispatcher
                handler_model = "debug.log_flood|debug.hub_read|debug.hub_write|device.label.*|device.settings.*|sbu.release|adc.read|plug.vbus_cycle|meter.reading|meter.temperature|clock.steer=async, _=sync",
                method_model = "plug_vbus_cycle=deferred, _=immediate",
                property_model = "_=get_set",
                $($cells)*
                const_model = "BOARD|IDS|INFO|BUILD_ID|DEFMT=fn",
                // no I2C slave mode on Donguru (ww_i2c STD-35): the group answers Unimplemented, clients see it omitted;
                // the board's reworks list (ww_board_info STD-24, STD-98) isn't served yet
                omit = ["device.board.REWORKS", "device.board.set_reworks", "device.firmware.BUILD_RECORD", "i2c.slave"],
                introspect = $introspect,
            );
        };
    }
    // and with the observe cells of `meter.reading`'s samples (feature `meter-samples`, API-39), rows of samples per
    // batch: ~20 ms of its sequences; or without any: no batcher, outbox or encoder, `Samples` refused (SRV-56)
    #[cfg(all(feature = "api-docs", feature = "meter-samples"))]
    server!(
        "with_docs",
        observe_cells = "root_meter",
        observe_batch = 32,
        observe_outbox = 1024,
    );
    #[cfg(all(feature = "api-docs", not(feature = "meter-samples")))]
    server!("with_docs", observe_samples = false,);
    #[cfg(all(not(feature = "api-docs"), feature = "meter-samples"))]
    server!(
        "no_docs",
        observe_cells = "root_meter",
        observe_batch = 32,
        observe_outbox = 1024,
    );
    #[cfg(all(not(feature = "api-docs"), not(feature = "meter-samples")))]
    server!("no_docs", observe_samples = false,);
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
        server_impl::image_facts(),
        ww_client_server::COMPACT_VERSION,
    )
}

/// Watches the composite device's own USB state for the hub (U301, port 1 carries the MCU's own USB) bouncing
/// after a brown-out (`src/hub.rs`, FW-18): a reset before the first configuration is normal enumeration, but once
/// the device has been configured, any later bus reset or loss of the configuration means something upstream
/// bounced the link without the host asking for it: exactly what the hub's own reset does to its downstream ports.
/// Also serves the strings only known at run time: the serial number and the WireWeaver interface's API id with
/// the user label.
struct UsbControl {
    serial: &'static str,
    api_id: &'static str,
    was_configured: bool,
}

impl UsbControl {
    fn lost(&mut self) {
        if self.was_configured {
            self.was_configured = false;
            hub_ctl::request_recovery();
        }
    }
}

impl Handler for UsbControl {
    fn reset(&mut self) {
        self.lost();
    }

    fn configured(&mut self, configured: bool) {
        if configured {
            self.was_configured = true;
            crate::boot::usb_configured();
        } else {
            self.lost();
        }
    }

    fn string(&self, index: u8) -> Option<&str> {
        match index {
            STRING_SERIAL => Some(self.serial),
            STRING_API_ID => Some(self.api_id),
            _ => None,
        }
    }
}

const MAX_USB_PACKET_LEN: usize = 64; // Full Speed

/// The WireWeaver function: vendor interface, API id as its string (with the label, so `Str::Runtime`), the
/// double-buffered bulk endpoints: the USB peripheral takes the next packet while the previous one is processed,
/// instead of making the host retry (through the board's hub that costs ~half the throughput, FW-24).
const WW_FUNCTION: Function = bedrock_usb::ww_function! {
    out: EP_WW_OUT, in: EP_WW_IN, max_packet: MAX_USB_PACKET_LEN as u16, buffering: bedrock_usb::Buffering::Double,
    user_crate: donguru_api::DONGURU_API_FULL_GID.crate_id,
    user_version: (
        donguru_api::DONGURU_API_FULL_GID.version.major.0,
        donguru_api::DONGURU_API_FULL_GID.version.minor.0,
        donguru_api::DONGURU_API_FULL_GID.version.patch.0
    ),
    api_id: Str::Runtime,
};
#[cfg(feature = "swd-host")]
const FUNCTIONS: &[Function] = &[WW_FUNCTION, cmsis_dap::FUNCTION];
#[cfg(not(feature = "swd-host"))]
const FUNCTIONS: &[Function] = &[WW_FUNCTION];

/// The composite USB device, WireWeaver and CMSIS-DAP v2 (`src/cmsis_dap.rs`): descriptors, endpoint registers
/// and USB SRAM are fixed at build time (bedrock_usb). One IAD per function, as Windows needs for a composite device.
const DEVICE: Device = Device {
    manufacturer: Str::Const("donguru"),
    product: Str::Const("donguru"),
    serial: Str::Runtime,
    // WinUSB on the WireWeaver function, at device level and in its function subset
    ms_os_20: Some(bedrock_usb::ww::MS_OS_20),
    functions: FUNCTIONS,
    ..Device::new(0xc0de, 0xcafe)
};
bedrock_usb::descriptors!(pub Desc = DEVICE);
bedrock_usb::state!(pub static USB_STATE: Desc);

/// String index of the WireWeaver interface's API id: the first interface with a string.
const STRING_API_ID: u8 = STRING_FIRST_INTERFACE;
const _: () = assert!(matches!(DEVICE.string(STRING_API_ID), Some(Str::Runtime)));

/// The USB driver, created in `main.rs` and handed to [start].
pub type UsbDriver = bedrock_usb::v4::Driver<Desc, Mmio>;
type UsbDevice = Usb<Desc, Mmio, UsbControl>;
type WwTx = FramedTx<'static, bedrock_usb::ww::Sender<BulkIn<Mmio, EP_WW_IN, Double>, Duration>>;
type WwRx = FramedRx<'static, bedrock_usb::ww::Receiver<BulkOut<Mmio, EP_WW_OUT, Double>>>;
/// The WireWeaver server on the USB function.
pub type UsbServer = ww_device::Server<'static, WwTx, WwRx, EmbassyClock>;

/// Messages are held this long to share a frame; the host is told the same. Full speed, for throughput (what
/// `wire_weaver_usb_embassy::UsbTimings::fs_higher_speed` sets).
const ACCUMULATION_TIME: core::time::Duration = core::time::Duration::from_micros(1000);
/// A packet the host doesn't take within this time: the host is gone without disconnecting.
const PACKET_SEND_TIMEOUT: Duration = Duration::from_millis(250);

/// The server's packet and message buffers.
struct ServerBuffers {
    /// USB packets in, messages put together from them
    rx: RxBuffer<MAX_USB_PACKET_LEN, MAX_MESSAGE_LEN>,
    /// The packet being filled for sending
    tx: [u8; MAX_USB_PACKET_LEN],
    /// Replies and link messages
    scratch: [u8; MAX_MESSAGE_LEN],
    /// Events sent from handlers and through `server.sink()`
    event_scratch: [u8; MAX_MESSAGE_LEN],
}

static SERVER_BUFFERS: ConstStaticCell<ServerBuffers> = ConstStaticCell::new(ServerBuffers {
    rx: RxBuffer::new(),
    tx: [0; MAX_USB_PACKET_LEN],
    scratch: [0; MAX_MESSAGE_LEN],
    event_scratch: [0; MAX_MESSAGE_LEN],
});
static USB: StaticCell<UsbDevice> = StaticCell::new();
static SERVER_STATE: StaticCell<ServerState> = StaticCell::new();
/// `server_impl::API_ID` with the user label (API-19), as the WireWeaver interface's string descriptor.
static API_ID_LABELED: StaticCell<[u8; 256]> = StaticCell::new();

/// Takes the endpoints of the composite USB device (WireWeaver + CMSIS-DAP v2, `src/cmsis_dap.rs`) from `driver`,
/// connects it and spawns the USB, WireWeaver server and CMSIS-DAP tasks. `swd_host` is handed to the CMSIS-DAP
/// task, which is the sole owner of the SWD-host pins (FW-15). `last_reset` is what `last_reset::boot` read, served
/// as `device.last_reset`. `settings` is the open settings store (`src/settings.rs`): its boot values are applied
/// here.
pub async fn start(
    spawner: Spawner,
    driver: UsbDriver,
    swd_host: swd_host::SwdHost,
    last_reset: bedrock_crash::Snapshot,
    settings: (&'static mut settings::Store, &'static mut settings::User),
) {
    // in a static, not on the task's future: the observe tables and stream states are a few hundred bytes
    let state =
        SERVER_STATE.init(ServerState::new(embassy_stm32::uid::uid(), last_reset, settings).await);
    state.apply_boot_settings().await;

    // server_impl::API_ID is reported in a USB string descriptor, `ww list` shows it without opening the device,
    // with the label from the settings (as it was at boot: a new label shows after the next one)
    let api_id = wire_weaver::api_id::with_label(
        server_impl::API_ID,
        state.user.get().label.as_str(),
        API_ID_LABELED.init([0; 256]),
    );

    let ww_out = driver.bulk_out::<EP_WW_OUT, Double>();
    let ww_in = driver.bulk_in::<EP_WW_IN, Double>();
    #[cfg(feature = "swd-host")]
    let dap = cmsis_dap::CmsisDap::new(&driver);
    driver.connect();
    let usb = USB.init(Usb::new(
        driver,
        UsbControl {
            serial: crate::boot::usb_serial(),
            api_id,
            was_configured: false,
        },
    ));

    let mut link_cfg = link_config();
    link_cfg.accumulation_time = ACCUMULATION_TIME;
    let ServerBuffers {
        rx,
        tx,
        scratch,
        event_scratch,
    } = SERVER_BUFFERS.take();
    let server = ww_device::Server::new(
        link_cfg,
        FramedTx::new(bedrock_usb::ww::Sender::new(ww_in, PACKET_SEND_TIMEOUT), tx),
        FramedRx::new(
            bedrock_usb::ww::Receiver::new(ww_out),
            rx.assembly_buf(MAX_USB_PACKET_LEN),
        ),
        EmbassyClock,
        scratch,
        event_scratch,
    );

    spawner.spawn(unwrap!(usb_task(usb)));
    spawner.spawn(unwrap!(server_task(server, state)));
    #[cfg(feature = "swd-host")]
    spawner.spawn(unwrap!(cmsis_dap::dap_task(dap, swd_host)));
    // no CMSIS-DAP task to own it: the pins keep the safe state `SwdHost::new` set (V_SBU 3.3 V, shifter and
    // pull-ups off, SBU and CC Hi-Z); dropping it would hand them back to their reset state
    #[cfg(not(feature = "swd-host"))]
    core::mem::forget(swd_host);
}

/// The bus and EP0 (bedrock_usb's `Usb::run`); the class endpoints are served by their own tasks.
#[embassy_executor::task]
async fn usb_task(usb: &'static mut UsbDevice) -> ! {
    usb.run().await
}

/// Serves requests; every [PUMP_PERIOD](crate::log::PUMP_PERIOD) moves the defmt log into its store
/// (`src/log.rs`) and lets the settings store tidy up; sends what reliable streams have; sends observed properties
/// (SRV-18) when they are due: the task sleeps until `next_wake` or until something changes outside it
/// ([OBSERVE]: `src/power.rs`, the ADC task's meter readings and samples), no fixed tick (SRV-25). Observing stops
/// when the link goes down: a host that comes back asks again; the log stays in its store for a host that resumes
/// it. Also wakes for the deferred answers of `plug.vbus_cycle` (`src/power.rs`).
#[embassy_executor::task]
async fn server_task(mut server: UsbServer, state: &'static mut ServerState) {
    let mut buf = [0u8; OBSERVE_BUF_LEN];
    #[cfg(feature = "meter-samples")]
    let scratch = SCRATCH.init(ww_stream::batch::Scratch::new());
    let mut stream_buf = [0u8; STREAM_BUF_LEN];
    let mut next_pump = Instant::now() + crate::log::PUMP_PERIOD;
    loop {
        // nothing is observed while the link is down (reset at LinkEvent::Down)
        let observe_at = state
            .observe
            .next_wake(Instant::now().as_micros())
            .map(ww_device::Instant::from_micros);
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
                        state.pd_messages.detach();
                    }
                }
            }
            Either5::Second(()) => {
                next_pump += crate::log::PUMP_PERIOD;
                state.settings_idle().await;
            }
            Either5::Third(()) => {
                while let Some(done) = power::take_cycle_done() {
                    if let Some(reply_to) = done.reply_to
                        && server.is_up()
                        && ServerState::plug_vbus_cycle_send_return(
                            &mut server.sink(),
                            reply_to,
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
        // the PD source's records (woken through OBSERVE): sent, or counted as lost while no host is there
        state.pd_pump(&mut server).await;
        if server.is_up() {
            let now_us = Instant::now().as_micros();
            if state
                .observe
                .next_wake(now_us)
                .is_some_and(|at| at <= now_us)
                && state
                    .poll_properties(
                        &mut server.sink(),
                        (),
                        now_us,
                        &mut buf,
                        #[cfg(feature = "meter-samples")]
                        scratch,
                    )
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
