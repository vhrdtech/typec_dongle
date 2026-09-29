//! Animated LEDs on PF0 (TIM14_CH1) and PF1 (TIM15_CH1N), each controlled independently via [`set`] and
//! [`set_brightness`].
//!
//! NOTE: The only remaining caller of the 64-bit multiply and divide routines is embassy's calculate_psc_arr.
//! That function runs once, when the PWM drivers are created, to work out the timer settings from 1 kHz.
//! Removing it would mean setting up TIM14 and TIM15 directly through the register-level API instead of SimplePwm and ComplementaryPwm.

use embassy_stm32::Peri;
use embassy_stm32::gpio::OutputType;
use embassy_stm32::peripherals::{PF0, PF1, TIM14, TIM15};
use embassy_stm32::time::{Hertz, khz};
use embassy_stm32::timer::Channel;
use embassy_stm32::timer::complementary_pwm::{ComplementaryPwm, ComplementaryPwmPin};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Ticker};

const PWM_FREQ: Hertz = khz(1);
/// Animation update period.
const TICK_MS: u32 = 10;
/// Blink edge duration, shortened to fit when on/off time is shorter.
const BLINK_FADE_MS: u32 = 100;

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Led {
    /// PF0, TIM14_CH1
    Power = 0,
    /// PF1, TIM15_CH1N
    Status = 1,
}

/// Peak brightness per [`Brightness`], perceptual (gamma corrected): 0 = off, 255 = full PWM duty.
const DIM: u8 = 40;
const NOMINAL: u8 = 128;
const BRIGHT: u8 = 255;

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Brightness {
    Dim,
    Nominal,
    Bright,
}

impl Brightness {
    fn value(self) -> u8 {
        match self {
            Brightness::Dim => DIM,
            Brightness::Nominal => NOMINAL,
            Brightness::Bright => BRIGHT,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Mode {
    Off,
    On,
    /// Fade 0 -> brightness -> 0, once per `period_ms`.
    Breath {
        period_ms: u32,
    },
    /// Eased fade up at the start of `on_ms`, eased fade down at the start of `off_ms`.
    Blink {
        on_ms: u32,
        off_ms: u32,
    },
}

type Sig<T> = Signal<CriticalSectionRawMutex, T>;
static MODES: [Sig<Mode>; 2] = [Signal::new(), Signal::new()];
static BRIGHTNESS: [Sig<Brightness>; 2] = [Signal::new(), Signal::new()];
/// Wakes the task when it is idle (no animation running).
static WAKE: Sig<()> = Signal::new();

/// Change the mode of one LED. Takes effect on the next animation tick; breath and blink restart from dark.
pub fn set(led: Led, mode: Mode) {
    MODES[led as usize].signal(mode);
    WAKE.signal(());
}

/// Change the peak brightness of one LED (default [`Brightness::Nominal`]) without restarting its animation.
pub fn set_brightness(led: Led, brightness: Brightness) {
    BRIGHTNESS[led as usize].signal(brightness);
    WAKE.signal(());
}

// All math below is u32: Cortex-M0+ has no 64-bit (or any) hardware divide.
struct State {
    mode: Mode,
    /// Breath: position in the cycle, one full u32 wrap = one period.
    /// Blink: ms since the start of the current on time.
    phase: u32,
    /// Breath: phase increment per tick. Blink: edge fade duration in ms (>= 1).
    step: u32,
}

impl State {
    fn new(mode: Mode) -> Self {
        let mode = match mode {
            Mode::Blink { on_ms: 0, .. } => Mode::Off,
            Mode::Blink { off_ms: 0, .. } => Mode::On,
            m => m,
        };
        let step = match mode {
            Mode::Breath { period_ms, .. } => u32::MAX / period_ms.max(TICK_MS) * TICK_MS,
            Mode::Blink { on_ms, off_ms, .. } => BLINK_FADE_MS.min(on_ms).min(off_ms).max(1),
            _ => 0,
        };
        Self {
            mode,
            phase: 0,
            step,
        }
    }

    /// Animation shape, perceptual 0..=65535, before brightness scaling.
    fn level(&self) -> u32 {
        match self.mode {
            Mode::Off => 0,
            Mode::On => 65535,
            Mode::Breath { .. } => {
                // triangle 0 -> 65535 -> 0
                let p = self.phase >> 15; // 0..131072
                let x = if p < 65536 { p } else { 131071 - p };
                ease(x)
            }
            Mode::Blink { on_ms, .. } => {
                let fade = self.step;
                let x = if self.phase < on_ms {
                    self.phase.min(fade) * 65535 / fade
                } else {
                    65535 - (self.phase - on_ms).min(fade) * 65535 / fade
                };
                ease(x)
            }
        }
    }

    fn animated(&self) -> bool {
        matches!(self.mode, Mode::Breath { .. } | Mode::Blink { .. })
    }

    fn advance(&mut self) {
        match self.mode {
            Mode::Blink { on_ms, off_ms, .. } => {
                self.phase += TICK_MS;
                let period = on_ms + off_ms;
                if self.phase >= period {
                    self.phase = if period > 0 { self.phase % period } else { 0 };
                }
            }
            _ => self.phase = self.phase.wrapping_add(self.step),
        }
    }
}

/// Smoothstep 3x^2 - 2x^3 on Q16 (0..=65535), for soft starts and stops.
fn ease(x: u32) -> u32 {
    // x^2 * (3 - 2x), with the second factor pre-shifted by 2 to fit in 32 bits
    let x2 = x * x >> 16;
    x2 * ((3 * 65536 - 2 * x) >> 2) >> 14
}

/// Map perceptual level (0..=65535) to PWM duty with gamma 2. `max_duty` must be <= 65536.
fn duty(level: u32, max_duty: u32) -> u32 {
    let l2 = level * level >> 16; // 0..=65534
    (l2 * max_duty + 0x8000) >> 16 // rounded: full level gives max_duty (within 1 count at 65536)
}

#[embassy_executor::task]
pub async fn led_task(
    tim14: Peri<'static, TIM14>,
    pf0: Peri<'static, PF0>,
    tim15: Peri<'static, TIM15>,
    pf1: Peri<'static, PF1>,
) {
    let mut pwm0 = SimplePwm::new(
        tim14,
        Some(PwmPin::new(pf0, OutputType::PushPull)),
        None,
        None,
        None,
        PWM_FREQ,
        CountingMode::EdgeAlignedUp,
    );
    let mut ch0 = pwm0.ch1();
    ch0.set_duty_cycle_fully_off();
    ch0.enable();

    let mut pwm1 = ComplementaryPwm::new(
        tim15,
        None,
        Some(ComplementaryPwmPin::new(pf1, OutputType::PushPull)),
        None,
        None,
        None,
        None,
        None,
        None,
        PWM_FREQ,
        CountingMode::EdgeAlignedUp,
    );
    // CH1N is the inverse of OC1REF, so the compare value is inverted below (CCR1 = max means off)
    let max1 = pwm1.get_max_duty();
    pwm1.set_duty(Channel::Ch1, max1);
    pwm1.enable(Channel::Ch1);

    let mut leds = [State::new(Mode::Off), State::new(Mode::Off)];
    let mut brightness = [Brightness::Nominal; 2];
    let mut ticker = Ticker::every(Duration::from_millis(TICK_MS as u64));
    loop {
        for i in 0..2 {
            if let Some(mode) = MODES[i].try_take() {
                leds[i] = State::new(mode);
            }
            if let Some(b) = BRIGHTNESS[i].try_take() {
                brightness[i] = b;
            }
        }
        let level = |i: usize| leds[i].level() * brightness[i].value() as u32 / 255;

        let max0 = ch0.max_duty_cycle();
        ch0.set_duty_cycle(duty(level(0), max0));
        pwm1.set_duty(Channel::Ch1, max1 - duty(level(1), max1));
        for led in &mut leds {
            led.advance();
        }

        if leds.iter().any(State::animated) {
            ticker.next().await;
        } else {
            // nothing to animate: sleep until a mode or brightness changes
            WAKE.wait().await;
            ticker.reset();
        }
    }
}
