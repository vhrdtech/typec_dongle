//! `donguru device` - enumerate and inspect dongles.

use std::io::Write;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use clap::Subcommand;
use donguru_core::levels::DeviceSummary;
use donguru_core::{DeviceEntry, Dongle, Selector};

#[derive(Debug, Subcommand)]
pub enum Device {
    /// List connected dongles
    #[command(alias = "ls")]
    List,
    /// Show identity, serial and firmware version
    Info,
    /// Persist a default device for this project
    Select {
        /// Device selector (serial number, index or USB path)
        selector: String,
    },
}

pub fn run(ctx: &Context, command: Device) -> anyhow::Result<ExitCode> {
    match command {
        Device::List => {
            let entries = runtime()?.block_on(donguru_core::device::list())?;
            print_list(ctx.format, &entries, &mut std::io::stdout().lock())?;
            if entries.is_empty() && ctx.format == Format::Text && !ctx.quiet {
                eprintln!("donguru: no Donguru connected");
            }
            Ok(ExitCode::Success)
        }
        Device::Info => {
            let selector = ctx
                .device
                .as_deref()
                .map(str::parse::<Selector>)
                .transpose()?;
            let (entry, summary) = runtime()?.block_on(async {
                let dongle = Dongle::open(selector.as_ref()).await?;
                let summary = dongle.device().summary().await?;
                let entry = dongle.entry().clone();
                dongle.close().await?;
                Ok::<_, donguru_core::Error>((entry, summary))
            })?;
            print_info(ctx.format, &entry, &summary, &mut std::io::stdout().lock())?;
            Ok(ExitCode::Success)
        }
        Device::Select { selector } => {
            super::stub(&format!("device select {selector}"), ExitCode::Failure)
        }
    }
}

/// The async runtime of one command: the WireWeaver client runs its event loop on it.
pub(crate) fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?)
}

fn entry_json(e: &DeviceEntry) -> serde_json::Value {
    serde_json::json!({
        "index": e.index,
        "serial": e.serial,
        "path": e.path,
        "api_version": e.api_version,
        "label": e.label,
    })
}

fn print_list(format: Format, entries: &[DeviceEntry], out: &mut impl Write) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            if entries.is_empty() {
                return Ok(());
            }
            let path_w = entries
                .iter()
                .map(|e| e.path.len())
                .max()
                .unwrap_or(0)
                .max(4);
            let serial_w = entries
                .iter()
                .map(|e| e.serial.len())
                .max()
                .unwrap_or(0)
                .max(6);
            writeln!(
                out,
                "{:<5} {:<serial_w$} {:<path_w$} {:<7} LABEL",
                "INDEX", "SERIAL", "PATH", "API"
            )?;
            for e in entries {
                let api = e.api_version.as_deref().unwrap_or("?");
                writeln!(
                    out,
                    "{:<5} {:<serial_w$} {:<path_w$} {api:<7} {}",
                    e.index, e.serial, e.path, e.label
                )?;
            }
        }
        Format::Json => {
            let list: Vec<_> = entries.iter().map(entry_json).collect();
            writeln!(out, "{}", serde_json::Value::Array(list))?;
        }
        Format::Jsonl => {
            for e in entries {
                writeln!(out, "{}", entry_json(e))?;
            }
        }
    }
    Ok(())
}

fn print_info(
    format: Format,
    entry: &DeviceEntry,
    s: &DeviceSummary,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let variant = format!("{:?}", s.variant);
    let firmware = s.firmware.as_ref().map(|f| {
        let v = &f.version;
        format!("{} {}.{}.{}", f.name.as_str(), v.major, v.minor, v.patch)
    });
    let board = s
        .board
        .as_ref()
        .map(|b| format!("{} rev {}", b.name.as_str(), b.revision.as_str()));
    match format {
        Format::Text => {
            let missing = "(not in this firmware)".to_string();
            writeln!(out, "path      {}", entry.path)?;
            writeln!(out, "serial    {}", entry.serial)?;
            writeln!(out, "variant   {variant}")?;
            writeln!(out, "board     {}", board.as_ref().unwrap_or(&missing))?;
            writeln!(out, "firmware  {}", firmware.as_ref().unwrap_or(&missing))?;
            writeln!(
                out,
                "api       {}",
                entry.api_version.as_deref().unwrap_or("?")
            )?;
            writeln!(out, "uid       {}", s.uid.as_ref().unwrap_or(&missing))?;
            writeln!(out, "label     {}", s.label.as_ref().unwrap_or(&missing))?;
        }
        Format::Json | Format::Jsonl => {
            let v = serde_json::json!({
                "path": entry.path,
                "serial": entry.serial,
                "variant": variant,
                "board": board,
                "firmware": firmware,
                "api_version": entry.api_version,
                "uid": s.uid,
                "label": s.label,
            });
            writeln!(out, "{v}")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::mock::MockDevice;

    #[test]
    fn info_from_the_mock() {
        let mock = MockDevice::start("cli/device_info");
        let rt = runtime().unwrap();
        let (entry, summary) = rt.block_on(async {
            let dongle = mock.open().await.unwrap();
            dongle.device().set_label("bench").await.unwrap();
            (
                dongle.entry().clone(),
                dongle.device().summary().await.unwrap(),
            )
        });
        let mut text = Vec::new();
        print_info(Format::Text, &entry, &summary, &mut text).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("variant   Main"), "{text}");
        assert!(text.contains("board     Donguru B153 rev A"), "{text}");
        assert!(text.contains("label     bench"), "{text}");

        let mut json = Vec::new();
        print_info(Format::Json, &entry, &summary, &mut json).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(json["variant"], "Main");
        assert_eq!(json["uid"], "0049002A3133510B37363436");
    }

    #[test]
    fn empty_list_prints_nothing_on_stdout() {
        let mut out = Vec::new();
        print_list(Format::Text, &[], &mut out).unwrap();
        assert!(out.is_empty());
        print_list(Format::Json, &[], &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "[]\n");
    }
}
