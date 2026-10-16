//! `donguru device label` - the user label (`UserInfo`): a short name and free-text notes (CLI-19, API-9).

use std::io::Write;

use clap::Args;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;

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
        print_label(
            ctx.format,
            &name,
            notes.as_deref(),
            &mut std::io::stdout().lock(),
        )?;
        Ok(ExitCode::Success)
    })
}

fn print_label(
    format: Format,
    name: &str,
    notes: Option<&str>,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match format {
        Format::Text => {
            writeln!(out, "name   {name:?}")?;
            match notes {
                Some(notes) if !notes.is_empty() => writeln!(out, "notes  {notes:?}")?,
                Some(_) => writeln!(out, "notes  (empty)")?,
                None => writeln!(out, "notes  (not in this firmware)")?,
            }
        }
        Format::Json | Format::Jsonl => writeln!(
            out,
            "{}",
            serde_json::json!({ "name": name, "notes": notes })
        )?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_with_notes() {
        let mut text = Vec::new();
        print_label(Format::Text, "bench", Some("on the shelf"), &mut text).unwrap();
        insta::assert_snapshot!(String::from_utf8(text).unwrap());
    }

    #[test]
    fn json_without_notes() {
        let mut json = Vec::new();
        print_label(Format::Json, "bench", None, &mut json).unwrap();
        insta::assert_snapshot!(String::from_utf8(json).unwrap());
    }
}
