//! Net names of the B153A MCU pins (U202, STM32G0B1CE, LQFP48), for the `debug` level: `dg debug pin set RELAY_EN
//! high` instead of `PA5`. The firmware only knows `McuPin { port, pin }`; the names live here, on the host.
//!
//! Source: `hw/b153_typec_serial/doc/hw_b153a.md` §11 "MCU pin map" (hw `cd5cc93`). Keep it in step with that table
//! when a revision changes a net.

use donguru_api::{GpioPort, McuPin};

/// One MCU pin and the net on it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Net {
    /// Net name as in the schematic and hw_b153a.md (`~` for active low).
    pub name: &'static str,
    /// Other names the same net is asked for by.
    pub aliases: &'static [&'static str],
    pub pin: McuPin,
    /// LQFP48 pin number.
    pub package_pin: u8,
    /// What it does, from hw_b153a.md.
    pub note: &'static str,
}

const fn net(
    name: &'static str,
    aliases: &'static [&'static str],
    port: GpioPort,
    pin: u8,
    package_pin: u8,
    note: &'static str,
) -> Net {
    Net {
        name,
        aliases,
        pin: McuPin { port, pin },
        package_pin,
        note,
    }
}

use GpioPort::{A, B, C, D, F};

/// Every MCU GPIO of the B153A, in package pin order.
pub const B153A: &[Net] = &[
    net(
        "HUB_RESET_N",
        &[],
        C,
        13,
        1,
        "hub RESET_N (rework on rev A), drive high to run the hub",
    ),
    net(
        "~PWR_FLT",
        &["PWR_FLT"],
        C,
        14,
        2,
        "U601 fault input, open drain, needs internal pull-up",
    ),
    net(
        "SBU_PU_EN",
        &[],
        C,
        15,
        3,
        "3K3 SBU pull-ups on, external pull-down",
    ),
    net(
        "LED_RED",
        &[],
        F,
        0,
        8,
        "red LED, 1K, active high (TIM14_CH1)",
    ),
    net(
        "LED_GREEN",
        &[],
        F,
        1,
        9,
        "green LED, 1K, active high (TIM15_CH1N)",
    ),
    net(
        "~HUB_RESET",
        &["NRST"],
        F,
        2,
        10,
        "NRST; rev A: cut from the hub",
    ),
    net("SBU2", &[], A, 0, 11, "plug SBU2, V_SBU domain"),
    net("SBU1", &[], A, 1, 12, "plug SBU1, V_SBU domain"),
    net("IO9", &[], A, 2, 13, "header pin 3"),
    net("AN1", &["VBUS_IN"], A, 3, 14, "receptacle VBUS / 7.17"),
    net(
        "HUB_I2C_EN",
        &[],
        A,
        4,
        15,
        "hub SDA pull-up supply, drive high; free in rev B",
    ),
    net("RELAY_EN", &[], A, 5, 16, "photo-MOS relay"),
    net(
        "PWR_EN",
        &[],
        A,
        6,
        17,
        "plug VBUS switch enable, external pull-down",
    ),
    net(
        "USER_SW_BTN",
        &["BUTTON"],
        A,
        7,
        18,
        "button + slide switch, analog (ADC1_IN7)",
    ),
    net("AN0", &["CURRENT"], B, 0, 19, "VBUS current (INA181)"),
    net("PB1", &[], B, 1, 20, "tied to PB3 / header pin 1"),
    net("SBU_OE", &[], B, 2, 21, "TXS0102 OE, external pull-down"),
    net("HUB_I2C_SCL", &[], B, 10, 22, "I2C2"),
    net("PB11", &[], B, 11, 23, "tied to SBU2_VT"),
    net("IO10", &[], B, 12, 24, "header pin 4"),
    net("IO5", &["A2"], B, 13, 25, "plug A2, no OVP"),
    net("HUB_I2C_SDA", &[], B, 14, 26, "I2C2"),
    net("PD1_CC2", &[], B, 15, 27, "UCPD1 CC2, receptacle"),
    net("PD1_CC1", &[], A, 8, 28, "UCPD1 CC1, receptacle"),
    net(
        "DATA_CUT",
        &[],
        A,
        9,
        29,
        "U701 S, external pull-down; UCPD1_DBCC1",
    ),
    net("GND", &[], C, 6, 30, "tied to GND, never output"),
    net("+3V3", &[], C, 7, 31, "tied to 3.3 V, never output"),
    net(
        "SBU_TO_USB",
        &[],
        A,
        10,
        32,
        "U901 S, external pull-down; UCPD1_DBCC2; variant strap at boot",
    ),
    net("USB_D-", &["USB_DM"], A, 11, 33, "MCU USB to hub port 1"),
    net("USB_D+", &["USB_DP"], A, 12, 34, "MCU USB to hub port 1"),
    net("SWDIO", &[], A, 13, 35, "receptacle SBU2, TP201"),
    net(
        "SWCLK_BOOT0",
        &["SWCLK", "BOOT0"],
        A,
        14,
        36,
        "receptacle SBU1, TP202, 100K pull-down, BOOT0",
    ),
    net(
        "CC_DA_EN",
        &[],
        A,
        15,
        37,
        "debug-accessory Rp on receptacle CC, external pull-down",
    ),
    net("PD2_CC1", &[], D, 0, 38, "UCPD2 CC1, plug"),
    net(
        "5V_EN",
        &["VBUS_SW_EN"],
        D,
        1,
        39,
        "header +5V switch, external pull-down; UCPD2_DBCC1",
    ),
    net("PD2_CC2", &[], D, 2, 40, "UCPD2 CC2, plug"),
    net(
        "VSBU_SEL",
        &[],
        D,
        3,
        41,
        "V_SBU 1.8 / 3.3 V, external pull-down; UCPD2_DBCC2",
    ),
    net("IO7", &[], B, 3, 42, "header pin 1"),
    net("IO8", &[], B, 4, 43, "header pin 2"),
    net("IO6", &["B2"], B, 5, 44, "plug B2, no OVP"),
    net("PB6", &[], B, 6, 45, "tied to SBU2_VT"),
    net("PB7", &[], B, 7, 46, "tied to SBU1_VT"),
    net(
        "SBU1_VT",
        &[],
        B,
        8,
        47,
        "plug SBU1 through the level shifter",
    ),
    net(
        "SBU2_VT",
        &[],
        B,
        9,
        48,
        "plug SBU2 through the level shifter",
    ),
];

/// The net `name` (any case, with or without the `~` of an active-low net, or an alias), or a port pin (`PA5`).
pub fn lookup(name: &str) -> Option<&'static Net> {
    let wanted = name.trim();
    let bare = |n: &str| n.trim_start_matches('~').to_ascii_uppercase();
    let wanted_bare = bare(wanted);
    B153A
        .iter()
        .find(|n| bare(n.name) == wanted_bare || n.aliases.iter().any(|a| bare(a) == wanted_bare))
        .or_else(|| port_pin(wanted).and_then(net_of))
}

/// The pin of net `name`, see [lookup]; a port pin (`PB7`) also when no net is on it.
pub fn pin(name: &str) -> Option<McuPin> {
    lookup(name).map(|n| n.pin).or_else(|| port_pin(name))
}

/// The net on `pin`.
pub fn net_of(pin: McuPin) -> Option<&'static Net> {
    B153A.iter().find(|n| n.pin == pin)
}

/// `PA5`, `pb12`: a port letter A..F and a pin 0..=15.
pub fn port_pin(s: &str) -> Option<McuPin> {
    let s = s.trim().to_ascii_uppercase();
    let rest = s.strip_prefix('P')?;
    let mut chars = rest.chars();
    let port = match chars.next()? {
        'A' => A,
        'B' => B,
        'C' => C,
        'D' => D,
        'E' => GpioPort::E,
        'F' => F,
        _ => return None,
    };
    let pin: u8 = chars.as_str().parse().ok()?;
    (pin <= 15).then_some(McuPin { port, pin })
}

/// `PA5`.
pub fn port_pin_name(pin: McuPin) -> String {
    let port = match pin.port {
        A => 'A',
        B => 'B',
        C => 'C',
        D => 'D',
        GpioPort::E => 'E',
        F => 'F',
    };
    format!("P{port}{}", pin.pin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_aliases_and_port_pins() {
        assert_eq!(pin("RELAY_EN"), Some(McuPin { port: A, pin: 5 }));
        assert_eq!(pin("relay_en"), Some(McuPin { port: A, pin: 5 }));
        assert_eq!(pin("PWR_FLT"), pin("~PWR_FLT"));
        assert_eq!(pin("NRST"), Some(McuPin { port: F, pin: 2 }));
        assert_eq!(pin("USB_DP"), Some(McuPin { port: A, pin: 12 }));
        assert_eq!(lookup("PA5").map(|n| n.name), Some("RELAY_EN"));
        assert_eq!(pin("pb12"), Some(McuPin { port: B, pin: 12 }));
        assert_eq!(
            pin("PE3"),
            Some(McuPin {
                port: GpioPort::E,
                pin: 3
            }),
            "a port pin without a net"
        );
        assert_eq!(pin("PA16"), None);
        assert_eq!(pin("NOPE"), None);
        assert_eq!(port_pin_name(McuPin { port: B, pin: 12 }), "PB12");
    }

    #[test]
    fn table_is_consistent() {
        for (i, a) in B153A.iter().enumerate() {
            assert!(a.pin.pin <= 15, "{}", a.name);
            for b in &B153A[i + 1..] {
                assert_ne!(a.pin, b.pin, "{} and {} on one pin", a.name, b.name);
                assert_ne!(a.name, b.name);
                assert_ne!(a.package_pin, b.package_pin, "{} and {}", a.name, b.name);
            }
        }
        assert_eq!(B153A.len(), 44);
    }
}
