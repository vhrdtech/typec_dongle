//! WireWeaver API of the donguru firmware (`donguru_api/` in the repo root), served by `fw/donguru/src/ww/`.
//!
//! The crate name and version identify the API on the wire (`DONGURU_API_FULL_GID`), so bump the version with every
//! change and follow the WireWeaver evolution rules to stay compatible with deployed devices and hosts:
//! https://github.com/vhrdtech/wire_weaver/blob/master/docs/evolution/rules.md
//! Data types used in the API go here too, with `#[derive_shrink_wrap]`.
//!
//! 0.2 is the whole v1 tree (API-8, design of record: tpm `prj/P2534-donguru/design/api-v1.md`): one level per part
//! of the dongle, the stdlib traits for everything generic. Items a firmware doesn't have yet answer
//! `Unimplemented`. Index rule for the arrays (`uart`, `i2c`, `gpio`): 0 = plug side, 1 = header.
//!
//! 0.3 makes `meter.reading` `+batched` (API-39: every ADC sequence as a sample), which changes its observe slot,
//! and is built on `ww_si` 0.2 (same bytes, new API model): firmware and host move to 0.3 together.
#![cfg_attr(not(feature = "std"), no_std)]

use wire_weaver::prelude::*;
pub use ww_gpio::Level;
pub use ww_si::{Ampere, Coulomb, DegreeCelsius, Hertz, Joule, Second, Volt, Watt};
pub use ww_uart::UartConfig;

/// Donguru: USB Type-C dongle with a hub, power switch and meter, and SWD / UART / I2C / GPIO on the plug SBU lines.
#[ww_api_root]
pub trait DonguruApi {
    /// Device contract: firmware, last reset, board, ids, label, counters, log, settings (API-9, API-10, API-19).
    ww_impl!(device: Device);
    /// The power and status LEDs as one indication: mode, brightness, identify (API-9).
    ww_impl!(leds: ww_indication::Indication);
    /// Power meter on the receptacle VBUS, in SI units (API-12).
    ww_impl!(meter: PowerMeter);
    /// DUT side: plug VBUS switch, data switch, orientation, CC (API-11).
    ww_impl!(plug: Plug);
    /// Header J101: VBUS_SW switch and the relay (API-11).
    ww_impl!(header: Header);
    /// Plug SBU lines: who owns them, V_SBU, pull-ups, release (API-14).
    ww_impl!(sbu: Sbu);
    /// Button and slide switch (API-13).
    ww_impl!(inputs: Inputs);
    /// The USB hub (U301) (API-17).
    ww_impl!(hub: Hub);
    /// SWD-host on the plug SBU lines, used through the CMSIS-DAP v2 interface (API-17).
    ww_impl!(swd: Swd);
    /// UARTs: 0 = plug SBU (USART3 / USART1), 1 = header IO7/IO8 (USART5) (API-14).
    ww_impl!(uart[]: ww_uart::Uart);
    /// I2C buses: 0 = plug SBU (I2C1 through the TXS0102), 1 = header IO7/IO8 (I2C3) (API-16).
    ww_impl!(i2c[]: ww_i2c::I2c);
    /// GPIO banks: 0 = plug SBU1, SBU2, A2, B2; 1 = header IO7..IO10 (API-15).
    ww_impl!(gpio[]: ww_gpio::Bank);
    /// ADC1 channels by index, with names in the capabilities (API-12).
    ww_impl!(adc: ww_adc::Adc);
    /// Bring-up and production test: raw pins, hub registers, UCPD, LEDs (API-18). `Unimplemented` in a firmware
    /// built without the `debug-api` feature.
    ww_impl!(debug: Bringup);
}

// ---------------------------------------------------------------------------------------------------------------
// device

/// The device contract: what every WireWeaver device answers, so generic host tools (`ww info`, `ww log`) work.
#[ww_trait]
pub trait Device {
    /// Firmware name, version, git, build id.
    ww_impl!(firmware: ww_firmware_info::FirmwareInfo);
    /// Why the firmware last started, boot count.
    ww_impl!(last_reset: ww_firmware_info::LastReset);
    /// Board name, revision and variant.
    ww_impl!(board: ww_board_info::BoardInfo);
    /// STM32 96-bit unique id, also the USB serial number.
    ww_impl!(uid: ww_uid::Uid);
    /// User label and notes, kept in the settings store (API-19).
    ww_impl!(label: ww_user_info::UserInfo);
    /// `cnt` counters: RAM and backup registers.
    ww_impl!(counters: ww_counters::Counters);
    /// defmt log (API-10).
    ww_impl!(log: ww_log::LogDefmt);
    /// Board variant from the hub PID (FW-22), typed.
    property!(ro variant: BoardVariant);
    /// Settings kept in flash (API-19).
    ww_impl!(settings: Settings);
}

/// Board variant, see [`Device::variant`]. New boards add variants: a host reads one it doesn't know as
/// [`Other`](BoardVariant::Other).
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, future_version = Other)]
pub enum BoardVariant {
    /// 2-port USB2512B hub (U301).
    Main,
    /// 4-port USB2514B hub (U301), not decided/built yet (P2534 #12).
    Plus,
    /// The fitted hub's power-on PID matched neither [`Main`](BoardVariant::Main) nor
    /// [`Plus`](BoardVariant::Plus).
    Unknown,
}

/// Settings kept in flash across power cycles (API-19). A write applies the value right away where it has a live
/// counterpart (`inputs.button_action`, `meter.window`, `meter.deadband`) and persists it; writing a live property
/// does not persist it. The label is `device.label`, persisted the same way.
#[ww_trait]
pub trait Settings {
    /// `plug.vbus_mode` at boot, default `ForceOff`.
    #[errors(InvalidArgument, Fault)]
    property!(rw plug_vbus_at_boot: VbusMode, CommonError<'i>);
    /// Boot value of `inputs.button_action`.
    #[errors(InvalidArgument, Fault)]
    property!(rw button_action: ButtonAction, CommonError<'i>);
    /// Boot value of `meter.window`.
    #[errors(InvalidArgument, Fault)]
    property!(rw meter_window: Second, CommonError<'i>);
    /// Boot value of `meter.deadband`.
    #[errors(InvalidArgument, Fault)]
    property!(rw meter_deadband: MeterDeadband, CommonError<'i>);
    /// Default config of `uart[0]`, also used by CDC-ACM until the tty sets its line coding.
    #[errors(InvalidArgument, Fault)]
    property!(rw uart_plug: UartConfig, CommonError<'i>);
    /// Every setting back to its default, the label too. Factory data (calibration) stays.
    #[errors(Fault)]
    fn reset() -> Result<(), CommonError<'i>>;
}

// ---------------------------------------------------------------------------------------------------------------
// meter

/// Power meter on the receptacle side: VBUS_IN and the current to the DUT, from one ADC window.
#[ww_trait]
pub trait PowerMeter {
    /// The latest reading, averaged over `window`. `OnChange` means a move beyond `deadband`. Observed in
    /// `Samples` mode: every ADC sequence (622 µs, ~1.6 kHz; 898 µs while `adc` samples the extended set), each
    /// 32x oversampled in hardware but not averaged over `window`, with its own time.
    property!(ro+observe+batched reading: MeterReading);
    /// MCU die temperature.
    property!(ro+observe temperature: DegreeCelsius);
    /// Charge and energy since boot or `reset_totals`.
    property!(ro+observe totals: MeterTotals);
    /// Restarts `totals` from zero.
    fn reset_totals();
    /// Averaging window behind `reading`, default 10 ms.
    #[errors(InvalidArgument)]
    property!(rw window: Second, CommonError<'i>);
    /// The change that counts for `OnChange` observers, default 20 mV and 10 mA.
    #[errors(InvalidArgument)]
    property!(rw deadband: MeterDeadband, CommonError<'i>);
    /// Calibration in use: offset and gain per channel, defaults until FW-9.
    property!(ro calibration: MeterCalibration);
}

/// One meter reading, all from the same ADC window (or sequence, for samples).
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    series(cells),
    derive(Clone, Debug, PartialEq)
)]
pub struct MeterReading {
    /// Receptacle VBUS (PA3).
    pub vbus_in: Volt,
    /// Current from the computer to the DUT (INA181, PB0), positive towards the DUT.
    pub current: Ampere,
    // power is `vbus_in` x `current`, computed by the host (`donguru_core::reading_power_w`); the U601 drop is not
    // measured
}

/// Totals since boot or [`PowerMeter::reset_totals`].
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq))]
pub struct MeterTotals {
    pub charge: Coulomb,
    pub energy: Joule,
    pub peak_current: Ampere,
    /// Time since the totals started.
    pub since: Second,
}

/// What counts as a change for `OnChange` observers of [`PowerMeter::reading`].
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq))]
pub struct MeterDeadband {
    pub voltage: Volt,
    pub current: Ampere,
}

/// Calibration of the meter channels.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq))]
pub struct MeterCalibration {
    /// From factory data (FW-9); false: nominal values.
    pub calibrated: bool,
    pub vbus_in_offset: Volt,
    /// Gain correction in parts per million, 0 = nominal.
    pub vbus_in_gain_ppm: i32,
    pub current_offset: Ampere,
    pub current_gain_ppm: i32,
}

// ---------------------------------------------------------------------------------------------------------------
// plug, header

/// DUT side of the dongle: plug VBUS (U601), data switch (U701), orientation of everything on SBU, CC.
#[ww_trait]
pub trait Plug {
    /// Plug VBUS switch state, with the trip reason when the firmware cut it.
    property!(ro+observe vbus: VbusState);
    /// How the firmware drives plug VBUS (U601). The limits are kept by the firmware: a software trip takes
    /// milliseconds, it is no fuse. A mode from a newer host is refused.
    #[errors(InvalidArgument, Refused)]
    property!(rw+observe vbus_mode: VbusMode, CommonError<'i>);
    /// Off for `off`, then back to `vbus_mode`; with `release_sbu` SBU goes to Hi-Z first so nothing back-powers the
    /// DUT. Answered when VBUS is back on. `WrongState` in `ForceOff`.
    #[errors(Common(Refused, InvalidArgument, WrongState), OverVoltage)]
    fn vbus_cycle(off: Second, release_sbu: bool) -> Result<VbusState, PowerError<'i>>;
    /// USB data to the DUT (U701, DATA_CUT PA9).
    #[errors(Refused)]
    property!(rw+observe data: DataPath, CommonError<'i>);
    /// Cable orientation for everything on SBU (SWD, UART, I2C, GPIO). Takes effect on the next connect or
    /// configure. `Auto` acts as `Normal` until CC detection (`cc`) works.
    property!(rw+observe orientation: Orientation);
    /// Detected CC polarity and pull, `Unknown` until UCPD2 is used.
    property!(ro+observe cc: CcState);
}

/// Header J101: the VBUS_SW output and the photo-MOS relay.
#[ww_trait]
pub trait Header {
    /// VBUS_SW switch (J101 pin 10, Q102, PD1) state, with the trip reason.
    property!(ro+observe vbus_sw: VbusState);
    /// How the firmware drives VBUS_SW, as [`Plug::vbus_mode`].
    #[errors(InvalidArgument, Refused)]
    property!(rw+observe vbus_sw_mode: VbusMode, CommonError<'i>);
    /// Relay U101 closed (PA5).
    property!(rw+observe relay: bool);
}

/// State of a VBUS switch.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum VbusState {
    Off,
    On,
    /// The switch latched off on its own (U601 FLTB); the next `on` re-arms it.
    Fault,
    /// The firmware cut it.
    Tripped(GuardTrip),
}

/// Why the firmware cut a VBUS switch.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum GuardTrip {
    OverVoltage,
    OverCurrent,
    OverPower,
}

/// How the firmware drives a VBUS switch. The limits apply to the receptacle VBUS (`meter.reading`). More modes
/// come later (`Auto` with PD negotiation); a host reads one it doesn't know as `Unknown`.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq), ww_repr = nib)]
pub enum VbusMode {
    /// Off.
    ForceOff,
    /// On while VBUS stays under `max_voltage`; above it off, reported as `Tripped(OverVoltage)`. Hosts default to
    /// 5.5 V.
    ForceOn { max_voltage: Volt },
    /// On, and off when any limit is exceeded, reported as `Tripped(..)`. The computer -> DUT path has no hardware
    /// current limit.
    Auto {
        max_voltage: Volt,
        max_current: Ampere,
        max_power: Watt,
    },
}

/// Errors of the VBUS switches.
#[derive_shrink_wrap(
    owned(feature = "std"),
    derive(Clone, Debug, PartialEq),
    ww_repr = unib32,
    display
)]
pub enum PowerError<'i> {
    /// {0}
    #[common]
    Common(CommonError<'i>),
    /// VBUS_IN is above the mode's maximum.
    OverVoltage { measured: Volt },
}

/// The plug's USB data lines to the DUT.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, no_future_version)]
pub enum DataPath {
    Connected,
    Cut,
}

/// Cable orientation for everything on the plug SBU lines. Three by nature, no future variant.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, no_future_version)]
pub enum Orientation {
    /// From the CC polarity once detected (`Plug::cc`), `Normal` until then.
    Auto,
    /// SBU1 = SWCLK / TX, SBU2 = SWDIO / RX.
    Normal,
    /// SBU1 = SWDIO / RX, SBU2 = SWCLK / TX.
    Swapped,
}

/// What the plug's CC lines show.
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    derive(Copy, Clone, Debug, PartialEq, Eq)
)]
pub struct CcState {
    pub polarity: CcPolarity,
    pub pull: CcPull,
}

/// Which CC line the DUT pulls.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, future_version = Other)]
pub enum CcPolarity {
    /// Not detected (yet).
    Unknown,
    Cc1,
    Cc2,
}

/// The pull the DUT has on its CC line.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, future_version = Other)]
pub enum CcPull {
    Unknown,
    Open,
    Rd,
    Ra,
    Rp,
}

// ---------------------------------------------------------------------------------------------------------------
// sbu

/// The plug SBU lines: one function owns them at a time. A function's `configure` takes them when they are free and
/// answers `InUse` while another one holds them.
#[ww_trait]
pub trait Sbu {
    /// Who has SBU now.
    property!(ro+observe function: SbuFunction);
    /// All SBU pins Hi-Z, level shifter off, pull-ups off, CC drive off; ends a CMSIS-DAP session and either UART
    /// owner.
    fn release();
    /// V_SBU as selected (PD3), set through `io_level` of a function.
    property!(ro+observe level: Volt);
    /// 3K3 pull-ups to V_SBU (PC15).
    #[errors(InUse)]
    property!(rw pullups: bool, CommonError<'i>);
    /// Plus: plug SBU to hub port 4 as USB. Main: `Unsupported`.
    #[errors(Common(InUse, Unsupported))]
    fn route_usb(enabled: bool) -> Result<(), SbuError<'i>>;
}

/// Who owns the plug SBU lines.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum SbuFunction {
    /// Hi-Z.
    Off,
    Swd,
    Uart {
        owner: UartOwner,
    },
    I2c,
    Gpio,
    /// Plus: hub port 4.
    Usb,
}

/// Who holds `uart[0]`: WireWeaver or the CDC-ACM tty.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum UartOwner {
    WireWeaver,
    Acm,
}

/// Errors of the SBU level.
#[derive_shrink_wrap(
    owned(feature = "std"),
    derive(Clone, Debug, PartialEq, Eq),
    ww_repr = unib32,
    display
)]
pub enum SbuError<'i> {
    /// {0}
    #[common]
    Common(CommonError<'i>),
}

// ---------------------------------------------------------------------------------------------------------------
// inputs, hub, swd

/// The button and the slide switch SW101.
#[ww_trait]
pub trait Inputs {
    /// Button state, with a press count so no click is lost between updates.
    property!(ro+observe button: ButtonState);
    /// Slide switch position.
    property!(ro+observe switch: SwitchPosition);
    /// Exact edges. Unit: one event.
    #[counted]
    stream!(events: InputEvent);
    /// What a button press does on the device itself, default nothing.
    #[errors(InvalidArgument)]
    property!(rw button_action: ButtonAction, CommonError<'i>);
}

/// Button state.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq))]
pub struct ButtonState {
    pub pressed: bool,
    /// Presses since boot.
    pub presses: u32,
    /// How long the last press was held.
    pub last_held: Second,
}

/// Slide switch SW101 position. Two by nature.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, no_future_version)]
pub enum SwitchPosition {
    A,
    B,
}

/// An input edge.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq), ww_repr = nib)]
pub enum InputEvent {
    Pressed,
    Released { held: Second },
    Switched { to: SwitchPosition },
}

/// What a button press does on the device.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum ButtonAction {
    /// Nothing: only reported.
    Off,
    ToggleVbus,
    CycleVbus,
    ToggleData,
}

/// The USB hub U301; the MCU's own USB is on its port 1.
#[ww_trait]
pub trait Hub {
    /// Hub state and recoveries (FW-18).
    property!(ro+observe state: HubState);
    /// Reset, configure and attach the hub. Answered first, done 100 ms later: our own USB link drops for ~2 s.
    fn recover();
}

/// Hub state.
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    derive(Copy, Clone, Debug, PartialEq, Eq)
)]
pub struct HubState {
    pub phase: HubPhase,
    /// Recoveries since boot.
    pub recoveries: u32,
    /// Why the hub was last lost, `None` if never.
    pub last_cause: Option<HubLoss>,
}

/// Hub phase.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum HubPhase {
    Reset,
    Configuring,
    Attached,
    Lost,
}

/// Why the hub was lost.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum HubLoss {
    /// Our own USB device was reset after it had been configured.
    DeviceDropped,
    /// The hub stopped answering on I2C.
    I2cProbe,
    /// `recover` was called.
    Requested,
}

/// SWD-host settings and session; the transfers go through the CMSIS-DAP v2 interface.
#[ww_trait]
pub trait Swd {
    /// How the SWD-host drives the plug SBU lines (FW-21), from the next transfer on. A mode from a newer host
    /// is refused.
    #[errors(InvalidArgument)]
    property!(rw mode: SwdMode, CommonError<'i>);
    /// CMSIS-DAP session.
    property!(ro+observe session: SwdSession);
}

/// How the SWD-host drives the plug SBU lines, see [`Swd::mode`]. A mode from a newer host arrives as `Unknown`.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum SwdMode {
    /// GPIO bit-bang on PA1/PA0, straight on the SBU lines, level shifter off: up to about 3.7 MHz.
    BitBang,
    /// SPI2 through the TXS0102 level shifter, up to 4 MHz. Experimental (FW-21): `Normal` orientation only,
    /// weak highs through the shifter's 10K pull-ups; not with the R211/R212 bypass fitted.
    Spi,
}

/// CMSIS-DAP session state.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Clone, Debug, PartialEq), ww_repr = nib)]
pub enum SwdSession {
    Idle,
    /// `orientation` is the one in use, never `Auto`.
    Connected {
        clock: Hertz,
        orientation: Orientation,
    },
}

// ---------------------------------------------------------------------------------------------------------------
// debug

/// Bring-up and production test. Refused always: NRST, supply pins, our USB and SWD, PA15, the VBUS enables (use
/// `plug` / `header`), the hub reset and I2C (use `hub_*`). `InUse` for a pin a running function owns.
#[ww_trait]
pub trait Bringup {
    /// Sets a pin's mode and, for outputs, its level.
    #[errors(Common(Refused, InUse, InvalidArgument))]
    fn pin_set(
        pin: McuPin,
        mode: PinMode,
        level: Option<Level>,
    ) -> Result<PinState, DebugError<'i>>;
    /// Reads a pin's registers.
    #[errors(Common(InvalidArgument))]
    fn pin_get(pin: McuPin) -> Result<PinState, DebugError<'i>>;
    /// Puts a pin on alternate function `af`.
    #[errors(Common(Refused, InUse, InvalidArgument))]
    fn pin_af(pin: McuPin, af: u8) -> Result<PinState, DebugError<'i>>;
    /// GPIOA..F registers now.
    fn pins() -> PinDump<'i>;
    /// GPIOA..F registers as captured first thing in main, before init.
    fn boot_pins() -> PinDump<'i>;
    /// SMBus block read from the hub.
    #[errors(Common(Busy, Timeout), Nack)]
    fn hub_read(reg: u8, len: u8) -> Result<MaxLen<RefVec<'i, u8>, 32>, DebugError<'i>>;
    /// SMBus block write to the hub.
    #[errors(Common(Busy, Timeout, Refused), Nack)]
    fn hub_write(reg: u8, data: MaxLen<RefVec<'i, u8>, 32>) -> Result<(), DebugError<'i>>;
    /// UCPD registers and CC line states.
    fn ucpd(port: UcpdPort) -> UcpdState;
    /// Sets a UCPD port's CC role. `Rp` refused on UCPD1 on rev A.
    #[errors(Common(Refused))]
    fn ucpd_role(port: UcpdPort, role: CcRole) -> Result<UcpdState, DebugError<'i>>;
    /// Drives one LED directly.
    fn led_set(led: Led, mode: LedMode);
    /// Reads 32-bit words of peripheral space (0x4000_0000..0x5000_2000) only.
    #[errors(Common(Refused, InvalidArgument))]
    fn reg_read(addr: u32, words: u8) -> Result<MaxLen<RefVec<'i, u32>, 32>, DebugError<'i>>;
    /// Logs `count` numbered lines at once, more than the log keeps up with: the host sees how many are lost.
    #[since = "0.2.1"]
    fn log_flood(count: u32);
}

/// An MCU pin: port and number.
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    derive(Copy, Clone, Debug, PartialEq, Eq)
)]
pub struct McuPin {
    pub port: GpioPort,
    /// 0..=15
    pub pin: u8,
}

/// STM32G0B1 GPIO port.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, no_future_version)]
pub enum GpioPort {
    A,
    B,
    C,
    D,
    E,
    F,
}

/// Pin mode for [`Bringup::pin_set`].
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum PinMode {
    Analog,
    Input,
    InputPullUp,
    InputPullDown,
    PushPull,
    OpenDrain,
    OpenDrainPullUp,
}

/// A pin's register bits.
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    derive(Copy, Clone, Debug, PartialEq, Eq)
)]
pub struct PinState {
    /// MODER: 0 input, 1 output, 2 alternate, 3 analog.
    pub moder: u8,
    /// OTYPER: open drain.
    pub open_drain: bool,
    /// OSPEEDR.
    pub speed: u8,
    /// PUPDR: 0 none, 1 up, 2 down.
    pub pull: u8,
    /// AFR.
    pub af: u8,
    /// IDR.
    pub input: bool,
    /// ODR.
    pub output: bool,
}

/// GPIOA..F registers.
#[derive_shrink_wrap(owned(feature = "std"), derive(Clone, Debug, PartialEq, Eq))]
pub struct PinDump<'i> {
    pub ports: MaxLen<RefVec<'i, PortRegs>, 6>,
}

/// One GPIO port's registers.
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    derive(Copy, Clone, Debug, PartialEq, Eq)
)]
pub struct PortRegs {
    pub port: GpioPort,
    pub moder: u32,
    pub otyper: u32,
    pub ospeedr: u32,
    pub pupdr: u32,
    pub idr: u32,
    pub odr: u32,
    pub afrl: u32,
    pub afrh: u32,
}

/// UCPD1 (receptacle) or UCPD2 (plug).
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib, no_future_version)]
pub enum UcpdPort {
    Ucpd1,
    Ucpd2,
}

/// UCPD registers.
#[derive_shrink_wrap(
    borrowed,
    owned(feature = "std"),
    derive(Copy, Clone, Debug, PartialEq, Eq)
)]
pub struct UcpdState {
    pub cr: u32,
    pub sr: u32,
    /// TYPEC_VSTATE_CC1 (SR bits 16..17).
    pub cc1: u8,
    /// TYPEC_VSTATE_CC2 (SR bits 18..19).
    pub cc2: u8,
}

/// A CC role for [`Bringup::ucpd_role`].
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum CcRole {
    Off,
    Rd,
    Rp0A9,
    Rp1A5,
    Rp3A0,
}

/// One of the two LEDs.
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum Led {
    Power,
    Status,
}

/// What an LED does, for [`Bringup::led_set`].
#[derive_shrink_wrap(borrowed, owned(feature = "std"), derive(Copy, Clone, Debug, PartialEq, Eq), ww_repr = nib)]
pub enum LedMode {
    Off,
    On,
    Blink { on_ms: u16, off_ms: u16 },
    Breath { period_ms: u16 },
}

/// Errors of the debug level.
#[derive_shrink_wrap(
    owned(feature = "std"),
    derive(Clone, Debug, PartialEq, Eq),
    ww_repr = unib32,
    display
)]
pub enum DebugError<'i> {
    /// {0}
    #[common]
    Common(CommonError<'i>),
    /// The hub did not acknowledge.
    Nack,
}
