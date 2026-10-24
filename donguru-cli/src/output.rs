//! One renderer for every command's result (CLI-15): a command builds a typed value that implements [Render] and
//! hands it to [Output], which writes `--format text` (through the [Theme], CLI-51), `json` (one value) or `jsonl`
//! (one value per line) to stdout. A failed command is reported by [report_error] on stderr in the same format
//! (CLI-52).
//!
//! The JSON shape of each value is part of the CLI's contract: scripts read it, so a renderer change that alters
//! it needs a CHANGELOG entry. The snapshot tests next to each command pin both forms.

use std::io::{self, Write};

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::theme::Theme;

/// A command result that can be shown in every `--format`.
pub trait Render {
    /// The human-readable form, `--format text`, with colour per `theme`.
    fn text(&self, out: &mut dyn Write, theme: &Theme) -> io::Result<()>;

    /// The machine-readable form: the one value `--format json` prints.
    fn json(&self) -> serde_json::Value;

    /// The records of `--format jsonl`, one per line: the [json](Render::json) value itself, or for a list its
    /// items.
    fn jsonl(&self) -> Vec<serde_json::Value> {
        vec![self.json()]
    }
}

/// Where a command's result goes: stdout in the selected format.
pub struct Output<W: Write> {
    format: Format,
    theme: Theme,
    out: W,
}

impl Output<io::StdoutLock<'static>> {
    /// stdout in the format and colour of `--format` / `--color`.
    pub fn stdout(ctx: &Context) -> Self {
        let theme = match ctx.format {
            Format::Text => Theme::for_stdout(ctx.color),
            Format::Json | Format::Jsonl => Theme::plain(),
        };
        Output::new(ctx.format, theme, io::stdout().lock())
    }
}

impl<W: Write> Output<W> {
    pub fn new(format: Format, theme: Theme, out: W) -> Self {
        Output { format, theme, out }
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// The whole result of a command.
    pub fn emit(&mut self, value: &dyn Render) -> anyhow::Result<()> {
        match self.format {
            Format::Text => value.text(&mut self.out, &self.theme)?,
            Format::Json => writeln!(self.out, "{}", value.json())?,
            Format::Jsonl => {
                for record in value.jsonl() {
                    writeln!(self.out, "{record}")?;
                }
            }
        }
        self.out.flush()?;
        Ok(())
    }

    /// One record of a stream (`power watch`, `device log --follow`): text, or one json line, flushed at once so
    /// a reader sees it live. Streaming commands refuse `--format json` before they start.
    pub fn record(&mut self, value: &dyn Render) -> anyhow::Result<()> {
        match self.format {
            Format::Text => value.text(&mut self.out, &self.theme)?,
            Format::Json | Format::Jsonl => writeln!(self.out, "{}", value.json())?,
        }
        self.out.flush()?;
        Ok(())
    }

    /// The writer, for tests.
    pub fn into_inner(self) -> W {
        self.out
    }
}

/// How `main` reports a failed command on stderr (CLI-52): under `--format text` one line, `donguru: error: ...`;
/// under `json` / `jsonl` one json object, so a script that asked for json gets json either way. stdout stays
/// data only: a `jsonl` stream that fails halfway has only records on stdout.
pub fn report_error(
    format: Format,
    err: &anyhow::Error,
    exit: ExitCode,
    out: &mut impl Write,
) -> io::Result<()> {
    match format {
        Format::Text => writeln!(out, "donguru: error: {err:#}"),
        Format::Json | Format::Jsonl => writeln!(out, "{}", error_json(err, exit)),
    }
}

/// `{"error": {"exit": "device_error", "code": 4, "message": "...", "kind": "refused"}}`: the shape documented in
/// `docs/user/scripting.md`. `kind` only for an error the dongle answered with (or an item its firmware lacks).
pub fn error_json(err: &anyhow::Error, exit: ExitCode) -> serde_json::Value {
    let mut e = serde_json::json!({
        "exit": exit.name(),
        "code": exit.code(),
        "message": format!("{err:#}"),
    });
    if let Some(kind) = err
        .downcast_ref::<donguru_core::Error>()
        .and_then(donguru_core::Error::kind_name)
    {
        e["kind"] = serde_json::json!(kind);
    }
    serde_json::json!({ "error": e })
}

/// Renders `value` as a string, without colour: for tests and snapshots.
pub fn render(format: Format, value: &dyn Render) -> String {
    let mut out = Output::new(format, Theme::plain(), Vec::new());
    // writing into a Vec can't fail
    let _ = out.emit(value);
    String::from_utf8_lossy(&out.into_inner()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct List(Vec<u32>);

    impl Render for List {
        fn text(&self, out: &mut dyn Write, theme: &Theme) -> io::Result<()> {
            for n in &self.0 {
                writeln!(out, "{}", theme.value(n))?;
            }
            Ok(())
        }

        fn json(&self) -> serde_json::Value {
            serde_json::json!(self.0)
        }

        fn jsonl(&self) -> Vec<serde_json::Value> {
            self.0.iter().map(|n| serde_json::json!(n)).collect()
        }
    }

    #[test]
    fn one_value_three_formats() {
        let l = List(vec![1, 2]);
        assert_eq!(render(Format::Text, &l), "1\n2\n");
        assert_eq!(render(Format::Json, &l), "[1,2]\n");
        assert_eq!(render(Format::Jsonl, &l), "1\n2\n");
    }

    #[test]
    fn errors_follow_the_format() {
        let err = anyhow::Error::new(donguru_core::Error::NoDevice("none connected".into()));
        let mut text = Vec::new();
        report_error(Format::Text, &err, ExitCode::NoDevice, &mut text).unwrap();
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "donguru: error: no matching device: none connected\n"
        );

        let mut json = Vec::new();
        report_error(Format::Jsonl, &err, ExitCode::NoDevice, &mut json).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "error": {
                "exit": "no_device",
                "code": 3,
                "message": "no matching device: none connected",
            }})
        );
    }

    #[test]
    fn device_errors_carry_their_kind() {
        let err = anyhow::Error::new(donguru_core::Error::Unimplemented {
            item: "Meter::reading".into(),
        })
        .context("power meter");
        let v = error_json(&err, ExitCode::DeviceError);
        assert_eq!(v["error"]["kind"], "unimplemented");
        assert_eq!(v["error"]["exit"], "device_error");
        assert_eq!(
            v["error"]["message"],
            "power meter: Meter::reading is not implemented by the device's firmware"
        );
    }
}
