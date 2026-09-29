//! Push button and switch sharing one ADC input (PA7, [`adc::Readings::button_switch_mv`]).
//!
//! Divider: [`R_PULLUP_OHM`] pull-up to VDD (= VDDA), push button with [`R_BUTTON_OHM`] and switch with
//! [`R_SWITCH_OHM`] pull-down. Levels as a fraction of VDDA:
//! - neither: 1
//! - switch on: 0.875 (2.94 V at 3.36 V)
//! - button pushed: 0.5 (1.68 V)
//! - both: 0.467 (1.57 V)
//!
//! The levels are computed from the measured VDDA, so supply variation cancels out and no per board
//! calibration is needed. The voltage is matched to the nearest level, [`State::INVALID`] if farther than
//! [`MAX_DEVIATION_MV`] from all of them.
//!
//! A new state is accepted once every sample for [`DEBOUNCE`] matched it. With the RC filter on PA7 (100 nF)
//! a sample taken during a transition can be in between two levels, or pass another one (e.g. pressing the
//! button with the switch on goes through the button only level), and with contact bounce the oversampled
//! average can be anywhere. [`State::INVALID`] is only accepted after [`INVALID_DEBOUNCE`], so such samples
//! are ignored and only a lasting fault is reported.

use embassy_time::{Duration, Instant, Ticker};

use crate::adc;

const SAMPLE_PERIOD: Duration = Duration::from_millis(10);
/// Time a new state must be stable to be accepted. Spans at least 2 ADC updates (one per DMA half buffer,
/// ~15 ms), so a single bad update is never accepted, whatever [`SAMPLE_PERIOD`] is.
const DEBOUNCE: Duration = Duration::from_millis(40);
/// Time an invalid voltage must last to be reported.
const INVALID_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Switch {
    Invalid,
    On,
    Off,
}

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Button {
    Invalid,
    Pushed,
    NotPushed,
}

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub struct State {
    pub switch: Switch,
    pub button: Button,
}

impl State {
    const INVALID: State = State {
        switch: Switch::Invalid,
        button: Button::Invalid,
    };
}

const R_PULLUP_OHM: u64 = 10_000;
const R_BUTTON_OHM: u64 = 10_000;
const R_SWITCH_OHM: u64 = 69_800;
/// Half the smallest gap between levels (button vs. both, ~110 mV at 3.3 V), 3x the error from 1% resistors.
const MAX_DEVIATION_MV: u32 = 50;

/// Divider ratio in 1/65536 of VDDA, rounded. Evaluated at compile time, so no 64-bit math at runtime.
const fn ratio_q16(r_down_ohm: u64) -> u32 {
    let den = R_PULLUP_OHM + r_down_ohm;
    (((r_down_ohm << 16) + den / 2) / den) as u32
}

const fn parallel_ohm(a: u64, b: u64) -> u64 {
    (a * b + (a + b) / 2) / (a + b)
}

/// Level of each switch / button combination in 1/65536 of VDDA.
const LEVELS: [(u32, Switch, Button); 4] = [
    (1 << 16, Switch::Off, Button::NotPushed),
    (ratio_q16(R_SWITCH_OHM), Switch::On, Button::NotPushed),
    (ratio_q16(R_BUTTON_OHM), Switch::Off, Button::Pushed),
    (
        ratio_q16(parallel_ohm(R_BUTTON_OHM, R_SWITCH_OHM)),
        Switch::On,
        Button::Pushed,
    ),
];

/// State whose level at `vdda_mv` is nearest to `mv`.
pub fn classify(mv: u16, vdda_mv: u16) -> State {
    let mut best = State::INVALID;
    let mut best_dist = u32::MAX;
    for (ratio, switch, button) in LEVELS {
        let level_mv = (vdda_mv as u32 * ratio + (1 << 15)) >> 16;
        let dist = (mv as u32).abs_diff(level_mv);
        if dist < best_dist {
            best_dist = dist;
            best = State { switch, button };
        }
    }
    if best_dist > MAX_DEVIATION_MV {
        State::INVALID
    } else {
        best
    }
}

#[embassy_executor::task]
pub async fn button_task() {
    let mut ticker = Ticker::every(SAMPLE_PERIOD);
    let mut state = State::INVALID;
    let mut candidate = State::INVALID;
    let mut candidate_since = Instant::now();
    loop {
        ticker.next().await;
        // No readings while the ADC is (re)initialized
        let readings = adc::latest();
        let sample = readings.map_or(State::INVALID, |r| classify(r.button_switch_mv, r.vdda_mv));
        let now = Instant::now();
        if sample != candidate {
            candidate = sample;
            candidate_since = now;
        }
        let debounce = if candidate == State::INVALID {
            INVALID_DEBOUNCE
        } else {
            DEBOUNCE
        };
        if candidate != state && now - candidate_since >= debounce {
            state = candidate;
            #[cfg(feature = "debug-button")]
            defmt::debug!(
                "switch {}, button {} ({} mV, VDDA {} mV)",
                state.switch,
                state.button,
                readings.map(|r| r.button_switch_mv),
                readings.map(|r| r.vdda_mv)
            );
        }
    }
}
