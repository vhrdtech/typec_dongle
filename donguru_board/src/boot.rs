//! Early boot (FW-12): the decisions `fw/donguru/src/boot.rs` takes first thing in `main`, before clocks and embassy,
//! as pure logic on register values.
//!
//! Firmware updates go through the STM32 ROM bootloader (USB DFU, AN2606): `device.boot.reboot(Updater)` writes a
//! request into the [boot word](BootWord) and resets; the next boot finds it, clears it and jumps to the ROM from
//! the reset state (AN2606 §4.1). The ROM reports [rom_serial] as its USB serial number, and so does the firmware,
//! so a host follows the board into the ROM and back. When the ROM leaves DFU it jumps to the firmware with its
//! own clocks, USB and vector table set up; [Entered::from_reset] tells that start apart, and the firmware resets
//! once to start clean (as B135 BOOT-10).
//!
//! The MCU's USB sits behind hub U301. On rev A its RESET_N (PC13 after the rework) has a 10K pull-down, so every
//! MCU reset would hold the hub in reset and the ROM would sit on a dead USB path: the firmware drives RESET_N from
//! RTC_OUT1 instead ([rtc_cr_hub]), which the backup domain keeps through a system reset. Rev B gets a pull-up there
//! (`hw/b153_typec_serial/change/change_b153b.md`) and needs none of it.

/// TAMP_BKP4R, the last of the five TAMP backup registers (RM0444 §31.6.12): the [BootWord]. `bkp_cnt!` counters
/// get the four below it (`BKPSRAM` in `fw/donguru/memory.x`).
pub const TAMP_BKP_BOOT: u32 = 0x4000_B110;

/// Where the ROM bootloader's vector table starts on the STM32G0B1 (AN2606 Table 2, system memory): initial MSP,
/// then the reset vector.
pub const ROM_BOOTLOADER: u32 = 0x1FFF_0000;

/// Upper half of a valid boot word ("BOOT"-ish): anything else (0 after a backup domain reset, noise) is
/// [BootWord::default].
const TAG: u32 = 0xB007_0000;
/// Bit of the ROM bootloader request.
const ROM_REQUEST: u32 = 1 << 15;

/// What the firmware keeps for its next boot in [TAMP_BKP_BOOT]. Survives a system reset, not a power cycle (no
/// VBAT on B153), which clears it: no request.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BootWord {
    /// Go to the ROM bootloader at the next boot.
    pub rom_request: bool,
}

impl BootWord {
    /// The word as read from the register: without the tag it holds nothing.
    pub fn from_word(word: u32) -> Self {
        if word & 0xFFFF_0000 != TAG {
            return Self::default();
        }
        Self {
            rom_request: word & ROM_REQUEST != 0,
        }
    }

    /// The word to write to the register.
    pub fn word(self) -> u32 {
        TAG | if self.rom_request { ROM_REQUEST } else { 0 }
    }
}

/// RCC_CFGR.SWS when the system clock is HSISYS, as after any reset (RM0444 §5.4.3, bits 5:3).
const SWS_HSISYS: u32 = 0;

/// The registers that say how the firmware was entered, read before anything is changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Entered {
    /// RCC_CFGR.
    pub rcc_cfgr: u32,
    /// RCC_APBENR1 (USBEN, bit 13, among the peripheral clocks).
    pub rcc_apbenr1: u32,
    /// SCB.VTOR.
    pub vtor: u32,
    /// NVIC_ISER: interrupts enabled.
    pub nvic_iser: u32,
}

/// RCC_APBENR1.USBEN.
const USBEN: u32 = 1 << 13;

impl Entered {
    /// The firmware started from a reset: system clock HSISYS, the USB clock off, no interrupt enabled and VTOR on
    /// address 0 (its reset value, flash aliased there) or on the firmware's own vector table at `own_vectors`. A
    /// jump from the ROM bootloader's leave fails at least one: it runs from HSI48 or the PLL, with its USB and
    /// its vector table.
    pub fn from_reset(&self, own_vectors: u32) -> bool {
        let sws = (self.rcc_cfgr >> 3) & 0b111;
        sws == SWS_HSISYS
            && self.rcc_apbenr1 & USBEN == 0
            && self.nvic_iser == 0
            && (self.vtor == 0 || self.vtor == own_vectors)
    }
}

/// RTC_CR bits that route an output to RTC_OUT1 (PC13) and shape it (RM0444 §30.6.3): OUT2EN, TAMPALRM_TYPE,
/// TAMPALRM_PU, COE, OSEL, POL.
const RTC_CR_OUT: u32 = 1 << 31 | 1 << 30 | 1 << 29 | 1 << 23 | 0b11 << 21 | 1 << 20;
/// RTC_CR.OSEL = alarm A: TAMPALRM on RTC_OUT1, push-pull (TAMPALRM_TYPE 0), alarm A never armed.
const OSEL_ALARM_A: u32 = 0b01 << 21;
/// RTC_CR.POL: TAMPALRM low while the alarm flag is set, so high while it never is.
const POL: u32 = 1 << 20;

/// RTC_CR with RTC_OUT1 (PC13) holding the hub's RESET_N (rev A): high (`run`) or low (hub in reset), from the RTC
/// in the backup domain, which keeps the level through a system reset (not a power cycle: the backup domain resets,
/// PC13 goes Hi-Z and R304 holds the hub in reset). The other bits of `cr` stay. B153A on omarchy-m1, 21 Oct 2026:
/// RTC_CR 0x0030_0000 kept the hub enumerated through two resets.
pub fn rtc_cr_hub(cr: u32, run: bool) -> u32 {
    (cr & !RTC_CR_OUT) | OSEL_ALARM_A | if run { POL } else { 0 }
}

/// RTC_CR drives the hub's RESET_N from RTC_OUT1 ([rtc_cr_hub]), high or low.
pub fn rtc_holds_hub(cr: u32) -> bool {
    cr & (0b11 << 21 | 1 << 31) == OSEL_ALARM_A
}

/// Length of [rom_serial] in ASCII.
pub const SERIAL_LEN: usize = 12;

/// The USB serial number the STM32 ROM bootloader reports in DFU mode for the 96-bit unique id `uid` (UID[31:0],
/// UID[63:32], UID[95:64]), in the form of ST's USB device library (wire_weaver `ww_stm32_dfu::rom_serial`): the sum
/// of the first and third word as 8 hex digits, then the upper 16 bits of the second word as 4, upper case.
pub fn rom_serial(uid: [u32; 3]) -> [u8; SERIAL_LEN] {
    let value = ((uid[0].wrapping_add(uid[2]) as u64) << 16) | (uid[1] >> 16) as u64;
    let mut out = [0; SERIAL_LEN];
    for (i, digit) in out.iter_mut().enumerate() {
        let nibble = (value >> (4 * (SERIAL_LEN - 1 - i))) & 0xF;
        *digit = b"0123456789ABCDEF"[nibble as usize];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_word_round_trip() {
        for rom_request in [false, true] {
            let w = BootWord { rom_request };
            assert_eq!(BootWord::from_word(w.word()), w);
        }
    }

    #[test]
    fn boot_word_without_tag_is_empty() {
        // a backup domain reset, noise, or the request bit alone
        for word in [0, 0xFFFF_FFFF, ROM_REQUEST, 0xB006_8000] {
            assert_eq!(BootWord::from_word(word), BootWord::default());
        }
    }

    const OWN: u32 = 0x0800_0000;

    fn reset() -> Entered {
        Entered {
            rcc_cfgr: 0,
            rcc_apbenr1: 0,
            vtor: 0,
            nvic_iser: 0,
        }
    }

    #[test]
    fn from_reset() {
        assert!(reset().from_reset(OWN));
        assert!(
            Entered {
                vtor: OWN,
                ..reset()
            }
            .from_reset(OWN)
        );
    }

    #[test]
    fn jumped_from_the_rom() {
        // each fact alone gives the jump away
        assert!(
            !Entered {
                vtor: ROM_BOOTLOADER,
                ..reset()
            }
            .from_reset(OWN)
        );
        assert!(
            !Entered {
                rcc_cfgr: 0b010 << 3,
                ..reset()
            }
            .from_reset(OWN)
        );
        assert!(
            !Entered {
                rcc_apbenr1: USBEN,
                ..reset()
            }
            .from_reset(OWN)
        );
        assert!(
            !Entered {
                nvic_iser: 1 << 8,
                ..reset()
            }
            .from_reset(OWN)
        );
        // SW (bits 2:0) asking for another clock does not count, SWS (5:3) does
        assert!(
            Entered {
                rcc_cfgr: 0b010,
                ..reset()
            }
            .from_reset(OWN)
        );
    }

    #[test]
    fn hub_hold() {
        // the value tested on B153A
        assert_eq!(rtc_cr_hub(0, true), 0x0030_0000);
        assert_eq!(rtc_cr_hub(0, false), 0x0020_0000);
        // other bits (e.g. the wakeup timer's WUTE, bit 10) stay, other outputs go
        assert_eq!(
            rtc_cr_hub(1 << 10 | 1 << 31 | 1 << 23, true),
            1 << 10 | 0x0030_0000
        );
        assert!(rtc_holds_hub(rtc_cr_hub(0, true)) && rtc_holds_hub(rtc_cr_hub(0, false)));
        assert!(!rtc_holds_hub(0) && !rtc_holds_hub(0x0030_0000 | 1 << 31));
    }

    #[test]
    fn rom_serial_on_a_b153a() {
        // B153A on omarchy-m1, 21 Oct 2026: firmware serial (the UID as 24 hex) 530016000A50315448313520, ROM DFU
        // serial 204B319B5431
        assert_eq!(
            &rom_serial([0x0016_0053, 0x5431_500A, 0x2035_3148]),
            b"204B319B5431"
        );
    }

    #[test]
    fn rom_serial_vectors() {
        // the vectors of ww_stm32_dfu::rom_serial
        assert_eq!(
            &rom_serial([0x0000_0010, 0x1234_5678, 0x0000_0001]),
            b"000000111234"
        );
        assert_eq!(
            &rom_serial([0xFFFF_FFFF, 0xABCD_0000, 0x0000_0002]),
            b"00000001ABCD"
        );
    }
}
