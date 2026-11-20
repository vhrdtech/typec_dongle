//! VBUS switch modes and the firmware-side guard of the donguru firmware (`fw/donguru/src/power.rs`, API-11).
//!
//! A [`Switch`] is one VBUS switch (plug U601, header Q102): the [`VbusMode`] a host set, the [`VbusState`] the
//! firmware reports, and the guard that cuts it when the receptacle VBUS, the current or the power goes over the
//! mode's limits, or when the switch reports a fault (U601 FLTB). The firmware drives the enable pin from
//! [`Switch::enabled`] after every call and feeds it one [`Sample`] per ADC update.
//!
//! No embassy and no GPIO in here, so the rules are unit-tested on the host, as [`crate::hub`] and `donguru_dap` do.

use donguru_api::{GuardTrip, VbusMode, VbusState};
use ww_si::ww_numeric::NumericValue;
use ww_si::{Prefix, Second, Volt};

/// FLTB low in this many samples in a row is a fault (one ADC update is ~15 ms): a glitch during soft start or an
/// ESD hit is not.
pub const FAULT_SAMPLES: u8 = 2;

/// Longest `vbus_cycle` off time, ms: a longer one is an `InvalidArgument` (most likely a unit mistake).
pub const MAX_CYCLE_OFF_MS: u32 = 60_000;

/// One measurement the guard checks: the worst values of one ADC update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Receptacle VBUS (`+V1`, PA3), mV.
    pub vbus_mv: i32,
    /// Current through the shunt, mA, + = computer -> DUT. The guard checks its magnitude.
    pub current_ma: i32,
}

impl Sample {
    /// |V x I| in mW, without 64-bit maths: V up to ~26 V and I up to ~7 A keep it in `i32`.
    pub fn power_mw(&self) -> i32 {
        (self
            .vbus_mv
            .saturating_mul(self.current_ma.saturating_abs())
            / 1000)
            .saturating_abs()
    }
}

/// The limits of a guarded mode, in mV / mA / mW.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_mv: i32,
    pub max_ma: Option<i32>,
    pub max_mw: Option<i32>,
}

impl Limits {
    /// The limits of `mode`, `None` for `ForceOff`. A limit that isn't a positive number we can read, or a mode
    /// from a newer host, is [`ModeError::Invalid`].
    pub fn from_mode(mode: &VbusMode) -> Result<Option<Limits>, ModeError> {
        let positive = |v: Option<i32>| v.filter(|v| *v > 0).ok_or(ModeError::Invalid);
        match mode {
            VbusMode::ForceOff => Ok(None),
            VbusMode::ForceOn { max_voltage } => Ok(Some(Limits {
                max_mv: positive(milli(max_voltage.prefix, &max_voltage.value))?,
                max_ma: None,
                max_mw: None,
            })),
            VbusMode::Auto {
                max_voltage,
                max_current,
                max_power,
            } => Ok(Some(Limits {
                max_mv: positive(milli(max_voltage.prefix, &max_voltage.value))?,
                max_ma: Some(positive(milli(max_current.prefix, &max_current.value))?),
                max_mw: Some(positive(milli(max_power.prefix, &max_power.value))?),
            })),
            #[allow(unreachable_patterns)] // future variants in the borrowed type
            _ => Err(ModeError::Invalid),
        }
    }

    /// The first limit `s` is over: voltage, then current, then power.
    pub fn check(&self, s: Sample) -> Option<GuardTrip> {
        if s.vbus_mv > self.max_mv {
            Some(GuardTrip::OverVoltage)
        } else if self
            .max_ma
            .is_some_and(|max| s.current_ma.saturating_abs() > max)
        {
            Some(GuardTrip::OverCurrent)
        } else if self.max_mw.is_some_and(|max| s.power_mw() > max) {
            Some(GuardTrip::OverPower)
        } else {
            None
        }
    }
}

/// Why a mode (or the end of a cycle) didn't switch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeError {
    /// A limit isn't a positive number in a unit we read, or the mode is unknown.
    Invalid,
    /// VBUS is already over the mode's `max_voltage`.
    OverVoltage { measured_mv: i32 },
    /// No measurement yet (ADC starting or switching channels): a guarded mode can't be checked.
    NoMeasurement,
    /// `Auto` on a switch whose current isn't measured (the header's VBUS_SW bypasses the shunt).
    CurrentNotMeasured,
}

/// Why a cycle didn't start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleError {
    /// The mode is `ForceOff`: there is nothing to return to.
    WrongState,
    /// Another cycle is running.
    Busy,
    /// The off time isn't a non-negative time up to [`MAX_CYCLE_OFF_MS`].
    Invalid,
}

/// One VBUS switch: mode, state, guard.
#[derive(Clone, Debug)]
pub struct Switch {
    mode: VbusMode,
    limits: Option<Limits>,
    state: VbusState,
    fault_samples: u8,
    cycling: bool,
    /// Bumped by every mode write: a cycle that ends after one doesn't override it.
    generation: u32,
    /// Whether [`Sample::current_ma`] is this switch's current; `Auto` is refused when it isn't.
    current_measured: bool,
}

impl Switch {
    /// `ForceOff` and off, the boot state (hw_b153a.md §14). `current_measured`: whether the samples' current is
    /// this switch's (plug: yes, header: no).
    pub const fn new(current_measured: bool) -> Self {
        Switch {
            mode: VbusMode::ForceOff,
            limits: None,
            state: VbusState::Off,
            fault_samples: 0,
            cycling: false,
            generation: 0,
            current_measured,
        }
    }

    pub fn mode(&self) -> &VbusMode {
        &self.mode
    }

    pub fn state(&self) -> VbusState {
        self.state
    }

    /// Whether the enable pin is to be high.
    pub fn enabled(&self) -> bool {
        self.state == VbusState::On
    }

    pub fn cycling(&self) -> bool {
        self.cycling
    }

    /// Sets `mode`. A guarded mode switches on when `sample` is under its `max_voltage`, else nothing changes.
    /// Any write re-arms a tripped or faulted switch (the enable pin went low when it tripped, so U601 sees the
    /// EN toggle it needs to leave its latch) and ends a running cycle.
    pub fn set_mode(&mut self, mode: VbusMode, sample: Option<Sample>) -> Result<(), ModeError> {
        let limits = Limits::from_mode(&mode)?;
        if !self.current_measured && matches!(mode, VbusMode::Auto { .. }) {
            return Err(ModeError::CurrentNotMeasured);
        }
        if let Some(limits) = limits {
            let s = sample.ok_or(ModeError::NoMeasurement)?;
            if s.vbus_mv > limits.max_mv {
                return Err(ModeError::OverVoltage {
                    measured_mv: s.vbus_mv,
                });
            }
        }
        self.mode = mode;
        self.limits = limits;
        self.state = if limits.is_some() {
            VbusState::On
        } else {
            VbusState::Off
        };
        self.fault_samples = 0;
        self.cycling = false;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    /// One ADC update: `sample` (`None` while the ADC restarts) and the switch's fault input (`true` = FLTB low;
    /// always `false` for a switch without one). Returns whether the state changed.
    pub fn on_sample(&mut self, sample: Option<Sample>, fault: bool) -> bool {
        if self.state != VbusState::On {
            self.fault_samples = 0;
            return false;
        }
        if fault {
            self.fault_samples = self.fault_samples.saturating_add(1);
            if self.fault_samples >= FAULT_SAMPLES {
                self.state = VbusState::Fault;
                return true;
            }
        } else {
            self.fault_samples = 0;
        }
        if let (Some(limits), Some(s)) = (self.limits, sample)
            && let Some(trip) = limits.check(s)
        {
            self.state = VbusState::Tripped(trip);
            return true;
        }
        false
    }

    /// Starts a cycle: off now, back to the mode with [`Switch::cycle_end`] after `off`. Returns the off time in
    /// ms and a token for `cycle_end`.
    pub fn cycle_start(&mut self, off: &Second) -> Result<(u32, u32), CycleError> {
        let off_ms = milli(off.prefix, &off.value)
            .filter(|ms| (0..=MAX_CYCLE_OFF_MS as i32).contains(ms))
            .ok_or(CycleError::Invalid)? as u32;
        if self.cycling {
            return Err(CycleError::Busy);
        }
        if self.limits.is_none() {
            return Err(CycleError::WrongState);
        }
        self.cycling = true;
        self.state = VbusState::Off;
        self.fault_samples = 0;
        Ok((off_ms, self.generation))
    }

    /// Ends the cycle started with `token`: back to the mode, through the guard as a mode write is. When the mode
    /// was written meanwhile, that write stands and the state is returned as it is. Over voltage leaves the switch
    /// `Tripped(OverVoltage)`.
    pub fn cycle_end(
        &mut self,
        token: u32,
        sample: Option<Sample>,
    ) -> Result<VbusState, ModeError> {
        if token != self.generation || !self.cycling {
            return Ok(self.state);
        }
        self.cycling = false;
        match self.set_mode(self.mode.clone(), sample) {
            Ok(()) => Ok(self.state),
            Err(e) => {
                if matches!(e, ModeError::OverVoltage { .. }) {
                    self.state = VbusState::Tripped(GuardTrip::OverVoltage);
                }
                Err(e)
            }
        }
    }
}

/// `value` x 10^prefix in thousandths of the unit (mV, mA, mW, ms), integer maths only; `None` for a number type
/// or prefix it doesn't read, or when it doesn't fit in `i32`.
pub fn milli(prefix: Prefix, value: &NumericValue) -> Option<i32> {
    let v: i32 = match *value {
        NumericValue::U8(v) => v as i32,
        NumericValue::U16(v) => v as i32,
        NumericValue::U32(v) => i32::try_from(v).ok()?,
        NumericValue::I32(v) => v,
        #[allow(unreachable_patterns)] // more number types on the host
        _ => return None,
    };
    match prefix {
        Prefix::Kilo => v.checked_mul(1_000_000),
        Prefix::Unit => v.checked_mul(1000),
        Prefix::Deci => v.checked_mul(100),
        Prefix::Centi => v.checked_mul(10),
        Prefix::Milli => Some(v),
        Prefix::Micro => Some(v / 1000),
        _ => None,
    }
}

/// `mv` as a [`Volt`] in mV, for answers.
pub fn volt(mv: i32) -> Volt {
    Volt::new(Prefix::Milli, NumericValue::I32(mv))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ww_si::{Ampere, Watt};

    fn force_on(mv: i32) -> VbusMode {
        VbusMode::ForceOn {
            max_voltage: volt(mv),
        }
    }

    fn auto(mv: i32, ma: i32, mw: i32) -> VbusMode {
        VbusMode::Auto {
            max_voltage: volt(mv),
            max_current: Ampere::new(Prefix::Milli, NumericValue::I32(ma)),
            max_power: Watt::new(Prefix::Milli, NumericValue::I32(mw)),
        }
    }

    fn s(mv: i32, ma: i32) -> Option<Sample> {
        Some(Sample {
            vbus_mv: mv,
            current_ma: ma,
        })
    }

    #[test]
    fn milli_units() {
        assert_eq!(milli(Prefix::Unit, &NumericValue::U8(5)), Some(5000));
        assert_eq!(milli(Prefix::Milli, &NumericValue::I32(5500)), Some(5500));
        assert_eq!(
            milli(Prefix::Micro, &NumericValue::U32(1_500_000)),
            Some(1500)
        );
        assert_eq!(milli(Prefix::Deci, &NumericValue::U16(55)), Some(5500));
        assert_eq!(milli(Prefix::Mega, &NumericValue::U8(1)), None);
        assert_eq!(milli(Prefix::Unit, &NumericValue::U32(u32::MAX)), None);
        assert_eq!(milli(Prefix::Kilo, &NumericValue::I32(10_000)), None);
    }

    #[test]
    fn boots_off() {
        let sw = Switch::new(true);
        assert_eq!(sw.state(), VbusState::Off);
        assert!(!sw.enabled());
        assert_eq!(sw.mode(), &VbusMode::ForceOff);
    }

    #[test]
    fn force_on_under_the_limit() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5100, 0)).unwrap();
        assert!(sw.enabled());
        // current isn't guarded in ForceOn
        assert!(!sw.on_sample(s(5100, 6000), false));
        assert!(sw.enabled());
    }

    #[test]
    fn refused_over_the_limit_changes_nothing() {
        let mut sw = Switch::new(true);
        assert_eq!(
            sw.set_mode(force_on(4000), s(5100, 0)),
            Err(ModeError::OverVoltage { measured_mv: 5100 })
        );
        assert_eq!(sw.state(), VbusState::Off);
        assert_eq!(sw.mode(), &VbusMode::ForceOff);
        assert_eq!(
            sw.set_mode(force_on(5500), None),
            Err(ModeError::NoMeasurement)
        );
        // ForceOff needs no measurement
        sw.set_mode(VbusMode::ForceOff, None).unwrap();
    }

    #[test]
    fn invalid_limits() {
        let mut sw = Switch::new(true);
        assert_eq!(
            sw.set_mode(force_on(0), s(5000, 0)),
            Err(ModeError::Invalid)
        );
        assert_eq!(
            sw.set_mode(auto(5500, -1, 1000), s(5000, 0)),
            Err(ModeError::Invalid)
        );
        let mega = VbusMode::ForceOn {
            max_voltage: Volt::new(Prefix::Mega, NumericValue::U8(1)),
        };
        assert_eq!(sw.set_mode(mega, s(5000, 0)), Err(ModeError::Invalid));
    }

    #[test]
    fn trips_on_a_later_rise() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        assert!(sw.on_sample(s(9000, 0), false));
        assert_eq!(sw.state(), VbusState::Tripped(GuardTrip::OverVoltage));
        assert!(!sw.enabled());
        // latched: VBUS coming back down doesn't switch it on again
        assert!(!sw.on_sample(s(5000, 0), false));
        assert!(!sw.enabled());
        // a write re-arms it
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        assert!(sw.enabled());
    }

    #[test]
    fn auto_trips_on_current_and_power() {
        let mut sw = Switch::new(true);
        sw.set_mode(auto(5500, 500, 10_000), s(5000, 0)).unwrap();
        assert!(!sw.on_sample(s(5000, 400), false));
        // the magnitude: the DUT feeding the computer trips too
        assert!(sw.on_sample(s(5000, -600), false));
        assert_eq!(sw.state(), VbusState::Tripped(GuardTrip::OverCurrent));

        sw.set_mode(auto(5500, 3000, 2000), s(5000, 0)).unwrap();
        assert!(!sw.on_sample(s(5000, 399), false));
        assert!(sw.on_sample(s(5000, 401), false));
        assert_eq!(sw.state(), VbusState::Tripped(GuardTrip::OverPower));
    }

    #[test]
    fn auto_needs_a_measured_current() {
        let mut sw = Switch::new(false);
        assert_eq!(
            sw.set_mode(auto(5500, 500, 2500), s(5000, 0)),
            Err(ModeError::CurrentNotMeasured)
        );
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        assert!(sw.enabled());
    }

    #[test]
    fn missing_samples_dont_trip() {
        let mut sw = Switch::new(true);
        sw.set_mode(auto(5500, 100, 100), s(5000, 0)).unwrap();
        assert!(!sw.on_sample(None, false));
        assert!(sw.enabled());
    }

    #[test]
    fn fault_is_debounced() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        assert!(!sw.on_sample(s(5000, 0), true));
        assert!(!sw.on_sample(s(5000, 0), false));
        assert!(!sw.on_sample(s(5000, 0), true));
        assert!(sw.on_sample(s(5000, 0), true));
        assert_eq!(sw.state(), VbusState::Fault);
        assert!(!sw.enabled());
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        assert!(sw.enabled());
    }

    #[test]
    fn cycle_returns_to_the_mode() {
        let mut sw = Switch::new(true);
        assert_eq!(
            sw.cycle_start(&Second::new(Prefix::Milli, NumericValue::U16(500))),
            Err(CycleError::WrongState)
        );
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        let (ms, token) = sw
            .cycle_start(&Second::new(Prefix::Milli, NumericValue::U16(500)))
            .unwrap();
        assert_eq!(ms, 500);
        assert!(!sw.enabled());
        assert_eq!(
            sw.cycle_start(&Second::new(Prefix::Unit, NumericValue::U8(1))),
            Err(CycleError::Busy)
        );
        // the guard doesn't act on an off switch
        assert!(!sw.on_sample(s(9000, 0), true));
        assert_eq!(sw.cycle_end(token, s(5000, 0)), Ok(VbusState::On));
        assert!(sw.enabled());
    }

    #[test]
    fn cycle_from_tripped_rearms() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        sw.on_sample(s(6000, 0), false);
        let (_, token) = sw
            .cycle_start(&Second::new(Prefix::Milli, NumericValue::U16(10)))
            .unwrap();
        assert_eq!(sw.cycle_end(token, s(5000, 0)), Ok(VbusState::On));
    }

    #[test]
    fn cycle_end_over_voltage_trips() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        let (_, token) = sw
            .cycle_start(&Second::new(Prefix::Milli, NumericValue::U16(10)))
            .unwrap();
        assert_eq!(
            sw.cycle_end(token, s(9000, 0)),
            Err(ModeError::OverVoltage { measured_mv: 9000 })
        );
        assert_eq!(sw.state(), VbusState::Tripped(GuardTrip::OverVoltage));
        assert!(!sw.cycling());
    }

    #[test]
    fn mode_write_during_a_cycle_wins() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        let (_, token) = sw
            .cycle_start(&Second::new(Prefix::Milli, NumericValue::U16(10)))
            .unwrap();
        sw.set_mode(VbusMode::ForceOff, None).unwrap();
        assert_eq!(sw.cycle_end(token, s(5000, 0)), Ok(VbusState::Off));
        assert!(!sw.enabled());
    }

    #[test]
    fn cycle_off_time_checked() {
        let mut sw = Switch::new(true);
        sw.set_mode(force_on(5500), s(5000, 0)).unwrap();
        assert_eq!(
            sw.cycle_start(&Second::new(Prefix::Unit, NumericValue::U8(61))),
            Err(CycleError::Invalid)
        );
        assert_eq!(
            sw.cycle_start(&Second::new(Prefix::Milli, NumericValue::I32(-1))),
            Err(CycleError::Invalid)
        );
        assert!(sw.enabled());
    }
}
