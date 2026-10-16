//! `donguru device` - enumerate and inspect dongles.

use std::io::Write;
use std::path::Path;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::project_config;
use clap::Subcommand;
use donguru_core::levels::DeviceSummary;
use donguru_core::{DeviceEntry, Dongle, Selector};

mod counters;
mod label;
mod log;
mod reset_cause;

pub use counters::Counters;
pub use label::Label;
pub use log::Log;
pub use reset_cause::ResetCause;

#[derive(Debug, Subcommand)]
pub enum Device {
    /// List connected dongles
    #[command(alias = "ls")]
    List,
    /// Show identity, serial and firmware version
    Info,
    /// Persist a default device for this project
    ///
    /// Resolves SELECTOR against the connected dongles and writes its serial number to the `device` key of
    /// `./donguru.toml`, so later commands in this directory (and below it) pick it without `--device`.
    Select {
        /// Device selector (serial number, index or USB path)
        selector: String,
    },
    /// Show or set the user label (`UserInfo`): a name and free-text notes
    Label(Label),
    /// Stored defmt log, decoded with the firmware's ELF (API-10)
    Log(Log),
    /// `cnt` counters, decoded with the firmware's ELF (API-9)
    Counters(Counters),
    /// Cause of the last reset, and the crash record if there is one (API-9)
    ResetCause(ResetCause),
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
            let selector = selector(ctx)?;
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
            let parsed: Selector = selector.parse()?;
            let entries = runtime()?.block_on(donguru_core::device::list())?;
            let entry = donguru_core::device::select(&entries, Some(&parsed))?;
            let pin = Selector::Serial(entry.serial.clone());
            let path = project_config::set_device_selector(&pin.to_string())?;
            print_select(ctx.format, &pin, &path, &mut std::io::stdout().lock())?;
            Ok(ExitCode::Success)
        }
        Device::Label(cmd) => label::run(ctx, cmd),
        Device::Log(cmd) => log::run(ctx, cmd),
        Device::Counters(cmd) => counters::run(ctx, cmd),
        Device::ResetCause(cmd) => reset_cause::run(ctx, cmd),
    }
}

fn print_select(
    format: Format,
    pin: &Selector,
    path: &Path,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match format {
        Format::Text => writeln!(out, "pinned {pin} in {}", path.display())?,
        Format::Json | Format::Jsonl => writeln!(
            out,
            "{}",
            serde_json::json!({ "device": pin.to_string(), "path": path })
        )?,
    }
    Ok(())
}

/// The device selector from `--device` / `DONGURU_DEVICE` / `./donguru.toml`, parsed; `None` auto-selects the only
/// connected dongle.
pub(crate) fn selector(ctx: &Context) -> anyhow::Result<Option<Selector>> {
    Ok(ctx
        .device
        .as_deref()
        .map(str::parse::<Selector>)
        .transpose()?)
}

/// Opens the selected dongle, for the subcommands that need one call and then close it.
pub(crate) async fn open(ctx: &Context) -> anyhow::Result<Dongle> {
    let selector = selector(ctx)?;
    Ok(Dongle::open(selector.as_ref()).await?)
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

/// Shared by `dg info` and `dg device info`: the device contract in one read (firmware, board/variant, uid,
/// label, last reset; CLI-7).
pub(crate) fn print_info(
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
    let last_reset = s.last_reset.as_ref().map(|r| format!("{:?}", r.cause));
    match format {
        Format::Text => {
            let missing = "(not in this firmware)".to_string();
            writeln!(out, "path        {}", entry.path)?;
            writeln!(out, "serial      {}", entry.serial)?;
            writeln!(out, "variant     {variant}")?;
            writeln!(out, "board       {}", board.as_ref().unwrap_or(&missing))?;
            writeln!(out, "firmware    {}", firmware.as_ref().unwrap_or(&missing))?;
            writeln!(
                out,
                "api         {}",
                entry.api_version.as_deref().unwrap_or("?")
            )?;
            writeln!(out, "uid         {}", s.uid.as_ref().unwrap_or(&missing))?;
            writeln!(out, "label       {}", s.label.as_ref().unwrap_or(&missing))?;
            writeln!(
                out,
                "last reset  {}",
                last_reset.as_ref().unwrap_or(&missing)
            )?;
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
                "last_reset_cause": last_reset,
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
        insta::assert_snapshot!(String::from_utf8(text).unwrap());

        let mut json = Vec::new();
        print_info(Format::Json, &entry, &summary, &mut json).unwrap();
        insta::assert_snapshot!(String::from_utf8(json).unwrap());
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
