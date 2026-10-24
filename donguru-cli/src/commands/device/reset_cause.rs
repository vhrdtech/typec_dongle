//! `donguru device reset-cause` - why the device last reset, and the crash record if there is one (CLI-19, API-9).

use std::io::{self, Write};

use clap::Args;
use ww_firmware_info::ResetRecordOwned;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;

#[derive(Debug, Args)]
pub struct ResetCause {
    /// Forget the crash record and zero the boot counter, after printing them
    #[arg(long)]
    clear: bool,
}

pub fn run(ctx: &Context, args: ResetCause) -> anyhow::Result<ExitCode> {
    super::runtime()?.block_on(async {
        let dongle = super::open(ctx).await?;
        let device = dongle.device();
        let record = device.last_reset().await?;
        let boot_count = device.boot_count().await?;
        Output::stdout(ctx).emit(&Shown {
            record: &record,
            boot_count,
        })?;
        if args.clear {
            device.clear_last_reset().await?;
            if ctx.format == Format::Text && !ctx.quiet {
                eprintln!("donguru: crash record and boot counter cleared");
            }
        }
        dongle.close().await?;
        Ok::<_, anyhow::Error>(ExitCode::Success)
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The last reset and the boot counter.
struct Shown<'a> {
    record: &'a ResetRecordOwned,
    boot_count: u32,
}

impl Render for Shown<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        const W: usize = 19;
        let record = self.record;
        let cause = format!("{:?}", record.cause);
        // a crash, a fault or a watchdog is the line to see first
        let crashed = record.fault.is_some()
            || !record.message.is_empty()
            || ["Panic", "Fault", "Watchdog", "Brown"]
                .iter()
                .any(|w| cause.contains(w));
        let cause = if crashed {
            th.bad(cause)
        } else {
            th.value(cause)
        };
        writeln!(out, "{:<W$}{cause}", th.key("cause"))?;
        writeln!(
            out,
            "{:<W$}{}",
            th.key("boot count"),
            th.value(self.boot_count)
        )?;
        writeln!(
            out,
            "{:<W$}{}",
            th.key("boots since crash"),
            th.value(record.boots_since_crash)
        )?;
        if let Some(uptime) = record.uptime {
            writeln!(
                out,
                "{:<W$}{} s",
                th.key("uptime at crash"),
                th.value(format!("{:.3}", uptime.to_f64()))
            )?;
        }
        let build_id = hex(record.build_id.as_slice());
        if !build_id.is_empty() {
            writeln!(
                out,
                "{:<W$}{}",
                th.key("firmware build id"),
                th.value(build_id)
            )?;
        }
        if !record.message.is_empty() {
            writeln!(
                out,
                "{:<W$}{}",
                th.key("message"),
                th.bad(format!("{:?}", record.message.as_str()))
            )?;
        }
        if let Some(fault) = &record.fault {
            writeln!(
                out,
                "{:<W$}{}",
                th.key("fault"),
                th.bad(format!("{fault:?}"))
            )?;
        }
        if !record.stack.is_empty() {
            let words: Vec<String> = record
                .stack
                .as_slice()
                .iter()
                .map(|w| format!("{w:#010x}"))
                .collect();
            writeln!(out, "{:<W$}{}", th.key("stack"), th.value(words.join(" ")))?;
        }
        Ok(())
    }

    fn json(&self) -> serde_json::Value {
        let record = self.record;
        serde_json::json!({
            "cause": format!("{:?}", record.cause),
            "boot_count": self.boot_count,
            "boots_since_crash": record.boots_since_crash,
            "uptime_s": record.uptime.map(|u| u.to_f64()),
            "build_id": hex(record.build_id.as_slice()),
            "message": record.message.as_str(),
            "fault": record.fault.as_ref().map(|f| format!("{f:?}")),
            "stack": record.stack.as_slice(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::render;
    use wire_weaver::shrink_wrap::MaxLen;
    use ww_firmware_info::ResetCause as Cause;

    fn record(cause: Cause, message: &str) -> ResetRecordOwned {
        ResetRecordOwned {
            cause,
            build_id: MaxLen::new_unchecked(vec![0xab, 0xcd]),
            uptime: None,
            fault: None,
            stack: MaxLen::new_unchecked(vec![]),
            message: MaxLen::new_unchecked(message.to_string()),
            boots_since_crash: 0,
        }
    }

    #[test]
    fn text() {
        let record = record(Cause::PowerOn, "");
        let shown = Shown {
            record: &record,
            boot_count: 1,
        };
        insta::assert_snapshot!(render(Format::Text, &shown));
    }

    #[test]
    fn json_with_a_message() {
        let record = record(Cause::Watchdog, "stuck in a loop");
        let shown = Shown {
            record: &record,
            boot_count: 4,
        };
        insta::assert_snapshot!(render(Format::Json, &shown));
    }
}
