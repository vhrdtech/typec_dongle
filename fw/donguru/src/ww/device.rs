//! `device`: the device contract (API-9, API-10) and the settings (API-19), and the values of the `const` items.

use super::ServerState;
use crate::hub;
use donguru_api::BoardVariant;
use wire_weaver::prelude::*;
use ww_board_info::{BoardSummary, RevisionSource};
use ww_counters::{BufferInfo, CountersInfo};
use ww_firmware_info::{BuildProfile, FirmwareSummary, Version, bounded_str, parse_u32};
use ww_log::{DefmtEncoding, DefmtInfo};
use ww_uid::UniqueId;
use ww_user_info::UserInfoCaps;

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
        Unimplemented.into()
    }

    pub(super) async fn device_last_reset_boot_count(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<u32> {
        Unimplemented.into()
    }

    pub(super) async fn device_last_reset_clear(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<()> {
        Unimplemented.into()
    }

    // ww_board_info::BoardInfo at device.board

    // ww_uid::Uid at device.uid

    // ww_user_info::UserInfo at device.label

    pub(super) async fn set_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_label_name(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 32>, ww_user_info::UserInfoError<'_>>
    {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["device.label.notes"]
    pub(super) async fn set_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>,
    ) -> SetResult<ww_user_info::UserInfoError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_label_notes(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<wire_weaver::shrink_wrap::MaxLen<&'_ str, 256>, ww_user_info::UserInfoError<'_>>
    {
        Unimplemented.into()
    }

    // ww_counters::Counters at device.counters

    pub(super) async fn device_counters_read(
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

    pub(super) async fn device_counters_reset(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _buffer: ww_counters::CounterBuffer,
    ) -> RpcResult<Result<(), ww_counters::CountersError<'_>>> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["device.counters.watch"]
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

    // ww_log::LogDefmt at device.log

    pub(super) async fn device_log_frames(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), ww_log::LogError<'_>>> {
        Unimplemented.into()
    }

    pub(super) async fn close_device_log_frames(&mut self, _cx: &mut Context<'_, impl EventOut>) {}

    pub(super) async fn stream_device_log_frames(
        &mut self,
    ) -> &mut ww_stream::tx::Reliable<super::LogStore> {
        &mut self.log_frames
    }

    // donguru_api::Settings at device.settings

    pub(super) async fn set_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _value: bool,
    ) -> SetResult<wire_weaver::ww_error::CommonError<'_>> {
        Unimplemented.into()
    }

    pub(super) async fn get_device_settings_plug_vbus_at_boot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> GetResult<bool, wire_weaver::ww_error::CommonError<'_>> {
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

/// Values of the `const` items, served without a handler. Minimal until the device contract is filled in (API-9:
/// build id, git and build time from the template's `build.rs`).
impl super::server_impl::DonguruApiConsts for ServerState {
    const DEVICE_FIRMWARE_FIRMWARE: FirmwareSummary<'static> = FirmwareSummary {
        name: bounded_str(env!("CARGO_PKG_NAME")),
        version: Version::new(
            parse_u32(env!("CARGO_PKG_VERSION_MAJOR")),
            parse_u32(env!("CARGO_PKG_VERSION_MINOR")),
            parse_u32(env!("CARGO_PKG_VERSION_PATCH")),
        ),
        git: None,
        built: None,
        profile: if cfg!(debug_assertions) {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        },
    };

    // no build id note in flash yet (API-9)
    const DEVICE_FIRMWARE_BUILD_ID: MaxLen<RefVec<'static, u8>, 32> =
        MaxLen::new_unchecked(RefVec::new_bytes(&[]));

    const DEVICE_FIRMWARE_BUILD_RECORD: MaxLen<RefVec<'static, u8>, 512> =
        MaxLen::new_unchecked(RefVec::new_bytes(&[]));

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

    const DEVICE_LABEL_CAPABILITIES: UserInfoCaps = UserInfoCaps {
        name_max: 32,
        notes_max: 0,
        persistent: false,
    };

    // the whole buffers until `cnt` tells the used words (API-9)
    fn device_counters_info(&self) -> CountersInfo<'_> {
        let ram = cnt::counters_ram_buffer().len() as u16;
        let bkp = cnt::counters_bkp_buffer().len() as u16;
        CountersInfo {
            format: 2,
            ram: BufferInfo {
                used_words: ram,
                capacity_words: ram,
            },
            backup: (bkp > 0).then_some(BufferInfo {
                used_words: bkp,
                capacity_words: bkp,
            }),
            build_id: MaxLen::new_unchecked(RefVec::new_bytes(&[])),
        }
    }

    const DEVICE_LOG_DEFMT: DefmtInfo<'static> = DefmtInfo {
        encoding: DefmtEncoding::Rzcobs,
        build_id: MaxLen::new_unchecked(RefVec::new_bytes(&[])),
    };

    const LEDS_NAMES: RefVec<'static, &'static str> = super::leds::NAMES;
}
