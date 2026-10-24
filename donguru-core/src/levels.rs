//! Typed facade over the generated client, one struct per level of the API tree (`dongle.plug()`,
//! `dongle.meter()`, ...): every item as an async method with [crate::Error], so the CLI, the GUI and the Python
//! bindings handle `Unimplemented`, device errors and link failures the same way.
//!
//! Levels made of a stdlib trait (`uart[]`, `i2c[]`, `gpio[]`, `adc`, `leds`, the contract under `device`) have the
//! calls Donguru's tools use here, and the generated client of the level through `raw()` for everything else.

use std::fmt;
use std::time::Duration;

use donguru_api::{
    BoardVariant, ButtonAction, ButtonState, CcRole, CcState, DataPath, DebugErrorOwned, HubState,
    Led, LedMode, McuPin, MeterCalibration, MeterDeadband, MeterReading, MeterTotals, Orientation,
    PinDumpOwned, PinMode, PinState, PowerErrorOwned, SbuErrorOwned, SbuFunction, SwdMode,
    SwdSession, SwitchPosition, UcpdPort, UcpdState, VbusMode, VbusState,
};
use wire_weaver::prelude::DeserializeShrinkWrapOwned;
use wire_weaver::shrink_wrap::MaxLen;
use wire_weaver::ww_error::{CommonErrorOwned, HasCommon};
use wire_weaver_client::{CallError, PreparedCall, PreparedRead, PreparedWrite};
use ww_si::{DegreeCelsius, Second, Volt};

use crate::client::DonguruClient;
use crate::client::generated::donguru_api_donguru_api as generated;
use crate::client::observe::*;
use crate::{Error, Result};

pub(crate) async fn read<T: DeserializeShrinkWrapOwned + fmt::Debug>(
    item: &str,
    p: PreparedRead<T>,
) -> Result<T> {
    p.read().await.map_err(|e| Error::client(item, e))
}

async fn write_plain(item: &str, p: PreparedWrite) -> Result<()> {
    p.write().await.map_err(|e| Error::client(item, e))
}

async fn write<E>(item: &str, p: PreparedWrite<CallError<E>>) -> Result<()>
where
    E: DeserializeShrinkWrapOwned
        + HasCommon<Common = CommonErrorOwned>
        + fmt::Display
        + fmt::Debug
        + Send
        + Sync,
    E: 'static,
{
    p.write().await.map_err(|e| Error::call(item, e))
}

pub(crate) async fn call_plain<T: DeserializeShrinkWrapOwned>(
    item: &str,
    p: PreparedCall<T>,
) -> Result<T> {
    p.call().await.map_err(|e| Error::client(item, e))
}

pub(crate) async fn call<T, E>(item: &str, p: PreparedCall<T, CallError<E>>) -> Result<T>
where
    T: DeserializeShrinkWrapOwned,
    E: DeserializeShrinkWrapOwned
        + HasCommon<Common = CommonErrorOwned>
        + fmt::Display
        + fmt::Debug
        + Send
        + Sync,
    E: 'static,
{
    p.call().await.map_err(|e| Error::call(item, e))
}

/// Subscribes to an observed level, see [crate::client::observe].
/// Bytes per second asked for `meter.reading` samples: ~1.6 k samples/s of about 4 bytes each, with room for a busy
/// DUT whose current changes every sample. The firmware grants up to 20 kB/s per level.
pub const METER_SAMPLES_BUDGET: u32 = 16_000;

async fn subscribe<U: wire_weaver_client::observe::LevelUpdate>(
    item: &str,
    p: PreparedObserve<U, U>,
) -> Result<Observer<U, U>> {
    p.subscribe().await.map_err(|e| Error::client(item, e))
}

macro_rules! level {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Copy, Clone)]
        pub struct $name<'a> {
            c: &'a DonguruClient,
        }

        impl<'a> $name<'a> {
            pub(crate) fn new(c: &'a DonguruClient) -> Self {
                Self { c }
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------------------------
// device

level!(
    /// `device`: the device contract (firmware, board, ids, label, counters, log) and the settings.
    Device
);

/// What `dg info` shows: the contract in one read. A part the firmware doesn't have yet is `None`.
#[derive(Clone, Debug)]
pub struct DeviceSummary {
    pub variant: BoardVariant,
    pub firmware: Option<ww_firmware_info::FirmwareSummaryOwned>,
    pub board: Option<ww_board_info::BoardSummaryOwned>,
    /// The STM32 96-bit UID, hex.
    pub uid: Option<String>,
    pub label: Option<String>,
    pub last_reset: Option<ww_firmware_info::ResetRecordOwned>,
}

/// `Ok(None)` for an item the firmware doesn't implement yet.
fn optional<T>(r: Result<T>) -> Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.is_unimplemented() => Ok(None),
        Err(e) => Err(e),
    }
}

impl<'a> Device<'a> {
    /// Board variant from the hub PID (FW-22).
    pub async fn variant(&self) -> Result<BoardVariant> {
        read("Device::variant", self.c.device().read_variant()).await
    }

    pub async fn firmware(&self) -> Result<ww_firmware_info::FirmwareSummaryOwned> {
        read(
            "FirmwareInfo::FIRMWARE",
            self.c.device().firmware().read_FIRMWARE(),
        )
        .await
    }

    pub async fn board(&self) -> Result<ww_board_info::BoardSummaryOwned> {
        read("BoardInfo::BOARD", self.c.device().board().read_BOARD()).await
    }

    /// The chip UID as hex, also the USB serial number.
    pub async fn uid(&self) -> Result<String> {
        let ids = read("Uid::IDS", self.c.device().uid().read_IDS()).await?;
        Ok(ids.first().map(|id| hex(&id.value)).unwrap_or_default())
    }

    pub async fn label(&self) -> Result<String> {
        let name = read("UserInfo::name", self.c.device().label().read_name()).await?;
        Ok(name.into_inner())
    }

    /// Sets the user label, at most 32 bytes.
    pub async fn set_label(&self, label: &str) -> Result<()> {
        let label = MaxLen::new(label.to_string())
            .map_err(|_| Error::Link(format!("label is longer than 32 bytes: {label:?}")))?;
        write("UserInfo::name", self.c.device().label().write_name(label)).await
    }

    /// Free-form notes about the unit; `Ok(None)` when the firmware has no room for them (`UserInfo::CAPABILITIES`).
    pub async fn notes(&self) -> Result<Option<String>> {
        optional(
            read("UserInfo::notes", self.c.device().label().read_notes())
                .await
                .map(|n| n.into_inner()),
        )
    }

    /// Sets the notes, at most 256 bytes.
    pub async fn set_notes(&self, notes: &str) -> Result<()> {
        let notes = MaxLen::new(notes.to_string())
            .map_err(|_| Error::Link(format!("notes are longer than 256 bytes: {notes:?}")))?;
        write(
            "UserInfo::notes",
            self.c.device().label().write_notes(notes),
        )
        .await
    }

    pub async fn last_reset(&self) -> Result<ww_firmware_info::ResetRecordOwned> {
        call_plain(
            "LastReset::last_reset",
            self.c.device().last_reset().last_reset(),
        )
        .await
    }

    /// Boots since the boot counter was cleared.
    pub async fn boot_count(&self) -> Result<u32> {
        call_plain(
            "LastReset::boot_count",
            self.c.device().last_reset().boot_count(),
        )
        .await
    }

    /// Forgets the crash record and zeroes the boot counter, after the host saved them.
    pub async fn clear_last_reset(&self) -> Result<()> {
        call_plain("LastReset::clear", self.c.device().last_reset().clear()).await
    }

    /// `device.counters`, decoded with the firmware's ELF found by its build id (CLI-19): [crate::counters].
    pub async fn counters(
        &self,
        elf: Option<&std::path::Path>,
        force: bool,
    ) -> Result<crate::counters::CountersReport> {
        crate::counters::report(self.c.device(), elf, force).await
    }

    /// `device.log`'s stored defmt frames so far, decoded with the firmware's ELF found by its build id (CLI-19):
    /// [crate::log].
    pub async fn log_dump(
        &self,
        elf: Option<&std::path::Path>,
        force: bool,
    ) -> Result<crate::log::LogReport> {
        crate::log::dump(self.c.device(), elf, force).await
    }

    /// Like [Device::log_dump], then keeps streaming new lines to `on_line` until the link drops or the device
    /// closes the log. `on_warning` runs once first if the ELF couldn't be checked or doesn't match (`--force`).
    pub async fn log_follow(
        &self,
        elf: Option<&std::path::Path>,
        force: bool,
        on_line: impl FnMut(&str),
        on_warning: impl FnOnce(&str),
    ) -> Result<()> {
        crate::log::follow(self.c.device(), elf, force, on_line, on_warning).await
    }

    /// Resets the dongle into `target`: `Updater` is the STM32 ROM bootloader (FW-12, API-58), `App` a plain restart.
    pub async fn reboot(&self, target: ww_dfu::BootTarget) -> Result<()> {
        match self.c.device().boot().reboot(target).call().await {
            Ok(()) => Ok(()),
            Err(CallError::Device(ww_dfu::Error::Unsupported)) => Err(Error::Unimplemented {
                item: "BootControl::reboot".into(),
            }),
            Err(CallError::Device(e)) => {
                Err(Error::Link(format!("BootControl::reboot refused: {e:?}")))
            }
            Err(CallError::Unimplemented { .. }) => Err(Error::Unimplemented {
                item: "BootControl::reboot".into(),
            }),
            Err(CallError::Client(e)) => Err(Error::client("BootControl::reboot", e)),
        }
    }

    /// Everything of the contract at once; parts the firmware doesn't implement yet are `None`.
    pub async fn summary(&self) -> Result<DeviceSummary> {
        Ok(DeviceSummary {
            variant: self.variant().await?,
            firmware: optional(self.firmware().await)?,
            board: optional(self.board().await)?,
            uid: optional(self.uid().await)?,
            label: optional(self.label().await)?,
            last_reset: optional(self.last_reset().await)?,
        })
    }

    pub fn settings(&self) -> Settings<'a> {
        Settings::new(self.c)
    }

    /// The generated client of `device`, for the stdlib contract traits (counters, log, ...).
    pub fn raw(&self) -> generated::donguru_api_device::DonguruApiDeviceClient<'a> {
        self.c.device()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

level!(
    /// `device.settings`: kept in flash (API-19).
    Settings
);

impl Settings<'_> {
    pub async fn plug_vbus_at_boot(&self) -> Result<VbusMode> {
        read(
            "Settings::plug_vbus_at_boot",
            self.c.device().settings().read_plug_vbus_at_boot(),
        )
        .await
    }

    pub async fn set_plug_vbus_at_boot(&self, mode: VbusMode) -> Result<()> {
        let p = self.c.device().settings().write_plug_vbus_at_boot(mode);
        write("Settings::plug_vbus_at_boot", p).await
    }

    pub async fn button_action(&self) -> Result<ButtonAction> {
        read(
            "Settings::button_action",
            self.c.device().settings().read_button_action(),
        )
        .await
    }

    pub async fn set_button_action(&self, action: ButtonAction) -> Result<()> {
        let p = self.c.device().settings().write_button_action(action);
        write("Settings::button_action", p).await
    }

    pub async fn meter_window(&self) -> Result<Second> {
        read(
            "Settings::meter_window",
            self.c.device().settings().read_meter_window(),
        )
        .await
    }

    pub async fn set_meter_window(&self, window: Second) -> Result<()> {
        let p = self.c.device().settings().write_meter_window(window);
        write("Settings::meter_window", p).await
    }

    pub async fn meter_deadband(&self) -> Result<MeterDeadband> {
        read(
            "Settings::meter_deadband",
            self.c.device().settings().read_meter_deadband(),
        )
        .await
    }

    pub async fn set_meter_deadband(&self, deadband: MeterDeadband) -> Result<()> {
        let p = self.c.device().settings().write_meter_deadband(deadband);
        write("Settings::meter_deadband", p).await
    }

    pub async fn uart_plug(&self) -> Result<ww_uart::UartConfig> {
        read(
            "Settings::uart_plug",
            self.c.device().settings().read_uart_plug(),
        )
        .await
    }

    pub async fn set_uart_plug(&self, config: ww_uart::UartConfig) -> Result<()> {
        let p = self.c.device().settings().write_uart_plug(config);
        write("Settings::uart_plug", p).await
    }

    /// Every setting and the label back to defaults; factory data stays.
    pub async fn reset(&self) -> Result<()> {
        call("Settings::reset", self.c.device().settings().reset()).await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// leds

level!(
    /// `leds`: the power and status LEDs as one `ww_indication::Indication`.
    Leds
);

impl<'a> Leds<'a> {
    pub async fn mode(&self) -> Result<ww_indication::Mode> {
        read("Indication::mode", self.c.leds().read_mode()).await
    }

    pub async fn set_mode(&self, mode: ww_indication::Mode) -> Result<()> {
        write_plain("Indication::mode", self.c.leds().write_mode(mode)).await
    }

    pub async fn brightness(&self) -> Result<ww_indication::Brightness> {
        read("Indication::brightness", self.c.leds().read_brightness()).await
    }

    pub async fn set_brightness(&self, brightness: ww_indication::Brightness) -> Result<()> {
        write_plain(
            "Indication::brightness",
            self.c.leds().write_brightness(brightness),
        )
        .await
    }

    /// Blinks the LEDs for `duration` so the dongle can be found on a bench.
    pub async fn identify(&self, duration: Second) -> Result<()> {
        call("Indication::identify", self.c.leds().identify(duration)).await
    }

    /// [identify](Self::identify) for `secs` seconds, sent as whole milliseconds (the firmware has no float numbers).
    pub async fn identify_secs(&self, secs: f64) -> Result<()> {
        self.identify(Second::new(
            ww_si::Prefix::Milli,
            (secs * 1000.0).round() as i32,
        ))
        .await
    }

    pub fn raw(&self) -> generated::ww_indication_indication::WwIndicationIndicationClient<'a> {
        self.c.leds()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// meter

level!(
    /// `meter`: the power meter on the receptacle VBUS, in SI units.
    Meter
);

impl Meter<'_> {
    /// One reading: VBUS_IN and current to the DUT, from the same ADC window; [crate::reading_power_w] for power.
    pub async fn reading(&self) -> Result<MeterReading> {
        read("PowerMeter::reading", self.c.meter().read_reading()).await
    }

    pub async fn temperature(&self) -> Result<DegreeCelsius> {
        read("PowerMeter::temperature", self.c.meter().read_temperature()).await
    }

    pub async fn totals(&self) -> Result<MeterTotals> {
        read("PowerMeter::totals", self.c.meter().read_totals()).await
    }

    pub async fn reset_totals(&self) -> Result<()> {
        call_plain("PowerMeter::reset_totals", self.c.meter().reset_totals()).await
    }

    pub async fn window(&self) -> Result<Second> {
        read("PowerMeter::window", self.c.meter().read_window()).await
    }

    pub async fn set_window(&self, window: Second) -> Result<()> {
        write("PowerMeter::window", self.c.meter().write_window(window)).await
    }

    pub async fn deadband(&self) -> Result<MeterDeadband> {
        read("PowerMeter::deadband", self.c.meter().read_deadband()).await
    }

    pub async fn set_deadband(&self, deadband: MeterDeadband) -> Result<()> {
        write(
            "PowerMeter::deadband",
            self.c.meter().write_deadband(deadband),
        )
        .await
    }

    pub async fn calibration(&self) -> Result<MeterCalibration> {
        read("PowerMeter::calibration", self.c.meter().read_calibration()).await
    }

    /// Observes `reading`, `temperature` and `totals` as `cfg` asks.
    pub async fn observe(&self, cfg: PowerMeterObserve) -> Result<Observer<PowerMeterUpdate>> {
        subscribe("PowerMeter", self.c.meter().observe(cfg)).await
    }

    /// Observes every `reading` sample (one per ADC sequence, ~1.6 kHz, API-39) in batches at most `max_age` old,
    /// with the budget they need: the device's default level budget (2 kB/s) carries about a third of them.
    pub async fn observe_samples(&self, max_age: Duration) -> Result<Observer<PowerMeterUpdate>> {
        let cfg = PowerMeterObserve::default().reading(samples(max_age));
        let p = self.c.meter().observe(cfg).budget(METER_SAMPLES_BUDGET);
        subscribe("PowerMeter", p).await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// plug, header

level!(
    /// `plug`: the DUT side, VBUS switch U601, data switch U701, orientation, CC.
    Plug
);

impl Plug<'_> {
    /// VBUS switch state with the trip reason.
    pub async fn vbus(&self) -> Result<VbusState> {
        read("Plug::vbus", self.c.plug().read_vbus()).await
    }

    pub async fn vbus_mode(&self) -> Result<VbusMode> {
        read("Plug::vbus_mode", self.c.plug().read_vbus_mode()).await
    }

    /// How the firmware drives VBUS; the firmware keeps the limits.
    pub async fn set_vbus_mode(&self, mode: VbusMode) -> Result<()> {
        write("Plug::vbus_mode", self.c.plug().write_vbus_mode(mode)).await
    }

    /// VBUS off for `off`, then back to the mode; with `release_sbu` SBU goes Hi-Z first. Answered when VBUS is
    /// back, so the timeout is `off` plus the usual one.
    pub async fn vbus_cycle(&self, off: Second, release_sbu: bool) -> Result<VbusState> {
        let timeout = std::time::Duration::from_secs_f64(off.to_f64().max(0.0)) + CALL_TIMEOUT;
        let p = self
            .c
            .plug()
            .vbus_cycle(off, release_sbu)
            .with_timeout(timeout);
        call::<_, PowerErrorOwned>("Plug::vbus_cycle", p).await
    }

    pub async fn data(&self) -> Result<DataPath> {
        read("Plug::data", self.c.plug().read_data()).await
    }

    pub async fn set_data(&self, data: DataPath) -> Result<()> {
        write("Plug::data", self.c.plug().write_data(data)).await
    }

    pub async fn orientation(&self) -> Result<Orientation> {
        read("Plug::orientation", self.c.plug().read_orientation()).await
    }

    pub async fn set_orientation(&self, orientation: Orientation) -> Result<()> {
        write_plain(
            "Plug::orientation",
            self.c.plug().write_orientation(orientation),
        )
        .await
    }

    pub async fn cc(&self) -> Result<CcState> {
        read("Plug::cc", self.c.plug().read_cc()).await
    }

    pub async fn observe(&self, cfg: PlugObserve) -> Result<Observer<PlugUpdate>> {
        subscribe("Plug", self.c.plug().observe(cfg)).await
    }
}

/// Timeout of a call on top of the time it takes on purpose (`vbus_cycle`).
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

level!(
    /// `header`: J101's VBUS_SW switch and the relay.
    Header
);

impl Header<'_> {
    pub async fn vbus_sw(&self) -> Result<VbusState> {
        read("Header::vbus_sw", self.c.header().read_vbus_sw()).await
    }

    pub async fn vbus_sw_mode(&self) -> Result<VbusMode> {
        read("Header::vbus_sw_mode", self.c.header().read_vbus_sw_mode()).await
    }

    pub async fn set_vbus_sw_mode(&self, mode: VbusMode) -> Result<()> {
        write(
            "Header::vbus_sw_mode",
            self.c.header().write_vbus_sw_mode(mode),
        )
        .await
    }

    /// Relay U101 closed.
    pub async fn relay(&self) -> Result<bool> {
        read("Header::relay", self.c.header().read_relay()).await
    }

    pub async fn set_relay(&self, closed: bool) -> Result<()> {
        write_plain("Header::relay", self.c.header().write_relay(closed)).await
    }

    pub async fn observe(&self, cfg: HeaderObserve) -> Result<Observer<HeaderUpdate>> {
        subscribe("Header", self.c.header().observe(cfg)).await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// sbu

level!(
    /// `sbu`: who owns the plug SBU lines, V_SBU, pull-ups, release.
    Sbu
);

impl Sbu<'_> {
    pub async fn function(&self) -> Result<SbuFunction> {
        read("Sbu::function", self.c.sbu().read_function()).await
    }

    /// Everything on SBU to Hi-Z; ends a CMSIS-DAP session and either UART owner.
    pub async fn release(&self) -> Result<()> {
        call_plain("Sbu::release", self.c.sbu().release()).await
    }

    pub async fn level(&self) -> Result<Volt> {
        read("Sbu::level", self.c.sbu().read_level()).await
    }

    pub async fn pullups(&self) -> Result<bool> {
        read("Sbu::pullups", self.c.sbu().read_pullups()).await
    }

    pub async fn set_pullups(&self, enabled: bool) -> Result<()> {
        write("Sbu::pullups", self.c.sbu().write_pullups(enabled)).await
    }

    /// Plus only: plug SBU to hub port 4.
    pub async fn route_usb(&self, enabled: bool) -> Result<()> {
        call::<_, SbuErrorOwned>("Sbu::route_usb", self.c.sbu().route_usb(enabled)).await
    }

    pub async fn observe(&self, cfg: SbuObserve) -> Result<Observer<SbuUpdate>> {
        subscribe("Sbu", self.c.sbu().observe(cfg)).await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// inputs, hub, swd

level!(
    /// `inputs`: the button and the slide switch.
    Inputs
);

impl<'a> Inputs<'a> {
    pub async fn button(&self) -> Result<ButtonState> {
        read("Inputs::button", self.c.inputs().read_button()).await
    }

    pub async fn switch(&self) -> Result<SwitchPosition> {
        read("Inputs::switch", self.c.inputs().read_switch()).await
    }

    pub async fn button_action(&self) -> Result<ButtonAction> {
        read(
            "Inputs::button_action",
            self.c.inputs().read_button_action(),
        )
        .await
    }

    pub async fn set_button_action(&self, action: ButtonAction) -> Result<()> {
        write(
            "Inputs::button_action",
            self.c.inputs().write_button_action(action),
        )
        .await
    }

    pub async fn observe(&self, cfg: InputsObserve) -> Result<Observer<InputsUpdate>> {
        subscribe("Inputs", self.c.inputs().observe(cfg)).await
    }

    /// The generated client, for the `events` stream.
    pub fn raw(&self) -> generated::donguru_api_inputs::DonguruApiInputsClient<'a> {
        self.c.inputs()
    }
}

level!(
    /// `hub`: the USB hub U301.
    Hub
);

impl Hub<'_> {
    pub async fn state(&self) -> Result<HubState> {
        read("Hub::state", self.c.hub().read_state()).await
    }

    /// Reset, configure and attach the hub; our own USB link drops for ~2 s after the answer.
    pub async fn recover(&self) -> Result<()> {
        call_plain("Hub::recover", self.c.hub().recover()).await
    }

    pub async fn observe(&self, cfg: HubObserve) -> Result<Observer<HubUpdate>> {
        subscribe("Hub", self.c.hub().observe(cfg)).await
    }
}

level!(
    /// `swd`: SWD-host settings and the CMSIS-DAP session.
    Swd
);

impl Swd<'_> {
    pub async fn mode(&self) -> Result<SwdMode> {
        read("Swd::mode", self.c.swd().read_mode()).await
    }

    pub async fn set_mode(&self, mode: SwdMode) -> Result<()> {
        write("Swd::mode", self.c.swd().write_mode(mode)).await
    }

    pub async fn session(&self) -> Result<SwdSession> {
        read("Swd::session", self.c.swd().read_session()).await
    }

    pub async fn observe(&self, cfg: SwdObserve) -> Result<Observer<SwdUpdate>> {
        subscribe("Swd", self.c.swd().observe(cfg)).await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// uart[], i2c[], gpio[], adc

/// Which side of the dongle an IO level is on: the index of `uart[]`, `i2c[]` and `gpio[]`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    /// The plug SBU lines (index 0).
    Plug,
    /// Header J101 IO7..IO10 (index 1).
    Header,
}

impl Side {
    pub fn index(self) -> u32 {
        match self {
            Side::Plug => 0,
            Side::Header => 1,
        }
    }
}

/// `uart[side]`: `ww_uart::Uart`.
#[derive(Copy, Clone)]
pub struct Uart<'a> {
    c: &'a DonguruClient,
    side: Side,
}

impl<'a> Uart<'a> {
    pub async fn capabilities(&self) -> Result<ww_uart::CapabilitiesOwned> {
        call_plain("Uart::capabilities", self.raw().capabilities()).await
    }

    /// Sets the line up, answers what was set: the baud rate the divider settled on, the idle time, the IO level.
    pub async fn configure(&self, config: ww_uart::UartConfig) -> Result<ww_uart::UartConfig> {
        call::<_, ww_uart::UartErrorOwned>("Uart::configure", self.raw().configure(config)).await
    }

    /// The generated client of this UART: configure, tx sink, rx and monitor streams, break.
    pub fn raw(&self) -> generated::ww_uart_uart::WwUartUartClient<'a> {
        self.c.uart(self.side.index())
    }
}

/// `i2c[side]`: `ww_i2c::I2c`.
#[derive(Copy, Clone)]
pub struct I2c<'a> {
    c: &'a DonguruClient,
    side: Side,
}

impl<'a> I2c<'a> {
    pub async fn capabilities(&self) -> Result<ww_i2c::I2cCapabilitiesOwned> {
        call_plain("I2c::i2c_capabilities", self.raw().i2c_capabilities()).await
    }

    pub async fn configure(&self, config: ww_i2c::I2cConfig) -> Result<ww_i2c::I2cConfig> {
        call::<_, ww_i2c::I2cErrorOwned>("I2c::i2c_configure", self.raw().i2c_configure(config))
            .await
    }

    /// Addresses that acknowledged.
    pub async fn scan(&self) -> Result<Vec<u8>> {
        let found =
            call::<_, ww_i2c::I2cErrorOwned>("I2c::i2c_scan", self.raw().i2c_scan()).await?;
        Ok(found.into_inner())
    }

    /// The generated client of this bus: transactions, line state, pull-ups.
    pub fn raw(&self) -> generated::ww_i2c_i_2_c::WwI2CI2CClient<'a> {
        self.c.i2c(self.side.index())
    }
}

/// `gpio[side]`: `ww_gpio::Bank`.
#[derive(Copy, Clone)]
pub struct Gpio<'a> {
    c: &'a DonguruClient,
    side: Side,
}

impl<'a> Gpio<'a> {
    pub async fn capabilities(&self) -> Result<ww_gpio::BankCapabilitiesOwned> {
        call_plain("Bank::capabilities", self.raw().capabilities()).await
    }

    pub async fn name(&self) -> Result<String> {
        call_plain("Bank::name", self.raw().name()).await
    }

    pub async fn input_level(&self, pin: u32) -> Result<ww_gpio::Level> {
        call_plain("Pin::input_level", self.raw().pin(pin).input_level()).await
    }

    pub async fn set_output_level(&self, pin: u32, level: ww_gpio::Level) -> Result<()> {
        call_plain(
            "Pin::set_output_level",
            self.raw().pin(pin).set_output_level(level),
        )
        .await
    }

    pub async fn set_mode(
        &self,
        pin: u32,
        mode: ww_gpio::Mode,
        initial: Option<ww_gpio::Level>,
    ) -> Result<()> {
        let p = self.raw().pin(pin).set_mode(mode, initial);
        call::<_, ww_gpio::GpioErrorOwned>("Pin::set_mode", p).await
    }

    /// The generated client of this bank: pins, events, port reads and writes.
    pub fn raw(&self) -> generated::ww_gpio_bank::WwGpioBankClient<'a> {
        self.c.gpio(self.side.index())
    }
}

level!(
    /// `adc`: ADC1 channels by index, with names in the capabilities.
    Adc
);

impl<'a> Adc<'a> {
    pub async fn capabilities(&self) -> Result<ww_adc::AdcCapabilitiesOwned> {
        call_plain("Adc::capabilities", self.c.adc().capabilities()).await
    }

    pub async fn read(&self, channel: u8) -> Result<ww_adc::Reading> {
        call::<_, ww_adc::AdcErrorOwned>("Adc::read", self.c.adc().read(channel)).await
    }

    pub fn raw(&self) -> generated::ww_adc_adc::WwAdcAdcClient<'a> {
        self.c.adc()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// debug

level!(
    /// `debug`: bring-up and production test. Pins by net name: [crate::nets].
    Debug
);

impl Debug<'_> {
    pub async fn pin_set(
        &self,
        pin: McuPin,
        mode: PinMode,
        level: Option<ww_gpio::Level>,
    ) -> Result<PinState> {
        call::<_, DebugErrorOwned>("Bringup::pin_set", self.c.debug().pin_set(pin, mode, level))
            .await
    }

    pub async fn pin_get(&self, pin: McuPin) -> Result<PinState> {
        call::<_, DebugErrorOwned>("Bringup::pin_get", self.c.debug().pin_get(pin)).await
    }

    pub async fn pin_af(&self, pin: McuPin, af: u8) -> Result<PinState> {
        call::<_, DebugErrorOwned>("Bringup::pin_af", self.c.debug().pin_af(pin, af)).await
    }

    pub async fn pins(&self) -> Result<PinDumpOwned> {
        call_plain("Bringup::pins", self.c.debug().pins()).await
    }

    pub async fn boot_pins(&self) -> Result<PinDumpOwned> {
        call_plain("Bringup::boot_pins", self.c.debug().boot_pins()).await
    }

    pub async fn hub_read(&self, reg: u8, len: u8) -> Result<Vec<u8>> {
        let data =
            call::<_, DebugErrorOwned>("Bringup::hub_read", self.c.debug().hub_read(reg, len))
                .await?;
        Ok(data.into_inner())
    }

    pub async fn hub_write(&self, reg: u8, data: &[u8]) -> Result<()> {
        let data = MaxLen::new(data.to_vec())
            .map_err(|_| Error::Link(format!("hub write of {} bytes, at most 32", data.len())))?;
        call::<_, DebugErrorOwned>("Bringup::hub_write", self.c.debug().hub_write(reg, data)).await
    }

    pub async fn ucpd(&self, port: UcpdPort) -> Result<UcpdState> {
        call_plain("Bringup::ucpd", self.c.debug().ucpd(port)).await
    }

    pub async fn ucpd_role(&self, port: UcpdPort, role: CcRole) -> Result<UcpdState> {
        call::<_, DebugErrorOwned>("Bringup::ucpd_role", self.c.debug().ucpd_role(port, role)).await
    }

    pub async fn led_set(&self, led: Led, mode: LedMode) -> Result<()> {
        call_plain("Bringup::led_set", self.c.debug().led_set(led, mode)).await
    }

    /// 32-bit words of peripheral space: word aligned, 1..=32, inside a block that holds a peripheral
    /// ([crate::debug::check_reg_read] decides it, as the firmware does).
    pub async fn reg_read(&self, addr: u32, words: u8) -> Result<Vec<u32>> {
        let data =
            call::<_, DebugErrorOwned>("Bringup::reg_read", self.c.debug().reg_read(addr, words))
                .await?;
        Ok(data.into_inner())
    }
}

// ---------------------------------------------------------------------------------------------------------------

/// Level accessors of a connected dongle, see [crate::Dongle].
impl crate::Dongle {
    pub fn device(&self) -> Device<'_> {
        Device::new(&self.client)
    }

    pub fn leds(&self) -> Leds<'_> {
        Leds::new(&self.client)
    }

    pub fn meter(&self) -> Meter<'_> {
        Meter::new(&self.client)
    }

    pub fn plug(&self) -> Plug<'_> {
        Plug::new(&self.client)
    }

    pub fn header(&self) -> Header<'_> {
        Header::new(&self.client)
    }

    pub fn sbu(&self) -> Sbu<'_> {
        Sbu::new(&self.client)
    }

    pub fn inputs(&self) -> Inputs<'_> {
        Inputs::new(&self.client)
    }

    pub fn hub(&self) -> Hub<'_> {
        Hub::new(&self.client)
    }

    pub fn swd(&self) -> Swd<'_> {
        Swd::new(&self.client)
    }

    pub fn uart(&self, side: Side) -> Uart<'_> {
        Uart {
            c: &self.client,
            side,
        }
    }

    pub fn i2c(&self, side: Side) -> I2c<'_> {
        I2c {
            c: &self.client,
            side,
        }
    }

    pub fn gpio(&self, side: Side) -> Gpio<'_> {
        Gpio {
            c: &self.client,
            side,
        }
    }

    pub fn adc(&self) -> Adc<'_> {
        Adc::new(&self.client)
    }

    pub fn debug(&self) -> Debug<'_> {
        Debug::new(&self.client)
    }
}
