//! `donguru device` - enumerate and inspect dongles.

use std::io::{self, Write};
use std::path::PathBuf;

use crate::cli::Format;
use crate::completion;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::project_config;
use crate::theme::Theme;
use clap::Subcommand;
use clap_complete::engine::ArgValueCandidates;
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
        #[arg(add = ArgValueCandidates::new(completion::devices))]
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
            Output::stdout(ctx).emit(&List(&entries))?;
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
            Output::stdout(ctx).emit(&Info::new(&entry, &summary))?;
            Ok(ExitCode::Success)
        }
        Device::Select { selector } => {
            let parsed: Selector = selector.parse()?;
            let entries = runtime()?.block_on(donguru_core::device::list())?;
            let entry = donguru_core::device::select(&entries, Some(&parsed))?;
            let pin = Selector::Serial(entry.serial.clone());
            let path = project_config::set_device_selector(&pin.to_string())?;
            Output::stdout(ctx).emit(&Selected { pin, path })?;
            Ok(ExitCode::Success)
        }
        Device::Label(cmd) => label::run(ctx, cmd),
        Device::Log(cmd) => log::run(ctx, cmd),
        Device::Counters(cmd) => counters::run(ctx, cmd),
        Device::ResetCause(cmd) => reset_cause::run(ctx, cmd),
    }
}

/// `device select`: the selector written and the file it went to.
struct Selected {
    pin: Selector,
    path: PathBuf,
}

impl Render for Selected {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(
            out,
            "pinned {} in {}",
            th.value(&self.pin),
            th.value(self.path.display())
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "device": self.pin.to_string(), "path": self.path })
    }
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

/// `device list`: a table in text (nothing at all when no dongle is connected), an array in json, a line per
/// dongle in jsonl.
struct List<'a>(&'a [DeviceEntry]);

impl Render for List<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let entries = self.0;
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
            "{:<5} {:<serial_w$} {:<path_w$} {:<7} {}",
            th.heading("INDEX"),
            th.heading("SERIAL"),
            th.heading("PATH"),
            th.heading("API"),
            th.heading("LABEL")
        )?;
        for e in entries {
            let api = match e.api_version.as_deref() {
                Some(api) => th.value(api),
                None => th.warn("?"),
            };
            writeln!(
                out,
                "{:<5} {:<serial_w$} {:<path_w$} {api:<7} {}",
                e.index,
                th.value(&e.serial),
                e.path,
                th.key(&e.label)
            )?;
        }
        Ok(())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::Value::Array(self.0.iter().map(entry_json).collect())
    }

    fn jsonl(&self) -> Vec<serde_json::Value> {
        self.0.iter().map(entry_json).collect()
    }
}

/// Shared by `dg info` and `dg device info`: the device contract in one read (firmware, board/variant, uid,
/// label, last reset; CLI-7).
pub(crate) struct Info {
    path: String,
    serial: String,
    api_version: Option<String>,
    variant: String,
    board: Option<String>,
    firmware: Option<String>,
    uid: Option<String>,
    label: Option<String>,
    last_reset: Option<String>,
}

impl Info {
    pub(crate) fn new(entry: &DeviceEntry, s: &DeviceSummary) -> Self {
        Info {
            path: entry.path.clone(),
            serial: entry.serial.clone(),
            api_version: entry.api_version.clone(),
            variant: format!("{:?}", s.variant),
            board: s
                .board
                .as_ref()
                .map(|b| format!("{} rev {}", b.name.as_str(), b.revision.as_str())),
            firmware: s.firmware.as_ref().map(|f| {
                let v = &f.version;
                format!("{} {}.{}.{}", f.name.as_str(), v.major, v.minor, v.patch)
            }),
            uid: s.uid.as_ref().map(ToString::to_string),
            label: s.label.as_ref().map(ToString::to_string),
            last_reset: s.last_reset.as_ref().map(|r| format!("{:?}", r.cause)),
        }
    }
}

impl Render for Info {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let row = |out: &mut dyn Write, key: &str, value: Option<&str>| match value {
            Some(v) => writeln!(out, "{:<12}{}", th.key(key), th.value(v)),
            None => writeln!(
                out,
                "{:<12}{}",
                th.key(key),
                th.dim("(not in this firmware)")
            ),
        };
        row(out, "path", Some(&self.path))?;
        row(out, "serial", Some(&self.serial))?;
        row(out, "variant", Some(&self.variant))?;
        row(out, "board", self.board.as_deref())?;
        row(out, "firmware", self.firmware.as_deref())?;
        writeln!(
            out,
            "{:<12}{}",
            th.key("api"),
            th.value(self.api_version.as_deref().unwrap_or("?"))
        )?;
        row(out, "uid", self.uid.as_deref())?;
        row(out, "label", self.label.as_deref())?;
        row(out, "last reset", self.last_reset.as_deref())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "serial": self.serial,
            "variant": self.variant,
            "board": self.board,
            "firmware": self.firmware,
            "api_version": self.api_version,
            "uid": self.uid,
            "label": self.label,
            "last_reset_cause": self.last_reset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::render;
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
        let info = Info::new(&entry, &summary);
        insta::assert_snapshot!(render(Format::Text, &info));
        insta::assert_snapshot!(render(Format::Json, &info));
    }

    #[test]
    fn empty_list_prints_nothing_on_stdout() {
        assert_eq!(render(Format::Text, &List(&[])), "");
        assert_eq!(render(Format::Json, &List(&[])), "[]\n");
        assert_eq!(render(Format::Jsonl, &List(&[])), "");
    }
}
