//! `donguru device reset-cause` - why the device last reset, and the crash record if there is one (CLI-19, API-9).

use std::io::Write;

use clap::Args;
use ww_firmware_info::ResetRecordOwned;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;

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
        print_reset_cause(
            ctx.format,
            &record,
            boot_count,
            &mut std::io::stdout().lock(),
        )?;
        if args.clear {
            device.clear_last_reset().await?;
            if ctx.format == Format::Text && !ctx.quiet {
                eprintln!("donguru: crash record and boot counter cleared");
            }
        }
        dongle.close().await?;
        Ok(ExitCode::Success)
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn print_reset_cause(
    format: Format,
    record: &ResetRecordOwned,
    boot_count: u32,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let build_id = hex(record.build_id.as_slice());
    match format {
        Format::Text => {
            const W: usize = 19;
            writeln!(out, "{:<W$}{:?}", "cause", record.cause)?;
            writeln!(out, "{:<W$}{boot_count}", "boot count")?;
            writeln!(
                out,
                "{:<W$}{}",
                "boots since crash", record.boots_since_crash
            )?;
            if let Some(uptime) = record.uptime {
                writeln!(
                    out,
                    "{:<W$}{:.3} s",
                    "uptime at crash",
                    uptime.to_f64_seconds()
                )?;
            }
            if !build_id.is_empty() {
                writeln!(out, "{:<W$}{build_id}", "firmware build id")?;
            }
            if !record.message.is_empty() {
                writeln!(out, "{:<W$}{:?}", "message", record.message.as_str())?;
            }
            if let Some(fault) = &record.fault {
                writeln!(out, "{:<W$}{fault:?}", "fault")?;
            }
            if !record.stack.is_empty() {
                let words: Vec<String> = record
                    .stack
                    .as_slice()
                    .iter()
                    .map(|w| format!("{w:#010x}"))
                    .collect();
                writeln!(out, "{:<W$}{}", "stack", words.join(" "))?;
            }
        }
        Format::Json | Format::Jsonl => {
            let v = serde_json::json!({
                "cause": format!("{:?}", record.cause),
                "boot_count": boot_count,
                "boots_since_crash": record.boots_since_crash,
                "uptime_s": record.uptime.map(|u| u.to_f64_seconds()),
                "build_id": build_id,
                "message": record.message.as_str(),
                "fault": record.fault.as_ref().map(|f| format!("{f:?}")),
                "stack": record.stack.as_slice(),
            });
            writeln!(out, "{v}")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut text = Vec::new();
        print_reset_cause(Format::Text, &record, 1, &mut text).unwrap();
        insta::assert_snapshot!(String::from_utf8(text).unwrap());
    }

    #[test]
    fn json_with_a_message() {
        let record = record(Cause::Watchdog, "stuck in a loop");
        let mut json = Vec::new();
        print_reset_cause(Format::Json, &record, 4, &mut json).unwrap();
        insta::assert_snapshot!(String::from_utf8(json).unwrap());
    }
}
