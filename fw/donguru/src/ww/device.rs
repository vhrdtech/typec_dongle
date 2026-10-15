//! `device`: the device contract (API-9, API-10) and the settings (API-19), and the values of the `const` items.

use super::ServerState;
use crate::{hub, last_reset};
use core::sync::atomic::AtomicU32;
use donguru_api::BoardVariant;
use wire_weaver::prelude::*;
use ww_board_info::{BoardSummary, RevisionSource};
use ww_counters::{BufferInfo, CountersInfo};
use ww_counters::{CounterBuffer, read_words, reset_words};
use ww_firmware_info::{BUILD_ID_MAX_LEN, FirmwareSummary};
use ww_log::DefmtInfo;
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
        self.label.name.set(&value);
        Set
    }

    pub(super) async fn get_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>, ww_user_info::UserInfoError<'_>>
    {
        Value(MaxLen::new_unchecked(self.label.name.get()))
    }

    // optional, can be left out with omit = ["device.label.notes"]
    pub(super) async fn set_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        self.label.notes.set(&value);
        Set
    }

    pub(super) async fn get_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>, ww_user_info::UserInfoError<'_>>
    {
        Value(MaxLen::new_unchecked(self.label.notes.get()))
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
        _value: donguru_api::VbusMode,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::VbusMode, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_device_settings_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: donguru_api::ButtonAction,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_settings_button_action(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::ButtonAction, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_device_settings_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: ww_si::Second,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_settings_meter_window(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_si::Second, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_device_settings_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: donguru_api::MeterDeadband,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_settings_meter_deadband(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<donguru_api::MeterDeadband, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn set_device_settings_uart_plug(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: ww_uart::UartConfig,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_settings_uart_plug(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<ww_uart::UartConfig, wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn device_settings_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), wire_weaver::ww_error::CommonError<'_>>> {
        Unimplemented.into()
    }
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

    // RAM only until the settings store (API-19)
    const DEVICE_LABEL_CAPABILITIES: UserInfoCaps = UserInfoCaps {
        name_max: MAX_NAME_LEN as u16,
        notes_max: MAX_NOTES_LEN as u16,
        persistent: false,
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

/// `device.label`: name and notes, in RAM until the settings store (API-19).
pub(super) struct Label {
    pub(super) name: Text<MAX_NAME_LEN>,
    pub(super) notes: Text<MAX_NOTES_LEN>,
}

impl Label {
    pub(super) const fn new() -> Self {
        Label {
            name: Text::new(),
            notes: Text::new(),
        }
    }
}

/// UTF-8 text of up to `N` bytes. The bound is checked when the request is read (`MaxLen`), so `set` never cuts.
pub(super) struct Text<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    const fn new() -> Self {
        Text {
            buf: [0; N],
            len: 0,
        }
    }

    pub(super) fn set(&mut self, s: &str) {
        let len = s.len().min(N);
        self.buf[..len].copy_from_slice(&s.as_bytes()[..len]);
        self.len = len;
    }

    pub(super) fn get(&self) -> &str {
        // only ever a whole `&str` (or empty) is copied in
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}
