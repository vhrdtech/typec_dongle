//! `donguru device counters` - the `cnt` counters, decoded with the firmware's ELF found by its build id (CLI-19,
//! API-9).

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use crate::completion;
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use donguru_core::counters::CountersReport;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render, Spinner};
use crate::theme::Theme;

#[derive(Debug, Args)]
pub struct Counters {
    /// Firmware ELF to decode with, instead of the one found by the device's build id
    #[arg(long, value_name = "FILE", add = ArgValueCompleter::new(completion::elf_files))]
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
        let spinner = args.watch.then(|| {
            Spinner::counting(
                ctx,
                format!("reading the counters every {} ms", args.interval),
                "snapshots",
            )
        });
        let mut out = match &spinner {
            Some(spinner) => Output::stdout(ctx).with_spinner(spinner),
            None => Output::stdout(ctx),
        };
        loop {
            let report = device.counters(args.elf.as_deref(), args.force).await?;
            if let Some(w) = &report.warning
                && !ctx.quiet
            {
                match &spinner {
                    Some(spinner) => spinner.suspend(|| eprintln!("donguru: {w}")),
                    None => eprintln!("donguru: {w}"),
                }
            }
            if args.watch {
                out.record(&report)?;
            } else {
                out.emit(&report)?;
            }
            if !args.watch {
                break;
            }
            tokio::time::sleep(Duration::from_millis(args.interval)).await;
        }
        dongle.close().await?;
        Ok::<_, anyhow::Error>(ExitCode::Success)
    })
}

impl Render for CountersReport {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        if self.counters.is_empty() {
            return writeln!(out, "{}", th.dim("no counters"));
        }
        let name_w = self
            .counters
            .iter()
            .map(|c| c.name.len())
            .max()
            .unwrap_or(0);
        for c in &self.counters {
            let unit = if c.unit.is_empty() {
                String::new()
            } else {
                format!(" {}", th.dim(&c.unit))
            };
            let severity = match c.severity.as_str() {
                "error" => th.bad(&c.severity),
                "warn" => th.warn(&c.severity),
                _ => th.dim(&c.severity),
            };
            // a warn / error counter that counted something is the row to look at
            let value = match c.severity.as_str() {
                "error" if c.value != 0 => th.bad(c.value),
                "warn" if c.value != 0 => th.warn(c.value),
                _ => th.value(c.value),
            };
            writeln!(
                out,
                "{:<4} {severity:7} {:name_w$} {value}{unit}",
                th.dim(c.buffer),
                th.key(&c.name),
            )?;
        }
        Ok(())
    }

    fn json(&self) -> serde_json::Value {
        // a plain struct of strings and numbers: serialising can't fail
        serde_json::to_value(self).unwrap_or_default()
    }

    fn jsonl(&self) -> Vec<serde_json::Value> {
        self.counters
            .iter()
            .map(|c| serde_json::to_value(c).unwrap_or_default())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::render;
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
        insta::assert_snapshot!(render(Format::Text, &one()));
    }

    #[test]
    fn json() {
        insta::assert_snapshot!(render(Format::Json, &one()));
        assert_eq!(render(Format::Jsonl, &one()).lines().count(), 2);
    }

    #[test]
    fn empty_report_says_so_in_text() {
        assert_eq!(render(Format::Text, &report(vec![])), "no counters\n");
    }
}
