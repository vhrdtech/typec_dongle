//! `donguru swd` - the SWD-host pins on the plug SBU lines (FW-15): cable orientation, the Hi-Z release used
//! by bench power-cut tests, and a memtest-style integrity check of the link (CLI-18). CMSIS-DAP itself is not a
//! `donguru` subcommand: probe-rs talks to it directly over USB (see the hardware checklist in
//! `docs/development/index.md`).

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render, Spinner};
use crate::theme::Theme;
use clap::{Args, Subcommand, ValueEnum};
use donguru_core::swd_memtest::probe::{DONGURU_PROBE, Target};
use donguru_core::swd_memtest::{Pattern, PatternResult, Region};
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Orientation {
    /// From the CC polarity once detected, normal until then
    Auto,
    /// SBU1 = SWCLK, SBU2 = SWDIO
    Normal,
    /// SBU1 = SWDIO, SBU2 = SWCLK
    Swapped,
}

impl From<Orientation> for api::Orientation {
    fn from(o: Orientation) -> Self {
        match o {
            Orientation::Auto => api::Orientation::Auto,
            Orientation::Normal => api::Orientation::Normal,
            Orientation::Swapped => api::Orientation::Swapped,
        }
    }
}

fn fmt_orientation(o: api::Orientation) -> &'static str {
    match o {
        api::Orientation::Auto => "auto",
        api::Orientation::Normal => "normal",
        api::Orientation::Swapped => "swapped",
    }
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
        Swd::Orientation { orientation } => orientation_cmd(ctx, orientation),
        Swd::Release => release(ctx),
        Swd::Memtest(args) => memtest(ctx, args),
    }
}

/// Shows, or sets and reads back, `plug.orientation`: the one orientation of everything on the plug SBU lines.
fn orientation_cmd(ctx: &Context, wanted: Option<Orientation>) -> anyhow::Result<ExitCode> {
    let now = super::with_dongle(ctx, async |dongle| orientation_on(dongle, wanted).await)?;
    match ctx.format {
        Format::Text => println!("orientation: {}", fmt_orientation(now)),
        Format::Json | Format::Jsonl => {
            println!(
                "{}",
                serde_json::json!({ "orientation": fmt_orientation(now) })
            )
        }
    }
    Ok(match wanted {
        Some(w) if api::Orientation::from(w) != now => ExitCode::Failure,
        _ => ExitCode::Success,
    })
}

async fn orientation_on(
    dongle: &Dongle,
    wanted: Option<Orientation>,
) -> donguru_core::Result<api::Orientation> {
    if let Some(wanted) = wanted {
        dongle.plug().set_orientation(wanted.into()).await?;
    }
    dongle.plug().orientation().await
}

async fn release_on(dongle: &Dongle) -> donguru_core::Result<api::SbuFunction> {
    dongle.sbu().release().await?;
    dongle.sbu().function().await
}

fn fmt_function(f: api::SbuFunction) -> &'static str {
    match f {
        api::SbuFunction::Off => "off",
        api::SbuFunction::Swd => "swd",
        api::SbuFunction::Uart { .. } => "uart",
        api::SbuFunction::I2c => "i2c",
        api::SbuFunction::Gpio => "gpio",
        api::SbuFunction::Usb => "usb",
        #[allow(unreachable_patterns)] // a function of a newer firmware
        _ => "unknown",
    }
}

/// `sbu.release`, then reads back who owns SBU: anything but `off` is a failure.
fn release(ctx: &Context) -> anyhow::Result<ExitCode> {
    let function = super::with_dongle(ctx, async |dongle| release_on(dongle).await)?;
    match ctx.format {
        Format::Text => println!("sbu: {}", fmt_function(function)),
        Format::Json | Format::Jsonl => {
            println!("{}", serde_json::json!({ "sbu": fmt_function(function) }))
        }
    }
    Ok(if matches!(function, api::SbuFunction::Off) {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
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
    let mut all = Vec::new();

    for pass in 1..=args.passes {
        for &speed in &args.speed {
            let mut target = Target::attach(&args.probe, &args.chip, speed)?;
            let region = region(&target, &args)?;
            let speed_khz = target.speed_khz;
            let spinner = Spinner::new(ctx, patterns[0].name());
            let mut out = Output::stdout(ctx).with_spinner(&spinner);
            if out.format() == Format::Text {
                out.record(&PassHeader {
                    pass,
                    passes: args.passes,
                    khz: speed_khz,
                    region,
                    seed,
                })?;
            }
            let mut next = patterns.iter().skip(1);
            let mut failed_write = None;
            let results = target.run(region, &patterns, seed, !args.no_restore, |r| {
                // `--format json` prints every run once at the end, in the summary
                if out.format() != Format::Json
                    && let Err(e) = out.record(&Run {
                        pass,
                        khz: speed_khz,
                        seed,
                        r,
                    })
                {
                    failed_write.get_or_insert(e);
                }
                if let Some(p) = next.next() {
                    spinner.set_message(p.name());
                }
            })?;
            if let Some(e) = failed_write {
                return Err(e);
            }
            all.extend(results.into_iter().map(|r| (pass, speed_khz, r)));
        }
    }

    let failed = all.iter().filter(|(_, _, r)| !r.passed()).count();
    Output::stdout(ctx).emit(&Summary { seed, all: &all })?;
    Ok(if failed == 0 {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
}

/// `▶ pass 1/2  4000 kHz  0x20000000..0x20024000 (144 KiB)  seed 42`, text only.
struct PassHeader {
    pass: u32,
    passes: u32,
    khz: u32,
    region: Region,
    seed: u64,
}

impl Render for PassHeader {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let r = &self.region;
        writeln!(
            out,
            "{} pass {}/{}  {} kHz  {:#x}..{:#x} ({} KiB)  seed {}",
            th.heading("▶"),
            self.pass,
            self.passes,
            self.khz,
            r.base,
            r.base + 4 * r.words as u64,
            r.words * 4 / 1024,
            self.seed,
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

/// One pattern run: a line (and its first mismatches) in text, one record in jsonl.
struct Run<'a> {
    pass: u32,
    khz: u32,
    seed: u64,
    r: &'a PatternResult,
}

impl Render for Run<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let r = self.r;
        let status = if r.passed() {
            th.ok("ok  ")
        } else {
            th.bad("FAIL")
        };
        let words = r.words_read + r.words_written;
        let rate = 4.0 * words as f64 / r.elapsed.as_secs_f64().max(1e-9) / 1024.0;
        writeln!(
            out,
            "  {status} {:<19} {:>8} words  {:>6.1} s  {:>5.1} KiB/s{}",
            r.pattern.name(),
            words,
            r.elapsed.as_secs_f64(),
            rate,
            if r.passed() {
                String::new()
            } else {
                format!("  {} mismatches", th.bad(r.mismatches))
            }
        )?;
        for m in &r.first_mismatches {
            writeln!(
                out,
                "       {:#010x}: expected {:#010x}, read {:#010x}, bits {:#010x}",
                m.addr,
                m.expected,
                m.actual,
                m.xor()
            )?;
        }
        Ok(())
    }

    fn json(&self) -> serde_json::Value {
        json(self.pass, self.khz, self.seed, self.r)
    }
}

/// The verdict over every run: a line in text, every run in json, nothing more in jsonl (its runs were the
/// records).
struct Summary<'a> {
    seed: u64,
    all: &'a [(u32, u32, PatternResult)],
}

impl Render for Summary<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let failed = self.all.iter().filter(|(_, _, r)| !r.passed()).count();
        let bytes: u64 = self
            .all
            .iter()
            .map(|(_, _, r)| 4 * (r.words_read + r.words_written))
            .sum();
        let verdict = if failed == 0 {
            th.ok("all patterns passed".to_string())
        } else {
            th.bad(format!("{failed} pattern run(s) failed"))
        };
        writeln!(
            out,
            "{verdict}: {} runs, {} KiB over the link, seed {}",
            self.all.len(),
            bytes / 1024,
            self.seed
        )
    }

    fn json(&self) -> serde_json::Value {
        let runs: Vec<_> = self
            .all
            .iter()
            .map(|(pass, khz, r)| json(*pass, *khz, self.seed, r))
            .collect();
        let passed = self.all.iter().all(|(_, _, r)| r.passed());
        serde_json::json!({ "seed": self.seed, "passed": passed, "runs": runs })
    }

    fn jsonl(&self) -> Vec<serde_json::Value> {
        Vec::new()
    }
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
    anyhow::ensure!(base.is_multiple_of(4), "--base must be word-aligned");
    let size = match (args.size, ram) {
        (Some(s), _) => s,
        (None, Some(r)) if (r.base..r.base + 4 * r.words as u64).contains(&base) => {
            r.base + 4 * r.words as u64 - base
        }
        _ => anyhow::bail!("--base is outside the chip's largest RAM region: give --size"),
    };
    anyhow::ensure!(
        size >= 8 && size.is_multiple_of(4),
        "--size must be a multiple of 4, at least 8"
    );
    Ok(Region {
        base,
        words: (size / 4) as usize,
    })
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
    use super::*;
    use donguru_core::mock::MockDevice;
    use crate::output::render;
    use donguru_core::swd_memtest::Mismatch;
    use std::time::Duration;

    fn result(pattern: Pattern, mismatches: Vec<Mismatch>) -> PatternResult {
        PatternResult {
            pattern,
            words_written: 36_864,
            words_read: 36_864,
            mismatches: mismatches.len() as u64,
            first_mismatches: mismatches,
            elapsed: Duration::from_millis(1_500),
        }
    }

    #[test]
    fn memtest_runs_and_summary_in_every_format() {
        let bad = Mismatch {
            addr: 0x2000_0010,
            expected: 0x5555_5555,
            actual: 0x5555_5554,
        };
        let all = vec![
            (1, 4000, result(Pattern::Solid, Vec::new())),
            (1, 4000, result(Pattern::Checkerboard, vec![bad])),
        ];
        let header = PassHeader {
            pass: 1,
            passes: 2,
            khz: 4000,
            region: Region {
                base: 0x2000_0000,
                words: 36_864,
            },
            seed: 42,
        };
        let run = |i: usize| Run {
            pass: all[i].0,
            khz: all[i].1,
            seed: 42,
            r: &all[i].2,
        };
        let summary = Summary {
            seed: 42,
            all: &all,
        };
        let text = [
            render(Format::Text, &header),
            render(Format::Text, &run(0)),
            render(Format::Text, &run(1)),
            render(Format::Text, &summary),
        ]
        .concat();
        insta::assert_snapshot!(text);
        insta::assert_snapshot!(render(Format::Jsonl, &run(1)));
        insta::assert_snapshot!(render(Format::Json, &summary));
        assert_eq!(render(Format::Jsonl, &summary), "");
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("0x2000_0000"), Ok(0x2000_0000));
        assert_eq!(parse_size("144K"), Ok(144 * 1024));
        assert_eq!(parse_size("1M"), Ok(1 << 20));
        assert_eq!(parse_size("4096"), Ok(4096));
        assert!(parse_size("0xZZ").is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn orientation_round_trip_and_release() {
        let mock = MockDevice::start("cli/swd_orientation");
        let dongle = mock.open().await.unwrap();
        assert_eq!(
            orientation_on(&dongle, None).await.unwrap(),
            api::Orientation::Auto
        );
        assert_eq!(
            orientation_on(&dongle, Some(Orientation::Swapped))
                .await
                .unwrap(),
            api::Orientation::Swapped
        );
        // release puts SBU to Hi-Z, also while another function holds it
        mock.with(|s| s.sbu_function = api::SbuFunction::Swd).await;
        assert_eq!(release_on(&dongle).await.unwrap(), api::SbuFunction::Off);
        dongle.close().await.unwrap();
    }

    #[test]
    fn orientation_names() {
        assert_eq!(fmt_orientation(Orientation::Swapped.into()), "swapped");
        assert_eq!(fmt_orientation(Orientation::Auto.into()), "auto");
    }
}
