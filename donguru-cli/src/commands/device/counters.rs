//! `donguru device counters` - the `cnt` counters, decoded with the firmware's ELF found by its build id (CLI-19,
//! API-9).

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use clap::Args;
use donguru_core::counters::CountersReport;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;

#[derive(Debug, Args)]
pub struct Counters {
    /// Firmware ELF to decode with, instead of the one found by the device's build id
    #[arg(long)]
    elf: Option<PathBuf>,

    /// Use the ELF although its build id differs from the device's
    #[arg(long)]
    force: bool,

    /// Keep reading until Ctrl+C, printing a snapshot every `--interval` (poll, until CLT-28 adds observe)
    #[arg(long)]
    watch: bool,

    /// Time between reads in `--watch`, in milliseconds
    #[arg(long, default_value_t = 1000)]
    interval: u64,
}

pub fn run(ctx: &Context, args: Counters) -> anyhow::Result<ExitCode> {
    if args.watch && ctx.format == Format::Text {
        anyhow::bail!(
            "`--watch` streams one snapshot per line, pass `--format json` or `--format jsonl`"
        );
    }
    super::runtime()?.block_on(async {
        let dongle = super::open(ctx).await?;
        let device = dongle.device();
        let mut out = std::io::stdout().lock();
        loop {
            let report = device.counters(args.elf.as_deref(), args.force).await?;
            if let Some(w) = &report.warning
                && !ctx.quiet
            {
                eprintln!("donguru: {w}");
            }
            print_counters(ctx.format, &report, &mut out)?;
            if !args.watch {
                break;
            }
            tokio::time::sleep(Duration::from_millis(args.interval)).await;
        }
        dongle.close().await?;
        Ok(ExitCode::Success)
    })
}

fn print_counters(
    format: Format,
    report: &CountersReport,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            if report.counters.is_empty() {
                writeln!(out, "no counters")?;
                return Ok(());
            }
            let name_w = report
                .counters
                .iter()
                .map(|c| c.name.len())
                .max()
                .unwrap_or(0);
            for c in &report.counters {
                let unit = if c.unit.is_empty() {
                    String::new()
                } else {
                    format!(" {}", c.unit)
                };
                writeln!(
                    out,
                    "{:<4} {:7} {:name_w$} {}{unit}",
                    c.buffer, c.severity, c.name, c.value
                )?;
            }
        }
        Format::Json => writeln!(out, "{}", serde_json::to_value(report)?)?,
        Format::Jsonl => {
            for c in &report.counters {
                writeln!(out, "{}", serde_json::to_value(c)?)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::counters::CounterValue;

    fn report(counters: Vec<CounterValue>) -> CountersReport {
        CountersReport {
            elf: "fw.elf".into(),
            build_id: "abcd".into(),
            counters,
            warning: None,
        }
    }

    fn one() -> CountersReport {
        report(vec![
            CounterValue {
                buffer: "RAM",
                name: "fw/hard_faults".into(),
                value: 3,
                unit: String::new(),
                severity: "warn".into(),
            },
            CounterValue {
                buffer: "BKP",
                name: "button/presses".into(),
                value: 128,
                unit: "presses".into(),
                severity: "info".into(),
            },
        ])
    }

    #[test]
    fn text_table() {
        let mut text = Vec::new();
        print_counters(Format::Text, &one(), &mut text).unwrap();
        insta::assert_snapshot!(String::from_utf8(text).unwrap());
    }

    #[test]
    fn json() {
        let mut json = Vec::new();
        print_counters(Format::Json, &one(), &mut json).unwrap();
        insta::assert_snapshot!(String::from_utf8(json).unwrap());
    }

    #[test]
    fn empty_report_says_so_in_text() {
        let mut out = Vec::new();
        print_counters(Format::Text, &report(vec![]), &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "no counters\n");
    }
}
