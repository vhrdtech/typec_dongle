//! Settings kept in flash (API-19): [UserSettings] in a `ww_settings` store in the last 4 pages of bank 2
//! (`SETTINGS` in `memory.x`), served as `device.settings` and `device.label` (`src/ww/device.rs`).
//!
//! The store keeps a record of the fields not at their default, power-cut safe at every word written and byte
//! erased; a firmware of any version reads and writes back the fields of the others. The flash is read through
//! `ww_ecc_flash`: a word left half-programmed by a power cut has an uncorrectable ECC error, which reads as zeros
//! (an item the store skips) instead of an NMI on every boot. See WireWeaver `docs/api/settings.md`.
//!
//! Writes and erases use embassy's blocking driver: a write of a record programs a few dozen double words (56 µs
//! each), a page erase takes ~25 ms. The code runs from bank 1 (and the first KiB of bank 2), so only code that
//! happens to sit in bank 2 waits during an erase; the executor waits for both. `Store::maintain` does the erase
//! ahead of the write that needs it, called from the server task after a write was answered.

use defmt::{info, warn};
use donguru_api::{ButtonAction, MeterDeadband, VbusMode};
use embassy_stm32::flash::{Blocking, FLASH_BASE, Flash};
use embassy_stm32::{Peri, peripherals};
use static_cell::StaticCell;
use wire_weaver::shrink_wrap::ArrayString;
use ww_ecc_flash::EccSafeFlash;
use ww_settings::{Debounce, Key};
use ww_si::{Ampere, Second, Volt};
use ww_uart::{DataBits, Mode, Parity, StopBits, UartConfig};
use ww_user_info::{MAX_NAME_LEN, MAX_NOTES_LEN};

/// Biggest record of [UserSettings] (every field set, the longest label and notes: about 360 bytes) and the store's
/// buffer for one item. The current records must stay under half a 2 KiB page.
const RECORD_LEN: usize = 512;

pub type SettingsFlash = EccSafeFlash<Flash<'static, Blocking>>;
pub type Store = ww_settings::Store<SettingsFlash, RECORD_LEN>;
pub type User = ww_settings::Settings<UserSettings, RECORD_LEN>;

/// What users change, in [Key::USER]. A slot belongs to its field for good: a removed field's slot goes into
/// `#[settings(retired(..))]`, see WireWeaver `docs/evolution/rules.md` ("Settings stored on a device").
#[derive(ww_settings::Settings, Clone, Debug, PartialEq)]
pub struct UserSettings {
    /// `device.label.name`.
    #[setting(slot = 0, default = ArrayString::new())]
    pub label: ArrayString<MAX_NAME_LEN>,
    /// `device.label.notes`.
    #[setting(slot = 1, default = ArrayString::new())]
    pub notes: ArrayString<MAX_NOTES_LEN>,
    /// `plug.vbus_mode` at boot.
    #[setting(slot = 2, default = VbusMode::ForceOff)]
    pub plug_vbus_at_boot: VbusMode,
    /// `inputs.button_action` at boot.
    #[setting(slot = 3, default = ButtonAction::Off)]
    pub button_action: ButtonAction,
    /// `meter.window` at boot.
    #[setting(slot = 4, default = Second::micro(crate::adc::DEFAULT_WINDOW_US as i32))]
    pub meter_window: Second,
    /// `meter.deadband` at boot.
    #[setting(slot = 5, default = default_deadband())]
    pub meter_deadband: MeterDeadband,
    /// `uart[0]` default config, also CDC-ACM's until the tty sets a line coding (API-14, API-20).
    #[setting(slot = 6, default = default_uart())]
    pub uart_plug: UartConfig,
}

/// 20 mV and 10 mA, what `meter.deadband` starts with.
pub fn default_deadband() -> MeterDeadband {
    MeterDeadband {
        voltage: Volt::milli(20i32),
        current: Ampere::milli(10i32),
    }
}

/// 115200 8N1, no flow control, V_SBU left as it is.
fn default_uart() -> UartConfig {
    UartConfig {
        baud_rate: ww_si::Baud::base(115_200u32),
        data_bits: DataBits::Eight,
        parity: Parity::None,
        stop_bits: StopBits::Stop1,
        mode: Mode::Asynchronous,
        prevent_back_feed: false,
        io_level: None,
        idle_chars: ww_uart::IDLE_CHARS_DEFAULT,
    }
}

unsafe extern "C" {
    /// `SETTINGS` in `memory.x`, as offsets from the start of flash.
    static __settings_start: u8;
    static __settings_end: u8;
}

static STORE: StaticCell<Store> = StaticCell::new();
static USER: StaticCell<User> = StaticCell::new();

/// Opens the store and loads [UserSettings]: defaults for what isn't stored, all of them when the flash can't be
/// read or holds a record of a newer format (left alone, see [ww_settings::Settings::is_read_only]). Both live in
/// statics: moved by value through the boot futures they would take their size a few times over in RAM.
pub async fn open(
    flash: Peri<'static, peripherals::FLASH>,
) -> (&'static mut Store, &'static mut User) {
    // linker symbols: only their addresses mean something, here offsets
    let range = (&raw const __settings_start) as u32..(&raw const __settings_end) as u32;
    // SAFETY: embassy's `Flash` offsets start at FLASH_BASE, which is where its memory is mapped
    let flash = unsafe { EccSafeFlash::new(Flash::new_blocking(flash), FLASH_BASE as u32) };
    let store = match Store::new(flash, range) {
        Ok(store) => STORE.init(store),
        // memory.x gives 4 whole pages, so this is a build mistake
        Err(_) => defmt::panic!("SETTINGS in memory.x is not a range of at least 2 whole pages"),
    };
    let user = USER.init(User::new(Key::USER, Debounce::default()));
    match user.load(store).await {
        Ok(report) => info!("settings: {}", report),
        Err(e) => {
            cnt::bkp_cnt!(settings_load_errors: u32, warn);
            warn!("settings not loaded, defaults: {}", e);
        }
    }
    if user.is_read_only() {
        warn!(
            "settings: record of a newer format, kept; writes refused until device.settings.reset"
        );
    }
    (store, user)
}

/// The `code` of the `Fault` a settings write answers with (`device.settings`, `device.label`):
/// 1 the flash failed, 2 a record of a newer format is stored and kept (`device.settings.reset` overwrites it),
/// 3 the record doesn't fit (a bug: [RECORD_LEN] is sized for every field at its longest).
pub fn fault_code(e: &ww_settings::Error<ww_ecc_flash::Error<embassy_stm32::flash::Error>>) -> u32 {
    use ww_settings::{Error, StoreError};
    match e {
        Error::Store(StoreError::Flash(_)) => 1,
        Error::ReadOnly => 2,
        Error::Store(_) | Error::TooBig(_) => 3,
    }
}
