//! `donguru debug` - bring-up and production test on the `debug` level (CLI-23, API-18): MCU pins by net name,
//! the boot pin capture, the hub's SMBus registers, the UCPD ports, peripheral registers. See `docs/user/debug.md`.
//!
//! The firmware decides what may be changed (refused pins, pins a running function holds, pin states that fight
//! each other on the board): a refusal comes back as the device's error with its reason, exit code 4.

use std::io::Write;

use clap::{Args, Subcommand, ValueEnum};
use clap_complete::engine::ArgValueCandidates;
use donguru_core::api::{CcRole, Level, McuPin, PinMode, PinState, PortRegs, UcpdPort};
use donguru_core::debug::{self, Pin};
use donguru_core::nets;

use crate::cli::Format;
use crate::completion;
use crate::context::Context;
use crate::exit::ExitCode;

/// An MCU pin by net name (`RELAY_EN`, `~PWR_FLT`) or port pin (`PA5`).
fn parse_pin(s: &str) -> Result<McuPin, String> {
    nets::pin(s).ok_or_else(|| {
        format!("{s:?} is neither a net of the B153A (`dg debug pins` lists them) nor a port pin like PA5")
    })
}

/// A number in decimal or `0x` hex.
fn parse_u32(s: &str) -> Result<u32, String> {
    let s = s.trim();
    let (digits, radix) = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => (hex, 16),
        None => (s, 10),
    };
    u32::from_str_radix(&digits.replace('_', ""), radix).map_err(|e| format!("{s:?}: {e}"))
}

fn parse_u8(s: &str) -> Result<u8, String> {
    let v = parse_u32(s)?;
    u8::try_from(v).map_err(|_| format!("{s:?}: 0..=255"))
}

/// What `pin set` asks for: a mode, or a level alone (a push-pull output at that level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Setting {
    #[value(aliases = ["1", "hi"], help = "Push-pull output, high [aliases: 1, hi]")]
    High,
    #[value(aliases = ["0", "lo"], help = "Push-pull output, low [aliases: 0, lo]")]
    Low,
    #[value(help = "Analog: input buffer off, the reset state")]
    Analog,
    #[value(aliases = ["in"], help = "Input, floating [aliases: in]")]
    Input,
    #[value(aliases = ["pu"], help = "Input with the internal pull-up [aliases: pu]")]
    InputPullUp,
    #[value(aliases = ["pd"], help = "Input with the internal pull-down [aliases: pd]")]
    InputPullDown,
    #[value(aliases = ["out", "pp"], help = "Push-pull output [aliases: out, pp]")]
    PushPull,
    #[value(aliases = ["od"], help = "Open-drain output [aliases: od]")]
    OpenDrain,
    #[value(aliases = ["odpu"], help = "Open-drain output with the internal pull-up [aliases: odpu]")]
    OpenDrainPullUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LevelArg {
    #[value(aliases = ["1", "hi"], help = "High [aliases: 1, hi]")]
    High,
    #[value(aliases = ["0", "lo"], help = "Low [aliases: 0, lo]")]
    Low,
}

impl From<LevelArg> for Level {
    fn from(l: LevelArg) -> Level {
        match l {
            LevelArg::High => Level::High,
            LevelArg::Low => Level::Low,
        }
    }
}

/// The mode and level `pin set SETTING [LEVEL]` asks of the device.
fn mode_level(
    setting: Setting,
    level: Option<LevelArg>,
) -> Result<(PinMode, Option<Level>), String> {
    let mode = match setting {
        Setting::High | Setting::Low if level.is_some() => {
            return Err("a level is given once: `high`, or `push-pull high`".into());
        }
        Setting::High => return Ok((PinMode::PushPull, Some(Level::High))),
        Setting::Low => return Ok((PinMode::PushPull, Some(Level::Low))),
        Setting::Analog => PinMode::Analog,
        Setting::Input => PinMode::Input,
        Setting::InputPullUp => PinMode::InputPullUp,
        Setting::InputPullDown => PinMode::InputPullDown,
        Setting::PushPull => PinMode::PushPull,
        Setting::OpenDrain => PinMode::OpenDrain,
        Setting::OpenDrainPullUp => PinMode::OpenDrainPullUp,
    };
    if level.is_some()
        && !matches!(
            mode,
            PinMode::PushPull | PinMode::OpenDrain | PinMode::OpenDrainPullUp
        )
    {
        return Err("a level only goes with an output mode".into());
    }
    Ok((mode, level.map(Level::from)))
}

/// One MCU pin.
#[derive(Debug, Clone, Args)]
pub struct PinArg {
    /// Net name (`RELAY_EN`, `~PWR_FLT`) or port pin (`PA5`)
    #[arg(value_name = "PIN", value_parser = parse_pin, add = ArgValueCandidates::new(completion::nets))]
    pub pin: McuPin,
}

#[derive(Debug, Subcommand)]
pub enum DebugPin {
    /// Registers of one pin: mode, pull, AF, the level it drives and the level it reads
    Get(PinArg),
    /// Set a pin up: `high` / `low` (push-pull output), or a mode with an optional level
    ///
    /// Refused always: NRST, supply pins, our own USB and SWD, CC_DA_EN, the VBUS enables (use `dg power` /
    /// `dg header`), the hub's reset and I2C (use `dg debug hub`). Refused while a function runs on them: the
    /// LEDs, the meter and button inputs, the header UART, V_SBU select, an SWD session, a UCPD role. Refused when
    /// the change would make two pins fight on the board (tied nets, SBU_OE with PA0 / PA1).
    Set {
        #[command(flatten)]
        pin: PinArg,
        /// `high`, `low`, or a mode
        #[arg(value_enum, value_name = "SETTING")]
        setting: Setting,
        /// Level of an output mode (kept as it is when left out)
        #[arg(value_enum, value_name = "LEVEL")]
        level: Option<LevelArg>,
    },
    /// Put a pin on an alternate function 0..=7 (type and pull stay)
    Af {
        #[command(flatten)]
        pin: PinArg,
        /// Alternate function, 0..=7 (STM32G0B1 datasheet, tables 15-20)
        #[arg(value_name = "AF", value_parser = clap::value_parser!(u8).range(0..=7))]
        af: u8,
    },
}

#[derive(Debug, Subcommand)]
pub enum DebugHub {
    /// Read registers of the hub (U301, USB2512B / USB2514B) over SMBus
    Read {
        /// First register, decimal or 0x hex
        #[arg(value_name = "REG", value_parser = parse_u8)]
        reg: u8,
        /// Bytes to read, 1..=32
        #[arg(value_name = "LEN", default_value = "1", value_parser = clap::value_parser!(u8).range(1..=32))]
        len: u8,
    },
    /// Write registers of the hub over SMBus (STATUS/COMMAND 0xFF is refused: use `dg hub recover`)
    Write {
        /// First register, decimal or 0x hex
        #[arg(value_name = "REG", value_parser = parse_u8)]
        reg: u8,
        /// Bytes, decimal or 0x hex, 1..=32 of them
        #[arg(value_name = "BYTE", required = true, num_args = 1..=32, value_parser = parse_u8)]
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PortArg {
    #[value(name = "1", help = "UCPD1: the receptacle, to the computer")]
    One,
    #[value(name = "2", help = "UCPD2: the plug, to the device under test")]
    Two,
}

impl From<PortArg> for UcpdPort {
    fn from(p: PortArg) -> UcpdPort {
        match p {
            PortArg::One => UcpdPort::Ucpd1,
            PortArg::Two => UcpdPort::Ucpd2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RoleArg {
    #[value(help = "CC PHY off, the CC pins released")]
    Off,
    #[value(help = "Sink: Rd on both CC lines")]
    Rd,
    #[value(help = "Source: Rp for default USB current (not on UCPD1 of rev A)")]
    Rp0a9,
    #[value(help = "Source: Rp for 1.5 A (not on UCPD1 of rev A)")]
    Rp1a5,
    #[value(help = "Source: Rp for 3.0 A (not on UCPD1 of rev A)")]
    Rp3a0,
}

impl From<RoleArg> for CcRole {
    fn from(r: RoleArg) -> CcRole {
        match r {
            RoleArg::Off => CcRole::Off,
            RoleArg::Rd => CcRole::Rd,
            RoleArg::Rp0a9 => CcRole::Rp0A9,
            RoleArg::Rp1a5 => CcRole::Rp1A5,
            RoleArg::Rp3a0 => CcRole::Rp3A0,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Debug {
    /// One MCU pin by net name or port pin
    #[command(subcommand)]
    Pin(DebugPin),
    /// Every bonded MCU pin with its net, mode and level
    Pins {
        /// The pins as the firmware captured them at boot, before setting anything up
        #[arg(long)]
        boot: bool,
        /// Only the pins not in their reset state (the level a pin reads is not compared)
        #[arg(long)]
        diff: bool,
    },
    /// The hub's SMBus registers
    #[command(subcommand)]
    Hub(DebugHub),
    /// UCPD ports: role and CC line states; `--role` sets one
    Ucpd {
        /// Port, both when left out
        #[arg(value_enum, value_name = "PORT")]
        port: Option<PortArg>,
        /// Set this CC role first (needs PORT)
        #[arg(long, value_enum, value_name = "ROLE", requires = "port")]
        role: Option<RoleArg>,
    },
    /// Read 32-bit words of peripheral space (never memory)
    Reg {
        /// Address, word aligned, decimal or 0x hex
        #[arg(value_name = "ADDR", value_parser = parse_u32)]
        addr: u32,
        /// Words to read, 1..=32
        #[arg(value_name = "N", default_value = "1", value_parser = clap::value_parser!(u8).range(1..=32))]
        words: u8,
    },
}

pub fn run(ctx: &Context, cmd: Debug) -> anyhow::Result<ExitCode> {
    let out = &mut std::io::stdout().lock();
    match cmd {
        Debug::Pin(DebugPin::Get(PinArg { pin })) => {
            let state = super::with_dongle(ctx, async |d| d.debug().pin_get(pin).await)?;
            print_pin(ctx.format, pin, &state, out)?;
        }
        Debug::Pin(DebugPin::Set {
            pin: PinArg { pin },
            setting,
            level,
        }) => {
            let (mode, level) = match mode_level(setting, level) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("donguru: {e}");
                    return Ok(ExitCode::BadArgs);
                }
            };
            let state =
                super::with_dongle(ctx, async |d| d.debug().pin_set(pin, mode, level).await)?;
            print_pin(ctx.format, pin, &state, out)?;
        }
        Debug::Pin(DebugPin::Af {
            pin: PinArg { pin },
            af,
        }) => {
            let state = super::with_dongle(ctx, async |d| d.debug().pin_af(pin, af).await)?;
            print_pin(ctx.format, pin, &state, out)?;
        }
        Debug::Pins { boot, diff } => {
            let dump = super::with_dongle(ctx, async |d| {
                if boot {
                    d.debug().boot_pins().await
                } else {
                    d.debug().pins().await
                }
            })?;
            if boot && dump.ports.is_empty() {
                anyhow::bail!("the firmware has no boot capture");
            }
            print_pins(ctx.format, &dump.ports, diff, out)?;
        }
        Debug::Hub(DebugHub::Read { reg, len }) => {
            let data = super::with_dongle(ctx, async |d| d.debug().hub_read(reg, len).await)?;
            print_hub(ctx.format, reg, &data, out)?;
        }
        Debug::Hub(DebugHub::Write { reg, data }) => {
            super::with_dongle(ctx, async |d| d.debug().hub_write(reg, &data).await)?;
            if !ctx.quiet {
                eprintln!(
                    "donguru: wrote {} byte(s) at hub register 0x{reg:02X}",
                    data.len()
                );
            }
        }
        Debug::Ucpd { port, role } => {
            let ports: Vec<UcpdPort> = match port {
                Some(p) => vec![p.into()],
                None => vec![UcpdPort::Ucpd1, UcpdPort::Ucpd2],
            };
            let states = super::with_dongle(ctx, async |d| {
                let mut states = Vec::new();
                for port in &ports {
                    let state = match role {
                        Some(role) => d.debug().ucpd_role(*port, role.into()).await?,
                        None => d.debug().ucpd(*port).await?,
                    };
                    states.push((*port, debug::ucpd(&state)));
                }
                Ok(states)
            })?;
            print_ucpd(ctx.format, &states, out)?;
        }
        Debug::Reg { addr, words } => {
            let data = super::with_dongle(ctx, async |d| d.debug().reg_read(addr, words).await)?;
            print_reg(ctx.format, addr, &data, out)?;
        }
    }
    Ok(ExitCode::Success)
}

fn net_name(pin: McuPin) -> &'static str {
    nets::net_of(pin).map(|n| n.name).unwrap_or("-")
}

fn level(high: bool) -> &'static str {
    if high { "high" } else { "low" }
}

fn pin_json(pin: McuPin, s: &PinState) -> serde_json::Value {
    serde_json::json!({
        "pin": nets::port_pin_name(pin),
        "net": nets::net_of(pin).map(|n| n.name),
        "mode": debug::mode_text(s),
        "moder": s.moder,
        "open_drain": s.open_drain,
        "speed": s.speed,
        "pull": s.pull,
        "af": s.af,
        "output": s.output,
        "input": s.input,
    })
}

fn print_pin(
    format: Format,
    pin: McuPin,
    s: &PinState,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match format {
        Format::Text => writeln!(
            out,
            "{} {}: {}, reads {}",
            nets::port_pin_name(pin),
            net_name(pin),
            debug::mode_text(s),
            level(s.input)
        )?,
        Format::Json | Format::Jsonl => writeln!(out, "{}", pin_json(pin, s))?,
    }
    Ok(())
}

fn print_pins(
    format: Format,
    ports: &[PortRegs],
    diff: bool,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let rows: Vec<(Pin, Option<PinState>)> = if diff {
        debug::diff_reset(ports)
            .into_iter()
            .map(|d| (d.pin, Some(d.reset)))
            .collect()
    } else {
        debug::pins(ports).into_iter().map(|p| (p, None)).collect()
    };
    match format {
        Format::Text => {
            if diff && rows.is_empty() {
                writeln!(out, "every pin in its reset state")?;
            }
            for (p, reset) in &rows {
                let net = p.net.map(|n| n.name).unwrap_or("-");
                let mode = debug::mode_text(&p.state);
                match reset {
                    Some(reset) => writeln!(
                        out,
                        "{:<5} {net:<14} {mode:<32} (reset: {})",
                        p.name(),
                        debug::mode_text(reset)
                    )?,
                    None => writeln!(
                        out,
                        "{:<5} {net:<14} {mode:<32} reads {}",
                        p.name(),
                        level(p.state.input)
                    )?,
                }
            }
        }
        Format::Json => {
            let rows: Vec<_> = rows
                .iter()
                .map(|(p, reset)| row_json(p, reset.as_ref()))
                .collect();
            writeln!(out, "{}", serde_json::Value::Array(rows))?;
        }
        Format::Jsonl => {
            for (p, reset) in &rows {
                writeln!(out, "{}", row_json(p, reset.as_ref()))?;
            }
        }
    }
    Ok(())
}

fn row_json(p: &Pin, reset: Option<&PinState>) -> serde_json::Value {
    let mut j = pin_json(p.pin, &p.state);
    if let Some(reset) = reset {
        j["reset"] = serde_json::Value::String(debug::mode_text(reset));
    }
    j
}

fn print_hub(format: Format, reg: u8, data: &[u8], out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            for (i, b) in data.iter().enumerate() {
                writeln!(out, "0x{:02X}  0x{b:02X}", reg as usize + i)?;
            }
        }
        Format::Json | Format::Jsonl => {
            writeln!(out, "{}", serde_json::json!({ "reg": reg, "data": data }))?
        }
    }
    Ok(())
}

fn port_name(p: UcpdPort) -> &'static str {
    match p {
        UcpdPort::Ucpd1 => "ucpd1",
        _ => "ucpd2",
    }
}

fn print_ucpd(
    format: Format,
    states: &[(UcpdPort, debug::Ucpd)],
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            for (port, u) in states {
                writeln!(
                    out,
                    "{}  role {:<6} cc1 {:<8} cc2 {:<8} CR 0x{:08X} SR 0x{:08X}",
                    port_name(*port),
                    u.role.map(debug::role_name).unwrap_or("?"),
                    u.cc1,
                    u.cc2,
                    u.cr,
                    u.sr
                )?;
            }
        }
        Format::Json | Format::Jsonl => {
            let j: serde_json::Map<String, serde_json::Value> = states
                .iter()
                .map(|(p, u)| Ok((port_name(*p).to_string(), serde_json::to_value(u)?)))
                .collect::<serde_json::Result<_>>()?;
            writeln!(out, "{}", serde_json::Value::Object(j))?;
        }
    }
    Ok(())
}

fn print_reg(format: Format, addr: u32, words: &[u32], out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            for (i, w) in words.iter().enumerate() {
                writeln!(out, "0x{:08X}  0x{w:08X}", addr + 4 * i as u32)?;
            }
        }
        Format::Json | Format::Jsonl => writeln!(
            out,
            "{}",
            serde_json::json!({ "addr": addr, "words": words })
        )?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::mock::MockDevice;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
    }

    fn text(f: impl FnOnce(&mut Vec<u8>) -> anyhow::Result<()>) -> String {
        let mut out = Vec::new();
        f(&mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn pins_by_net_or_port_pin() {
        let relay = parse_pin("RELAY_EN").unwrap();
        assert_eq!(parse_pin("pa5").unwrap(), relay);
        assert!(parse_pin("NOPE").is_err());
        assert!(parse_pin("PA16").is_err());
        assert_eq!(parse_u32("0x4002_1000"), Ok(0x4002_1000));
        assert_eq!(parse_u8("255"), Ok(255));
        assert!(parse_u8("0x100").is_err());
    }

    #[test]
    fn settings_to_mode_and_level() {
        assert_eq!(
            mode_level(Setting::High, None),
            Ok((PinMode::PushPull, Some(Level::High)))
        );
        assert_eq!(
            mode_level(Setting::OpenDrain, Some(LevelArg::Low)),
            Ok((PinMode::OpenDrain, Some(Level::Low)))
        );
        assert!(mode_level(Setting::High, Some(LevelArg::Low)).is_err());
        assert!(mode_level(Setting::Input, Some(LevelArg::High)).is_err());
    }

    #[test]
    fn debug_output_from_the_mock() {
        let mock = MockDevice::start("cli/debug");
        let relay = parse_pin("RELAY_EN").unwrap();
        let (set, refused, boot, now, hub, ucpd, reg) = rt().block_on(async {
            let dongle = mock.open().await.unwrap();
            let d = dongle.debug();
            let set = d
                .pin_set(relay, PinMode::PushPull, Some(Level::High))
                .await
                .unwrap();
            let refused = d
                .pin_set(parse_pin("LED_RED").unwrap(), PinMode::Input, None)
                .await
                .unwrap_err();
            let boot = d.boot_pins().await.unwrap();
            let now = d.pins().await.unwrap();
            let hub = d.hub_read(0x00, 4).await.unwrap();
            d.ucpd_role(UcpdPort::Ucpd2, CcRole::Rp1A5).await.unwrap();
            let ucpd = vec![
                (
                    UcpdPort::Ucpd1,
                    debug::ucpd(&d.ucpd(UcpdPort::Ucpd1).await.unwrap()),
                ),
                (
                    UcpdPort::Ucpd2,
                    debug::ucpd(&d.ucpd(UcpdPort::Ucpd2).await.unwrap()),
                ),
            ];
            let reg = d.reg_read(0x5000_0000, 2).await.unwrap();
            dongle.close().await.unwrap();
            (set, refused, boot, now, hub, ucpd, reg)
        });

        insta::assert_snapshot!("pin_set", text(|o| print_pin(Format::Text, relay, &set, o)));
        insta::assert_snapshot!(
            "pin_set_json",
            text(|o| print_pin(Format::Json, relay, &set, o))
        );
        let refused = anyhow::Error::from(refused);
        assert!(matches!(ExitCode::from(&refused), ExitCode::DeviceError));
        insta::assert_snapshot!("pin_in_use", format!("{refused:#}"));
        insta::assert_snapshot!(
            "boot_diff",
            text(|o| print_pins(Format::Text, &boot.ports, true, o))
        );
        insta::assert_snapshot!(
            "pins_diff",
            text(|o| print_pins(Format::Text, &now.ports, true, o))
        );
        insta::assert_snapshot!(
            "pins_diff_jsonl",
            text(|o| print_pins(Format::Jsonl, &now.ports, true, o))
        );
        insta::assert_snapshot!(
            "pins",
            text(|o| print_pins(Format::Text, &now.ports, false, o))
        );
        insta::assert_snapshot!("hub_read", text(|o| print_hub(Format::Text, 0x00, &hub, o)));
        insta::assert_snapshot!("ucpd", text(|o| print_ucpd(Format::Text, &ucpd, o)));
        insta::assert_snapshot!("ucpd_json", text(|o| print_ucpd(Format::Json, &ucpd, o)));
        insta::assert_snapshot!(
            "reg",
            text(|o| print_reg(Format::Text, 0x5000_0000, &reg, o))
        );
    }
}
