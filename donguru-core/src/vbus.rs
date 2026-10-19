//! Builds a [`VbusMode`] from plain volts and amps, so the CLI, the GUI and Python don't each depend on `ww_si`'s
//! constructors to drive `plug.vbus_mode` / `header.vbus_sw_mode` (API-11, `docs/user/power.md`).
//!
//! Values are built as integer millivolts/milliamps/milliwatts, never floats: the firmware's `ww_si` has only the
//! `i32`/`u32` number types (`fw/donguru/Cargo.toml`, flash and no FPU reasons, `AGENTS.md` "M0+ rules"), so a
//! `NumericValue::F32` a host sends doesn't decode there at all (`PropertyDesFailed`, found testing CLI-11 on
//! B153A: `Volt::volts_f32` passed the mock, which has no such restriction, but not the real firmware).

use donguru_api::VbusMode;
use ww_si::{Ampere, Prefix, Volt, Watt};

/// `max_voltage` hosts use for [`force_on`] / [`auto`] unless told otherwise: enough for a plain USB VBUS, refuses
/// a PD charger that renegotiated above 5 V.
pub const DEFAULT_MAX_VOLTAGE_V: f64 = 5.5;

fn millivolts(v: f64) -> Volt {
    Volt::milli((v * 1000.0).round() as i32)
}

fn milliamps(a: f64) -> Ampere {
    Ampere::milli((a * 1000.0).round() as i32)
}

fn milliwatts(w: f64) -> Watt {
    Watt::new(Prefix::Milli, (w * 1000.0).round() as i32)
}

/// `ForceOn { max_voltage }` at `max_voltage_v` volts.
pub fn force_on(max_voltage_v: f64) -> VbusMode {
    VbusMode::ForceOn {
        max_voltage: millivolts(max_voltage_v),
    }
}

/// `Auto { max_voltage, max_current, max_power }` at `max_voltage_v` volts and `max_current_a` amps. `max_power`
/// is their product, so it never trips before the voltage or current limit already would: a current trip with no
/// extra power ceiling to pick.
pub fn auto(max_voltage_v: f64, max_current_a: f64) -> VbusMode {
    VbusMode::Auto {
        max_voltage: millivolts(max_voltage_v),
        max_current: milliamps(max_current_a),
        max_power: milliwatts(max_voltage_v * max_current_a),
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
