//! Builds a [`VbusMode`] from plain volts and amps, so the CLI, the GUI and Python don't each depend on `ww_si`'s
//! constructors to drive `plug.vbus_mode` / `header.vbus_sw_mode` (API-11, `docs/user/power.md`).

use donguru_api::VbusMode;
use ww_si::{Ampere, Prefix, Volt, Watt};

/// `max_voltage` hosts use for [`force_on`] / [`auto`] unless told otherwise: enough for a plain USB VBUS, refuses
/// a PD charger that renegotiated above 5 V.
pub const DEFAULT_MAX_VOLTAGE_V: f64 = 5.5;

/// `ForceOn { max_voltage }` at `max_voltage_v` volts.
pub fn force_on(max_voltage_v: f64) -> VbusMode {
    VbusMode::ForceOn {
        max_voltage: Volt::volts_f32(max_voltage_v as f32),
    }
}

/// `Auto { max_voltage, max_current, max_power }` at `max_voltage_v` volts and `max_current_a` amps. `max_power`
/// is their product, so it never trips before the voltage or current limit already would: a current trip with no
/// extra power ceiling to pick.
pub fn auto(max_voltage_v: f64, max_current_a: f64) -> VbusMode {
    VbusMode::Auto {
        max_voltage: Volt::volts_f32(max_voltage_v as f32),
        max_current: Ampere::amperes_f32(max_current_a as f32),
        max_power: Watt::new(Prefix::Unit, (max_voltage_v * max_current_a) as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_on_carries_the_voltage() {
        match force_on(5.5) {
            VbusMode::ForceOn { max_voltage } => assert_eq!(max_voltage.to_f64(), 5.5),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn auto_power_is_the_product() {
        match auto(20.0, 2.0) {
            VbusMode::Auto {
                max_voltage,
                max_current,
                max_power,
            } => {
                assert_eq!(max_voltage.to_f64(), 20.0);
                assert_eq!(max_current.to_f64(), 2.0);
                assert!((max_power.to_f64() - 40.0).abs() < 1e-3);
            }
            other => panic!("{other:?}"),
        }
    }
}
