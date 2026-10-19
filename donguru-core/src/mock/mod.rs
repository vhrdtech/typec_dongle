//! A Donguru in the same process (CORE-3): the generated `DonguruApi` server on [ww_device::Server], reached through
//! wire_weaver_client's `in_process` transport. The host side is exactly the one used over USB (link setup, version
//! check, timeouts, observe), so the CLI, the GUI and the Python bindings are tested against it instead of a
//! hand-written transport trait.
//!
//! ```no_run
//! # async fn demo() -> donguru_core::Result<()> {
//! let mock = donguru_core::mock::MockDevice::start("tests/plug");
//! let dongle = mock.open().await?;
//! assert_eq!(dongle.device().variant().await?, donguru_core::api::BoardVariant::Main);
//! # Ok(()) }
//! ```
//!
//! What the mock models ([MockState]), with the rules the firmware keeps:
//! - `device`: variant, board, UID, label with notes, settings (in RAM), last reset (cause, message, boot count,
//!   `clear`); counters and the log report their `INFO`/`DEFMT` (zero used words, no build id, no session), their
//!   RPCs (`read`, `reset`, `watch`, the log stream) answer `Unimplemented` until API-10/the real counter layout
//!   land in the firmware.
//! - `leds`: mode, brightness, identify (`Unsupported` above 60 s).
//! - `meter`: the reading, temperature and totals the test puts into the state; window 1 ms..=1 s.
//! - `plug` / `header`: `vbus_mode` with the guard against the meter's `vbus_in` (above the mode's `max_voltage`
//!   the switch is `Tripped(OverVoltage)`; a `max_voltage` above 21 V is `InvalidArgument`), `vbus_cycle`
//!   (`WrongState` in `ForceOff`, `OverVoltage` above the limit), data, orientation, relay.
//! - `sbu`: function, release, pull-ups (`InUse` while SWD holds SBU), `route_usb` (`Unsupported` on Main).
//! - `inputs`, `hub`, `swd`: state, button action, recover, mode.
//! - `debug`: a register model of the pins with the firmware's refused pins, hub registers, UCPD roles (`Rp` on
//!   UCPD1 refused), `reg_read` in peripheral space only.
//! - `uart[]`, `i2c[]`, `gpio[]`, `adc` and the streams answer `Unimplemented`, as the firmware does until their
//!   pieces land; the arrays have their shape (`uart[0..2]`, `gpio[0..2].pin[0..4]`).

mod server;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use donguru_api::*;
use tokio::sync::{mpsc, oneshot};
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use wire_weaver::{EventWriter, MessageSink, WireWeaverAsyncApiBackend};
use wire_weaver_client::in_process::{self, TokioClock};
use ww_firmware_info::ResetCause;
use ww_si::{Ampere, Baud, Coulomb, DegreeCelsius, Joule, Prefix, Second, Volt, Watt};

use crate::device::DeviceEntry;
use crate::{Dongle, Result};

pub use server::MockServer;

/// Longest message of the mock's link, as the firmware's.
const MAX_MESSAGE_LEN: usize = 1024;
/// Biggest `PropertyUpdate` event, as the firmware's.
const OBSERVE_BUF_LEN: usize = 256;

/// Settings of `device.settings`, kept in RAM by the mock.
#[derive(Clone, Debug)]
pub struct MockSettings {
    pub plug_vbus_at_boot: VbusMode,
    pub button_action: ButtonAction,
    pub meter_window: Second,
    pub meter_deadband: MeterDeadband,
    pub uart_plug: ww_uart::UartConfig,
}

impl Default for MockSettings {
    fn default() -> Self {
        MockSettings {
            plug_vbus_at_boot: VbusMode::ForceOff,
            button_action: ButtonAction::Off,
            meter_window: Second::new(Prefix::Milli, 10i32),
            meter_deadband: default_deadband(),
            uart_plug: ww_uart::UartConfig {
                baud_rate: Baud::new(Prefix::Unit, 115_200u32),
                data_bits: ww_uart::DataBits::Eight,
                parity: ww_uart::Parity::None,
                stop_bits: ww_uart::StopBits::Stop1,
                mode: ww_uart::Mode::Asynchronous,
                prevent_back_feed: true,
                io_level: None,
                idle_chars: ww_uart::IDLE_CHARS_DEFAULT,
            },
        }
    }
}

fn default_deadband() -> MeterDeadband {
    MeterDeadband {
        voltage: Volt::new(Prefix::Milli, 20i32),
        current: Ampere::new(Prefix::Milli, 10i32),
    }
}

/// The dongle the mock models. Tests change it with [MockDevice::with].
#[derive(Clone, Debug)]
pub struct MockState {
    pub variant: BoardVariant,
    /// The chip UID; [serial](MockState::serial) is its hex.
    pub uid: [u8; 12],
    pub serial: String,
    pub label: String,
    pub label_notes: String,
    /// `device.last_reset`: cause, message and boots-since-crash a test can set, `clear()` resets them (API-9).
    pub last_reset_cause: ResetCause,
    pub last_reset_message: String,
    pub last_reset_boots_since_crash: u32,
    /// Boots since `clear()`.
    pub boot_count: u32,
    pub settings: MockSettings,
    pub leds_mode: ww_indication::Mode,
    pub leds_brightness: ww_indication::Brightness,
    /// `leds.identify` calls.
    pub identified: u32,
    pub reading: MeterReading,
    pub temperature: DegreeCelsius,
    pub totals: MeterTotals,
    pub meter_window: Second,
    pub meter_deadband: MeterDeadband,
    pub calibration: MeterCalibration,
    pub plug_vbus: VbusState,
    pub plug_vbus_mode: VbusMode,
    /// `plug.vbus_cycle` calls that switched VBUS.
    pub vbus_cycles: u32,
    pub plug_data: DataPath,
    pub orientation: Orientation,
    pub cc: CcState,
    pub header_vbus: VbusState,
    pub header_vbus_mode: VbusMode,
    pub relay: bool,
    pub sbu_function: SbuFunction,
    pub sbu_level: Volt,
    pub sbu_pullups: bool,
    pub button: ButtonState,
    pub switch: SwitchPosition,
    pub button_action: ButtonAction,
    pub hub: HubState,
    pub swd_mode: SwdMode,
    pub swd_session: SwdSession,
    /// GPIOA..F, 16 pins each.
    pub pins: [[PinState; 16]; 6],
    /// The hub's SMBus registers.
    pub hub_regs: Vec<u8>,
    pub ucpd_role: [CcRole; 2],
    /// `debug.led_set`, by [Led] (power, status).
    pub leds: [LedMode; 2],
}

impl Default for MockState {
    /// A B153A Main at boot: everything off, data connected, SBU Hi-Z, 5 V on the receptacle, nothing drawn.
    fn default() -> Self {
        let uid = [
            0x00, 0x49, 0x00, 0x2A, 0x31, 0x33, 0x51, 0x0B, 0x37, 0x36, 0x34, 0x36,
        ];
        MockState {
            variant: BoardVariant::Main,
            uid,
            serial: uid.iter().map(|b| format!("{b:02X}")).collect(),
            label: String::new(),
            label_notes: String::new(),
            last_reset_cause: ResetCause::PowerOn,
            last_reset_message: String::new(),
            last_reset_boots_since_crash: 0,
            boot_count: 1,
            settings: MockSettings::default(),
            leds_mode: ww_indication::Mode::Normal,
            leds_brightness: ww_indication::Brightness::Normal,
            identified: 0,
            reading: MeterReading {
                vbus_in: Volt::new(Prefix::Milli, 5_000i32),
                current: Ampere::new(Prefix::Milli, 0i32),
                power: Watt::new(Prefix::Milli, 0i32),
            },
            temperature: DegreeCelsius::new(Prefix::Unit, 30i32),
            totals: MockState::zero_totals(),
            meter_window: Second::new(Prefix::Milli, 10i32),
            meter_deadband: default_deadband(),
            calibration: MeterCalibration {
                calibrated: false,
                vbus_in_offset: Volt::new(Prefix::Milli, 0i32),
                vbus_in_gain_ppm: 0,
                current_offset: Ampere::new(Prefix::Milli, 0i32),
                current_gain_ppm: 0,
            },
            plug_vbus: VbusState::Off,
            plug_vbus_mode: VbusMode::ForceOff,
            vbus_cycles: 0,
            plug_data: DataPath::Connected,
            orientation: Orientation::Auto,
            cc: CcState {
                polarity: CcPolarity::Unknown,
                pull: CcPull::Unknown,
            },
            header_vbus: VbusState::Off,
            header_vbus_mode: VbusMode::ForceOff,
            relay: false,
            sbu_function: SbuFunction::Off,
            sbu_level: Volt::new(Prefix::Milli, 3_300i32),
            sbu_pullups: false,
            button: ButtonState {
                pressed: false,
                presses: 0,
                last_held: Second::new(Prefix::Milli, 0i32),
            },
            switch: SwitchPosition::A,
            button_action: ButtonAction::Off,
            hub: HubState {
                phase: HubPhase::Attached,
                recoveries: 0,
                last_cause: None,
            },
            swd_mode: SwdMode::BitBang,
            swd_session: SwdSession::Idle,
            pins: [[RESET_PIN; 16]; 6],
            hub_regs: (0..=255).collect(),
            ucpd_role: [CcRole::Off, CcRole::Off],
            leds: [LedMode::Off, LedMode::Off],
        }
    }
}

/// A pin after reset: analog, no pull.
const RESET_PIN: PinState = PinState {
    moder: 3,
    open_drain: false,
    speed: 0,
    pull: 0,
    af: 0,
    input: false,
    output: false,
};

const PORTS: [GpioPort; 6] = [
    GpioPort::A,
    GpioPort::B,
    GpioPort::C,
    GpioPort::D,
    GpioPort::E,
    GpioPort::F,
];

impl MockState {
    pub fn zero_totals() -> MeterTotals {
        MeterTotals {
            charge: Coulomb::new(Prefix::Milli, 0i32),
            energy: Joule::new(Prefix::Milli, 0i32),
            peak_current: Ampere::new(Prefix::Milli, 0i32),
            since: Second::new(Prefix::Milli, 0i32),
        }
    }

    /// What a VBUS switch does in `mode` with the receptacle at `reading.vbus_in`.
    fn apply(&self, mode: &VbusMode) -> VbusState {
        match mode {
            VbusMode::ForceOff => VbusState::Off,
            _ if self.reading.vbus_in.to_f64() > max_voltage(mode) => {
                VbusState::Tripped(GuardTrip::OverVoltage)
            }
            _ => VbusState::On,
        }
    }

    fn pin_mut(&mut self, pin: McuPin) -> &mut PinState {
        &mut self.pins[pin.port as usize][pin.pin as usize & 15]
    }

    fn dump(&self) -> Vec<PortRegs> {
        PORTS
            .iter()
            .map(|&port| {
                let pins = &self.pins[port as usize];
                let field = |bits: u32, f: &dyn Fn(&PinState) -> u32| {
                    pins.iter()
                        .enumerate()
                        .fold(0, |acc, (i, p)| acc | f(p) << (bits * i as u32))
                };
                PortRegs {
                    port,
                    moder: field(2, &|p| p.moder as u32),
                    otyper: field(1, &|p| p.open_drain as u32),
                    ospeedr: field(2, &|p| p.speed as u32),
                    pupdr: field(2, &|p| p.pull as u32),
                    idr: field(1, &|p| p.input as u32),
                    odr: field(1, &|p| p.output as u32),
                    afrl: (0..8).fold(0, |acc, i| acc | (pins[i].af as u32) << (4 * i)),
                    afrh: (8..16).fold(0, |acc, i| acc | (pins[i].af as u32) << (4 * (i - 8))),
                }
            })
            .collect()
    }

    /// The pins as captured at boot: every one at reset.
    fn reset_dump() -> Vec<PortRegs> {
        MockState::default().dump()
    }

    fn ucpd(&self, port: UcpdPort) -> UcpdState {
        let role = self.ucpd_role[port as usize];
        UcpdState {
            // ANAMODE / ANASUBMODE stand-in: the role as a number
            cr: match role {
                CcRole::Off => 0,
                CcRole::Rd => 1,
                CcRole::Rp0A9 => 2,
                CcRole::Rp1A5 => 3,
                CcRole::Rp3A0 => 4,
                _ => 0xff,
            },
            sr: 0,
            cc1: 0,
            cc2: 0,
        }
    }
}

/// `max_voltage` of a mode in volts, 0 for `ForceOff`.
fn max_voltage(mode: &VbusMode) -> f64 {
    match mode {
        VbusMode::ForceOn { max_voltage } | VbusMode::Auto { max_voltage, .. } => {
            max_voltage.to_f64()
        }
        _ => 0.0,
    }
}

/// The firmware refuses a limit no USB source gives (above 21 V, below 0).
fn check_mode(mode: &VbusMode) -> core::result::Result<(), CommonError<'static>> {
    match mode {
        VbusMode::ForceOff => Ok(()),
        VbusMode::ForceOn { .. } | VbusMode::Auto { .. }
            if (0.0..=21.0).contains(&max_voltage(mode)) =>
        {
            Ok(())
        }
        VbusMode::ForceOn { .. } | VbusMode::Auto { .. } => Err(CommonError::InvalidArgument {
            arg: 0,
            reason: MaxLen::new_unchecked("max_voltage 0..=21 V"),
        }),
        #[allow(unreachable_patterns)] // a mode from a newer host
        _ => Err(CommonError::InvalidArgument {
            arg: 0,
            reason: MaxLen::new_unchecked("unknown mode"),
        }),
    }
}

fn check_window(window: Second) -> core::result::Result<(), CommonError<'static>> {
    if (0.001..=1.0).contains(&window.to_f64()) {
        Ok(())
    } else {
        Err(CommonError::InvalidArgument {
            arg: 0,
            reason: MaxLen::new_unchecked("window 1 ms..=1 s"),
        })
    }
}

fn invalid_pin() -> DebugError<'static> {
    DebugError::Common(CommonError::InvalidArgument {
        arg: 0,
        reason: MaxLen::new_unchecked("pin 0..=15"),
    })
}

/// The pins the firmware never hands to `debug` (design: Refused pins): NRST, supply, our USB and SWD, PA15, the
/// VBUS enables, the hub reset and I2C.
fn check_pin(pin: McuPin) -> core::result::Result<(), DebugError<'static>> {
    use GpioPort::*;
    if pin.pin > 15 {
        return Err(invalid_pin());
    }
    let refused = match (pin.port, pin.pin) {
        (F, 2) => "NRST",
        (C, 6) | (C, 7) => "supply pin",
        (A, 11) | (A, 12) => "our USB",
        (A, 13) | (A, 14) => "our SWD",
        (A, 15) => "CC_DA_EN",
        (A, 6) | (D, 1) => "VBUS enable: use plug / header",
        (C, 13) | (A, 4) | (B, 10) | (B, 14) => "hub: use hub_read / hub_write",
        _ => return Ok(()),
    };
    Err(DebugError::Common(CommonError::Refused {
        reason: MaxLen::new_unchecked(refused),
    }))
}

fn set_pin_mode(state: &mut PinState, mode: PinMode) {
    let (moder, open_drain, pull) = match mode {
        PinMode::Analog => (3, false, 0),
        PinMode::Input => (0, false, 0),
        PinMode::InputPullUp => (0, false, 1),
        PinMode::InputPullDown => (0, false, 2),
        PinMode::PushPull => (1, false, 0),
        PinMode::OpenDrain => (1, true, 0),
        PinMode::OpenDrainPullUp => (1, true, 1),
        #[allow(unreachable_patterns)] // a mode from a newer host
        _ => (state.moder, state.open_drain, state.pull),
    };
    state.moder = moder;
    state.open_drain = open_drain;
    state.pull = pull;
}

type Job = Box<dyn FnOnce(&mut MockServer) + Send>;

/// A running mock dongle, stopped when dropped. Each needs its own `path` (one per test).
pub struct MockDevice {
    path: String,
    jobs: mpsc::UnboundedSender<Job>,
    drop_requests: Arc<AtomicBool>,
}

impl MockDevice {
    /// Starts a mock in the state of a dongle at boot ([MockState::default]).
    pub fn start(path: &str) -> MockDevice {
        MockDevice::start_with(path, MockState::default())
    }

    /// Starts a mock in `state`, on a thread of its own.
    pub fn start_with(path: &str, state: MockState) -> MockDevice {
        let (jobs, mut job_rx) = mpsc::unbounded_channel::<Job>();
        let drop_requests = Arc::new(AtomicBool::new(false));
        // registered before the thread runs, so a connect right after start finds it
        let (tx, rx) = in_process::device(path, MAX_MESSAGE_LEN);
        let mut backend = Backend {
            server: MockServer::new(state),
            drop_requests: drop_requests.clone(),
        };
        std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async move {
                let mut scratch = [0u8; MAX_MESSAGE_LEN];
                let mut event_scratch = [0u8; MAX_MESSAGE_LEN];
                let mut observe_buf = [0u8; OBSERVE_BUF_LEN];
                let mut observe_scratch = Box::new(ww_stream::batch::Scratch::<
                    { ww_stream::batch::scratch_bytes::<donguru_api::MeterReading>(server::api_impl::OBSERVE_BATCH) },
                >::new());
                let config = ww_device::LinkConfig::new(
                    DONGURU_API_FULL_GID,
                    server::api_impl::api_hash(),
                    ww_client_server::COMPACT_VERSION,
                );
                let clock = TokioClock::new();
                let started = tokio::time::Instant::now();
                let mut server =
                    ww_device::Server::new(config, tx, rx, clock, &mut scratch, &mut event_scratch);
                let mut tick = tokio::time::interval(std::time::Duration::from_millis(1));
                loop {
                    tokio::select! {
                        ready = server.wait() => {
                            if let Some(ww_device::LinkEvent::Down(_)) = server.handle(ready, &mut backend).await {
                                backend.server.observe.reset();
                            }
                        }
                        _ = tick.tick() => {
                            if server.is_up() && backend.server.observe.budget_in_use() > 0 {
                                let now_us = started.elapsed().as_micros() as u64;
                                backend.server.sample_meter(now_us);
                                _ = backend.server.poll_properties(&mut server.sink(), (), now_us, &mut observe_buf, &mut observe_scratch).await;
                            }
                        }
                        job = job_rx.recv() => match job {
                            Some(job) => job(&mut backend.server),
                            None => break, // MockDevice dropped
                        }
                    }
                }
            });
        });
        MockDevice {
            path: path.to_string(),
            jobs,
            drop_requests,
        }
    }

    /// The entry to open it with [Dongle::open_entry].
    pub fn entry(&self) -> DeviceEntry {
        DeviceEntry::in_process(&self.path)
    }

    /// Opens the mock as a [Dongle].
    pub async fn open(&self) -> Result<Dongle> {
        Dongle::open_entry(self.entry()).await
    }

    /// Runs `f` on the mock's state in its thread and returns what it returns. Observed properties are marked
    /// changed afterwards, so observers see what `f` did.
    pub async fn with<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut MockState) -> R + Send + 'static,
    ) -> R {
        let (done, result) = oneshot::channel();
        let job: Job = Box::new(move |server| {
            let r = f(&mut server.state);
            server.mark_all_changed();
            _ = done.send(r);
        });
        self.jobs
            .send(job)
            .expect("the mock's thread runs while MockDevice lives");
        result
            .await
            .expect("the mock's thread runs while MockDevice lives")
    }

    /// A copy of the state.
    pub async fn state(&self) -> MockState {
        self.with(|s| s.clone()).await
    }

    /// From now on ignore every request without an answer, as a stuck device.
    pub fn drop_requests(&self, drop: bool) {
        self.drop_requests.store(drop, Ordering::Relaxed);
    }
}

struct Backend {
    server: MockServer,
    drop_requests: Arc<AtomicBool>,
}

impl WireWeaverAsyncApiBackend for Backend {
    type Medium = ();

    async fn process_bytes<'a>(
        &mut self,
        out: &mut EventWriter<'_, impl MessageSink>,
        medium: (),
        data: &[u8],
        scratch: &'a mut [u8],
    ) -> core::result::Result<&'a [u8], ShrinkWrapError> {
        if self.drop_requests.load(Ordering::Relaxed) {
            return Ok(&[]);
        }
        self.server
            .process_request_bytes(data, scratch, out, medium)
            .await
    }

    fn version(&self) -> wire_weaver::ww_version::FullVersion<'_> {
        DONGURU_API_FULL_GID
    }
}
