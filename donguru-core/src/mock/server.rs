//! The mock's handlers: `DonguruApi` served from [MockState], generated from `ww api scaffold donguru_api --alloc
//! --use-async --server MockServer`. What it models is in [super]; items it doesn't model answer `Unimplemented`, as
//! a firmware that doesn't have them yet.

use donguru_api::*;
use wire_weaver::prelude::*;
use wire_weaver::ww_error::CommonError;
use ww_board_info::{BoardSummary, RevisionSource};
use ww_firmware_info::{BuildProfile, FirmwareSummary, Version};

use super::{
    MockSettings, MockState, check_mode, check_pin, check_window, invalid_pin, max_voltage,
    set_pin_mode,
};

/// The mock device's server: [MockState] plus what the generated code needs.
pub struct MockServer {
    /// The modelled dongle, changed by handlers and by [super::MockDevice::with].
    pub state: MockState,
    /// State of the counted stream `device_counters_watch` (never opened: `Unimplemented`)
    pub device_counters_watch_stream: ww_stream::tx::Counted,
    /// State of the reliable stream `device_log_frames` (never opened: `Unimplemented`)
    pub device_log_frames_stream: ww_stream::tx::Reliable<ww_stream::store::RamRing<4096>>,
    /// State of the counted stream `inputs_events` (never opened: `Unimplemented`)
    pub inputs_events_stream: ww_stream::tx::Counted,
    /// Counted streams of `uart[]` and `gpio[].pin[]`, never opened (`Unimplemented`)
    pub unopened: ww_stream::tx::Counted,
    /// `uart[].tx` credit: nothing accepted
    pub uart_tx_credit: ww_stream::sink::SinkCredit,
    /// Sessions of counted and reliable streams opened so far
    pub stream_sessions: u32,
    /// Observed properties: what the host observes and when it is due, see `poll_properties`; `meter.reading`
    /// samples (API-39) taken every tick of the mock's loop, see [MockServer::sample_meter]
    pub observe: api_impl::ObserveState<api_impl::ObserveBatches>,
    /// Backing store of borrowed replies (`debug.pins`, `debug.reg_read`, `device.uid.IDS`)
    dump: Vec<PortRegs>,
    words: Vec<u32>,
    uid: [ww_uid::UniqueId<'static>; 1],
}

impl MockServer {
    pub fn new(state: MockState) -> Self {
        let uid: &'static [u8] = Box::leak(state.uid.to_vec().into_boxed_slice());
        MockServer {
            state,
            device_counters_watch_stream: ww_stream::tx::Counted::new(),
            device_log_frames_stream: ww_stream::tx::Reliable::new(ww_stream::store::RamRing::new()),
            inputs_events_stream: ww_stream::tx::Counted::new(),
            unopened: ww_stream::tx::Counted::new(),
            uart_tx_credit: ww_stream::sink::SinkCredit::new(0),
            stream_sessions: 0,
            observe: api_impl::ObserveState::new(
                ww_stream::observe::ObserveLimits::DEFAULT,
                api_impl::ObserveBatches::new(ww_stream::batch::DEFAULT_POLICY),
            ),
            dump: vec![],
            words: vec![],
            uid: [ww_uid::UniqueId {
                kind: ww_uid::UidKind::ChipUnique,
                source: ww_uid::UidSource::InternalIc,
                value: MaxLen::new_unchecked(RefVec::new_bytes(uid)),
            }],
        }
    }

    /// One `meter.reading` sample at `now_us` (the mock's clock): the reading the test put into the state, as the
    /// firmware takes one per ADC sequence.
    pub fn sample_meter(&mut self, now_us: u64) {
        let reading = self.state.reading.clone();
        self.observe.samplers().root_meter.sample(
            Second::micro(now_us),
            api_impl::PowerMeterSamples { reading },
        );
    }

    /// Marks every observed property changed, after [super::MockDevice::with] changed the state from outside.
    pub fn mark_all_changed(&mut self) {
        self.mark_changed_meter_reading();
        self.mark_changed_meter_temperature();
        self.mark_changed_meter_totals();
        self.mark_changed_plug_vbus();
        self.mark_changed_plug_vbus_mode();
        self.mark_changed_plug_data();
        self.mark_changed_plug_orientation();
        self.mark_changed_plug_cc();
        self.mark_changed_header_vbus_sw();
        self.mark_changed_header_vbus_sw_mode();
        self.mark_changed_header_relay();
        self.mark_changed_sbu_function();
        self.mark_changed_sbu_level();
        self.mark_changed_inputs_button();
        self.mark_changed_inputs_switch();
        self.mark_changed_hub_state();
        self.mark_changed_swd_session();
    }
}

// Handlers in API order; their docs are on the traits (donguru_api and the stdlib crates).
impl MockServer {
    // ---- donguru_api::DonguruApi at root ----

    fn valid_indices_root_uart(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..2)
    }

    fn valid_indices_root_i_2_c(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..2)
    }

    fn valid_indices_root_gpio(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..2)
    }

    // ---- donguru_api::Device at device ----

    async fn get_device_variant(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::BoardVariant, ()> {
        Value(self.state.variant)
    }

    // ---- ww_firmware_info::FirmwareInfo at device.firmware ----

    // ---- ww_firmware_info::LastReset at device.last_reset ----

    async fn device_last_reset_last_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<ww_firmware_info::ResetRecord<'_>> {
        Ready(ww_firmware_info::ResetRecord {
            cause: self.state.last_reset_cause,
            build_id: MaxLen::new_unchecked(RefVec::new_bytes(&[])),
            uptime: None,
            fault: None,
            stack: MaxLen::new_unchecked(RefVec::Slice { slice: &[] }),
            message: MaxLen::new_unchecked(self.state.last_reset_message.as_str()),
            boots_since_crash: self.state.last_reset_boots_since_crash,
        })
    }

    async fn device_last_reset_boot_count(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<u32> {
        Ready(self.state.boot_count)
    }

    async fn device_last_reset_clear(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        self.state.last_reset_cause = ww_firmware_info::ResetCause::PowerOn;
        self.state.last_reset_message.clear();
        self.state.last_reset_boots_since_crash = 0;
        self.state.boot_count = 0;
        Ready(())
    }

    // ---- ww_board_info::BoardInfo at device.board ----

    // ---- ww_uid::Uid at device.uid ----

    // ---- ww_user_info::UserInfo at device.label ----

    async fn set_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        self.state.label = value.to_string();
        Set
    }

    async fn get_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>, ww_user_info::UserInfoError<'_>>
    {
        Value(MaxLen::new_unchecked(self.state.label.as_str()))
    }

    // optional, can be left out with omit = ["device.label.notes"]
    async fn set_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        self.state.label_notes = value.to_string();
        Set
    }

    async fn get_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>, ww_user_info::UserInfoError<'_>>
    {
        Value(MaxLen::new_unchecked(self.state.label_notes.as_str()))
    }

    // ---- ww_counters::Counters at device.counters ----

    async fn device_counters_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _buffer: ww_counters::CounterBuffer,
        _offset: u16,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<
                wire_weaver::shrink_wrap::RefVec<'_, wire_weaver::shrink_wrap::UVlq32>,
                64,
            >,
            ww_counters::CountersError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    async fn device_counters_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _buffer: ww_counters::CounterBuffer,
    ) -> RpcResult<Result<(), ww_counters::CountersError<'_>>> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["device.counters.watch"]
    async fn device_counters_watch(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _period: ww_si::Second,
    ) -> RpcResult<Result<ww_si::Second, ww_counters::CountersError<'_>>> {
        Unimplemented.into()
    }

    async fn close_device_counters_watch(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    async fn stream_device_counters_watch(&mut self) -> &mut ww_stream::tx::Counted {
        &mut self.device_counters_watch_stream
    }

    // ---- ww_log::LogDefmt at device.log ----

    async fn device_log_frames(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), ww_log::LogError<'_>>> {
        Unimplemented.into()
    }

    async fn close_device_log_frames(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    async fn stream_device_log_frames(
        &mut self,
    ) -> &mut ww_stream::tx::Reliable<ww_stream::store::RamRing<4096>> {
        &mut self.device_log_frames_stream
    }

    // ---- donguru_api::Settings at device.settings ----

    async fn set_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::VbusMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Some(code) = self.state.settings_fault {
            return SetError(CommonError::Fault { code });
        }
        if let Err(e) = check_mode(&value) {
            return SetError(e);
        }
        self.state.settings.plug_vbus_at_boot = value;
        Set
    }

    async fn get_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.settings.plug_vbus_at_boot.clone())
    }

    async fn set_device_settings_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::ButtonAction,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Some(code) = self.state.settings_fault {
            return SetError(CommonError::Fault { code });
        }
        self.state.settings.button_action = value;
        self.state.button_action = value;
        Set
    }

    async fn get_device_settings_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::ButtonAction, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.settings.button_action)
    }

    async fn set_device_settings_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_si::Second,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Some(code) = self.state.settings_fault {
            return SetError(CommonError::Fault { code });
        }
        if let Err(e) = check_window(value) {
            return SetError(e);
        }
        self.state.settings.meter_window = value;
        self.state.meter_window = value;
        Set
    }

    async fn get_device_settings_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Second, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.settings.meter_window)
    }

    async fn set_device_settings_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::MeterDeadband,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Some(code) = self.state.settings_fault {
            return SetError(CommonError::Fault { code });
        }
        self.state.settings.meter_deadband = value.clone();
        self.state.meter_deadband = value;
        Set
    }

    async fn get_device_settings_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterDeadband, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.settings.meter_deadband.clone())
    }

    async fn set_device_settings_uart_plug(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_uart::UartConfig,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Some(code) = self.state.settings_fault {
            return SetError(CommonError::Fault { code });
        }
        self.state.settings.uart_plug = value;
        Set
    }

    async fn get_device_settings_uart_plug(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_uart::UartConfig, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.settings.uart_plug.clone())
    }

    async fn device_settings_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), wire_weaver::ww_error::CommonError<'_>>> {
        if let Some(code) = self.state.settings_fault {
            return Ready(Err(CommonError::Fault { code }));
        }
        self.state.settings = MockSettings::default();
        self.state.label.clear();
        Ready(Ok(()))
    }

    // ---- ww_indication::Indication at leds ----

    async fn set_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_indication::Mode,
    ) -> SetResult<()> {
        self.state.leds_mode = value;
        Set
    }

    async fn get_leds_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Mode, ()> {
        Value(self.state.leds_mode)
    }

    // optional, can be left out with omit = ["leds.brightness"]
    async fn set_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_indication::Brightness,
    ) -> SetResult<()> {
        self.state.leds_brightness = value;
        Set
    }

    async fn get_leds_brightness(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_indication::Brightness, ()> {
        Value(self.state.leds_brightness)
    }

    // optional, can be left out with omit = ["leds.identify"]
    async fn leds_identify(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        duration: ww_si::Second,
    ) -> RpcResult<Result<(), wire_weaver::ww_error::CommonError<'_>>> {
        if duration.to_f64() > 60.0 {
            // a range this device doesn't do; identify lists only Unsupported and Busy
            return Ready(Err(CommonError::Unsupported));
        }
        self.state.identified += 1;
        Ready(Ok(()))
    }

    // ---- donguru_api::PowerMeter at meter ----

    async fn get_meter_reading(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterReading, ()> {
        Value(self.state.reading.clone())
    }

    async fn get_meter_temperature(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::DegreeCelsius, ()> {
        Value(self.state.temperature)
    }

    async fn get_meter_totals(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterTotals, ()> {
        Value(self.state.totals.clone())
    }

    async fn meter_reset_totals(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        self.state.totals = MockState::zero_totals();
        self.mark_changed_meter_totals();
        Ready(())
    }

    async fn set_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_si::Second,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Err(e) = check_window(value) {
            return SetError(e);
        }
        self.state.meter_window = value;
        Set
    }

    async fn get_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Second, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.meter_window)
    }

    async fn set_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::MeterDeadband,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        self.state.meter_deadband = value;
        Set
    }

    async fn get_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterDeadband, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.meter_deadband.clone())
    }

    async fn get_meter_calibration(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterCalibration, ()> {
        Value(self.state.calibration.clone())
    }

    // ---- donguru_api::Plug at plug ----

    async fn get_plug_vbus(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusState, ()> {
        Value(self.state.plug_vbus)
    }

    async fn set_plug_vbus_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::VbusMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Err(e) = check_mode(&value) {
            return SetError(e);
        }
        self.state.plug_vbus = self.state.apply(&value);
        self.state.plug_vbus_mode = value;
        self.mark_changed_plug_vbus_mode();
        self.mark_changed_plug_vbus();
        Set
    }

    async fn get_plug_vbus_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.plug_vbus_mode.clone())
    }

    async fn plug_vbus_cycle(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        off: ww_si::Second,
        release_sbu: bool,
    ) -> RpcResult<Result<donguru_api::VbusState, donguru_api::PowerError<'_>>> {
        if matches!(self.state.plug_vbus_mode, VbusMode::ForceOff) {
            return Ready(Err(PowerError::Common(CommonError::WrongState {
                reason: MaxLen::new_unchecked("vbus_mode is ForceOff"),
            })));
        }
        if off.to_f64() < 0.0 || off.to_f64() > 10.0 {
            return Ready(Err(PowerError::Common(CommonError::InvalidArgument {
                arg: 0,
                reason: MaxLen::new_unchecked("0 to 10 s"),
            })));
        }
        let max = max_voltage(&self.state.plug_vbus_mode);
        if self.state.reading.vbus_in.to_f64() > max {
            return Ready(Err(PowerError::OverVoltage {
                measured: self.state.reading.vbus_in,
            }));
        }
        if release_sbu {
            self.state.sbu_function = SbuFunction::Off;
            self.mark_changed_sbu_function();
        }
        self.state.vbus_cycles += 1;
        self.state.plug_vbus = VbusState::On;
        self.mark_changed_plug_vbus();
        Ready(Ok(VbusState::On))
    }

    async fn set_plug_data(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::DataPath,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        self.state.plug_data = value;
        self.mark_changed_plug_data();
        Set
    }

    async fn get_plug_data(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::DataPath, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.plug_data)
    }

    async fn set_plug_orientation(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::Orientation,
    ) -> SetResult<()> {
        self.state.orientation = value;
        self.mark_changed_plug_orientation();
        Set
    }

    async fn get_plug_orientation(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::Orientation, ()> {
        Value(self.state.orientation)
    }

    async fn get_plug_cc(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::CcState, ()> {
        Value(self.state.cc)
    }

    // ---- donguru_api::Header at header ----

    async fn get_header_vbus_sw(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusState, ()> {
        Value(self.state.header_vbus)
    }

    async fn set_header_vbus_sw_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::VbusMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if let Err(e) = check_mode(&value) {
            return SetError(e);
        }
        self.state.header_vbus = self.state.apply(&value);
        self.state.header_vbus_mode = value;
        self.mark_changed_header_vbus_sw_mode();
        self.mark_changed_header_vbus_sw();
        Set
    }

    async fn get_header_vbus_sw_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.header_vbus_mode.clone())
    }

    async fn set_header_relay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: bool,
    ) -> SetResult<()> {
        self.state.relay = value;
        self.mark_changed_header_relay();
        Set
    }

    async fn get_header_relay(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<bool, ()> {
        Value(self.state.relay)
    }

    // ---- donguru_api::Sbu at sbu ----

    async fn get_sbu_function(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::SbuFunction, ()> {
        Value(self.state.sbu_function)
    }

    async fn sbu_release(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        self.state.sbu_function = SbuFunction::Off;
        self.state.sbu_pullups = false;
        self.mark_changed_sbu_function();
        Ready(())
    }

    async fn get_sbu_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Volt, ()> {
        Value(self.state.sbu_level)
    }

    async fn set_sbu_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: bool,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        if matches!(self.state.sbu_function, SbuFunction::Swd) {
            return SetError(CommonError::InUse {
                by: MaxLen::new_unchecked("swd"),
            });
        }
        self.state.sbu_pullups = value;
        Set
    }

    async fn get_sbu_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<bool, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.sbu_pullups)
    }

    async fn sbu_route_usb(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        enabled: bool,
    ) -> RpcResult<Result<(), donguru_api::SbuError<'_>>> {
        match self.state.variant {
            BoardVariant::Plus => {
                self.state.sbu_function = if enabled {
                    SbuFunction::Usb
                } else {
                    SbuFunction::Off
                };
                self.mark_changed_sbu_function();
                Ready(Ok(()))
            }
            _ => Ready(Err(SbuError::Common(CommonError::Unsupported))),
        }
    }

    // ---- donguru_api::Inputs at inputs ----

    async fn get_inputs_button(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::ButtonState, ()> {
        Value(self.state.button.clone())
    }

    async fn get_inputs_switch(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::SwitchPosition, ()> {
        Value(self.state.switch)
    }

    async fn inputs_events(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        RpcResult::Ready(())
    }

    async fn close_inputs_events(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    async fn stream_inputs_events(&mut self) -> &mut ww_stream::tx::Counted {
        &mut self.inputs_events_stream
    }

    async fn set_inputs_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::ButtonAction,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        self.state.button_action = value;
        Set
    }

    async fn get_inputs_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::ButtonAction, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.button_action)
    }

    // ---- donguru_api::Hub at hub ----

    async fn get_hub_state(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::HubState, ()> {
        Value(self.state.hub)
    }

    async fn hub_recover(&mut self, _cx: &mut Context<'_, impl EventOut>) -> RpcResult<()> {
        self.state.hub.recoveries += 1;
        self.state.hub.last_cause = Some(HubLoss::Requested);
        self.mark_changed_hub_state();
        Ready(())
    }

    // ---- donguru_api::Swd at swd ----

    async fn set_swd_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::SwdMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        self.state.swd_mode = value;
        Set
    }

    async fn get_swd_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::SwdMode, wire_weaver::ww_error::CommonError<'_>> {
        Value(self.state.swd_mode)
    }

    async fn get_swd_session(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::SwdSession, ()> {
        Value(self.state.swd_session.clone())
    }

    // ---- ww_uart::Uart at uart[] ----

    async fn uart_rx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<()> {
        RpcResult::Ready(())
    }

    async fn close_uart_rx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) {
    }

    async fn stream_uart_rx(&mut self, _index_chain: [UNib32; 1]) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    fn credit_uart_tx(&mut self, _index_chain: [UNib32; 1]) -> &mut ww_stream::sink::SinkCredit {
        &mut self.uart_tx_credit
    }

    async fn sideband_uart_tx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _sideband: ww_client_server::StreamSideband,
    ) -> Option<ww_client_server::StreamSideband> {
        // TODO: None sends no reply
        None
    }

    async fn write_uart_tx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _data: &wire_weaver::shrink_wrap::tail_bytes::TailBytes<'_>,
    ) {
    }

    async fn uart_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_uart::Capabilities<'_>> {
        Unimplemented.into()
    }

    async fn uart_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_uart::UartConfig,
    ) -> RpcResult<Result<ww_uart::UartConfig, ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    async fn uart_set_pin_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _pin: ww_uart::Pin,
        _is_high: bool,
    ) -> RpcResult<Result<(), ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    async fn uart_send_break(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _duration: ww_si::Second,
    ) -> RpcResult<Result<(), ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    async fn uart_tx_pending(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<u32> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["uart.monitor"]
    async fn uart_monitor(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_uart::MonitorConfig,
    ) -> RpcResult<Result<ww_uart::MonitorConfig, ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    async fn close_uart_monitor(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) {
    }

    async fn stream_uart_monitor(
        &mut self,
        _index_chain: [UNib32; 1],
    ) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    // ---- ww_i2c::I2c at i2c[] ----

    async fn i2c_i2c_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_i2c::I2cCapabilities<'_>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_i2c::I2cConfig,
    ) -> RpcResult<Result<ww_i2c::I2cConfig, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_write(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _addr: ww_i2c::I2cAddress,
        _data: wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 512>,
        _delay_after: Option<ww_si::Second>,
    ) -> RpcResult<Result<ww_i2c::I2cTransactionTimeRange, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _addr: ww_i2c::I2cAddress,
        _kind: ww_i2c::I2cReadKind<'_>,
        _len: u16,
        _delay_after: Option<ww_si::Second>,
    ) -> RpcResult<Result<ww_i2c::I2cReadEnvelope<'_>, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_scan(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 128>,
            ww_i2c::I2cError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    async fn i2c_i2c_transaction(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _addr: ww_i2c::I2cAddress,
        _ops: wire_weaver::shrink_wrap::MaxLen<
            wire_weaver::shrink_wrap::RefVec<'_, ww_i2c::I2cOp<'_>>,
            16,
        >,
    ) -> RpcResult<Result<ww_i2c::I2cTransactionResult<'_>, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_lines_state(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_i2c::I2cLinesState> {
        Unimplemented.into()
    }

    async fn i2c_i2c_wiggle_clock(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _count: u8,
    ) -> RpcResult<Result<ww_i2c::I2cLinesState, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_set_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _enabled: bool,
    ) -> RpcResult<Result<(), ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    async fn i2c_i2c_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<bool> {
        Unimplemented.into()
    }

    // ---- ww_gpio::Bank at gpio[] ----

    fn valid_indices_root_gpio_pin(&mut self, _index_chain: [UNib32; 1]) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..4)
    }

    async fn gpio_read_inputs(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _pins: wire_weaver::shrink_wrap::MaxLen<
            wire_weaver::shrink_wrap::RefVec<'_, wire_weaver::shrink_wrap::UNib32>,
            64,
        >,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<
                wire_weaver::shrink_wrap::RefVec<'_, ww_gpio::Level>,
                64,
            >,
            ww_gpio::GpioError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    async fn gpio_write_outputs(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _levels: wire_weaver::shrink_wrap::MaxLen<
            wire_weaver::shrink_wrap::RefVec<'_, ww_gpio::PinLevel>,
            64,
        >,
    ) -> RpcResult<Result<(), ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    async fn gpio_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_gpio::BankCapabilities<'_>> {
        Unimplemented.into()
    }

    async fn gpio_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_gpio::BankConfig,
    ) -> RpcResult<Result<ww_gpio::BankConfig, ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    async fn gpio_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<&'_ str> {
        Unimplemented.into()
    }

    // ---- ww_gpio::Pin at gpio[].pin[] ----

    async fn gpio_pin_set_output_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _level: ww_gpio::Level,
    ) -> RpcResult<()> {
        Unimplemented.into()
    }

    async fn gpio_pin_toggle(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<()> {
        Unimplemented.into()
    }

    async fn gpio_pin_output_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::Level> {
        Unimplemented.into()
    }

    async fn gpio_pin_input_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::Level> {
        Unimplemented.into()
    }

    async fn gpio_pin_event(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _enabled: ww_gpio::IoPinEnabledEvents<'_>,
    ) -> RpcResult<Result<(), ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    async fn close_gpio_pin_event(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) {
    }

    async fn stream_gpio_pin_event(
        &mut self,
        _index_chain: [UNib32; 2],
    ) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    async fn gpio_pin_set_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _mode: ww_gpio::Mode,
        _initial: Option<ww_gpio::Level>,
    ) -> RpcResult<Result<(), ww_gpio::GpioError<'_>>> {
        Unimplemented.into()
    }

    async fn gpio_pin_mode(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::Mode> {
        Unimplemented.into()
    }

    async fn set_gpio_pin_pull(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _value: ww_gpio::Pull,
    ) -> SetResult<ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    async fn get_gpio_pin_pull(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> GetResult<ww_gpio::Pull, ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    async fn set_gpio_pin_speed(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
        _value: ww_gpio::Speed,
    ) -> SetResult<ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    async fn get_gpio_pin_speed(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> GetResult<ww_gpio::Speed, ww_gpio::GpioError<'_>> {
        Unimplemented.into()
    }

    async fn gpio_pin_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 2],
    ) -> RpcResult<ww_gpio::PinCapabilities<'_>> {
        Unimplemented.into()
    }

    // ---- ww_adc::Adc at adc ----

    async fn adc_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _channel: u8,
    ) -> RpcResult<Result<ww_adc::Reading, ww_adc::AdcError<'_>>> {
        Unimplemented.into()
    }

    async fn adc_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<ww_adc::AdcCapabilities<'_>> {
        Unimplemented.into()
    }

    // ---- donguru_api::Bringup at debug ----

    async fn debug_pin_set(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        pin: donguru_api::McuPin,
        mode: donguru_api::PinMode,
        level: Option<ww_gpio::Level>,
    ) -> RpcResult<Result<donguru_api::PinState, donguru_api::DebugError<'_>>> {
        if let Err(e) = check_pin(pin) {
            return Ready(Err(e));
        }
        let state = self.state.pin_mut(pin);
        set_pin_mode(state, mode);
        if let Some(level) = level {
            state.output = matches!(level, Level::High);
            state.input = state.output;
        }
        Ready(Ok(*state))
    }

    async fn debug_pin_get(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        pin: donguru_api::McuPin,
    ) -> RpcResult<Result<donguru_api::PinState, donguru_api::DebugError<'_>>> {
        if pin.pin > 15 {
            return Ready(Err(invalid_pin()));
        }
        Ready(Ok(*self.state.pin_mut(pin)))
    }

    async fn debug_pin_af(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        pin: donguru_api::McuPin,
        af: u8,
    ) -> RpcResult<Result<donguru_api::PinState, donguru_api::DebugError<'_>>> {
        if let Err(e) = check_pin(pin) {
            return Ready(Err(e));
        }
        if af > 15 {
            return Ready(Err(DebugError::Common(CommonError::InvalidArgument {
                arg: 1,
                reason: MaxLen::new_unchecked("AF 0..=15"),
            })));
        }
        let state = self.state.pin_mut(pin);
        state.moder = 2;
        state.af = af;
        Ready(Ok(*state))
    }

    async fn debug_pins(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<donguru_api::PinDump<'_>> {
        self.dump = self.state.dump();
        Ready(PinDump {
            ports: MaxLen::new_unchecked(RefVec::Slice { slice: &self.dump }),
        })
    }

    async fn debug_boot_pins(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<donguru_api::PinDump<'_>> {
        self.dump = MockState::reset_dump();
        Ready(PinDump {
            ports: MaxLen::new_unchecked(RefVec::Slice { slice: &self.dump }),
        })
    }

    async fn debug_hub_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        reg: u8,
        len: u8,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 32>,
            donguru_api::DebugError<'_>,
        >,
    > {
        let (reg, len) = (reg as usize, len as usize);
        if len > 32 || reg + len > self.state.hub_regs.len() {
            return Ready(Err(DebugError::Nack));
        }
        Ready(Ok(MaxLen::new_unchecked(RefVec::new_bytes(
            &self.state.hub_regs[reg..reg + len],
        ))))
    }

    async fn debug_hub_write(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        reg: u8,
        data: wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 32>,
    ) -> RpcResult<Result<(), donguru_api::DebugError<'_>>> {
        let reg = reg as usize;
        if reg + data.len() > self.state.hub_regs.len() {
            return Ready(Err(DebugError::Nack));
        }
        self.state.hub_regs[reg..reg + data.len()].copy_from_slice(data.as_slice());
        Ready(Ok(()))
    }

    async fn debug_ucpd(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        port: donguru_api::UcpdPort,
    ) -> RpcResult<donguru_api::UcpdState> {
        Ready(self.state.ucpd(port))
    }

    async fn debug_ucpd_role(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        port: donguru_api::UcpdPort,
        role: donguru_api::CcRole,
    ) -> RpcResult<Result<donguru_api::UcpdState, donguru_api::DebugError<'_>>> {
        if port == UcpdPort::Ucpd1 && matches!(role, CcRole::Rp0A9 | CcRole::Rp1A5 | CcRole::Rp3A0)
        {
            return Ready(Err(DebugError::Common(CommonError::Refused {
                reason: MaxLen::new_unchecked("Rp on UCPD1 on rev A"),
            })));
        }
        self.state.ucpd_role[port as usize] = role;
        Ready(Ok(self.state.ucpd(port)))
    }

    async fn debug_led_set(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        led: donguru_api::Led,
        mode: donguru_api::LedMode,
    ) -> RpcResult<()> {
        self.state.leds[if led == Led::Power { 0 } else { 1 }] = mode;
        Ready(())
    }

    // the mock keeps no log (`device.log` answers Unimplemented): nothing to flood
    async fn debug_log_flood(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _count: u32,
    ) -> RpcResult<()> {
        Ready(())
    }

    async fn debug_reg_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        addr: u32,
        words: u8,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u32>, 32>,
            donguru_api::DebugError<'_>,
        >,
    > {
        let end = addr as u64 + 4 * words as u64;
        if words > 32 {
            return Ready(Err(DebugError::Common(CommonError::InvalidArgument {
                arg: 1,
                reason: MaxLen::new_unchecked("at most 32 words"),
            })));
        }
        if addr < 0x4000_0000 || end > 0x5000_2000 || !addr.is_multiple_of(4) {
            return Ready(Err(DebugError::Common(CommonError::Refused {
                reason: MaxLen::new_unchecked("peripheral space only"),
            })));
        }
        self.words = (0..words as u32).map(|i| addr + 4 * i).collect();
        Ready(Ok(MaxLen::new_unchecked(RefVec::Slice {
            slice: &self.words,
        })))
    }

    fn new_stream_session(&mut self) -> u32 {
        // TODO: put a boot counter into the high bits (or use a random number), so that a cursor from
        // before a reboot doesn't match a new session
        self.stream_sessions = self.stream_sessions.wrapping_add(1);
        self.stream_sessions
    }

    fn observe_state(&mut self) -> &mut api_impl::ObserveState<api_impl::ObserveBatches> {
        &mut self.observe
    }
}

/// Values of the `const` items. `FIRMWARE` names the mock, so a host can tell it from a real dongle.
impl api_impl::DonguruApiConsts for MockServer {
    const DEVICE_FIRMWARE_FIRMWARE: FirmwareSummary<'static> = FirmwareSummary {
        name: MaxLen::new_unchecked("donguru-mock"),
        version: Version::new(0, 2, 0),
        git: None,
        built: None,
        profile: BuildProfile::Debug,
    };

    const DEVICE_FIRMWARE_BUILD_ID: MaxLen<RefVec<'static, u8>, 32> =
        MaxLen::new_unchecked(RefVec::new_bytes(&[]));

    const DEVICE_FIRMWARE_BUILD_RECORD: MaxLen<RefVec<'static, u8>, 512> =
        MaxLen::new_unchecked(RefVec::new_bytes(&[]));

    fn device_board_board(&self) -> BoardSummary<'_> {
        BoardSummary {
            name: MaxLen::new_unchecked("Donguru B153"),
            revision: MaxLen::new_unchecked("A"),
            revision_source: RevisionSource::Assumed,
            variant: MaxLen::new_unchecked(match self.state.variant {
                BoardVariant::Main => "Main",
                BoardVariant::Plus => "Plus",
                _ => "Unknown",
            }),
            bom_variant: MaxLen::new_unchecked(""),
            serial: MaxLen::new_unchecked(self.state.serial.as_str()),
            build_date: None,
        }
    }

    fn device_uid_ids(&self) -> MaxLen<RefVec<'_, ww_uid::UniqueId<'_>>, 4> {
        MaxLen::new_unchecked(RefVec::Slice { slice: &self.uid })
    }

    const DEVICE_LABEL_CAPABILITIES: ww_user_info::UserInfoCaps = ww_user_info::UserInfoCaps {
        name_max: 32,
        notes_max: 256,
        persistent: false,
    };

    fn device_counters_info(&self) -> ww_counters::CountersInfo<'_> {
        ww_counters::CountersInfo {
            format: 2,
            ram: ww_counters::BufferInfo {
                used_words: 0,
                capacity_words: 0,
            },
            backup: None,
            build_id: MaxLen::new_unchecked(RefVec::new_bytes(&[])),
        }
    }

    const DEVICE_LOG_DEFMT: ww_log::DefmtInfo<'static> = ww_log::DefmtInfo {
        encoding: ww_log::DefmtEncoding::Rzcobs,
        build_id: MaxLen::new_unchecked(RefVec::new_bytes(&[])),
    };

    const LEDS_NAMES: RefVec<'static, &'static str> = RefVec::Slice {
        slice: &["power", "status"],
    };
}

pub mod api_impl {
    wire_weaver::ww_codegen!(
        donguru_api :: DonguruApi for super::MockServer,
        server = true, no_alloc = true, use_async = true,
        method_model = "_=immediate",
        property_model = "_=get_set",
        const_model = "BOARD|IDS|INFO=fn",
        // as the firmware: no I2C slave mode (ww_i2c STD-35), its group answers Unimplemented
        omit = ["i2c.slave"],
        introspect = "with_docs",
    );
}
