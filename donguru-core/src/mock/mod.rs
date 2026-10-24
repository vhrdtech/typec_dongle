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
//! - `device`: variant, board, UID, label with notes, settings (in RAM; `settings_fault` makes the next write or
//!   `reset` answer `Fault { code }`, as the real flash store does, CLI-47); last reset (cause, message, boot
//!   count, `clear`); counters and the log report their `INFO`/`DEFMT` (zero used words, no build id, no session),
//!   their RPCs (`read`, `reset`, `watch`, the log stream) answer `Unimplemented` until API-10/the real counter
//!   layout land in the firmware.
//! - `leds`: mode, brightness, identify (`Unsupported` above 60 s).
//! - `meter`: the reading, temperature and totals the test puts into the state; window 1 ms..=1 s.
//! - `plug` / `header`: `vbus_mode` with the guard against the meter's `vbus_in` (above the mode's `max_voltage`
//!   the switch is `Tripped(OverVoltage)`; a `max_voltage` above 21 V is `InvalidArgument`), `vbus_cycle`
//!   (`WrongState` in `ForceOff`, `OverVoltage` above the limit), data, orientation, relay.
//! - `sbu`: function, release, pull-ups (`InUse` while SWD holds SBU), `route_usb` (`Unsupported` on Main).
//! - `inputs`, `hub`, `swd`: state, button action, recover, mode.
//! - `debug`: a register model of the pins under the firmware's own rules (`donguru_board::pins`: refused pins,
//!   the claim table with what runs at boot and an SWD session, the electrical rules), hub registers (`hub_write`
//!   refuses STATUS/COMMAND), UCPD roles with the real UCPD_CR bits (`Rp` on UCPD1 refused, rev A), `reg_read` in
//!   the firmware's peripheral blocks (GPIO from the model, other words answer their address).
//! - `uart[1]` (the header): the `tx` credit sink takes 2048 bytes ahead and loops them back to the counted `rx`
//!   stream, as a jumper between IO7 and IO8 does on the firmware.
//! - the rest of `uart[]`, `i2c[]`, `gpio[]`, `adc` and the streams answer `Unimplemented`, as the firmware does until their
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
use ww_si::{Ampere, Baud, Coulomb, DegreeCelsius, Joule, Prefix, Second, Volt};

use donguru_board::pins::{
    Change, Claims, Denied, Holder, MODE_ANALOG, MODE_INPUT, MODE_OUTPUT, PORTS, Port, Ports,
    bonded, check, port_index, reset_state,
};
use donguru_board::ucpd::{self, role_fields};

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
    /// Targets of the `device.boot.reboot` calls, in call order (API-58); the mock does not reset.
    pub reboots: Vec<ww_dfu::BootTarget>,
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
    /// GPIOA..F, 16 pins each; `input` (IDR) is what the pin reads.
    pub pins: [[PinState; 16]; 6],
    /// Functions holding pins, as the firmware's table ([Claims::boot] at start); an SWD session holds its pins
    /// on top while `swd_session` is `Connected`.
    pub claims: Claims,
    /// The hub's SMBus registers.
    pub hub_regs: Vec<u8>,
    pub ucpd_role: [CcRole; 2],
    /// `debug.led_set`, by [Led] (power, status).
    pub leds: [LedMode; 2],
    /// A test sets this to make the next `device.settings` write (or `reset`) answer
    /// `CommonError::Fault { code }` instead of applying, as the real settings store does on a flash failure (1),
    /// a newer firmware's record (2) or a record too big (3).
    pub settings_fault: Option<u32>,
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
            reboots: Vec::new(),
            settings: MockSettings::default(),
            leds_mode: ww_indication::Mode::Normal,
            leds_brightness: ww_indication::Brightness::Normal,
            identified: 0,
            reading: MeterReading {
                vbus_in: Volt::new(Prefix::Milli, 5_000i32),
                current: Ampere::new(Prefix::Milli, 0i32),
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
            pins: pins_of(&reset_state()),
            claims: Claims::boot(),
            hub_regs: (0..=255).collect(),
            ucpd_role: [CcRole::Off, CcRole::Off],
            leds: [LedMode::Off, LedMode::Off],
            settings_fault: None,
        }
    }
}

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

    fn pin(&self, pin: McuPin) -> PinState {
        self.pins[port_index(pin.port)][pin.pin as usize & 15]
    }

    /// The pins as the rules see them (IDR left out).
    fn ports(&self) -> Ports {
        PORTS.map(|port| {
            let pins = &self.pins[port_index(port)];
            let field = |bits: u32, f: &dyn Fn(&PinState) -> u32| {
                pins.iter()
                    .enumerate()
                    .fold(0, |acc, (i, p)| acc | f(p) << (bits * i as u32))
            };
            Port {
                moder: field(2, &|p| p.moder as u32),
                otyper: field(1, &|p| p.open_drain as u32),
                ospeedr: field(2, &|p| p.speed as u32),
                pupdr: field(2, &|p| p.pull as u32),
                odr: field(1, &|p| p.output as u32),
                afrl: (0..8).fold(0, |acc, i| acc | (pins[i].af as u32) << (4 * i)),
                afrh: (8..16).fold(0, |acc, i| acc | (pins[i].af as u32) << (4 * (i - 8))),
            }
        })
    }

    /// The claim table the firmware has now: an SWD session holds its pins while connected, and the plug CC
    /// pins (`SwdCc`) when UCPD2 doesn't hold them, as `SwdHost::connected` claims them.
    fn claims_now(&self) -> Claims {
        let mut claims = self.claims;
        if matches!(self.swd_session, SwdSession::Connected { .. })
            && claims.claim(Holder::Swd).is_ok()
        {
            let _ = claims.claim(Holder::SwdCc);
        }
        claims
    }

    /// `pin_get`: a pin on the LQFP48.
    fn pin_get(&self, pin: McuPin) -> core::result::Result<PinState, Denied> {
        if pin.pin > 15 || !bonded(pin) {
            return Err(Denied::Invalid {
                arg: 0,
                reason: "not a pin of the LQFP48",
            });
        }
        Ok(self.pin(pin))
    }

    /// `pin_set` / `pin_af` under the firmware's rules ([donguru_board::pins::check]).
    fn pin_change(
        &mut self,
        pin: McuPin,
        change: Change,
    ) -> core::result::Result<PinState, Denied> {
        let mut ports = self.ports();
        let write = check(pin, change, &self.claims_now(), &ports)?;
        let port = port_index(pin.port);
        write.apply(pin.pin, &mut ports[port]);
        let input = self.pins[port].map(|p| p.input);
        self.pins[port] = pins_of(&ports)[port];
        for (p, was) in self.pins[port].iter_mut().zip(input) {
            p.input = was;
        }
        let state = &mut self.pins[port][pin.pin as usize];
        // what the pin reads: an output its level, a pulled input its pull; a floating one keeps what it read
        state.input = match (state.moder, state.pull) {
            (MODE_OUTPUT, _) => state.output,
            (MODE_INPUT, 1) => true,
            (MODE_INPUT, 2) | (MODE_ANALOG, _) => false,
            _ => state.input,
        };
        Ok(*state)
    }

    fn dump(&self) -> Vec<PortRegs> {
        let ports = self.ports();
        PORTS
            .iter()
            .map(|&port| {
                let r = &ports[port_index(port)];
                let idr = self.pins[port_index(port)]
                    .iter()
                    .enumerate()
                    .fold(0, |acc, (i, p)| acc | (p.input as u32) << i);
                PortRegs {
                    port,
                    moder: r.moder,
                    otyper: r.otyper,
                    ospeedr: r.ospeedr,
                    pupdr: r.pupdr,
                    idr,
                    odr: r.odr,
                    afrl: r.afrl,
                    afrh: r.afrh,
                }
            })
            .collect()
    }

    /// The pins as captured at boot: every one at reset, IDR 0.
    fn reset_dump() -> Vec<PortRegs> {
        let reset = reset_state();
        PORTS
            .iter()
            .map(|&port| {
                let r = &reset[port_index(port)];
                PortRegs {
                    port,
                    moder: r.moder,
                    otyper: r.otyper,
                    ospeedr: r.ospeedr,
                    pupdr: r.pupdr,
                    idr: 0,
                    odr: r.odr,
                    afrl: r.afrl,
                    afrh: r.afrh,
                }
            })
            .collect()
    }

    /// `reg_read`: GPIOA..F from the pin model, every other word its own address.
    fn reg(&self, addr: u32) -> u32 {
        if !(0x5000_0000..0x5000_1800).contains(&addr) {
            return addr;
        }
        let dump = self.dump();
        let r = &dump[((addr - 0x5000_0000) / 0x400) as usize];
        match addr & 0x3FF {
            0x00 => r.moder,
            0x04 => r.otyper,
            0x08 => r.ospeedr,
            0x0C => r.pupdr,
            0x10 => r.idr,
            0x14 => r.odr,
            0x20 => r.afrl,
            0x24 => r.afrh,
            _ => 0,
        }
    }

    fn ucpd(&self, port: UcpdPort) -> UcpdState {
        let role = self.ucpd_role[port as usize];
        // the UCPD_CR bits the firmware writes (RM0444 §34.7.2): CCENABLE 11:10, ANAMODE 9, ANASUBMODE 8:7
        let cr = match role_fields(port, role, false) {
            Ok(f) => {
                (if f.enabled { 3 << 10 } else { 0 }) | (f.sink as u32) << 9 | (f.rp as u32) << 7
            }
            Err(_) => 0,
        };
        UcpdState {
            cr,
            sr: 0,
            cc1: 0,
            cc2: 0,
        }
    }

    /// `ucpd_role`: refused as on the firmware (rev A: no `Rp` on UCPD1), the CC pins claimed while a role is on.
    fn ucpd_role(
        &mut self,
        port: UcpdPort,
        role: CcRole,
    ) -> core::result::Result<UcpdState, Denied> {
        // the mock is a B153A
        let fields = role_fields(port, role, true)?;
        let holder = ucpd::holder(port);
        if fields.enabled {
            // against what holds pins now, an SWD session included
            self.claims_now().claim(holder).map_err(Denied::InUse)?;
            self.claims.claim(holder).map_err(Denied::InUse)?;
        } else {
            self.claims.release(holder);
        }
        self.ucpd_role[port as usize] = role;
        Ok(self.ucpd(port))
    }
}

/// The API error of a refused change, as the firmware answers it.
fn denied(d: Denied) -> DebugError<'static> {
    DebugError::Common(match d {
        Denied::Invalid { arg, reason } => CommonError::InvalidArgument {
            arg,
            reason: MaxLen::new_unchecked(reason),
        },
        Denied::Refused(reason) => CommonError::Refused {
            reason: MaxLen::new_unchecked(reason),
        },
        Denied::InUse(holder) => CommonError::InUse {
            by: MaxLen::new_unchecked(holder.name()),
        },
    })
}

/// Pins of the rules' register view, IDR 0.
fn pins_of(ports: &Ports) -> [[PinState; 16]; 6] {
    core::array::from_fn(|port| {
        let r = &ports[port];
        core::array::from_fn(|pin| {
            let pin = pin as u8;
            PinState {
                moder: r.mode(pin),
                open_drain: r.open_drain(pin),
                speed: r.speed(pin),
                pull: r.pull(pin),
                af: r.af(pin),
                input: false,
                output: r.output(pin),
            }
        })
    })
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
                                backend.server.uart_rx.detach();
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
