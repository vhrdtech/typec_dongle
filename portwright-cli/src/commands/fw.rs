//! `donguru fw` - firmware version and update over USB, no debug probe (CLI-10, FW-12).

use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand};
use clap_complete::engine::ArgValueCompleter;
use donguru_core::update::{FirmwareImage, FirmwareVersion, UpdateReport, UpdateStep};
use indicatif::{ProgressBar, ProgressStyle};

use crate::completion;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::{self, Theme};

/// Where `cargo build --release` in a checkout of this repository puts the firmware.
const DEFAULT_ELF: &str = "fw/donguru/target/thumbv6m-none-eabi/release/donguru";

#[derive(Debug, Subcommand)]
pub enum Fw {
    /// Show the running firmware: name, version, git commit, build profile and API version
    Version,
    /// Write a firmware ELF over USB and wait for it to start
    ///
    /// Reboots the dongle into the STM32 ROM bootloader, erases, writes and verifies the image, starts it and
    /// prints the old and the new version. Takes about 45 seconds. The USB hub in the dongle stays up, so devices
    /// behind it are not disturbed until the new firmware starts.
    Update(Update),
}

#[derive(Debug, Args)]
pub struct Update {
    /// Firmware ELF, `-` for stdin [default: fw/donguru/target/thumbv6m-none-eabi/release/donguru]
    #[arg(value_name = "ELF", add = ArgValueCompleter::new(completion::elf_files))]
    elf: Option<PathBuf>,
}

pub fn run(ctx: &Context, cmd: Fw) -> anyhow::Result<ExitCode> {
    match cmd {
        Fw::Version => version(ctx),
        Fw::Update(args) => update(ctx, args),
    }
}

fn version(ctx: &Context) -> anyhow::Result<ExitCode> {
    let (entry, fw) = super::with_dongle(ctx, async |dongle| {
        let fw = dongle.device().firmware_version().await?;
        Ok((dongle.entry().clone(), fw))
    })?;
    Output::stdout(ctx).emit(&Version {
        api: entry.api_version.as_deref(),
        fw: &fw,
    })?;
    Ok(ExitCode::Success)
}

/// The running firmware and the API version it answers with.
struct Version<'a> {
    api: Option<&'a str>,
    fw: &'a FirmwareVersion,
}

impl Render for Version<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(out, "{}", th.value(self.fw))?;
        if let Some(api) = self.api {
            writeln!(out, "{} {}", th.key("donguru_api"), th.value(api))?;
        }
        Ok(())
    }

    fn json(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self.fw).unwrap_or_default();
        v["api_version"] = self.api.into();
        v
    }
}

fn update(ctx: &Context, args: Update) -> anyhow::Result<ExitCode> {
    let image = read_image(args.elf)?;
    let selector = super::selector(ctx)?;
    let bar = Bar::new(ctx);
    let report = super::runtime()?.block_on(async {
        let dongle = donguru_core::Dongle::open(selector.as_ref()).await?;
        dongle
            .update_firmware(&image, |step, done, total| bar.step(step, done, total))
            .await
    });
    bar.finish();
    let report = report?;
    Output::stdout(ctx).emit(&Report(&report))?;
    Ok(ExitCode::Success)
}

fn read_image(elf: Option<PathBuf>) -> anyhow::Result<FirmwareImage> {
    match elf {
        Some(p) if p.as_os_str() == "-" => {
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes)?;
            Ok(FirmwareImage::from_elf(&bytes)?)
        }
        Some(p) => Ok(FirmwareImage::from_elf_file(&p)?),
        None => {
            let p = PathBuf::from(DEFAULT_ELF);
            anyhow::ensure!(
                p.is_file(),
                "no firmware ELF given and {DEFAULT_ELF} does not exist here: `donguru fw update FILE.elf`"
            );
            Ok(FirmwareImage::from_elf_file(&p)?)
        }
    }
}

/// What an update did: the old and the new firmware.
struct Report<'a>(&'a UpdateReport);

impl Render for Report<'_> {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let r = self.0;
        let old = r
            .old
            .as_ref()
            .map_or("(unknown)".to_string(), |o| o.to_string());
        writeln!(
            out,
            "{}: {} -> {}",
            th.key(&r.serial),
            th.dim(old),
            th.ok(&r.new)
        )?;
        writeln!(out, "{} bytes in {:.1} s", th.value(r.bytes), r.seconds)
    }

    fn json(&self) -> serde_json::Value {
        serde_json::to_value(self.0).unwrap_or_default()
    }
}

/// The progress bar on stderr: hidden when stderr is not a terminal or with `-q`; plain without colour.
struct Bar {
    bar: ProgressBar,
}

impl Bar {
    fn new(ctx: &Context) -> Self {
        let colour = theme::colour_enabled(ctx.color, std::io::stderr().is_terminal());
        if ctx.quiet || !std::io::stderr().is_terminal() {
            return Bar {
                bar: ProgressBar::hidden(),
            };
        }
        let bar = ProgressBar::new(1);
        let bar_style = if colour {
            "{bar:30.cyan/blue}"
        } else {
            "{bar:30}"
        };
        bar.set_style(
            ProgressStyle::with_template(&format!(
                "{{msg:<28}} {bar_style} {{bytes}}/{{total_bytes}} {{elapsed}}"
            ))
            .expect("valid template"),
        );
        bar.enable_steady_tick(Duration::from_millis(120));
        Bar { bar }
    }

    fn step(&self, step: UpdateStep, done: u64, total: u64) {
        self.bar.set_message(step.to_string());
        match step {
            UpdateStep::Erase | UpdateStep::Write | UpdateStep::Verify => {
                self.bar.set_length(total);
                self.bar.set_position(done);
            }
            _ => {
                self.bar.set_length(1);
                self.bar.set_position(0);
            }
        }
    }

    fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Format;
    use crate::output::render;

    fn fw() -> FirmwareVersion {
        FirmwareVersion {
            name: "donguru".into(),
            version: "0.20.18".into(),
            git: Some("841ef1f".into()),
            dirty: true,
            profile: "release".into(),
        }
    }

    #[test]
    fn version_text_and_json() {
        let fw = fw();
        let v = Version {
            api: Some("0.5.1"),
            fw: &fw,
        };
        assert_eq!(
            render(Format::Text, &v),
            "donguru 0.20.18 (841ef1f-dirty, release)\ndonguru_api 0.5.1\n"
        );
        let v: serde_json::Value = serde_json::from_str(&render(Format::Json, &v)).unwrap();
        assert_eq!(v["version"], "0.20.18");
        assert_eq!(v["git"], "841ef1f");
        assert_eq!(v["api_version"], "0.5.1");
    }

    #[test]
    fn report_prints_old_and_new() {
        let r = UpdateReport {
            serial: "204B319B5431".into(),
            old: Some(fw()),
            new: FirmwareVersion {
                version: "0.20.19".into(),
                dirty: false,
                ..fw()
            },
            bytes: 415_000,
            seconds: 43.21,
        };
        assert_eq!(
            render(Format::Text, &Report(&r)),
            "204B319B5431: donguru 0.20.18 (841ef1f-dirty, release) -> donguru 0.20.19 (841ef1f, release)\n415000 bytes in 43.2 s\n"
        );
        let v: serde_json::Value =
            serde_json::from_str(&render(Format::Json, &Report(&r))).unwrap();
        assert_eq!(v["new"]["version"], "0.20.19");
        assert_eq!(v["serial"], "204B319B5431");
    }
}
