//! Firmware version and firmware update over USB without a probe (CLI-10, FW-12).
//!
//! The update asks the running firmware to reboot into the STM32 ROM bootloader (`device.boot.reboot(Updater)`),
//! follows the dongle by its USB serial number (the same in the firmware and in the ROM, FW-12) while the image is
//! written and verified over DFU, and connects to the firmware that came back to tell the new version.

use std::fmt;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use wire_weaver_client::DeviceInfo;
use wire_weaver_client::follow::{self, Bus, Follow, Timeouts};
use wire_weaver_client::ww_stm32_dfu::{self, Image, Stage};

use crate::device::{DeviceEntry, Dongle};
use crate::{Error, Result};

/// What a firmware says about itself (`device.firmware`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FirmwareVersion {
    pub name: String,
    pub version: String,
    /// First 7 hex digits of the commit it was built from, `None` when built outside git.
    pub git: Option<String>,
    /// Uncommitted changes were present at build time.
    pub dirty: bool,
    pub profile: String,
}

impl FirmwareVersion {
    pub fn from_summary(s: &ww_firmware_info::FirmwareSummaryOwned) -> Self {
        let v = &s.version;
        let (git, dirty) = match &s.git {
            Some(g) => {
                let hex: String = g.commit.iter().map(|b| format!("{b:02x}")).collect();
                (Some(hex.chars().take(7).collect()), g.dirty)
            }
            None => (None, false),
        };
        let profile = match &s.profile {
            ww_firmware_info::BuildProfileOwned::Release => "release".to_string(),
            ww_firmware_info::BuildProfileOwned::Debug => "debug".to_string(),
            ww_firmware_info::BuildProfileOwned::Other(name) => name.as_str().to_string(),
            _ => "unknown".to_string(),
        };
        FirmwareVersion {
            name: s.name.as_str().to_string(),
            version: format!("{}.{}.{}", v.major, v.minor, v.patch),
            git,
            dirty,
            profile,
        }
    }
}

impl fmt::Display for FirmwareVersion {
    /// `donguru 0.20.18 (a1b2c3d-dirty, release)`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} (", self.name, self.version)?;
        if let Some(git) = &self.git {
            write!(f, "{git}{}, ", if self.dirty { "-dirty" } else { "" })?;
        }
        write!(f, "{})", self.profile)
    }
}

/// A firmware image to write: an ELF file, as `cargo build` makes it.
pub struct FirmwareImage {
    image: Image,
}

impl FirmwareImage {
    pub fn from_elf(bytes: &[u8]) -> Result<Self> {
        let image = Image::from_elf(bytes)
            .map_err(|e| Error::Link(format!("reading the firmware ELF: {e}")))?;
        Ok(FirmwareImage { image })
    }

    pub fn from_elf_file(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Link(format!("reading {}: {e}", path.display())))?;
        Self::from_elf(&bytes)
    }

    /// Bytes to write.
    pub fn len(&self) -> usize {
        self.image.len()
    }

    pub fn is_empty(&self) -> bool {
        self.image.is_empty()
    }
}

/// Where an update is, for a progress bar.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UpdateStep {
    /// Asking the firmware to reboot into the ROM bootloader.
    Reboot,
    /// Waiting for the ROM bootloader to show up on USB.
    WaitBootloader,
    Erase,
    Write,
    Verify,
    /// Waiting for the new firmware to show up on USB.
    WaitFirmware,
}

impl fmt::Display for UpdateStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UpdateStep::Reboot => "reboot into the bootloader",
            UpdateStep::WaitBootloader => "wait for the bootloader",
            UpdateStep::Erase => "erase",
            UpdateStep::Write => "write",
            UpdateStep::Verify => "verify",
            UpdateStep::WaitFirmware => "wait for the firmware",
        })
    }
}

/// How an update went.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateReport {
    /// USB serial number, the same before and after.
    pub serial: String,
    /// `None` when the old firmware's `device.firmware` could not be read.
    pub old: Option<FirmwareVersion>,
    pub new: FirmwareVersion,
    /// Bytes written.
    pub bytes: usize,
    pub seconds: f64,
}

/// Timeouts of the steps of an update.
pub type UpdateTimeouts = Timeouts;

impl crate::levels::Device<'_> {
    /// The running firmware's name, version and build.
    pub async fn firmware_version(&self) -> Result<FirmwareVersion> {
        Ok(FirmwareVersion::from_summary(&self.firmware().await?))
    }
}

impl Dongle {
    /// Updates the firmware of this dongle and connects to the new one. See [update_with] for what happens.
    pub async fn update_firmware(
        self,
        image: &FirmwareImage,
        mut on_step: impl FnMut(UpdateStep, u64, u64),
    ) -> Result<UpdateReport> {
        update_with(
            self,
            image,
            &mut follow::UsbBus,
            async |info: &DeviceInfo| {
                let serial = info.serials.first().cloned().unwrap_or_default();
                ww_stm32_dfu::usb::NusbIo::open_by_serial(&serial, 0).await
            },
            async |serial: &str| {
                let entries = crate::device::list().await?;
                let sel = crate::Selector::Serial(serial.to_string());
                Dongle::open_entry(crate::device::select(&entries, Some(&sel))?.clone()).await
            },
            Timeouts::default(),
            &mut on_step,
        )
        .await
    }
}

/// The USB serial number the ROM bootloader reports for this dongle: the firmware's own serial when it is already in
/// the ROM's 12-hex form (FW-12), else computed from the UID (older firmware, which has no `device.boot` anyway).
fn rom_serial(entry: &DeviceEntry, uid_hex: Option<&str>) -> String {
    if entry.serial.len() == 12 {
        return entry.serial.clone();
    }
    let words = uid_hex.filter(|h| h.len() == 24).and_then(|h| {
        let word = |i: usize| u32::from_str_radix(&h[i * 8..i * 8 + 8], 16).ok();
        // the UID is shown as the bytes of UID[0], UID[1], UID[2], little-endian
        let le = |i: usize| word(i).map(u32::swap_bytes);
        Some([le(0)?, le(1)?, le(2)?])
    });
    match words {
        Some(w) => follow::stm32_serial(w),
        None => entry.serial.clone(),
    }
}

/// The update, with the bus, the DFU interface and the reconnect given (a simulation in the tests):
/// 1. read the old firmware's version,
/// 2. `device.boot.reboot(Updater)`, close the link,
/// 3. wait for the ROM bootloader with the same serial, erase, write and verify `image`, jump to it,
/// 4. wait for the firmware to come back, `reconnect` to it and read its version.
pub async fn update_with<I: ww_stm32_dfu::DfuIo>(
    dongle: Dongle,
    image: &FirmwareImage,
    bus: &mut impl Bus,
    open: impl AsyncFn(&DeviceInfo) -> std::result::Result<I, ww_stm32_dfu::Error>,
    reconnect: impl AsyncFn(&str) -> Result<Dongle>,
    timeouts: Timeouts,
    on_step: &mut impl FnMut(UpdateStep, u64, u64),
) -> Result<UpdateReport> {
    let started = Instant::now();
    let entry = dongle.entry().clone();
    let old = dongle.device().firmware_version().await.ok();
    let uid = dongle.device().uid().await.ok();
    let serial = rom_serial(&entry, uid.as_deref());

    on_step(UpdateStep::Reboot, 0, 1);
    dongle.device().reboot(ww_dfu::BootTarget::Updater).await?;
    // the firmware resets right away: the link is gone, closing it can fail
    let _ = dongle.close().await;
    on_step(UpdateStep::WaitBootloader, 0, 1);

    let mut progress = |stage: Stage, done: u64, total: u64| {
        let step = match stage {
            Stage::Erase => UpdateStep::Erase,
            Stage::Write => UpdateStep::Write,
            Stage::Verify => UpdateStep::Verify,
        };
        on_step(step, done, total);
    };
    // the wait for the firmware is the last thing `stm32_dfu_update_with` does: the bar says so from the verify end
    follow::stm32_dfu_update_with(
        &Follow::new(&serial),
        bus,
        open,
        &image.image,
        None,
        &mut progress,
        timeouts,
    )
    .await
    .map_err(follow_error)?;

    let back = reconnect(&serial).await?;
    let new = back.device().firmware_version().await;
    let _ = back.close().await;
    Ok(UpdateReport {
        serial,
        old,
        new: new?,
        bytes: image.len(),
        seconds: started.elapsed().as_secs_f64(),
    })
}

fn follow_error(e: follow::FollowError) -> Error {
    match e {
        follow::FollowError::Timeout { .. } => Error::Timeout(format!("firmware update: {e}")),
        e => Error::Link(format!("firmware update: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockDevice;
    use wire_weaver_client::UsbLocation;
    use wire_weaver_client::ww_stm32_dfu::proto::{ROM_DFU_PID, ST_VID};
    use wire_weaver_client::ww_stm32_dfu::sim::SimStm32Dfu;

    /// The mock's UID as the three words the ROM builds its serial number from.
    fn uid_words(uid: [u8; 12]) -> [u32; 3] {
        let w = |i: usize| u32::from_le_bytes(uid[i * 4..i * 4 + 4].try_into().unwrap());
        [w(0), w(1), w(2)]
    }

    /// A bus with the simulated ROM on it between the reboot request and the leave.
    struct SimBus<'m> {
        mock: &'m MockDevice,
        sim: SimStm32Dfu,
        serial: String,
    }

    impl Bus for SimBus<'_> {
        async fn list(&mut self) -> anyhow::Result<Vec<DeviceInfo>> {
            let rebooted = !self.mock.state().await.reboots.is_empty();
            if !rebooted {
                return Ok(vec![]);
            }
            if self.sim.left().is_some() {
                return Ok(vec![DeviceInfo {
                    location: "sim c0de:cafe".into(),
                    manufacturer: "vhrd.tech".into(),
                    product: "donguru".into(),
                    serials: vec![self.serial.clone()],
                    user_label: String::new(),
                    api: Some(wire_weaver_client::ApiInfo {
                        gid: crate::device::API_GID.into(),
                        version: semver::Version::new(0, 5, 1),
                        signature: wire_weaver_client::ww_version::ApiHashOwned { hash: vec![] },
                    }),
                    usb: None,
                }]);
            }
            Ok(vec![DeviceInfo {
                location: format!("sim {ST_VID:04x}:{ROM_DFU_PID:04x}"),
                manufacturer: "STMicroelectronics".into(),
                product: "STM32  BOOTLOADER".into(),
                serials: vec![self.serial.clone()],
                user_label: String::new(),
                api: None,
                usb: Some(UsbLocation {
                    bus_id: "1".into(),
                    port_chain: vec![3],
                    vid: ST_VID,
                    pid: ROM_DFU_PID,
                }),
            }])
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn update_reboots_writes_and_reads_the_version_back() {
        let mock = MockDevice::start("core/update");
        let uid = mock.state().await.uid;
        let sim = SimStm32Dfu::h72x(uid_words(uid));
        let serial = sim.serial();
        let image = FirmwareImage {
            image: Image::from_bin(0x0800_0000, vec![0xA5; 3000]).unwrap(),
        };
        let mut bus = SimBus {
            mock: &mock,
            sim: sim.clone(),
            serial: serial.clone(),
        };
        let mut steps = vec![];
        let report = update_with(
            mock.open().await.unwrap(),
            &image,
            &mut bus,
            async |_: &DeviceInfo| Ok(sim.clone()),
            async |s: &str| {
                assert_eq!(s, serial);
                mock.open().await
            },
            Timeouts::default(),
            &mut |step, _, _| {
                if steps.last() != Some(&step) {
                    steps.push(step);
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(mock.state().await.reboots, [ww_dfu::BootTarget::Updater]);
        assert_eq!(&sim.memory(0x0800_0000, 3000), &vec![0xA5; 3000]);
        assert_eq!(sim.left(), Some(0x0800_0000));
        assert_eq!(report.serial, serial);
        assert_eq!(report.bytes, 3000);
        assert_eq!(report.old.as_ref(), Some(&report.new));
        assert_eq!(report.new.to_string(), "donguru-mock 0.2.0 (debug)");
        assert_eq!(
            steps,
            [
                UpdateStep::Reboot,
                UpdateStep::WaitBootloader,
                UpdateStep::Erase,
                UpdateStep::Write,
                UpdateStep::Verify
            ]
        );
    }

    #[test]
    fn rom_serial_from_the_uid_of_a_firmware_with_the_long_serial() {
        let entry = DeviceEntry::in_process("x");
        let uid = "530016000A50315448313520";
        assert_eq!(rom_serial(&entry, Some(uid)), "204B319B5431");
        let mut short = entry;
        short.serial = "204B319B5431".into();
        assert_eq!(rom_serial(&short, None), "204B319B5431");
    }
}
