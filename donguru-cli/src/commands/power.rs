//! `donguru power` - plug VBUS (U601), USB data stays with `donguru usb`, the power meter, and (CLI-11) a combined
//! view of plug + header + meter. See `docs/user/power.md` for the VBUS modes and the guard the firmware keeps.

use std::io::{self, Write};
use std::time::Duration;

use clap::{Args, Subcommand};

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render, Spinner};
use crate::theme::Theme;
use donguru_core::api::{
    DataPath, GuardTrip, MeterReading, MeterTotals, Second, VbusMode, VbusState,
};
use donguru_core::client::observe::{
    BatchedOwned, ObserveEvent, PowerMeterObserve, on_change as on_change_mode, periodic,
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
pub(crate) fn parse_quantity(s: &str, unit: char) -> Result<f64, String> {
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

/// [fmt_vbus_state] in the colour of the state: on is good, a fault or a trip bad.
pub(crate) fn vbus_state_text(theme: &Theme, state: VbusState) -> String {
    let text = fmt_vbus_state(state);
    match state {
        VbusState::On => theme.ok(text).to_string(),
        VbusState::Off => text,
        VbusState::Fault | VbusState::Tripped(_) => theme.bad(text).to_string(),
        VbusState::Unknown(_) => theme.warn(text).to_string(),
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

fn reading_text(theme: &Theme, r: &MeterReading) -> String {
    format!(
        "{} V   {} mA   {} mW",
        theme.value(format!("{:.3}", r.vbus_in.to_f64())),
        theme.value(format!("{:.1}", r.current.to_f64() * 1000.0)),
        theme.value(format!("{:.1}", donguru_core::reading_power_w(r) * 1000.0))
    )
}

/// `power meter`: one reading.
struct Reading(MeterReading);

impl Render for Reading {
    fn text(&self, out: &mut dyn Write, theme: &Theme) -> io::Result<()> {
        writeln!(out, "{}", reading_text(theme, &self.0))
    }

    fn json(&self) -> serde_json::Value {
        reading_json(&self.0)
    }
}

fn reading_json(r: &MeterReading) -> serde_json::Value {
    serde_json::json!({
        "vbus_in_v": r.vbus_in.to_f64(),
        "current_ma": r.current.to_f64() * 1000.0,
        "power_mw": donguru_core::reading_power_w(r) * 1000.0,
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
    Output::stdout(ctx).emit(&s)?;
    Ok(ExitCode::Success)
}

impl Render for Status {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let s = self;
        let row = |out: &mut dyn Write, group: &str, key: &str, value: &dyn std::fmt::Display| {
            writeln!(out, "{:<8}{:<7}{value}", th.heading(group), th.key(key))
        };
        row(out, "plug", "vbus", &vbus_state_text(th, s.plug_vbus))?;
        row(out, "", "mode", &th.value(fmt_vbus_mode(&s.plug_mode)))?;
        let data = match s.data {
            DataPath::Connected => th.ok(fmt_data(s.data)).to_string(),
            DataPath::Cut => fmt_data(s.data).to_string(),
        };
        row(out, "", "data", &data)?;
        row(out, "header", "vbus", &vbus_state_text(th, s.header_vbus))?;
        row(out, "", "mode", &th.value(fmt_vbus_mode(&s.header_mode)))?;
        row(
            out,
            "",
            "relay",
            &th.value(if s.relay { "closed" } else { "open" }),
        )?;
        match &s.meter {
            Some(r) => writeln!(out, "{:<8}{}", th.heading("meter"), reading_text(th, r)),
            None => writeln!(
                out,
                "{:<8}{}",
                th.heading("meter"),
                th.dim("(not implemented by the device's firmware)")
            ),
        }
    }

    fn json(&self) -> serde_json::Value {
        let s = self;
        serde_json::json!({
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
        })
    }
}

/// The state a switch ended in after `on` / `off` / `cycle`: `plug vbus: on`.
pub(crate) struct VbusReport<'a> {
    pub label: &'a str,
    pub state: VbusState,
}

impl Render for VbusReport<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(
            out,
            "{} {}: {}",
            th.heading(self.label),
            th.key("vbus"),
            vbus_state_text(th, self.state)
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "label": self.label, "vbus": state_json(self.state) })
    }
}

/// Prints the resulting state and turns it into an exit code: success only when VBUS actually ended up `On`
/// (`off` instead asks for `Off`), so a trip or a fault that leaves it otherwise is a scriptable failure.
pub(crate) fn report_vbus_state(
    ctx: &Context,
    label: &str,
    state: VbusState,
    want_on: bool,
) -> anyhow::Result<ExitCode> {
    Output::stdout(ctx).emit(&VbusReport { label, state })?;
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
    // a long off time counts down on stderr (CLI-53); a short one would only flicker
    let spinner = (off >= Duration::from_secs(1)).then(|| Spinner::new(ctx, ""));
    let state = super::with_dongle(ctx, async |dongle| {
        let plug = dongle.plug();
        let cycle = plug.vbus_cycle(off_s, release_sbu);
        let Some(spinner) = &spinner else {
            return cycle.await;
        };
        let started = std::time::Instant::now();
        let countdown = async {
            loop {
                let left = off.saturating_sub(started.elapsed());
                spinner.set_message(format!(
                    "plug VBUS off, back on in {:.1} s",
                    left.as_secs_f64()
                ));
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        tokio::select! {
            state = cycle => state,
            never = countdown => never,
        }
    })?;
    drop(spinner);
    report_vbus_state(ctx, "plug", state, true)
}

fn meter(ctx: &Context) -> anyhow::Result<ExitCode> {
    let reading = super::with_dongle(ctx, async |dongle| dongle.meter().reading().await)?;
    Output::stdout(ctx).emit(&Reading(reading))?;
    Ok(ExitCode::Success)
}

/// `power totals`.
struct Totals(MeterTotals);

impl Render for Totals {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let t = &self.0;
        writeln!(
            out,
            "{} mAh   {} mWh   {} {} mA   {} {}",
            th.value(format!("{:.2}", t.charge.to_f64() / 3.6)),
            th.value(format!("{:.2}", t.energy.to_f64() / 3.6)),
            th.key("peak"),
            th.value(format!("{:.1}", t.peak_current.to_f64() * 1000.0)),
            th.key("since"),
            th.value(humantime::format_duration(Duration::from_secs_f64(
                t.since.to_f64().max(0.0)
            )))
        )
    }

    fn json(&self) -> serde_json::Value {
        let t = &self.0;
        serde_json::json!({
            "charge_mah": t.charge.to_f64() / 3.6,
            "energy_mwh": t.energy.to_f64() / 3.6,
            "peak_current_ma": t.peak_current.to_f64() * 1000.0,
            "since_s": t.since.to_f64(),
        })
    }
}

fn totals(ctx: &Context, reset: bool) -> anyhow::Result<ExitCode> {
    let totals = super::with_dongle(ctx, async |dongle| {
        let meter = dongle.meter();
        if reset {
            meter.reset_totals().await?;
        }
        meter.totals().await
    })?;
    Output::stdout(ctx).emit(&Totals(totals))?;
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
    let mut out = Output::stdout(ctx);
    let selector = super::selector(ctx)?;
    super::runtime()?.block_on(async {
        let dongle = donguru_core::Dongle::open(selector.as_ref()).await?;
        let mut observer = if sampled {
            dongle.meter().observe_samples(interval).await?
        } else {
            let mode = if on_change {
                on_change_mode(interval)
            } else {
                periodic(interval)
            };
            dongle
                .meter()
                .observe(PowerMeterObserve::default().reading(mode))
                .await?
        };
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
                                            out.record(&SampleLine { t_s: t.as_ref().map(Second::to_f64), reading: r })?;
                                        }
                                    }
                                    Err(e) => eprintln!("donguru: malformed samples: {e}"),
                                }
                            }
                            Some(BatchedOwned::One(r)) => {
                                out.record(&WatchLine { elapsed: started.elapsed(), reading: r })?;
                            }
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
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(ExitCode::Success)
}

/// One sample of `--samples`: its device time (seconds since the dongle started) and the reading.
struct SampleLine {
    t_s: Option<f64>,
    reading: MeterReading,
}

impl Render for SampleLine {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        match self.t_s {
            Some(t) => writeln!(
                out,
                "{}  {}",
                th.dim(format!("{t:>12.6}s")),
                reading_text(th, &self.reading)
            ),
            None => writeln!(
                out,
                "{}  {}",
                th.dim(format!("{:>13}", "?")),
                reading_text(th, &self.reading)
            ),
        }
    }

    fn json(&self) -> serde_json::Value {
        let mut line = reading_json(&self.reading);
        line["device_t_s"] = serde_json::json!(self.t_s);
        line
    }
}

/// One reading of `power watch`, with the time since the command started.
struct WatchLine {
    elapsed: Duration,
    reading: MeterReading,
}

impl Render for WatchLine {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(
            out,
            "{}  {}",
            th.dim(format!("+{:>8.3}s", self.elapsed.as_secs_f64())),
            reading_text(th, &self.reading)
        )
    }

    fn json(&self) -> serde_json::Value {
        let mut line = reading_json(&self.reading);
        line["t_s"] = serde_json::json!(self.elapsed.as_secs_f64());
        line
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
        insta::assert_snapshot!(crate::output::render(Format::Text, &s));
        insta::assert_snapshot!(crate::output::render(Format::Json, &s));
        let r = Reading(s.meter.clone().unwrap());
        insta::assert_snapshot!(crate::output::render(Format::Text, &r));
        insta::assert_snapshot!(crate::output::render(Format::Json, &r));
        let totals = Totals(dongle.meter().totals().await.unwrap());
        insta::assert_snapshot!(crate::output::render(Format::Text, &totals));
        insta::assert_snapshot!(crate::output::render(Format::Json, &totals));
        dongle.close().await.unwrap();
    }

    #[test]
    fn vbus_report_and_stream_lines() {
        let tripped = VbusReport {
            label: "plug",
            state: VbusState::Tripped(GuardTrip::OverCurrent),
        };
        insta::assert_snapshot!(crate::output::render(Format::Text, &tripped));
        insta::assert_snapshot!(crate::output::render(Format::Json, &tripped));
        let reading = MeterReading {
            vbus_in: mv(5_012),
            current: donguru_core::api::Ampere::milli(250),
        };
        let line = WatchLine {
            elapsed: Duration::from_millis(1_250),
            reading: reading.clone(),
        };
        insta::assert_snapshot!(crate::output::render(Format::Text, &line));
        insta::assert_snapshot!(crate::output::render(Format::Jsonl, &line));
        let sample = SampleLine {
            t_s: Some(12.5),
            reading,
        };
        insta::assert_snapshot!(crate::output::render(Format::Text, &sample));
        insta::assert_snapshot!(crate::output::render(Format::Jsonl, &sample));
    }

    #[test]
    fn colour_only_with_a_theme_that_has_it() {
        let s = VbusReport {
            label: "plug",
            state: VbusState::On,
        };
        let mut out = crate::output::Output::new(Format::Text, Theme::new(true), Vec::new());
        out.emit(&s).unwrap();
        let coloured = String::from_utf8(out.into_inner()).unwrap();
        assert!(coloured.contains('\x1b'), "{coloured:?}");
        assert_eq!(crate::output::render(Format::Text, &s), "plug vbus: on\n");
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
        // a switch that tripped after it was on (the mock refuses to switch on over the limit, CORE-6)
        mock.with(|s| s.plug_vbus = VbusState::Tripped(GuardTrip::OverVoltage))
            .await;
        let dongle = mock.open().await.unwrap();
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
