//! `donguru power` - plug VBUS (U601), USB data stays with `donguru usb`, the power meter, and (CLI-11) a combined
//! view of plug + header + meter. See `docs/user/power.md` for the VBUS modes and the guard the firmware keeps.

use std::io::Write;
use std::time::Duration;

use clap::{Args, Subcommand};

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use donguru_core::api::{
    DataPath, GuardTrip, MeterReading, MeterTotals, Second, VbusMode, VbusState,
};
use donguru_core::client::observe::{
    BatchedOwned, ObserveEvent, PowerMeterObserve, on_change as on_change_mode, periodic, samples,
};

/// Default delay between two samples of `power watch`.
const DEFAULT_WATCH_INTERVAL: &str = "200ms";
/// Default off time of `power cycle`.
const DEFAULT_CYCLE_OFF: &str = "500ms";

/// Parse a human-readable duration (e.g. `250ms`, `1s`) and reject zero.
fn parse_interval(duration: &str) -> Result<Duration, String> {
    let interval = humantime::parse_duration(duration).map_err(|err| err.to_string())?;
    if interval.is_zero() {
        return Err("interval must be greater than zero".to_string());
    }
    Ok(interval)
}

/// A volt or an ampere value, as a plain number with an optional unit suffix (`20`, `20V`, `2A`).
fn parse_quantity(s: &str, unit: char) -> Result<f64, String> {
    let trimmed = s.trim();
    let trimmed = trimmed
        .strip_suffix(unit)
        .or_else(|| trimmed.strip_suffix(unit.to_ascii_lowercase()))
        .unwrap_or(trimmed)
        .trim();
    let value: f64 = trimmed.parse().map_err(|_| format!("{s}: not a number"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!("{s}: must be a non-negative number"));
    }
    Ok(value)
}

pub(crate) fn parse_volt(s: &str) -> Result<f64, String> {
    parse_quantity(s, 'V')
}

pub(crate) fn parse_ampere(s: &str) -> Result<f64, String> {
    parse_quantity(s, 'A')
}

/// Shared by `power on` and `header vbus on`: the guard limits of the mode to switch to.
#[derive(Debug, Clone, Args)]
pub struct VbusOnArgs {
    /// Raise the voltage limit above the 5.5 V default (e.g. for a PD charger): `20` or `20V`
    #[arg(long, value_name = "VOLT", value_parser = parse_volt)]
    pub allow_voltage: Option<f64>,

    /// Switch to Auto mode and cut above this current: `2` or `2A`
    #[arg(long, value_name = "AMPERE", value_parser = parse_ampere)]
    pub trip: Option<f64>,
}

/// The mode `args` asks for: `ForceOn` at the voltage limit, or `Auto` once `--trip` gives a current limit too.
pub(crate) fn mode_from_args(args: &VbusOnArgs) -> VbusMode {
    let max_voltage = args
        .allow_voltage
        .unwrap_or(donguru_core::vbus::DEFAULT_MAX_VOLTAGE_V);
    match args.trip {
        Some(max_current) => donguru_core::vbus::auto(max_voltage, max_current),
        None => donguru_core::vbus::force_on(max_voltage),
    }
}

pub(crate) fn fmt_trip(reason: GuardTrip) -> &'static str {
    match reason {
        GuardTrip::OverVoltage => "over-voltage",
        GuardTrip::OverCurrent => "over-current",
        GuardTrip::OverPower => "over-power",
        GuardTrip::Unknown(_) => "unknown (newer firmware)",
    }
}

fn trip_json(reason: GuardTrip) -> &'static str {
    match reason {
        GuardTrip::OverVoltage => "over_voltage",
        GuardTrip::OverCurrent => "over_current",
        GuardTrip::OverPower => "over_power",
        GuardTrip::Unknown(_) => "unknown",
    }
}

pub(crate) fn fmt_vbus_state(state: VbusState) -> String {
    match state {
        VbusState::Off => "off".to_string(),
        VbusState::On => "on".to_string(),
        VbusState::Fault => "fault (latched; write the mode again to re-arm)".to_string(),
        VbusState::Tripped(reason) => format!("tripped ({})", fmt_trip(reason)),
        VbusState::Unknown(_) => "unknown (newer firmware)".to_string(),
    }
}

pub(crate) fn state_json(state: VbusState) -> serde_json::Value {
    match state {
        VbusState::Off => serde_json::json!("off"),
        VbusState::On => serde_json::json!("on"),
        VbusState::Fault => serde_json::json!("fault"),
        VbusState::Tripped(reason) => serde_json::json!({ "tripped": trip_json(reason) }),
        VbusState::Unknown(_) => serde_json::json!("unknown"),
    }
}

pub(crate) fn fmt_vbus_mode(mode: &VbusMode) -> String {
    match mode {
        VbusMode::ForceOff => "off".to_string(),
        VbusMode::ForceOn { max_voltage } => format!("on, max {:.2} V", max_voltage.to_f64()),
        VbusMode::Auto {
            max_voltage,
            max_current,
            max_power,
        } => format!(
            "auto, max {:.2} V / {:.3} A / {:.2} W",
            max_voltage.to_f64(),
            max_current.to_f64(),
            max_power.to_f64()
        ),
        VbusMode::Unknown(_) => "unknown (newer firmware)".to_string(),
    }
}

pub(crate) fn mode_json(mode: &VbusMode) -> serde_json::Value {
    match mode {
        VbusMode::ForceOff => serde_json::json!({ "mode": "force_off" }),
        VbusMode::ForceOn { max_voltage } => serde_json::json!({
            "mode": "force_on",
            "max_voltage_v": max_voltage.to_f64(),
        }),
        VbusMode::Auto {
            max_voltage,
            max_current,
            max_power,
        } => serde_json::json!({
            "mode": "auto",
            "max_voltage_v": max_voltage.to_f64(),
            "max_current_a": max_current.to_f64(),
            "max_power_w": max_power.to_f64(),
        }),
        VbusMode::Unknown(_) => serde_json::json!({ "mode": "unknown" }),
    }
}

fn fmt_data(data: DataPath) -> &'static str {
    match data {
        DataPath::Connected => "connected",
        DataPath::Cut => "cut",
    }
}

fn reading_text(r: &MeterReading) -> String {
    format!(
        "{:.3} V   {:.1} mA   {:.1} mW",
        r.vbus_in.to_f64(),
        r.current.to_f64() * 1000.0,
        r.power.to_f64() * 1000.0
    )
}

fn reading_json(r: &MeterReading) -> serde_json::Value {
    serde_json::json!({
        "vbus_in_v": r.vbus_in.to_f64(),
        "current_ma": r.current.to_f64() * 1000.0,
        "power_mw": r.power.to_f64() * 1000.0,
    })
}

#[derive(Debug, Subcommand)]
pub enum Power {
    /// Plug VBUS, header VBUS_SW and the meter in one table
    Status,
    /// Turn plug VBUS on (`ForceOn`, or `Auto` with `--trip`)
    On(VbusOnArgs),
    /// Turn plug VBUS off
    Off,
    /// Off for a while, then back to the mode it was in
    Cycle {
        /// How long to stay off
        #[arg(long, value_name = "DURATION", default_value = DEFAULT_CYCLE_OFF, value_parser = parse_interval)]
        off: Duration,
        /// Take the plug SBU lines to Hi-Z first, so nothing on them back-powers the DUT
        #[arg(long)]
        release_sbu: bool,
    },
    /// One meter reading: VBUS_IN, current to the DUT, power
    Meter,
    /// Stream meter readings until interrupted (Ctrl-C)
    Watch {
        /// Delay between two samples, the on-change debounce with `--on-change`, or how old a batch of samples
        /// may get with `--samples`
        #[arg(
            short,
            long,
            value_name = "DURATION",
            env = "DONGURU_POWER_INTERVAL",
            default_value = DEFAULT_WATCH_INTERVAL,
            value_parser = parse_interval,
        )]
        interval: Duration,
        /// Send only when the reading moves beyond the meter's deadband, instead of every interval
        #[arg(long, conflicts_with = "samples")]
        on_change: bool,
        /// Every ADC sequence (~1.6 kHz, not averaged over the meter window), each with its device time
        #[arg(long)]
        samples: bool,
    },
    /// Charge and energy totals since boot or the last reset
    Totals {
        /// Zero the totals first
        #[arg(long)]
        reset: bool,
    },
}

pub fn run(ctx: &Context, command: Power) -> anyhow::Result<ExitCode> {
    match command {
        Power::Status => status(ctx),
        Power::On(args) => on(ctx, args),
        Power::Off => off(ctx),
        Power::Cycle { off, release_sbu } => cycle(ctx, off, release_sbu),
        Power::Meter => meter(ctx),
        Power::Watch {
            interval,
            on_change,
            samples,
        } => watch(ctx, interval, on_change, samples),
        Power::Totals { reset } => totals(ctx, reset),
    }
}

struct Status {
    plug_vbus: VbusState,
    plug_mode: VbusMode,
    data: DataPath,
    header_vbus: VbusState,
    header_mode: VbusMode,
    relay: bool,
    meter: Option<MeterReading>,
}

fn status(ctx: &Context) -> anyhow::Result<ExitCode> {
    let s = super::with_dongle(ctx, async |dongle| {
        let plug = dongle.plug();
        let header = dongle.header();
        Ok(Status {
            plug_vbus: plug.vbus().await?,
            plug_mode: plug.vbus_mode().await?,
            data: plug.data().await?,
            header_vbus: header.vbus_sw().await?,
            header_mode: header.vbus_sw_mode().await?,
            relay: header.relay().await?,
            meter: super::optional(dongle.meter().reading().await)?,
        })
    })?;
    print_status(ctx.format, &s, &mut std::io::stdout().lock())?;
    Ok(ExitCode::Success)
}

fn print_status(format: Format, s: &Status, out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            writeln!(out, "plug    vbus   {}", fmt_vbus_state(s.plug_vbus))?;
            writeln!(out, "        mode   {}", fmt_vbus_mode(&s.plug_mode))?;
            writeln!(out, "        data   {}", fmt_data(s.data))?;
            writeln!(out, "header  vbus   {}", fmt_vbus_state(s.header_vbus))?;
            writeln!(out, "        mode   {}", fmt_vbus_mode(&s.header_mode))?;
            writeln!(
                out,
                "        relay  {}",
                if s.relay { "closed" } else { "open" }
            )?;
            match &s.meter {
                Some(r) => writeln!(out, "meter   {}", reading_text(r))?,
                None => writeln!(out, "meter   (not implemented by the device's firmware)")?,
            }
        }
        Format::Json | Format::Jsonl => {
            let v = serde_json::json!({
                "plug": {
                    "vbus": state_json(s.plug_vbus),
                    "mode": mode_json(&s.plug_mode),
                    "data": fmt_data(s.data),
                },
                "header": {
                    "vbus": state_json(s.header_vbus),
                    "mode": mode_json(&s.header_mode),
                    "relay": s.relay,
                },
                "meter": s.meter.as_ref().map(reading_json),
            });
            writeln!(out, "{v}")?;
        }
    }
    Ok(())
}

/// Prints the resulting state and turns it into an exit code: success only when VBUS actually ended up `On`
/// (`off` instead asks for `Off`), so a trip or a fault that leaves it otherwise is a scriptable failure.
pub(crate) fn report_vbus_state(
    ctx: &Context,
    label: &str,
    state: VbusState,
    want_on: bool,
) -> anyhow::Result<ExitCode> {
    match ctx.format {
        Format::Text => println!("{label} vbus: {}", fmt_vbus_state(state)),
        Format::Json | Format::Jsonl => {
            println!(
                "{}",
                serde_json::json!({ "label": label, "vbus": state_json(state) })
            )
        }
    }
    let ok = if want_on {
        matches!(state, VbusState::On)
    } else {
        matches!(state, VbusState::Off)
    };
    Ok(if ok {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
}

fn on(ctx: &Context, args: VbusOnArgs) -> anyhow::Result<ExitCode> {
    let mode = mode_from_args(&args);
    let state = super::with_dongle(ctx, async |dongle| {
        dongle.plug().set_vbus_mode(mode).await?;
        dongle.plug().vbus().await
    })?;
    report_vbus_state(ctx, "plug", state, true)
}

fn off(ctx: &Context) -> anyhow::Result<ExitCode> {
    let state = super::with_dongle(ctx, async |dongle| {
        dongle.plug().set_vbus_mode(VbusMode::ForceOff).await?;
        dongle.plug().vbus().await
    })?;
    report_vbus_state(ctx, "plug", state, false)
}

fn cycle(ctx: &Context, off: Duration, release_sbu: bool) -> anyhow::Result<ExitCode> {
    // Integer milliseconds: the firmware's `ww_si` has no `f32` (see `donguru_core::vbus`'s doc comment).
    let off_s = Second::milli(off.as_millis() as i32);
    let state = super::with_dongle(ctx, async |dongle| {
        dongle.plug().vbus_cycle(off_s, release_sbu).await
    })?;
    report_vbus_state(ctx, "plug", state, true)
}

fn meter(ctx: &Context) -> anyhow::Result<ExitCode> {
    let reading = super::with_dongle(ctx, async |dongle| dongle.meter().reading().await)?;
    match ctx.format {
        Format::Text => println!("{}", reading_text(&reading)),
        Format::Json | Format::Jsonl => println!("{}", reading_json(&reading)),
    }
    Ok(ExitCode::Success)
}

fn totals_text(t: &MeterTotals) -> String {
    format!(
        "{:.2} mAh   {:.2} mWh   peak {:.1} mA   since {}",
        t.charge.to_f64() / 3.6,
        t.energy.to_f64() / 3.6,
        t.peak_current.to_f64() * 1000.0,
        humantime::format_duration(Duration::from_secs_f64(t.since.to_f64().max(0.0)))
    )
}

fn totals_json(t: &MeterTotals) -> serde_json::Value {
    serde_json::json!({
        "charge_mah": t.charge.to_f64() / 3.6,
        "energy_mwh": t.energy.to_f64() / 3.6,
        "peak_current_ma": t.peak_current.to_f64() * 1000.0,
        "since_s": t.since.to_f64(),
    })
}

fn totals(ctx: &Context, reset: bool) -> anyhow::Result<ExitCode> {
    let totals = super::with_dongle(ctx, async |dongle| {
        let meter = dongle.meter();
        if reset {
            meter.reset_totals().await?;
        }
        meter.totals().await
    })?;
    match ctx.format {
        Format::Text => println!("{}", totals_text(&totals)),
        Format::Json | Format::Jsonl => println!("{}", totals_json(&totals)),
    }
    Ok(ExitCode::Success)
}

fn watch(
    ctx: &Context,
    interval: Duration,
    on_change: bool,
    sampled: bool,
) -> anyhow::Result<ExitCode> {
    if matches!(ctx.format, Format::Json) {
        anyhow::bail!("`power watch` streams data: use `--format text` or `--format jsonl`");
    }
    let text = matches!(ctx.format, Format::Text);
    let selector = super::selector(ctx)?;
    super::runtime()?.block_on(async {
        let dongle = donguru_core::Dongle::open(selector.as_ref()).await?;
        let mode = if sampled {
            samples(interval)
        } else if on_change {
            on_change_mode(interval)
        } else {
            periodic(interval)
        };
        let mut observer = dongle
            .meter()
            .observe(PowerMeterObserve::default().reading(mode))
            .await?;
        let started = std::time::Instant::now();
        loop {
            tokio::select! {
                event = observer.recv_event() => match event {
                    Ok(ObserveEvent::Update(u)) => {
                        if u.missed > 0 {
                            eprintln!("donguru: {} sample(s) missed", u.missed);
                        }
                        match u.value.reading {
                            Some(BatchedOwned::Samples(series)) => {
                                match BatchedOwned::Samples(series)
                                    .into_samples(u.t, u.sample_times.as_deref())
                                {
                                    Ok(rows) => {
                                        for (r, t) in rows {
                                            print_sample_line(text, t.as_ref().map(Second::to_f64), &r);
                                        }
                                    }
                                    Err(e) => eprintln!("donguru: malformed samples: {e}"),
                                }
                            }
                            Some(BatchedOwned::One(r)) => print_watch_line(text, started.elapsed(), &r),
                            None => {}
                        }
                    }
                    Ok(ObserveEvent::Disconnected) => eprintln!("donguru: link down, waiting..."),
                    Ok(ObserveEvent::Resumed) => eprintln!("donguru: link back"),
                    Ok(ObserveEvent::Dropped { events }) => {
                        eprintln!("donguru: {events} update(s) dropped, the queue was full");
                    }
                    Err(e) => {
                        eprintln!("donguru: {e}");
                        break;
                    }
                },
                _ = tokio::signal::ctrl_c() => break,
            }
        }
        dongle.close().await?;
        Ok::<(), donguru_core::Error>(())
    })?;
    Ok(ExitCode::Success)
}

/// One sample of `--samples`: its device time (seconds since the dongle started) and the reading.
fn print_sample_line(text: bool, t_s: Option<f64>, r: &MeterReading) {
    if text {
        match t_s {
            Some(t) => println!("{t:>12.6}s  {}", reading_text(r)),
            None => println!("{:>13}  {}", "?", reading_text(r)),
        }
    } else {
        let mut line = serde_json::json!({ "device_t_s": t_s });
        if let (serde_json::Value::Object(m), serde_json::Value::Object(r)) =
            (&mut line, reading_json(r))
        {
            m.extend(r);
        }
        println!("{line}");
    }
}

fn print_watch_line(text: bool, elapsed: Duration, r: &MeterReading) {
    if text {
        println!("+{:>8.3}s  {}", elapsed.as_secs_f64(), reading_text(r));
    } else {
        println!(
            "{}",
            serde_json::json!({ "t_s": elapsed.as_secs_f64() })
                .as_object()
                .map(|m| {
                    let mut m = m.clone();
                    if let serde_json::Value::Object(r) = reading_json(r) {
                        m.extend(r);
                    }
                    serde_json::Value::Object(m)
                })
                .unwrap()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::api::Volt;
    use donguru_core::mock::MockDevice;

    fn mv(v: i32) -> Volt {
        Volt::milli(v)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn status_text_and_json() {
        let mock = MockDevice::start("cli/power_status");
        let dongle = mock.open().await.unwrap();
        let s = Status {
            plug_vbus: dongle.plug().vbus().await.unwrap(),
            plug_mode: dongle.plug().vbus_mode().await.unwrap(),
            data: dongle.plug().data().await.unwrap(),
            header_vbus: dongle.header().vbus_sw().await.unwrap(),
            header_mode: dongle.header().vbus_sw_mode().await.unwrap(),
            relay: dongle.header().relay().await.unwrap(),
            meter: Some(dongle.meter().reading().await.unwrap()),
        };
        let mut text = Vec::new();
        print_status(Format::Text, &s, &mut text).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("plug    vbus   off"), "{text}");
        assert!(text.contains("meter   5.000 V"), "{text}");

        let mut json = Vec::new();
        print_status(Format::Json, &s, &mut json).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(json["plug"]["vbus"], "off");
        assert_eq!(json["plug"]["data"], "connected");
        dongle.close().await.unwrap();
    }

    #[test]
    fn mode_from_args_defaults_to_5_5v() {
        let args = VbusOnArgs {
            allow_voltage: None,
            trip: None,
        };
        match mode_from_args(&args) {
            VbusMode::ForceOn { max_voltage } => assert_eq!(max_voltage.to_f64(), 5.5),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn mode_from_args_with_trip_is_auto() {
        let args = VbusOnArgs {
            allow_voltage: Some(20.0),
            trip: Some(2.0),
        };
        match mode_from_args(&args) {
            VbusMode::Auto {
                max_voltage,
                max_current,
                ..
            } => {
                assert_eq!(max_voltage.to_f64(), 20.0);
                assert_eq!(max_current.to_f64(), 2.0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn quantity_parses_plain_and_suffixed() {
        assert_eq!(parse_volt("5.5"), Ok(5.5));
        assert_eq!(parse_volt("20V"), Ok(20.0));
        assert_eq!(parse_ampere("2A"), Ok(2.0));
        assert!(parse_volt("-1").is_err());
        assert!(parse_volt("nope").is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn on_reports_a_trip_as_failure() {
        let mock = MockDevice::start("cli/power_trip");
        mock.with(|s| s.reading.vbus_in = mv(9_000)).await;
        let dongle = mock.open().await.unwrap();
        dongle
            .plug()
            .set_vbus_mode(VbusMode::ForceOn {
                max_voltage: mv(5_500),
            })
            .await
            .unwrap();
        let state = dongle.plug().vbus().await.unwrap();
        assert_eq!(state, VbusState::Tripped(GuardTrip::OverVoltage));
        let ctx = Context {
            device: None,
            format: Format::Text,
            color: crate::cli::ColorMode::Never,
            verbosity: 0,
            quiet: true,
        };
        let code = report_vbus_state(&ctx, "plug", state, true).unwrap();
        assert!(matches!(code, ExitCode::Failure));
        dongle.close().await.unwrap();
    }
}
