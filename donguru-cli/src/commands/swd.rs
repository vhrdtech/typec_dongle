//! `donguru swd` - the SWD-host pins on the plug SBU lines (FW-15): cable orientation, the Hi-Z release used
//! by bench power-cut tests, and a memtest-style integrity check of the link (CLI-18). CMSIS-DAP itself is not a
//! `donguru` subcommand: probe-rs talks to it directly over USB (see the hardware checklist in
//! `docs/development/index.md`).

use crate::cli::{ColorMode, Format};
use crate::context::Context;
use crate::exit::ExitCode;
use crate::theme::DEFAULT_PALETTE;
use clap::builder::styling::Style;
use clap::{Args, Subcommand, ValueEnum};
use donguru_core::swd_memtest::probe::{DONGURU_PROBE, Target};
use donguru_core::swd_memtest::{Pattern, PatternResult, Region};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Orientation {
    /// SBU1 = SWCLK, SBU2 = SWDIO
    Normal,
    /// SBU1 = SWDIO, SBU2 = SWCLK
    Swapped,
}

#[derive(Debug, Subcommand)]
pub enum Swd {
    /// Show or set the cable orientation used for SWD-over-SBU
    ///
    /// Takes effect on the next CMSIS-DAP `DAP_Connect`; does not affect an already connected probe-rs session.
    Orientation {
        /// New orientation; omit to show the current one
        #[arg(value_enum)]
        orientation: Option<Orientation>,
    },

    /// Release the SWD-host pins to Hi-Z (SWCLK, SWDIO and the SBU level shifter), independent of any
    /// CMSIS-DAP session
    ///
    /// For a test harness to call right before cutting the DUT's power, so nothing on the plug SBU lines can
    /// back-power it.
    Release,

    /// Check the SWD link end to end: crafted patterns written to the target's RAM and read back
    ///
    /// Halts the target, saves the RAM region, runs memtest-style patterns (data bus, address bus, solid,
    /// checkerboard, address-in-address, moving inversions, byte lanes, seeded random) at each SWD speed, then
    /// restores the region and lets the target run on. Any mismatch is a link error: the address and the bits
    /// that flipped are printed. Exit code 1 on any mismatch.
    Memtest(MemtestArgs),
}

#[derive(Debug, Args)]
pub struct MemtestArgs {
    /// probe-rs target name of the chip behind the probe, e.g. STM32G0B1CETx
    #[arg(long, env = "DONGURU_SWD_CHIP")]
    chip: String,

    /// Debug probe as VID:PID[:SERIAL]; any CMSIS-DAP probe works, Donguru by default
    #[arg(long, env = "DONGURU_SWD_PROBE", default_value = DONGURU_PROBE)]
    probe: String,

    /// SWD speeds to test in kHz, comma separated; the probe may round them
    #[arg(long, value_delimiter = ',', default_value = "1000")]
    speed: Vec<u32>,

    /// Start of the RAM region (0x... or decimal); default: the chip's largest RAM region
    #[arg(long, value_parser = parse_size)]
    base: Option<u64>,

    /// Size of the region in bytes (0x..., decimal, or with K/M); default: up to the end of that RAM region
    #[arg(long, value_parser = parse_size)]
    size: Option<u64>,

    /// Patterns to run, comma separated; default: all
    #[arg(long, value_enum, value_delimiter = ',')]
    pattern: Vec<PatternArg>,

    /// Seed of the random pattern; default: from the clock (printed, so a failure can be replayed)
    #[arg(long)]
    seed: Option<u64>,

    /// Repeat the whole run this many times
    #[arg(long, default_value_t = 1)]
    passes: u32,

    /// Leave the target halted with test data in RAM instead of restoring the region and resuming
    #[arg(long)]
    no_restore: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PatternArg {
    DataBus,
    AddressBus,
    Solid,
    Checkerboard,
    AddressInAddress,
    MovingInversions,
    ByteLanes,
    Random,
}

impl From<PatternArg> for Pattern {
    fn from(p: PatternArg) -> Self {
        match p {
            PatternArg::DataBus => Pattern::DataBus,
            PatternArg::AddressBus => Pattern::AddressBus,
            PatternArg::Solid => Pattern::Solid,
            PatternArg::Checkerboard => Pattern::Checkerboard,
            PatternArg::AddressInAddress => Pattern::AddressInAddress,
            PatternArg::MovingInversions => Pattern::MovingInversions,
            PatternArg::ByteLanes => Pattern::ByteLanes,
            PatternArg::Random => Pattern::Random,
        }
    }
}

pub fn run(ctx: &Context, cmd: Swd) -> anyhow::Result<ExitCode> {
    match cmd {
        Swd::Orientation { orientation: None } => super::stub("swd orientation", ExitCode::Failure),
        Swd::Orientation {
            orientation: Some(orientation),
        } => super::stub(
            &format!("swd orientation {orientation:?}"),
            ExitCode::Failure,
        ),
        Swd::Release => super::stub("swd release", ExitCode::Failure),
        Swd::Memtest(args) => memtest(ctx, args),
    }
}

fn memtest(ctx: &Context, args: MemtestArgs) -> anyhow::Result<ExitCode> {
    let patterns: Vec<Pattern> = if args.pattern.is_empty() {
        Pattern::ALL.to_vec()
    } else {
        args.pattern.iter().map(|&p| p.into()).collect()
    };
    let seed = args.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
    });
    let text = matches!(ctx.format, Format::Text);
    let paint = Paint::new(ctx.color);
    let mut all = Vec::new();

    for pass in 1..=args.passes {
        for &speed in &args.speed {
            let mut target = Target::attach(&args.probe, &args.chip, speed)?;
            let region = region(&target, &args)?;
            if text {
                println!(
                    "{} pass {pass}/{}  {} kHz  {:#x}..{:#x} ({} KiB)  seed {seed}",
                    paint.header("▶"),
                    args.passes,
                    target.speed_khz,
                    region.base,
                    region.base + 4 * region.words as u64,
                    region.words * 4 / 1024,
                );
            }
            let spinner = spinner(ctx, text);
            let speed_khz = target.speed_khz;
            spinner.set_message(patterns[0].name());
            let mut next = patterns.iter().skip(1);
            let results = target.run(region, &patterns, seed, !args.no_restore, |r| {
                spinner.suspend(|| report(ctx, &paint, pass, speed_khz, seed, r));
                if let Some(p) = next.next() {
                    spinner.set_message(p.name());
                }
            })?;
            spinner.finish_and_clear();
            all.extend(results.into_iter().map(|r| (pass, speed_khz, r)));
        }
    }

    let failed = all.iter().filter(|(_, _, r)| !r.passed()).count();
    match ctx.format {
        Format::Text => {
            let bytes: u64 = all
                .iter()
                .map(|(_, _, r)| 4 * (r.words_read + r.words_written))
                .sum();
            let verdict = if failed == 0 {
                paint.valid("all patterns passed")
            } else {
                paint.error(&format!("{failed} pattern run(s) failed"))
            };
            println!(
                "{verdict}: {} runs, {} KiB over the link, seed {seed}",
                all.len(),
                bytes / 1024
            );
        }
        Format::Json => {
            let runs: Vec<_> = all
                .iter()
                .map(|(pass, khz, r)| json(*pass, *khz, seed, r))
                .collect();
            println!(
                "{}",
                serde_json::json!({ "seed": seed, "passed": failed == 0, "runs": runs })
            );
        }
        Format::Jsonl => {}
    }
    Ok(if failed == 0 {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
}

fn region(target: &Target, args: &MemtestArgs) -> anyhow::Result<Region> {
    let ram = target.largest_ram();
    let base = match (args.base, ram) {
        (Some(b), _) => b,
        (None, Some(r)) => r.base,
        (None, None) => {
            anyhow::bail!("the chip has no RAM region in its memory map: give --base and --size")
        }
    };
    anyhow::ensure!(base % 4 == 0, "--base must be word-aligned");
    let size = match (args.size, ram) {
        (Some(s), _) => s,
        (None, Some(r)) if (r.base..r.base + 4 * r.words as u64).contains(&base) => {
            r.base + 4 * r.words as u64 - base
        }
        _ => anyhow::bail!("--base is outside the chip's largest RAM region: give --size"),
    };
    anyhow::ensure!(
        size >= 8 && size % 4 == 0,
        "--size must be a multiple of 4, at least 8"
    );
    Ok(Region {
        base,
        words: (size / 4) as usize,
    })
}

fn report(ctx: &Context, paint: &Paint, pass: u32, khz: u32, seed: u64, r: &PatternResult) {
    match ctx.format {
        Format::Text => {
            let status = if r.passed() {
                paint.valid("ok  ")
            } else {
                paint.error("FAIL")
            };
            let words = r.words_read + r.words_written;
            let rate = 4.0 * words as f64 / r.elapsed.as_secs_f64().max(1e-9) / 1024.0;
            println!(
                "  {status} {:<19} {:>8} words  {:>6.1} s  {:>5.1} KiB/s{}",
                r.pattern.name(),
                words,
                r.elapsed.as_secs_f64(),
                rate,
                if r.passed() {
                    String::new()
                } else {
                    format!("  {} mismatches", r.mismatches)
                }
            );
            for m in &r.first_mismatches {
                println!(
                    "       {:#010x}: expected {:#010x}, read {:#010x}, bits {:#010x}",
                    m.addr,
                    m.expected,
                    m.actual,
                    m.xor()
                );
            }
        }
        Format::Jsonl => println!("{}", json(pass, khz, seed, r)),
        Format::Json => {}
    }
}

fn json(pass: u32, khz: u32, seed: u64, r: &PatternResult) -> serde_json::Value {
    serde_json::json!({
        "pass": pass,
        "speed_khz": khz,
        "pattern": r.pattern.name(),
        "seed": seed,
        "passed": r.passed(),
        "words_read": r.words_read,
        "words_written": r.words_written,
        "seconds": r.elapsed.as_secs_f64(),
        "mismatches": r.mismatches,
        "first_mismatches": r.first_mismatches.iter().map(|m| serde_json::json!({
            "addr": m.addr, "expected": m.expected, "read": m.actual,
        })).collect::<Vec<_>>(),
    })
}

fn spinner(ctx: &Context, text: bool) -> ProgressBar {
    if !text || ctx.quiet || !std::io::stderr().is_terminal() {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner} {msg} {elapsed}").expect("valid template"),
    );
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

/// Colour on stdout per `--color` and `NO_COLOR`.
struct Paint(bool);

impl Paint {
    fn new(mode: ColorMode) -> Self {
        Paint(match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
            }
        })
    }

    fn with(&self, style: Style, s: &str) -> String {
        if self.0 {
            format!("{style}{s}{style:#}")
        } else {
            s.to_string()
        }
    }

    fn header(&self, s: &str) -> String {
        self.with(
            Style::new().fg_color(Some(DEFAULT_PALETTE.header())).bold(),
            s,
        )
    }

    fn valid(&self, s: &str) -> String {
        self.with(Style::new().fg_color(Some(DEFAULT_PALETTE.valid())), s)
    }

    fn error(&self, s: &str) -> String {
        self.with(
            Style::new().fg_color(Some(DEFAULT_PALETTE.error())).bold(),
            s,
        )
    }
}

/// `0x...`, decimal, or decimal with a K or M (binary) suffix.
fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num, mul) = match s.chars().last() {
        Some('k' | 'K') => (&s[..s.len() - 1], 1024),
        Some('m' | 'M') => (&s[..s.len() - 1], 1024 * 1024),
        _ => (s, 1),
    };
    let v = match num.strip_prefix("0x").or_else(|| num.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(&hex.replace('_', ""), 16),
        None => num.replace('_', "").parse(),
    }
    .map_err(|e| format!("{s}: {e}"))?;
    v.checked_mul(mul).ok_or_else(|| format!("{s}: too large"))
}

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("0x2000_0000"), Ok(0x2000_0000));
        assert_eq!(parse_size("144K"), Ok(144 * 1024));
        assert_eq!(parse_size("1M"), Ok(1 << 20));
        assert_eq!(parse_size("4096"), Ok(4096));
        assert!(parse_size("0xZZ").is_err());
    }
}
