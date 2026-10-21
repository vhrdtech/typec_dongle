//! `device.boot` (`ww_dfu::BootControl`) and `device.dfu.node` (API-58): firmware updates through the STM32 ROM
//! bootloader (FW-12). The firmware sits alone at the start of flash: `reboot(Updater)` resets into the ROM (USB DFU
//! behind the hub, `ww stm32dfu flash`), the boot state is always `Confirmed`, there is nothing to revert to. The
//! resets themselves are `crate::boot`'s.

use super::ServerState;
use crate::boot::{REBOOT, Reboot};
use wire_weaver::prelude::*;
use ww_dfu::{
    BootState, BootTarget, Error, LastResult, NodeInfo, Offers, Outcome, Profile, Role, Seal,
};

impl ServerState {
    pub(super) fn device_boot_boot_state(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<BootState> {
        Ready(BootState::Confirmed)
    }

    /// Nothing on trial: `Ok` without effect (`ww_dfu`).
    pub(super) fn device_boot_confirm(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), Error>> {
        Ready(Ok(()))
    }

    /// No previous image to go back to (`ww_dfu`: `Unsupported` without one).
    pub(super) fn device_boot_revert(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<Result<(), Error>> {
        Ready(Err(Error::Unsupported))
    }

    /// Reset into the firmware or into the STM32 ROM bootloader as the updater. The reply goes out first: the reset
    /// is done by `crate::boot::reboot_task` a few tens of milliseconds later.
    pub(super) fn device_boot_reboot(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        into: BootTarget,
    ) -> RpcResult<Result<(), Error>> {
        REBOOT.signal(match into {
            BootTarget::App => Reboot::App,
            BootTarget::Updater => Reboot::RomBootloader,
        });
        Ready(Ok(()))
    }

    /// The firmware in place, updated by the ROM bootloader (`device.boot` only, like an RP2350 with BOOTSEL).
    /// Images are not sealed: no product id, version, floor or manifest.
    pub(super) fn device_dfu_node_node(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
    ) -> RpcResult<NodeInfo<'_>> {
        Ready(NodeInfo {
            role: Role::App,
            profile: Profile::InPlace,
            product: MaxLen::new_unchecked("Donguru B153"),
            geometry_id: 0,
            product_id: 0,
            seal: Seal::Unsigned,
            app_version: None,
            version_floor: 0,
            manifest_digest: None,
            boot_state: BootState::Confirmed,
            // the ROM bootloader took over after a boot loop (FW-45); requested updates are not tracked
            last_result: LastResult {
                outcome: if crate::boot::fell_back() {
                    Outcome::Recovery
                } else {
                    Outcome::Nothing
                },
                version: 0,
            },
            offers: Offers {
                boot_control: true,
                inventory: false,
                stage: false,
                stage_bulk: false,
            },
        })
    }
}
