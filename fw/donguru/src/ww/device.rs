//! `device`: the device contract (API-9, API-10) and the settings (API-19), and the values of the `const` items.

use super::{ServerState, meter};
use crate::power::{self, Which};
use crate::settings::UserSettings;
use crate::{hub, last_reset};
use cnt::cnt;
use core::sync::atomic::AtomicU32;
use defmt::{info, warn};
use donguru_api::BoardVariant;
use donguru_power::Limits;
use embassy_time::Instant;
use wire_weaver::prelude::*;
use wire_weaver::shrink_wrap::ArrayString;
use wire_weaver::ww_error::CommonError;
use ww_board_info::{BoardSummary, RevisionSource};
use ww_counters::{BufferInfo, CountersInfo};
use ww_counters::{CounterBuffer, read_words, reset_words};
use ww_firmware_info::{BUILD_ID_MAX_LEN, FirmwareSummary};
use ww_log::DefmtInfo;
use ww_settings::Persisted;
use ww_uid::UniqueId;
use ww_user_info::{MAX_NAME_LEN, MAX_NOTES_LEN, UserInfoCaps};

impl ServerState {
    // donguru_api::Device at device

    pub(super) async fn get_device_variant(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::BoardVariant, ()> {
        Value(hub::board_variant())
    }

    // ww_firmware_info::FirmwareInfo at device.firmware

    // ww_firmware_info::LastReset at device.last_reset

    pub(super) async fn device_last_reset_last_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<ww_firmware_info::ResetRecord<'_>> {
        Ready(last_reset::record(&self.last_reset))
    }

    pub(super) async fn device_last_reset_boot_count(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<u32> {
        Ready(self.last_reset.boot_count())
    }

    pub(super) async fn device_last_reset_clear(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        bedrock_crash::clear(&mut self.last_reset);
        Ready(())
    }

    // ww_board_info::BoardInfo at device.board

    // ww_uid::Uid at device.uid

    // ww_user_info::UserInfo at device.label

    pub(super) async fn set_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        // the bound is checked when the request is read (MaxLen), so this never cuts
        let name = ArrayString::try_from(*value).unwrap_or_default();
        match {
            self.user.set(now_ms(), |s| s.label = name);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e.into()),
        }
    }

    pub(super) async fn get_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>, ww_user_info::UserInfoError<'_>>
    {
        Value(MaxLen::new_unchecked(self.user.get().label.as_str()))
    }

    // optional, can be left out with omit = ["device.label.notes"]
    pub(super) async fn set_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        let notes = ArrayString::try_from(*value).unwrap_or_default();
        match {
            self.user.set(now_ms(), |s| s.notes = notes);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e.into()),
        }
    }

    pub(super) async fn get_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>, ww_user_info::UserInfoError<'_>>
    {
        Value(MaxLen::new_unchecked(self.user.get().notes.as_str()))
    }

    // ww_counters::Counters at device.counters

    pub(super) async fn device_counters_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        buffer: ww_counters::CounterBuffer,
        offset: u16,
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<
                wire_weaver::shrink_wrap::RefVec<'_, wire_weaver::shrink_wrap::UVlq32>,
                64,
            >,
            ww_counters::CountersError<'_>,
        >,
    > {
        let (words, used) = counter_buffer(buffer);
        Ready(read_words(words, used, offset, &mut self.counters_page))
    }

    pub(super) async fn device_counters_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        buffer: ww_counters::CounterBuffer,
    ) -> RpcResult<Result<(), ww_counters::CountersError<'_>>> {
        reset_words(counter_buffer(buffer).0);
        Ready(Ok(()))
    }

    // optional, can be left out with omit = ["device.counters.watch"]; polling `read` covers USB
    pub(super) async fn device_counters_watch(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _period: ww_si::Second,
    ) -> RpcResult<Result<ww_si::Second, ww_counters::CountersError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn close_device_counters_watch(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) {
    }

    pub(super) async fn stream_device_counters_watch(&mut self) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }

    // ww_log::LogDefmt at device.log: the defmt log (src/log.rs). Its session starts at boot, so a host that resumes
    // it gets the log from before it connected (and from before a crash); opening it anew drops what is kept.

    pub(super) async fn device_log_frames(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), ww_log::LogError<'_>>> {
        Ready(Ok(()))
    }

    pub(super) async fn close_device_log_frames(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    pub(super) async fn stream_device_log_frames(&mut self) -> &mut crate::log::Stream {
        &mut self.log_frames
    }

    // donguru_api::Settings at device.settings

    pub(super) async fn set_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::VbusMode,
    ) -> SetResult<CommonError<'_>> {
        // what plug.vbus_mode would refuse whatever VBUS is; the voltage is checked at boot
        if Limits::from_mode(&value).is_err() {
            return SetError(meter::invalid("not a mode plug.vbus_mode takes"));
        }
        match {
            self.user.set(now_ms(), |s| s.plug_vbus_at_boot = value);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e),
        }
    }

    pub(super) async fn get_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, CommonError<'_>> {
        Value(self.user.get().plug_vbus_at_boot.clone())
    }

    // stored only: the button does nothing on its own until FW-4 / API-13
    pub(super) async fn set_device_settings_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::ButtonAction,
    ) -> SetResult<CommonError<'_>> {
        match {
            self.user.set(now_ms(), |s| s.button_action = value);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e),
        }
    }

    pub(super) async fn get_device_settings_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::ButtonAction, CommonError<'_>> {
        Value(self.user.get().button_action)
    }

    pub(super) async fn set_device_settings_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_si::Second,
    ) -> SetResult<CommonError<'_>> {
        let Some(us) = meter::window_us_of(&value) else {
            return SetError(meter::invalid(meter::WINDOW_INVALID));
        };
        crate::adc::set_window_us(us);
        match {
            self.user.set(now_ms(), |s| s.meter_window = value);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e),
        }
    }

    pub(super) async fn get_device_settings_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Second, CommonError<'_>> {
        Value(self.user.get().meter_window)
    }

    pub(super) async fn set_device_settings_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: donguru_api::MeterDeadband,
    ) -> SetResult<CommonError<'_>> {
        let Some((mv, ma)) = meter::deadband_of(&value) else {
            return SetError(meter::invalid(meter::DEADBAND_INVALID));
        };
        self.meter.set_deadband(mv, ma);
        match {
            self.user.set(now_ms(), |s| s.meter_deadband = value);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e),
        }
    }

    pub(super) async fn get_device_settings_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterDeadband, CommonError<'_>> {
        Value(self.user.get().meter_deadband.clone())
    }

    // stored only: uart[0] comes with API-14
    pub(super) async fn set_device_settings_uart_plug(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: ww_uart::UartConfig,
    ) -> SetResult<CommonError<'_>> {
        match {
            self.user.set(now_ms(), |s| s.uart_plug = value);
            self.persist_settings().await
        } {
            Ok(()) => Set,
            Err(e) => SetError(e),
        }
    }

    pub(super) async fn get_device_settings_uart_plug(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_uart::UartConfig, CommonError<'_>> {
        Value(self.user.get().uart_plug.clone())
    }

    pub(super) async fn device_settings_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), CommonError<'_>>> {
        self.user.reset_to_defaults(now_ms());
        // the live values a settings write sets go back with them; plug VBUS is only a boot value
        let defaults = UserSettings::default();
        if let Some(us) = meter::window_us_of(&defaults.meter_window) {
            crate::adc::set_window_us(us);
        }
        if let Some((mv, ma)) = meter::deadband_of(&defaults.meter_deadband) {
            self.meter.set_deadband(mv, ma);
        }
        Ready(self.persist_settings().await)
    }
}

impl ServerState {
    /// Writes the record before answering, so `Ok` means the settings survive a power cut (what `ww_user_info`
    /// promises for the label; the same for every setting). One copy for every handler: they change the settings in
    /// RAM, then await this.
    async fn persist_settings(&mut self) -> Result<(), CommonError<'static>> {
        match self.user.persist(self.store).await {
            Ok(Persisted::Written(stats)) => {
                info!("settings written: {}", stats);
                self.settings_maintain = true;
                Ok(())
            }
            Ok(Persisted::Unchanged) => Ok(()),
            Err(e) => {
                cnt!(settings_write_errors: u32, warn);
                warn!("settings not written: {}", e);
                Err(CommonError::Fault {
                    code: crate::settings::fault_code(&e),
                })
            }
        }
    }

    /// Boot values from the settings that only apply at boot: plug VBUS, through the guard once the ADC has a
    /// reading (a guarded mode is refused without one, or above its `max_voltage`: VBUS stays off then).
    pub(super) async fn apply_boot_settings(&mut self) {
        if let Some(us) = meter::window_us_of(&self.user.get().meter_window) {
            crate::adc::set_window_us(us);
        }
        let mode = self.user.get().plug_vbus_at_boot.clone();
        if mode != donguru_api::VbusMode::ForceOff {
            let _ = meter::readings().await;
            if power::set_mode(Which::Plug, mode).is_err() {
                cnt::bkp_cnt!(plug_vbus_at_boot_refused: u32, warn);
            }
        }
    }

    /// Settings work for a quiet moment of the server task: the erase ahead after a write (so the write that starts
    /// a new page doesn't erase), and a write that failed before, tried again once its debounce is over.
    pub(super) async fn settings_idle(&mut self) {
        if self.user.due_at().is_some_and(|at| at <= now_ms()) {
            let _ = self.persist_settings().await;
        }
        if core::mem::take(&mut self.settings_maintain)
            && let Err(e) = self.store.maintain().await
        {
            cnt!(settings_erase_errors: u32, warn);
            warn!("settings erase ahead failed: {}", e);
        }
    }
}

fn now_ms() -> u64 {
    Instant::now().as_millis()
}

/// Values of the `const` items, served without a handler and read once per connection by a host.
impl super::server_impl::DonguruApiConsts for ServerState {
    // name, version, git commit, build time and profile, from build.rs (ww_firmware_info::build::emit())
    const DEVICE_FIRMWARE_FIRMWARE: FirmwareSummary<'static> =
        ww_firmware_info::firmware_summary!();

    // GNU build id, kept in flash by memory.x: a host finds this firmware's ELF by it
    fn device_firmware_build_id(&self) -> MaxLen<RefVec<'_, u8>, BUILD_ID_MAX_LEN> {
        MaxLen::new_unchecked(RefVec::new_bytes(ww_firmware_info::build_id()))
    }

    // revision: the firmware is built for rev A; the variant comes from the PA10 strap (FW-22)
    fn device_board_board(&self) -> BoardSummary<'_> {
        let variant = match hub::board_variant() {
            BoardVariant::Main => "Main",
            BoardVariant::Plus => "Plus",
            _ => "Unknown",
        };
        BoardSummary {
            name: MaxLen::new_unchecked("Donguru B153"),
            revision: MaxLen::new_unchecked("A"),
            revision_source: RevisionSource::Assumed,
            variant: MaxLen::new_unchecked(variant),
            bom_variant: MaxLen::new_unchecked(""),
            serial: MaxLen::new_unchecked(embassy_stm32::uid::uid_hex()),
            build_date: None,
        }
    }

    fn device_uid_ids(&self) -> MaxLen<RefVec<'_, UniqueId<'_>>, 4> {
        MaxLen::new_unchecked(RefVec::Slice { slice: &self.uid })
    }

    // kept in the settings store (API-19)
    const DEVICE_LABEL_CAPABILITIES: UserInfoCaps = UserInfoCaps {
        name_max: MAX_NAME_LEN as u16,
        notes_max: MAX_NOTES_LEN as u16,
        persistent: true,
    };

    fn device_counters_info(&self) -> CountersInfo<'_> {
        let bkp = cnt::counters_bkp_buffer();
        CountersInfo {
            format: cnt::SIGNATURE_VERSION,
            ram: BufferInfo::new(cnt::ram_used_words(), cnt::counters_ram_buffer()),
            backup: (!bkp.is_empty()).then(|| BufferInfo::new(cnt::bkp_used_words(), bkp)),
            build_id: MaxLen::new_unchecked(RefVec::new_bytes(ww_firmware_info::build_id())),
        }
    }

    // the encoding defmt is built with, found at run time; a host decodes the frames with the ELF of this build id
    fn device_log_defmt(&self) -> DefmtInfo<'_> {
        DefmtInfo {
            encoding: crate::log::encoding(),
            build_id: MaxLen::new_unchecked(RefVec::new_bytes(ww_firmware_info::build_id())),
        }
    }

    const LEDS_NAMES: RefVec<'static, &'static str> = super::leds::NAMES;
}

/// A `cnt` buffer and its used words.
fn counter_buffer(buffer: CounterBuffer) -> (&'static [AtomicU32], usize) {
    match buffer {
        CounterBuffer::Ram => (cnt::counters_ram_buffer(), cnt::ram_used_words()),
        CounterBuffer::Backup => (cnt::counters_bkp_buffer(), cnt::bkp_used_words()),
    }
}
