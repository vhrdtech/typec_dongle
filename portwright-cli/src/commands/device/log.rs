//! `donguru device log` - the stored defmt log, decoded with the firmware's ELF found by its build id (CLI-19,
//! API-10).

use std::io::{self, Write};
use std::path::PathBuf;

use crate::completion;
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use donguru_core::log::LogReport;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render, Spinner};
use crate::theme::Theme;

#[derive(Debug, Args)]
pub struct Log {
    /// Firmware ELF to decode with, instead of the one found by the device's build id
    #[arg(long, value_name = "FILE", add = ArgValueCompleter::new(completion::elf_files))]
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
            let spinner = Spinner::counting(ctx, "following the device log", "lines");
            let mut out = Output::stdout(ctx).with_spinner(&spinner);
            device
                .log_follow(
                    args.elf.as_deref(),
                    args.force,
                    |line| {
                        let _ = out.record(&Line(line));
                    },
                    |warning| spinner.suspend(|| eprintln!("donguru: {warning}")),
                )
                .await?;
        } else {
            let report = device.log_dump(args.elf.as_deref(), args.force).await?;
            if let Some(w) = &report.warning
                && !ctx.quiet
            {
                eprintln!("donguru: {w}");
            }
            Output::stdout(ctx).emit(&report)?;
        }
        dongle.close().await?;
        Ok::<_, anyhow::Error>(ExitCode::Success)
    })
}

/// One decoded log line: `{"line": "..."}` in json.
struct Line<'a>(&'a str);

impl Render for Line<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(out, "{}", line_text(th, self.0))
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "line": self.0 })
    }
}

/// A line in colour: the defmt level (and the timestamp before it), and the host's own `--- ... ---` notes.
fn line_text(th: &Theme, line: &str) -> String {
    if line.starts_with("---") {
        return th.warn(line).to_string();
    }
    let mut words = line.splitn(3, ' ');
    let (first, second) = (words.next().unwrap_or(""), words.next());
    let level = |w: &str| -> Option<String> {
        Some(match w {
            "ERROR" => th.bad(w).to_string(),
            "WARN" => th.warn(w).to_string(),
            "INFO" => th.ok(w).to_string(),
            "DEBUG" | "TRACE" => th.dim(w).to_string(),
            _ => return None,
        })
    };
    if let Some(l) = level(first) {
        return format!("{l}{}", &line[first.len()..]);
    }
    if let Some(second) = second
        && let Some(l) = level(second)
    {
        let rest = &line[first.len() + 1 + second.len()..];
        return format!("{} {l}{rest}", th.dim(first));
    }
    line.to_string()
}

impl Render for LogReport {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        if self.lines.is_empty() {
            return writeln!(out, "{}", th.dim("the device's log is empty"));
        }
        for line in &self.lines {
            writeln!(out, "{}", line_text(th, line))?;
        }
        Ok(())
    }

    fn json(&self) -> serde_json::Value {
        // strings and numbers: serialising can't fail
        serde_json::to_value(self).unwrap_or_default()
    }

    fn jsonl(&self) -> Vec<serde_json::Value> {
        self.lines.iter().map(|l| Line(l).json()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::render;

    #[test]
    fn empty_log_says_so_in_text() {
        assert_eq!(
            render(Format::Text, &LogReport::default()),
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
        let out = render(Format::Jsonl, &report);
        assert_eq!(out.lines().count(), 2);
        let first: serde_json::Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
        assert_eq!(first["line"], "INFO boot");
    }

    #[test]
    fn levels_in_colour_text_unchanged() {
        let plain = Theme::plain();
        for line in [
            "0.001 INFO boot",
            "WARN low",
            "--- 3 log messages lost ---",
            "x",
        ] {
            assert_eq!(line_text(&plain, line), line);
        }
        let colour = Theme::new(true);
        let l = line_text(&colour, "0.001 ERROR boom now");
        assert!(
            l.contains("ERROR") && l.ends_with(" boom now") && l.contains('\x1b'),
            "{l:?}"
        );
    }
}
