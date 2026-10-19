//! `donguru settings` - `device.settings` kept in flash (API-19, CLI-47): plug VBUS at boot, the button action,
//! the meter's window and deadband, and the plug UART's default line settings. A write answers `Ok` only once the
//! value is actually stored (`CommonError::Fault` otherwise, see [fault_note]); `settings reset` puts every
//! setting and the label back to its default, factory data stays. The design doc's `reset --factory` isn't in the
//! API (FEATURES.md CLI-47) and so isn't here either.

use std::io::Write;

use clap::{Args, Subcommand, ValueEnum};

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use donguru_core::api::{Ampere, ButtonAction, MeterDeadband, Second, VbusMode, Volt};
use donguru_core::vbus;
use ww_si::{Baud, Prefix};
use ww_uart::{DataBits, Mode as UartMode, Parity, StopBits, UartConfig};

use super::power;

/// A setting of `device.settings`, named as the API property it stores.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum SettingKey {
    /// `plug.vbus_mode` at boot: `off`, `on [--max-voltage V]`, `auto --trip A [--max-voltage V]`
    PlugVbusAtBoot,
    /// Boot value of `inputs.button_action`: `off`, `toggle-vbus`, `cycle-vbus` or `toggle-data`
    ButtonAction,
    /// Boot value of `meter.window`: a duration, 1 ms..=1 s, e.g. `10ms`
    MeterWindow,
    /// Boot value of `meter.deadband`: `VOLT,AMPERE` (e.g. `20mV,10mA`), or `--voltage`/`--current`
    MeterDeadband,
    /// Default config of `uart[0]`: `"BAUD BITSPARITYSTOP"` (quoted), e.g. `"115200 8N1"`
    UartPlug,
}

#[derive(Debug, Args)]
pub struct Set {
    /// Setting to change
    #[arg(value_enum, value_name = "KEY")]
    key: SettingKey,

    /// New value; see the key's help for its shape. Quote it if it has a space (`uart_plug`)
    #[arg(value_name = "VALUE")]
    value: Option<String>,

    /// `plug_vbus_at_boot on|auto`: the voltage limit of the switch at boot (default 5.5 V)
    #[arg(long, value_name = "VOLT", value_parser = power::parse_volt)]
    max_voltage: Option<f64>,

    /// `plug_vbus_at_boot auto`: cut above this current
    #[arg(long, value_name = "AMPERE", value_parser = power::parse_ampere)]
    trip: Option<f64>,

    /// `meter_deadband`: the voltage part, instead of VALUE (needs `--current` too)
    #[arg(long, value_name = "VOLT", value_parser = parse_small_volt)]
    voltage: Option<f64>,

    /// `meter_deadband`: the current part, instead of VALUE (needs `--voltage` too)
    #[arg(long, value_name = "AMPERE", value_parser = parse_small_ampere)]
    current: Option<f64>,
}

#[derive(Debug, Subcommand)]
pub enum Settings {
    /// Show every setting
    Show,
    /// Change one setting; prints the value actually stored
    Set(Set),
    /// Every setting and the label back to defaults; factory data stays
    Reset,
}

pub fn run(ctx: &Context, command: Settings) -> anyhow::Result<ExitCode> {
    match command {
        Settings::Show => show(ctx),
        Settings::Set(args) => set(ctx, args),
        Settings::Reset => reset(ctx),
    }
}

/// Prints the write's device error's [`donguru_core::Error::fault_note`] before letting it propagate, so a flash
/// failure, a newer firmware's record or a too-big record read as a sentence, not just "Hardware fault 0x1".
fn checked<T>(result: donguru_core::Result<T>) -> donguru_core::Result<T> {
    if let Err(e) = &result
        && let Some(note) = e.fault_note()
    {
        eprintln!("donguru: {note}");
    }
    result
}

// ---------------------------------------------------------------------------------------------------------------
// show

struct Values {
    plug_vbus_at_boot: VbusMode,
    button_action: ButtonAction,
    meter_window: Second,
    meter_deadband: MeterDeadband,
    uart_plug: UartConfig,
}

fn show(ctx: &Context) -> anyhow::Result<ExitCode> {
    let v = super::with_dongle(ctx, async |dongle| {
        let s = dongle.device().settings();
        Ok(Values {
            plug_vbus_at_boot: s.plug_vbus_at_boot().await?,
            button_action: s.button_action().await?,
            meter_window: s.meter_window().await?,
            meter_deadband: s.meter_deadband().await?,
            uart_plug: s.uart_plug().await?,
        })
    })?;
    print_show(ctx.format, &v, &mut std::io::stdout().lock())?;
    Ok(ExitCode::Success)
}

fn print_show(format: Format, v: &Values, out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            writeln!(
                out,
                "plug_vbus_at_boot   {}",
                power::fmt_vbus_mode(&v.plug_vbus_at_boot)
            )?;
            writeln!(
                out,
                "button_action       {}",
                fmt_button_action(v.button_action)
            )?;
            writeln!(out, "meter_window        {}", fmt_window(v.meter_window))?;
            writeln!(
                out,
                "meter_deadband      {}",
                fmt_deadband(&v.meter_deadband)
            )?;
            writeln!(out, "uart_plug           {}", fmt_uart_plug(&v.uart_plug))?;
        }
        Format::Json | Format::Jsonl => {
            let j = serde_json::json!({
                "plug_vbus_at_boot": power::mode_json(&v.plug_vbus_at_boot),
                "button_action": button_action_json(v.button_action),
                "meter_window": window_json(v.meter_window),
                "meter_deadband": deadband_json(&v.meter_deadband),
                "uart_plug": uart_plug_json(&v.uart_plug),
            });
            writeln!(out, "{j}")?;
        }
    }
    Ok(())
}

fn print_one(ctx: &Context, key: &str, text: &str, json: serde_json::Value) {
    match ctx.format {
        Format::Text => println!("{key}  {text}"),
        Format::Json | Format::Jsonl => println!("{}", serde_json::json!({ key: json })),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// set

fn set(ctx: &Context, args: Set) -> anyhow::Result<ExitCode> {
    match args.key {
        SettingKey::PlugVbusAtBoot => set_plug_vbus_at_boot(ctx, &args),
        SettingKey::ButtonAction => set_button_action(ctx, &args),
        SettingKey::MeterWindow => set_meter_window(ctx, &args),
        SettingKey::MeterDeadband => set_meter_deadband(ctx, &args),
        SettingKey::UartPlug => set_uart_plug(ctx, &args),
    }
}

fn set_plug_vbus_at_boot(ctx: &Context, args: &Set) -> anyhow::Result<ExitCode> {
    let value = args
        .value
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("plug_vbus_at_boot needs a value: off, on or auto"))?;
    let mode = match value {
        "off" => {
            if args.max_voltage.is_some() || args.trip.is_some() {
                anyhow::bail!("plug_vbus_at_boot off takes no --max-voltage or --trip");
            }
            VbusMode::ForceOff
        }
        "on" => {
            if args.trip.is_some() {
                anyhow::bail!(
                    "plug_vbus_at_boot on takes no --trip (use `auto --trip` for a current limit)"
                );
            }
            vbus::force_on(args.max_voltage.unwrap_or(vbus::DEFAULT_MAX_VOLTAGE_V))
        }
        "auto" => {
            let trip = args
                .trip
                .ok_or_else(|| anyhow::anyhow!("plug_vbus_at_boot auto needs --trip AMPERE"))?;
            vbus::auto(
                args.max_voltage.unwrap_or(vbus::DEFAULT_MAX_VOLTAGE_V),
                trip,
            )
        }
        other => anyhow::bail!("{other}: expected off, on or auto"),
    };
    let got = super::with_dongle(ctx, async |dongle| {
        let s = dongle.device().settings();
        checked(s.set_plug_vbus_at_boot(mode).await)?;
        s.plug_vbus_at_boot().await
    })?;
    print_one(
        ctx,
        "plug_vbus_at_boot",
        &power::fmt_vbus_mode(&got),
        power::mode_json(&got),
    );
    Ok(ExitCode::Success)
}

fn parse_button_action(s: &str) -> Result<ButtonAction, String> {
    match s {
        "off" => Ok(ButtonAction::Off),
        "toggle-vbus" => Ok(ButtonAction::ToggleVbus),
        "cycle-vbus" => Ok(ButtonAction::CycleVbus),
        "toggle-data" => Ok(ButtonAction::ToggleData),
        other => Err(format!(
            "{other}: expected off, toggle-vbus, cycle-vbus or toggle-data"
        )),
    }
}

fn fmt_button_action(a: ButtonAction) -> &'static str {
    match a {
        ButtonAction::Off => "off",
        ButtonAction::ToggleVbus => "toggle-vbus",
        ButtonAction::CycleVbus => "cycle-vbus",
        ButtonAction::ToggleData => "toggle-data",
        ButtonAction::Unknown(_) => "unknown (newer firmware)",
    }
}

fn button_action_json(a: ButtonAction) -> &'static str {
    match a {
        ButtonAction::Off => "off",
        ButtonAction::ToggleVbus => "toggle_vbus",
        ButtonAction::CycleVbus => "cycle_vbus",
        ButtonAction::ToggleData => "toggle_data",
        ButtonAction::Unknown(_) => "unknown",
    }
}

fn set_button_action(ctx: &Context, args: &Set) -> anyhow::Result<ExitCode> {
    let value = args.value.as_deref().ok_or_else(|| {
        anyhow::anyhow!("button_action needs a value: off, toggle-vbus, cycle-vbus or toggle-data")
    })?;
    let action = parse_button_action(value).map_err(anyhow::Error::msg)?;
    let got = super::with_dongle(ctx, async |dongle| {
        let s = dongle.device().settings();
        checked(s.set_button_action(action).await)?;
        s.button_action().await
    })?;
    print_one(
        ctx,
        "button_action",
        fmt_button_action(got),
        serde_json::json!(button_action_json(got)),
    );
    Ok(ExitCode::Success)
}

fn fmt_window(window: Second) -> String {
    humantime::format_duration(std::time::Duration::from_secs_f64(window.to_f64().max(0.0)))
        .to_string()
}

fn window_json(window: Second) -> serde_json::Value {
    serde_json::json!(window.to_f64())
}

fn set_meter_window(ctx: &Context, args: &Set) -> anyhow::Result<ExitCode> {
    let value = args
        .value
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("meter_window needs a duration, e.g. 10ms (1 ms..=1 s)"))?;
    let duration = humantime::parse_duration(value).map_err(|e| anyhow::anyhow!("{value}: {e}"))?;
    if duration.is_zero() {
        anyhow::bail!("{value}: must be greater than zero");
    }
    let window = Second::milli(duration.as_millis() as i32);
    let got = super::with_dongle(ctx, async |dongle| {
        let s = dongle.device().settings();
        checked(s.set_meter_window(window).await)?;
        s.meter_window().await
    })?;
    print_one(ctx, "meter_window", &fmt_window(got), window_json(got));
    Ok(ExitCode::Success)
}

/// A volt or an ampere value with an optional milli prefix (`20mV`, `0.02V`, a bare number as the base unit).
fn parse_small_quantity(s: &str, unit: char) -> Result<f64, String> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    let milli_suffix = format!("m{}", unit.to_ascii_lowercase());
    let (digits, scale) = if let Some(n) = lower.strip_suffix(&milli_suffix) {
        (n, 1e-3)
    } else if let Some(n) = lower.strip_suffix(unit.to_ascii_lowercase()) {
        (n, 1.0)
    } else {
        (lower.as_str(), 1.0)
    };
    let value: f64 = digits
        .trim()
        .parse()
        .map_err(|_| format!("{s}: not a number"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!("{s}: must be a non-negative number"));
    }
    Ok(value * scale)
}

pub(crate) fn parse_small_volt(s: &str) -> Result<f64, String> {
    parse_small_quantity(s, 'V')
}

pub(crate) fn parse_small_ampere(s: &str) -> Result<f64, String> {
    parse_small_quantity(s, 'A')
}

fn fmt_deadband(d: &MeterDeadband) -> String {
    format!(
        "{:.0} mV, {:.0} mA",
        d.voltage.to_f64() * 1000.0,
        d.current.to_f64() * 1000.0
    )
}

fn deadband_json(d: &MeterDeadband) -> serde_json::Value {
    serde_json::json!({
        "voltage_mv": d.voltage.to_f64() * 1000.0,
        "current_ma": d.current.to_f64() * 1000.0,
    })
}

fn set_meter_deadband(ctx: &Context, args: &Set) -> anyhow::Result<ExitCode> {
    let (voltage, current) = match (&args.value, args.voltage, args.current) {
        (Some(value), None, None) => {
            let (v, a) = value
                .split_once(',')
                .ok_or_else(|| anyhow::anyhow!("{value}: expected VOLT,AMPERE, e.g. 20mV,10mA"))?;
            (
                parse_small_volt(v).map_err(anyhow::Error::msg)?,
                parse_small_ampere(a).map_err(anyhow::Error::msg)?,
            )
        }
        (None, Some(v), Some(a)) => (v, a),
        (None, Some(_), None) | (None, None, Some(_)) => {
            anyhow::bail!("meter_deadband needs both --voltage and --current")
        }
        (None, None, None) => {
            anyhow::bail!("meter_deadband needs VALUE (VOLT,AMPERE) or --voltage and --current")
        }
        (Some(_), _, _) => {
            anyhow::bail!("meter_deadband: give VALUE or --voltage/--current, not both")
        }
    };
    let deadband = MeterDeadband {
        voltage: Volt::milli((voltage * 1000.0).round() as i32),
        current: Ampere::milli((current * 1000.0).round() as i32),
    };
    let got = super::with_dongle(ctx, async |dongle| {
        let s = dongle.device().settings();
        checked(s.set_meter_deadband(deadband).await)?;
        s.meter_deadband().await
    })?;
    print_one(
        ctx,
        "meter_deadband",
        &fmt_deadband(&got),
        deadband_json(&got),
    );
    Ok(ExitCode::Success)
}

fn data_bits_digit(d: DataBits) -> char {
    match d {
        DataBits::Five => '5',
        DataBits::Six => '6',
        DataBits::Seven => '7',
        DataBits::Eight => '8',
        DataBits::Nine => '9',
        DataBits::Unknown(_) => '?',
    }
}

fn parity_letter(p: Parity) -> char {
    match p {
        Parity::None => 'N',
        Parity::Odd => 'O',
        Parity::Even => 'E',
        Parity::Unknown(_) => '?',
    }
}

fn stop_bits_digit(s: StopBits) -> char {
    match s {
        StopBits::Stop1 => '1',
        StopBits::Stop2 => '2',
    }
}

fn fmt_mode(m: UartMode) -> &'static str {
    match m {
        UartMode::Asynchronous => "async",
        UartMode::HardwareFlowControl => "rts/cts",
        UartMode::SoftwareFlowControl => "xon/xoff",
        UartMode::HighZ => "hi-z",
        UartMode::Test => "test",
        UartMode::Unknown(_) => "unknown",
    }
}

fn fmt_framing(cfg: &UartConfig) -> String {
    format!(
        "{} {}{}{}",
        cfg.baud_rate.to_f64() as u64,
        data_bits_digit(cfg.data_bits),
        parity_letter(cfg.parity),
        stop_bits_digit(cfg.stop_bits),
    )
}

fn fmt_uart_plug(cfg: &UartConfig) -> String {
    let mut s = fmt_framing(cfg);
    s.push_str(&format!(
        "  ({}, {})",
        fmt_mode(cfg.mode),
        if cfg.prevent_back_feed {
            "back-feed guard on"
        } else {
            "back-feed guard off"
        }
    ));
    s
}

fn uart_plug_json(cfg: &UartConfig) -> serde_json::Value {
    serde_json::json!({
        "baud": cfg.baud_rate.to_f64() as u64,
        "data_bits": data_bits_digit(cfg.data_bits).to_string(),
        "parity": parity_letter(cfg.parity).to_string(),
        "stop_bits": stop_bits_digit(cfg.stop_bits).to_string(),
        "mode": fmt_mode(cfg.mode),
        "prevent_back_feed": cfg.prevent_back_feed,
        "io_level_v": cfg.io_level.map(|v| v.to_f64()),
        "idle_chars": cfg.idle_chars,
    })
}

/// `"115200 8N1"` -> baud, data bits, parity, stop bits.
fn parse_uart_value(value: &str) -> Result<(u32, DataBits, Parity, StopBits), String> {
    let mut parts = value.split_whitespace();
    let usage = || format!("{value}: expected \"BAUD BITSPARITYSTOP\", e.g. \"115200 8N1\"");
    let baud_s = parts.next().ok_or_else(usage)?;
    let framing = parts.next().ok_or_else(usage)?;
    if parts.next().is_some() {
        return Err(usage());
    }
    let baud: u32 = baud_s
        .parse()
        .map_err(|_| format!("{baud_s}: not a baud rate"))?;
    let mut chars = framing.chars();
    let data_bits = match chars.next() {
        Some('5') => DataBits::Five,
        Some('6') => DataBits::Six,
        Some('7') => DataBits::Seven,
        Some('8') => DataBits::Eight,
        Some('9') => DataBits::Nine,
        _ => return Err(format!("{framing}: data bits must be 5-9")),
    };
    let parity = match chars.next() {
        Some('N') | Some('n') => Parity::None,
        Some('O') | Some('o') => Parity::Odd,
        Some('E') | Some('e') => Parity::Even,
        _ => return Err(format!("{framing}: parity must be N, O or E")),
    };
    let stop_bits = match chars.next() {
        Some('1') => StopBits::Stop1,
        Some('2') => StopBits::Stop2,
        _ => return Err(format!("{framing}: stop bits must be 1 or 2")),
    };
    if chars.next().is_some() {
        return Err(format!("{framing}: expected BITSPARITYSTOP, e.g. 8N1"));
    }
    Ok((baud, data_bits, parity, stop_bits))
}

fn set_uart_plug(ctx: &Context, args: &Set) -> anyhow::Result<ExitCode> {
    let value = args.value.as_deref().ok_or_else(|| {
        anyhow::anyhow!("uart_plug needs a value: \"BAUD BITSPARITYSTOP\", e.g. \"115200 8N1\"")
    })?;
    let (baud, data_bits, parity, stop_bits) =
        parse_uart_value(value).map_err(anyhow::Error::msg)?;
    let got = super::with_dongle(ctx, async |dongle| {
        let s = dongle.device().settings();
        let mut cfg = s.uart_plug().await?;
        cfg.baud_rate = Baud::new(Prefix::Unit, baud);
        cfg.data_bits = data_bits;
        cfg.parity = parity;
        cfg.stop_bits = stop_bits;
        checked(s.set_uart_plug(cfg).await)?;
        s.uart_plug().await
    })?;
    print_one(ctx, "uart_plug", &fmt_uart_plug(&got), uart_plug_json(&got));
    Ok(ExitCode::Success)
}

// ---------------------------------------------------------------------------------------------------------------
// reset

fn reset(ctx: &Context) -> anyhow::Result<ExitCode> {
    if !ctx.quiet {
        eprintln!("donguru: resetting every setting and the label to defaults; factory data stays");
    }
    super::with_dongle(ctx, async |dongle| {
        checked(dongle.device().settings().reset().await)
    })?;
    match ctx.format {
        Format::Text => println!("settings reset"),
        Format::Json | Format::Jsonl => println!("{}", serde_json::json!({ "reset": true })),
    }
    Ok(ExitCode::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::mock::MockDevice;

    #[test]
    fn small_quantity_parses_plain_milli_and_base() {
        assert_eq!(parse_small_volt("20mV"), Ok(0.02));
        assert_eq!(parse_small_volt("0.02V"), Ok(0.02));
        assert_eq!(parse_small_volt("0.02"), Ok(0.02));
        assert_eq!(parse_small_ampere("10mA"), Ok(0.01));
        assert!(parse_small_volt("nope").is_err());
        assert!(parse_small_volt("-1mV").is_err());
    }

    #[test]
    fn button_action_round_trips() {
        for (s, a) in [
            ("off", ButtonAction::Off),
            ("toggle-vbus", ButtonAction::ToggleVbus),
            ("cycle-vbus", ButtonAction::CycleVbus),
            ("toggle-data", ButtonAction::ToggleData),
        ] {
            assert_eq!(parse_button_action(s), Ok(a));
            assert_eq!(fmt_button_action(a), s);
        }
        assert!(parse_button_action("nope").is_err());
    }

    #[test]
    fn uart_value_parses_baud_and_framing() {
        let (baud, data_bits, parity, stop_bits) = parse_uart_value("115200 8N1").unwrap();
        assert_eq!(baud, 115_200);
        assert!(matches!(data_bits, DataBits::Eight));
        assert!(matches!(parity, Parity::None));
        assert!(matches!(stop_bits, StopBits::Stop1));

        let (baud, data_bits, parity, stop_bits) = parse_uart_value("9600 7E2").unwrap();
        assert_eq!(baud, 9_600);
        assert!(matches!(data_bits, DataBits::Seven));
        assert!(matches!(parity, Parity::Even));
        assert!(matches!(stop_bits, StopBits::Stop2));

        assert!(parse_uart_value("115200").is_err(), "missing framing");
        assert!(parse_uart_value("115200 8N1 extra").is_err(), "extra token");
        assert!(parse_uart_value("115200 4N1").is_err(), "bad data bits");
    }

    #[test]
    fn show_text_and_json() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let mock = MockDevice::start("cli/settings_show");
        let v = rt.block_on(async {
            let dongle = mock.open().await.unwrap();
            let s = dongle.device().settings();
            let v = Values {
                plug_vbus_at_boot: s.plug_vbus_at_boot().await.unwrap(),
                button_action: s.button_action().await.unwrap(),
                meter_window: s.meter_window().await.unwrap(),
                meter_deadband: s.meter_deadband().await.unwrap(),
                uart_plug: s.uart_plug().await.unwrap(),
            };
            dongle.close().await.unwrap();
            v
        });
        let mut text = Vec::new();
        print_show(Format::Text, &v, &mut text).unwrap();
        insta::assert_snapshot!(String::from_utf8(text).unwrap());

        let mut json = Vec::new();
        print_show(Format::Json, &v, &mut json).unwrap();
        insta::assert_snapshot!(String::from_utf8(json).unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn set_and_reset_through_the_mock() {
        let mock = MockDevice::start("cli/settings_set");
        let dongle = mock.open().await.unwrap();
        let s = dongle.device().settings();

        s.set_plug_vbus_at_boot(vbus::force_on(5.5)).await.unwrap();
        assert_eq!(
            s.plug_vbus_at_boot().await.unwrap(),
            VbusMode::ForceOn {
                max_voltage: Volt::milli(5_500)
            }
        );

        s.set_button_action(ButtonAction::CycleVbus).await.unwrap();
        assert_eq!(s.button_action().await.unwrap(), ButtonAction::CycleVbus);

        let window = Second::milli(humantime::parse_duration("20ms").unwrap().as_millis() as i32);
        s.set_meter_window(window).await.unwrap();
        assert_eq!(s.meter_window().await.unwrap(), window);

        let (baud, data_bits, parity, stop_bits) = parse_uart_value("9600 7E2").unwrap();
        let mut cfg = s.uart_plug().await.unwrap();
        cfg.baud_rate = Baud::new(Prefix::Unit, baud);
        cfg.data_bits = data_bits;
        cfg.parity = parity;
        cfg.stop_bits = stop_bits;
        s.set_uart_plug(cfg).await.unwrap();
        let got = s.uart_plug().await.unwrap();
        assert_eq!(got.baud_rate, Baud::new(Prefix::Unit, 9_600u32));
        assert!(matches!(got.data_bits, DataBits::Seven));

        s.reset().await.unwrap();
        assert_eq!(s.plug_vbus_at_boot().await.unwrap(), VbusMode::ForceOff);
        assert_eq!(s.button_action().await.unwrap(), ButtonAction::Off);

        dongle.close().await.unwrap();
    }
}
