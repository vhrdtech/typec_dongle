//! `donguru led` - the power and status LEDs as one indication: mode, brightness and a blink to find the dongle on
//! a bench (CLI-21, API-9). Items an older firmware doesn't have answer `Unimplemented`: "firmware does not support
//! this yet", exit code 4.

use crate::commands::power::parse_quantity;
use crate::context::Context;
use crate::exit::ExitCode;
use clap::{Subcommand, ValueEnum};
use donguru_core::Dongle;
use donguru_core::ww_indication::{Brightness, Mode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ModeArg {
    /// Every LED off, including alerts
    AllOff,
    /// Only what must stay visible: faults and power
    Essential,
    /// Every LED behaves as designed
    Normal,
}

impl From<ModeArg> for Mode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::AllOff => Mode::AllOff,
            ModeArg::Essential => Mode::OnlyEssential,
            ModeArg::Normal => Mode::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BrightnessArg {
    /// Night mode, non-distracting
    Lowest,
    /// Day mode, non-distracting
    Low,
    /// Good visibility inside
    Normal,
    /// For alerts inside
    High,
    /// Visible outside
    Highest,
}

impl From<BrightnessArg> for Brightness {
    fn from(b: BrightnessArg) -> Self {
        match b {
            BrightnessArg::Lowest => Brightness::Lowest,
            BrightnessArg::Low => Brightness::Low,
            BrightnessArg::Normal => Brightness::Normal,
            BrightnessArg::High => Brightness::High,
            BrightnessArg::Highest => Brightness::Highest,
        }
    }
}

fn parse_seconds(s: &str) -> Result<f64, String> {
    let secs = parse_quantity(s, 's')?;
    if secs == 0.0 || secs > 600.0 {
        return Err(format!("{s}: must be above 0 and at most 600 s"));
    }
    Ok(secs)
}

#[derive(Debug, Subcommand)]
pub enum Led {
    /// Show or set what the LEDs do
    Mode {
        /// New mode; omit to show the current one
        #[arg(value_enum)]
        mode: Option<ModeArg>,
    },

    /// Show or set the LED brightness
    Brightness {
        /// New brightness; omit to show the current one
        #[arg(value_enum)]
        level: Option<BrightnessArg>,
    },

    /// Blink the LEDs so this dongle can be found on a bench, then return to the mode
    Identify {
        /// How long to blink in seconds (`5` or `5s`, at most 600)
        #[arg(value_name = "SECONDS", value_parser = parse_seconds, default_value = "5")]
        duration: f64,
    },
}

fn fmt_mode(m: Mode) -> &'static str {
    match m {
        Mode::AllOff => "all-off",
        Mode::OnlyEssential => "essential",
        Mode::Normal => "normal",
        #[allow(unreachable_patterns)] // a mode of a newer firmware
        _ => "unknown",
    }
}

fn fmt_brightness(b: Brightness) -> &'static str {
    match b {
        Brightness::Lowest => "lowest",
        Brightness::Low => "low",
        Brightness::Normal => "normal",
        Brightness::High => "high",
        Brightness::Highest => "highest",
        #[allow(unreachable_patterns)] // a level of a newer firmware
        _ => "unknown",
    }
}

async fn mode_on(dongle: &Dongle, wanted: Option<ModeArg>) -> donguru_core::Result<Mode> {
    if let Some(wanted) = wanted {
        dongle.leds().set_mode(wanted.into()).await?;
    }
    dongle.leds().mode().await
}

async fn brightness_on(
    dongle: &Dongle,
    wanted: Option<BrightnessArg>,
) -> donguru_core::Result<Brightness> {
    if let Some(wanted) = wanted {
        dongle.leds().set_brightness(wanted.into()).await?;
    }
    dongle.leds().brightness().await
}

fn print_value(ctx: &Context, key: &str, value: &str) -> anyhow::Result<()> {
    crate::output::Output::stdout(ctx).emit(&crate::output::KeyValue(key, value))
}

pub fn run(ctx: &Context, command: Led) -> anyhow::Result<ExitCode> {
    match command {
        Led::Mode { mode } => {
            let now = super::with_dongle(ctx, async |dongle| mode_on(dongle, mode).await)?;
            print_value(ctx, "mode", fmt_mode(now))?;
            Ok(match mode {
                Some(w) if Mode::from(w) != now => ExitCode::Failure,
                _ => ExitCode::Success,
            })
        }
        Led::Brightness { level } => {
            let now = super::with_dongle(ctx, async |dongle| brightness_on(dongle, level).await)?;
            print_value(ctx, "brightness", fmt_brightness(now))?;
            Ok(match level {
                Some(w) if Brightness::from(w) != now => ExitCode::Failure,
                _ => ExitCode::Success,
            })
        }
        Led::Identify { duration } => {
            super::with_dongle(ctx, async |dongle| {
                dongle.leds().identify_secs(duration).await
            })?;
            print_value(ctx, "identify", &format!("{duration} s"))?;
            Ok(ExitCode::Success)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::mock::MockDevice;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mode_brightness_identify() {
        let mock = MockDevice::start("cli/led");
        let dongle = mock.open().await.unwrap();
        assert_eq!(mode_on(&dongle, None).await.unwrap(), Mode::Normal);
        assert_eq!(
            mode_on(&dongle, Some(ModeArg::Essential)).await.unwrap(),
            Mode::OnlyEssential
        );
        assert_eq!(
            brightness_on(&dongle, Some(BrightnessArg::Lowest))
                .await
                .unwrap(),
            Brightness::Lowest
        );
        dongle.leds().identify_secs(2.5).await.unwrap();
        assert_eq!(mock.state().await.identified, 1);
        // the mock does no identify above 60 s: the error is `Unsupported`, not a hang or a panic
        assert!(dongle.leds().identify_secs(120.0).await.is_err());
        dongle.close().await.unwrap();
    }

    #[test]
    fn duration_is_checked() {
        assert_eq!(parse_seconds("5s"), Ok(5.0));
        assert_eq!(parse_seconds("0.5"), Ok(0.5));
        assert!(parse_seconds("0").is_err());
        assert!(parse_seconds("601").is_err());
        assert!(parse_seconds("x").is_err());
    }

    #[test]
    fn names() {
        assert_eq!(fmt_mode(ModeArg::AllOff.into()), "all-off");
        assert_eq!(fmt_brightness(BrightnessArg::Highest.into()), "highest");
    }
}
