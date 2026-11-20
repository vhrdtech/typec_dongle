//! `donguru gpio` - read and drive general-purpose I/O pins.

use std::time::Duration;

use crate::context::Context;
use crate::exit::ExitCode;
use clap::{Args, Subcommand, ValueEnum};

/// Default delay between two samples of `gpio watch`.
const DEFAULT_WATCH_INTERVAL: &str = "100ms";

/// Parse a human-readable duration (e.g. `250ms`, `1s`, `1m30s`) and reject zero.
///
/// A unit is mandatory: `humantime` rejects a bare number such as `100`.
fn parse_interval(duration: &str) -> Result<Duration, String> {
    let interval = humantime::parse_duration(duration).map_err(|err| err.to_string())?;
    if interval.is_zero() {
        return Err("interval must be greater than zero".to_string());
    }
    Ok(interval)
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Pin {
    #[value(aliases = ["p1", "1"], help = "First GPIO pin [aliases: p1, 1]")]
    Pin1,
    #[value(aliases = ["p2", "2"], help = "Second GPIO pin [aliases: p2, 2]")]
    Pin2,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Mode {
    #[value(aliases = ["in", "i"], help = "Sample the pin, high impedance [aliases: in, i]")]
    Input,
    #[value(aliases = ["out", "o"], help = "Drive the pin [aliases: out, o]")]
    Output,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Level {
    #[value(aliases = ["1", "hi", "on"], help = "Logic high [aliases: 1, hi, on]")]
    High,
    #[value(aliases = ["0", "lo", "off"], help = "Logic low [aliases: 0, lo, off]")]
    Low,
}

/// A single pin, shared by the commands that operate on exactly one pin.
#[derive(Debug, Clone, Copy, Args)]
pub struct GpioPin {
    #[arg(
        value_enum,
        value_name = "PIN",
        help = "Pin to operate on [aliases: p1|1, p2|2]"
    )]
    /// Pin to operate on
    ///
    /// The pin may be given by its short alias, e.g. `1` or `p2`.
    pub pin: Pin,
}

/// One or more pins, shared by the commands that operate on a pin set.
#[derive(Debug, Clone, Args)]
pub struct Pins {
    #[arg(
        required = true,
        value_enum,
        value_name = "PIN",
        help = "Pin(s) to operate on [aliases: p1|1, p2|2]"
    )]
    /// Pin(s) to operate on
    ///
    /// Each pin may be given by its short alias, e.g. `1` or `p2`.
    pub pins: Vec<Pin>,
}

#[derive(Debug, Subcommand)]
pub enum Gpio {
    /// Read one or more input pins
    ///
    /// Prints the current level of every given pin and exits.
    #[command(alias = "get")]
    Read {
        #[command(flatten)]
        pins: Pins,
    },

    /// Control the level of an output pin
    ///
    /// The pin must be configured as an output first, see `donguru gpio configure`.
    #[command(alias = "set")]
    Write {
        #[command(flatten)]
        pin: GpioPin,

        #[arg(
            value_enum,
            value_name = "LEVEL",
            help = "Level to drive [aliases: 1|hi|on, 0|lo|off]"
        )]
        /// Level to drive the pin to
        level: Level,
    },

    /// Configure a GPIO to be either an input or output pin
    #[command(alias = "mode")]
    Configure {
        #[command(flatten)]
        pin: GpioPin,

        #[arg(
            value_enum,
            value_name = "MODE",
            help = "Mode to configure the pin for [aliases: in|i, out|o]"
        )]
        /// Direction to configure the pin for
        mode: Mode,
    },

    /// Stream the status of one or multiple input pins
    ///
    /// Samples the given pins until interrupted (Ctrl-C). Combine with
    /// `--format jsonl` to append the stream to a log file.
    Watch {
        #[command(flatten)]
        pins: Pins,

        /// Delay between two samples, e.g. `250ms`, `1s` or `1m30s`
        #[arg(
            short,
            long,
            value_name = "DURATION",
            env = "DONGURU_GPIO_INTERVAL",
            default_value = DEFAULT_WATCH_INTERVAL,
            value_parser = parse_interval,
        )]
        interval: Duration,
    },
}

fn fmt_pins(pins: &[Pin]) -> String {
    pins.iter()
        .map(|pin| format!("{pin:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn run(_ctx: &Context, cmd: Gpio) -> anyhow::Result<ExitCode> {
    match cmd {
        Gpio::Read { pins } => super::stub(
            &format!("gpio read {}", fmt_pins(&pins.pins)),
            ExitCode::Failure,
        ),
        Gpio::Write { pin, level } => super::stub(
            &format!("gpio write {:?} {level:?}", pin.pin),
            ExitCode::Failure,
        ),
        Gpio::Configure { pin, mode } => super::stub(
            &format!("gpio mode {:?} {mode:?}", pin.pin),
            ExitCode::Failure,
        ),
        Gpio::Watch { pins, interval } => super::stub(
            &format!(
                "gpio watch {} [interval: {}]",
                fmt_pins(&pins.pins),
                humantime::format_duration(interval)
            ),
            ExitCode::Failure,
        ),
    }
}
