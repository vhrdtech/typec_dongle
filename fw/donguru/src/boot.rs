//! Firmware update through the STM32 ROM bootloader (FW-12): the early boot that runs first thing in `main`, before
//! clocks and embassy, and the reboots `device.boot.reboot` asks for (`src/ww/boot.rs`). The decisions are
//! `donguru_board::boot`'s, host-tested; this module does the register accesses.
//!
//! - `reboot(Updater)`: [reboot] writes the ROM request into the boot word (TAMP_BKP4R) and resets. [early] finds it,
//!   clears it and jumps to the ROM bootloader from the reset state (AN2606 §4.1): USB DFU behind the hub, which
//!   `ww dfu stm32 flash` writes.
//! - The ROM's leave jumps to the firmware with its clocks, USB and vector table set up: [early] sees it
//!   (`Entered::from_reset`) and resets once, so embassy starts from the reset state (as B135 BOOT-10).
//! - On rev A the hub stays up through those resets because RTC_OUT1 holds its RESET_N (`src/hub.rs`).
//! - A boot loop (FW-45): [early] counts every boot in the boot word, [healthy_task] clears the count once USB is
//!   configured or the firmware has run `HEALTHY_AFTER_S`. After `BOOT_LOOP_LIMIT` boots without that, [early]
//!   returns [Start::Rescue]: `main` brings the hub up and nothing else, and [rescue] resets into the ROM bootloader
//!   (the hub must be configured for the ROM's USB to be seen, and after a power cycle nothing has done that yet).
//!   The next firmware reports it in `device.dfu.node` as `Outcome::Recovery` ([fell_back]).

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};
use cortex_m::peripheral::SCB;
use defmt::{error, info, unwrap, warn};
pub use donguru_board::boot::Start;
use donguru_board::boot::{
    BOOT_LOOP_LIMIT, BootWord, Entered, HEALTHY_AFTER_S, ROM_BOOTLOADER, SERIAL_LEN, TAMP_BKP_BOOT,
    rom_serial,
};
use embassy_futures::select::{Either, select};
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

/// Read-modify-write of the boot word at run time (backup registers writable since `init::init`): the tasks that
/// write it and the early boot never run at the same time, the critical section keeps an interrupt out of the way.
fn update_word(f: impl FnOnce(BootWord) -> BootWord) -> BootWord {
    cortex_m::interrupt::free(|_| {
        // SAFETY: TAMP_BKP4R, clocked and writable since init::init
        unsafe {
            let word = f(BootWord::from_word(boot_register().read_volatile()));
            boot_register().write_volatile(word.word());
            word
        }
    })
}

/// The ROM bootloader took over after a boot loop before this firmware ([BootWord::fell_back], read by [early]).
static FELL_BACK: AtomicBool = AtomicBool::new(false);

/// The ROM bootloader took over after a boot loop (FW-45) and no update was requested since: `device.dfu.node`'s
/// last result.
pub fn fell_back() -> bool {
    FELL_BACK.load(Ordering::Relaxed)
}

/// First thing in `main` after RTT: resets once when the ROM bootloader jumped here, goes to the ROM bootloader when
/// asked to (never returns then), counts the boot otherwise and says how to start.
pub fn early() -> Start {
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
    let (word, next, start) = with_backup_registers(|| {
        // SAFETY: TAMP_BKP4R, clocked and writable here; the word is ours alone (BKPSRAM in memory.x ends below it)
        let word = BootWord::from_word(unsafe { boot_register().read_volatile() });
        // one request, one ROM session: the ROM's leave comes back here without it; else this boot counts
        let (next, start) = if word.rom_request {
            (word.take_request(), Start::App)
        } else {
            word.count_boot()
        };
        unsafe { boot_register().write_volatile(next.word()) };
        (word, next, start)
    });
    if word.rom_request {
        info!("boot: ROM bootloader requested");
        // SAFETY: straight from reset, the backup register access undone
        unsafe { jump_to_rom() }
    }
    FELL_BACK.store(next.fell_back, Ordering::Relaxed);
    match start {
        Start::App if next.fell_back => {
            warn!("boot: the ROM bootloader took over after a boot loop before this firmware (FW-45)")
        }
        Start::App if next.unhealthy_boots > 1 => warn!(
            "boot: {=u8} in a row without a healthy mark, the ROM bootloader after {=u8} (FW-45)",
            next.unhealthy_boots,
            BOOT_LOOP_LIMIT
        ),
        Start::App => {}
        Start::Rescue => error!(
            "boot loop: {=u8} boots without USB configured or {=u64} s up, the ROM bootloader takes over (FW-45)",
            BOOT_LOOP_LIMIT,
            HEALTHY_AFTER_S
        ),
    }
    start
}

/// Set by `src/ww/mod.rs`'s USB handler when the device is configured: a healthy boot.
static USB_CONFIGURED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// USB is configured: this boot is healthy ([healthy_task]).
pub fn usb_configured() {
    USB_CONFIGURED.signal(());
}

/// Marks the boot healthy once USB is configured or the firmware has run [HEALTHY_AFTER_S] (a dongle that only
/// charges never sees a USB host): the boot-loop count starts over (FW-45).
#[embassy_executor::task]
pub async fn healthy_task() {
    let why = match select(
        USB_CONFIGURED.wait(),
        Timer::after_secs(HEALTHY_AFTER_S),
    )
    .await
    {
        Either::First(()) => "USB configured",
        Either::Second(()) => "up long enough",
    };
    update_word(BootWord::healthy);
    info!("boot: healthy ({=str})", why);
}

/// The rescue boot (FW-45), once `main` has brought the hub up: resets into the ROM bootloader.
pub fn rescue() -> ! {
    // SAFETY: TAMP_BKP4R, writable since init::init
    unsafe { boot_register().write_volatile(BootWord::rescue().word()) };
    warn!("boot: hub up, reset into the ROM bootloader");
    SCB::sys_reset()
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
    update_word(|w| w.reboot(matches!(into, Reboot::RomBootloader)));
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
