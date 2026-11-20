//! `donguru device label` - the user label (`UserInfo`): a short name and free-text notes (CLI-19, API-9).

use std::io::{self, Write};

use clap::Args;

use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;

#[derive(Debug, Args)]
pub struct Label {
    /// New name, at most 32 bytes; omit to show the current name and notes
    name: Option<String>,

    /// New notes, at most 256 bytes
    #[arg(long)]
    notes: Option<String>,
}

pub fn run(ctx: &Context, args: Label) -> anyhow::Result<ExitCode> {
    super::runtime()?.block_on(async {
        let dongle = super::open(ctx).await?;
        let device = dongle.device();
        if let Some(name) = &args.name {
            device.set_label(name).await?;
        }
        if let Some(notes) = &args.notes {
            device.set_notes(notes).await?;
        }
        let name = device.label().await?;
        let notes = device.notes().await?;
        dongle.close().await?;
        Output::stdout(ctx).emit(&Shown { name, notes })?;
        Ok::<_, anyhow::Error>(ExitCode::Success)
    })
}

/// The label after any change: name and notes (`None` from a firmware without notes).
struct Shown {
    name: String,
    notes: Option<String>,
}

impl Render for Shown {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(
            out,
            "{}   {}",
            th.key("name"),
            th.value(format!("{:?}", self.name))
        )?;
        match self.notes.as_deref() {
            Some(notes) if !notes.is_empty() => {
                writeln!(
                    out,
                    "{}  {}",
                    th.key("notes"),
                    th.value(format!("{notes:?}"))
                )
            }
            Some(_) => writeln!(out, "{}  {}", th.key("notes"), th.dim("(empty)")),
            None => writeln!(
                out,
                "{}  {}",
                th.key("notes"),
                th.dim("(not in this firmware)")
            ),
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "name": self.name, "notes": self.notes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Format;
    use crate::output::render;

    fn shown(notes: Option<&str>) -> Shown {
        Shown {
            name: "bench".into(),
            notes: notes.map(Into::into),
        }
    }

    #[test]
    fn text_with_notes() {
        insta::assert_snapshot!(render(Format::Text, &shown(Some("on the shelf"))));
    }

    #[test]
    fn json_without_notes() {
        insta::assert_snapshot!(render(Format::Json, &shown(None)));
    }
}
