//! `donguru device log` - the stored defmt log, decoded with the firmware's ELF found by its build id (CLI-19,
//! API-10).

use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use donguru_core::log::LogReport;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;

#[derive(Debug, Args)]
pub struct Log {
    /// Firmware ELF to decode with, instead of the one found by the device's build id
    #[arg(long)]
    elf: Option<PathBuf>,

    /// Use the ELF although its build id differs from the device's
    #[arg(long)]
    force: bool,

    /// Keep printing new lines as the device logs them, until Ctrl+C
    #[arg(long)]
    follow: bool,
}

pub fn run(ctx: &Context, args: Log) -> anyhow::Result<ExitCode> {
    if args.follow && ctx.format == Format::Json {
        anyhow::bail!(
            "`--follow` streams one line at a time, pass `--format text` or `--format jsonl`"
        );
    }
    super::runtime()?.block_on(async {
        let dongle = super::open(ctx).await?;
        let device = dongle.device();
        if args.follow {
            let format = ctx.format;
            let mut out = std::io::stdout().lock();
            device
                .log_follow(
                    args.elf.as_deref(),
                    args.force,
                    |line| {
                        let _ = print_line(format, line, &mut out);
                    },
                    |warning| eprintln!("donguru: {warning}"),
                )
                .await?;
        } else {
            let report = device.log_dump(args.elf.as_deref(), args.force).await?;
            if let Some(w) = &report.warning
                && !ctx.quiet
            {
                eprintln!("donguru: {w}");
            }
            print_report(ctx.format, &report, &mut std::io::stdout().lock())?;
        }
        dongle.close().await?;
        Ok(ExitCode::Success)
    })
}

fn print_line(format: Format, line: &str, out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => writeln!(out, "{line}")?,
        Format::Json | Format::Jsonl => writeln!(out, "{}", serde_json::json!({ "line": line }))?,
    }
    Ok(())
}

fn print_report(format: Format, report: &LogReport, out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            if report.lines.is_empty() {
                writeln!(out, "the device's log is empty")?;
            } else {
                for line in &report.lines {
                    writeln!(out, "{line}")?;
                }
            }
        }
        Format::Json => writeln!(out, "{}", serde_json::to_value(report)?)?,
        Format::Jsonl => {
            for line in &report.lines {
                print_line(Format::Jsonl, line, out)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_log_says_so_in_text() {
        let mut out = Vec::new();
        print_report(Format::Text, &LogReport::default(), &mut out).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "the device's log is empty\n"
        );
    }

    #[test]
    fn lines_as_jsonl() {
        let report = LogReport {
            lines: vec!["INFO boot".into(), "WARN low battery".into()],
            lost: 0,
            warning: None,
        };
        let mut out = Vec::new();
        print_report(Format::Jsonl, &report, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert_eq!(out.lines().count(), 2);
        let first: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
        assert_eq!(first["line"], "INFO boot");
    }
}
