//! Animated LEDs on PF0 (TIM14_CH1) and PF1 (TIM15_CH1N): each runs its own animation ([`set`]), shown through one
//! global indication (API-9, `leds` in the API): which LEDs are [visible](set_visible), one [peak](set_peak)
//! brightness for both, and [`identify`], which blinks both for a while and then returns to the animations. The
//! animation, its math and the driver loop are embedded_bedrock's `bedrock_led` (DEV-12, host-tested there); this
//! module sets up the timers.
//!
//! NOTE: The 64-bit divide routines have one caller left, embassy's calculate_psc_arr. That function runs once, when
//! the PWM drivers are created, to work out the timer settings from 1 kHz. Removing it would mean setting up TIM14
//! and TIM15 directly through the register-level API instead of SimplePwm and ComplementaryPwm. The 64-bit multiply
//! has a second caller, bedrock_led's ease (once per LED and tick).

use bedrock_led::{Channel, Indication, Pwm};
pub use bedrock_led::{DEFAULT_PEAK, Mode};
use embassy_stm32::gpio::OutputType;
use embassy_stm32::peripherals::TIM15;
use embassy_stm32::time::{Hertz, khz};
use embassy_stm32::timer::complementary_pwm::{ComplementaryPwm, ComplementaryPwmPin};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};

use crate::LedResources;

const PWM_FREQ: Hertz = khz(1);

#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Led {
    /// PF0, TIM14_CH1
    Power = 0,
    /// PF1, TIM15_CH1N
    Status = 1,
}

static LEDS: Indication<2> = Indication::new();

/// Change the mode of one LED. Takes effect on the next animation tick; breath and blink restart from dark.
pub fn set(led: Led, mode: Mode) {
    LEDS.set(led as usize, mode);
}

/// Show or hide one LED (default shown); its animation keeps running hidden.
pub fn set_visible(led: Led, visible: bool) {
    LEDS.set_visible(led as usize, visible);
}

/// Peak brightness of both LEDs (default [`DEFAULT_PEAK`]), without restarting their animations.
pub fn set_peak(peak: u8) {
    LEDS.set_peak(peak);
}

/// Blink both LEDs at full brightness for `ms` (restarts a running identify, 0 ends it), then show the animations
/// again.
pub fn identify(ms: u32) {
    LEDS.identify(ms);
}

/// TIM15_CH1N: the inverse of OC1REF, so the compare value is inverted (CCR1 = max means off). ComplementaryPwm has
/// no embedded-hal `SetDutyCycle`.
struct StatusLed {
    pwm: ComplementaryPwm<'static, TIM15>,
    max: u32,
}

impl Channel for StatusLed {
    fn max_duty(&self) -> u32 {
        self.max
    }

    fn set_duty(&mut self, duty: u32) {
        self.pwm
            .set_duty(embassy_stm32::timer::Channel::Ch1, self.max - duty);
    }
}

#[embassy_executor::task]
pub async fn led_task(r: LedResources) {
    let mut pwm0 = SimplePwm::new(
        r.tim14,
        Some(PwmPin::new(r.pf0, OutputType::PushPull)),
        None,
        None,
        None,
        PWM_FREQ,
        CountingMode::EdgeAlignedUp,
    );
    let mut ch0 = pwm0.ch1();
    ch0.set_duty_cycle_fully_off();
    ch0.enable();
    let mut power = Pwm::new(ch0);

    let mut pwm1 = ComplementaryPwm::new(
        r.tim15,
        None,
        Some(ComplementaryPwmPin::new(r.pf1, OutputType::PushPull)),
        None,
        None,
        None,
        None,
        None,
        None,
        PWM_FREQ,
        CountingMode::EdgeAlignedUp,
    );
    let max = pwm1.get_max_duty();
    pwm1.set_duty(embassy_stm32::timer::Channel::Ch1, max);
    pwm1.enable(embassy_stm32::timer::Channel::Ch1);
    let mut status = StatusLed { pwm: pwm1, max };

    LEDS.run([Mode::Off; 2], |leds| {
        power.set_level(leds.level(Led::Power as usize));
        status.set_level(leds.level(Led::Status as usize));
    })
    .await
}
