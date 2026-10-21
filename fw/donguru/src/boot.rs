//! Firmware update through the STM32 ROM bootloader (FW-12): the early boot that runs first thing in `main`, before
//! clocks and embassy, and the reboots `device.boot.reboot` asks for (`src/ww/boot.rs`). The decisions are
//! `donguru_board::boot`'s, host-tested; this module does the register accesses.
//!
//! - `reboot(Updater)`: [reboot] writes the ROM request into the boot word (TAMP_BKP4R) and resets. [early] finds it,
//!   clears it and jumps to the ROM bootloader from the reset state (AN2606 §4.1): USB DFU behind the hub, which
//!   `ww stm32dfu flash` writes.
//! - The ROM's leave jumps to the firmware with its clocks, USB and vector table set up: [early] sees it
//!   (`Entered::from_reset`) and resets once, so embassy starts from the reset state (as B135 BOOT-10).
//! - On rev A the hub stays up through those resets because RTC_OUT1 holds its RESET_N (`src/hub.rs`).

use core::mem::MaybeUninit;
use cortex_m::peripheral::SCB;
use defmt::{error, info, unwrap, warn};
use donguru_board::boot::{
    BootWord, Entered, ROM_BOOTLOADER, SERIAL_LEN, TAMP_BKP_BOOT, rom_serial,
};
use embassy_stm32::pac;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use static_cell::StaticCell;

/// What [early] saw before it reset because the firmware was not entered by a reset: kept across that one reset
/// (`.uninit` RAM, left alone by the startup code), logged by the boot after it.
#[repr(C)]
struct Jumped {
    /// [JUMPED] when `entered` is valid.
    magic: u32,
    entered: Entered,
}

/// "JUMP" in ASCII.
const JUMPED: u32 = 0x4A55_4D50;

#[unsafe(link_section = ".uninit.donguru_boot")]
static mut BEFORE_RESET: MaybeUninit<Jumped> = MaybeUninit::uninit();

/// Takes what [BEFORE_RESET] holds, if valid, and invalidates it.
fn take_before_reset() -> Option<Entered> {
    // SAFETY: only the early boot touches it, before anything runs concurrently; every bit pattern of the fields is
    // a valid u32
    unsafe {
        let p = (&raw mut BEFORE_RESET).cast::<Jumped>();
        let magic = (&raw mut (*p).magic).read_volatile();
        (&raw mut (*p).magic).write_volatile(0);
        (magic == JUMPED).then(|| (&raw const (*p).entered).read_volatile())
    }
}

/// The registers that tell a reset from the ROM's jump, read before anything changes them.
fn entered() -> Entered {
    // SAFETY: plain reads of core registers
    let (vtor, nvic_iser) = unsafe {
        let p = cortex_m::Peripherals::steal();
        (p.SCB.vtor.read(), p.NVIC.iser[0].read())
    };
    Entered {
        rcc_cfgr: pac::RCC.cfgr().read().0,
        rcc_apbenr1: pac::RCC.apbenr1().read().0,
        vtor,
        nvic_iser,
    }
}

/// Where this image's vector table is (cortex-m-rt's `__vector_table`).
fn own_vectors() -> u32 {
    unsafe extern "C" {
        static __vector_table: u32;
    }
    &raw const __vector_table as u32
}

/// Runs `f` with the TAMP backup registers clocked and writable (PWR and RTC APB clocks, PWR_CR1.DBP), then puts
/// the three bits back as they were: before embassy's init they are off, as the ROM expects them after a reset.
fn with_backup_registers<R>(f: impl FnOnce() -> R) -> R {
    let rcc = pac::RCC;
    let pwr = pac::PWR;
    let apbenr1 = rcc.apbenr1().read();
    rcc.apbenr1().modify(|w| {
        w.set_pwren(true);
        w.set_rtcapben(true);
    });
    let _ = rcc.apbenr1().read(); // the clocks are on before the first access
    let dbp = pwr.cr1().read().dbp();
    pwr.cr1().modify(|w| w.set_dbp(true));
    let result = f();
    pwr.cr1().modify(|w| w.set_dbp(dbp));
    rcc.apbenr1().modify(|w| {
        w.set_pwren(apbenr1.pwren());
        w.set_rtcapben(apbenr1.rtcapben());
    });
    result
}

fn boot_register() -> *mut u32 {
    TAMP_BKP_BOOT as *mut u32
}

/// First thing in `main` after RTT: resets once when the ROM bootloader jumped here, goes to the ROM bootloader when
/// asked to (never returns then).
pub fn early() {
    let entered = entered();
    let before_reset = take_before_reset();
    if !entered.from_reset(own_vectors()) {
        if before_reset.is_none() {
            // SAFETY: as in take_before_reset
            unsafe {
                (&raw mut BEFORE_RESET).write(MaybeUninit::new(Jumped {
                    magic: JUMPED,
                    entered,
                }))
            };
            SCB::sys_reset();
        }
        // a second time in a row: going on beats a reset loop
        cnt::cnt!(boot_jumped_twice: u32, error);
        error!("not entered by a reset, again after a reset: going on");
    }
    if let Some(before) = before_reset {
        info!("reset once, entered by a jump before: {}", before);
    }
    let word = with_backup_registers(|| {
        // SAFETY: TAMP_BKP4R, clocked and writable here; the word is ours alone (BKPSRAM in memory.x ends below it)
        let word = BootWord::from_word(unsafe { boot_register().read_volatile() });
        if word.rom_request {
            // one request, one ROM session: the ROM's leave comes back here without it
            unsafe { boot_register().write_volatile(BootWord::default().word()) };
        }
        word
    });
    if word.rom_request {
        info!("boot: ROM bootloader requested");
        // SAFETY: straight from reset, the backup register access undone
        unsafe { jump_to_rom() }
    }
}

/// Jumps into the ROM bootloader (AN2606 §4.1: from user code with peripheral clocks, PLL and interrupts as after
/// reset).
///
/// # Safety
///
/// Only right after reset: clocks, PLL and NVIC as the reset left them.
unsafe fn jump_to_rom() -> ! {
    // SAFETY: the ROM's vector table; it sets up the rest
    unsafe {
        (*SCB::PTR).vtor.write(ROM_BOOTLOADER);
        cortex_m::asm::bootload(ROM_BOOTLOADER as *const u32)
    }
}

/// Where `device.boot.reboot` goes.
#[derive(Clone, Copy, defmt::Format)]
pub enum Reboot {
    /// The firmware again.
    App,
    /// The STM32 ROM bootloader, USB DFU.
    RomBootloader,
}

/// Set by `device.boot.reboot`: [reboot_task] resets into it.
pub static REBOOT: Signal<CriticalSectionRawMutex, Reboot> = Signal::new();

/// Time for the reply to `device.boot.reboot` to leave over USB before the reset.
const REPLY_MS: u64 = 50;

/// Resets into what [REBOOT] asks for, [REPLY_MS] after it. Backup registers are writable since `init::init`.
#[embassy_executor::task]
pub async fn reboot_task() {
    let into = REBOOT.wait().await;
    warn!("reboot into {}", into);
    Timer::after_millis(REPLY_MS).await;
    let word = BootWord {
        rom_request: matches!(into, Reboot::RomBootloader),
    };
    // SAFETY: TAMP_BKP4R, writable since init::init
    unsafe { boot_register().write_volatile(word.word()) };
    SCB::sys_reset()
}

/// The USB serial number: the one the ROM bootloader reports for this chip (`donguru_board::boot::rom_serial`), so a
/// host follows the board into the ROM and back by it. The full UID stays in `device.uid`.
pub fn usb_serial() -> &'static str {
    static SERIAL: StaticCell<[u8; SERIAL_LEN]> = StaticCell::new();
    let uid = embassy_stm32::uid::uid();
    let word =
        |i: usize| u32::from_le_bytes([uid[4 * i], uid[4 * i + 1], uid[4 * i + 2], uid[4 * i + 3]]);
    let serial = SERIAL.init(rom_serial([word(0), word(1), word(2)]));
    // upper case hex digits only
    unwrap!(core::str::from_utf8(serial).ok())
}
