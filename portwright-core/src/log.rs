//! `device.log`'s stored defmt frames (`ww_log::LogDefmt`), decoded with the firmware's ELF found by its build id
//! (CLI-19, API-10).

use std::path::Path;
use std::time::Duration;

use defmt_decoder::{DecodeError, StreamDecoder, Table};
use wire_weaver::prelude::UVlq64;
use wire_weaver_client::StreamEvent;
use wire_weaver_client::ww_client_server::stream::StreamCursor;

use crate::client::generated::donguru_api_donguru_api::donguru_api_device::DonguruApiDeviceClient;
use crate::levels::read;
use crate::{Error, Result};

/// What `device log` prints.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct LogReport {
    /// One line per message, plus a `--- N log messages lost ---` marker where the device dropped some.
    pub lines: Vec<String>,
    pub lost: u64,
    /// Something worth telling the user: the ELF couldn't be checked, or doesn't match (`--force`).
    pub warning: Option<String>,
}

/// Longest wait for the next frame once the log has started arriving.
const IDLE: Duration = Duration::from_millis(1500);

fn load_table(path: &Path) -> Result<Table> {
    let bytes = std::fs::read(path).map_err(|e| Error::Link(format!("{}: {e}", path.display())))?;
    match Table::parse(&bytes) {
        Ok(Some(t)) => Ok(t),
        Ok(None) => Err(Error::Link(format!(
            "{} has no defmt table",
            path.display()
        ))),
        Err(e) => Err(Error::Link(format!("{}: {e:#}", path.display()))),
    }
}

/// The decoded table to read the log with, and a warning worth telling the user (ELF not checked, or forced).
async fn resolve_table(
    client: &DonguruApiDeviceClient<'_>,
    elf: Option<&Path>,
    force: bool,
) -> Result<(Table, Option<String>)> {
    let info = read("LogDefmt::DEFMT", client.log().read_DEFMT()).await?;
    let build_id: Vec<u8> = info.build_id.as_slice().to_vec();
    let resolved = cnt_core::elf::resolve(elf, Some(&build_id), force)
        .map_err(|e| Error::Link(format!("resolving the firmware ELF: {e:#}")))?;
    let table = load_table(&resolved.path)?;
    Ok((table, resolved.warning))
}

/// Opens the device's current log session from its start (position 0): `None` when it hasn't logged anything
/// since boot (no session yet).
async fn resume_from_start(
    client: &DonguruApiDeviceClient<'_>,
) -> Result<
    Option<(
        wire_weaver_client::PositionedStream<ww_log::DefmtFramesOwned>,
        u64,
    )>,
> {
    let status = read("LogDefmt::frames_status", client.log().frames_status()).await?;
    let Some(session) = status.session else {
        return Ok(None);
    };
    let cursor = StreamCursor {
        session,
        pos: UVlq64(0),
    };
    let frames = client
        .log()
        .frames_resume(cursor)
        .call()
        .await
        .map_err(|e| Error::client("LogDefmt::frames_resume", e))?;
    Ok(Some((frames, status.next.0)))
}

/// Feeds one stream item's bytes to `decoder`, appending every whole frame it yields to `lines`.
fn decode_item(bytes: &[u8], decoder: &mut dyn StreamDecoder, lines: &mut Vec<String>) {
    decoder.received(bytes);
    loop {
        match decoder.decode() {
            Ok(frame) => lines.push(frame.display(false).to_string()),
            Err(DecodeError::UnexpectedEof) => break,
            Err(DecodeError::Malformed) => {
                lines.push("--- malformed frame (wrong ELF?) ---".to_string());
                break;
            }
        }
    }
}

/// One pass over the log's store from the start, decoding whatever is in it right now. Doesn't move the device's
/// own read cursor (a fresh resume from position 0 each time), so running it again shows the same history.
pub async fn dump(
    client: DonguruApiDeviceClient<'_>,
    elf: Option<&Path>,
    force: bool,
) -> Result<LogReport> {
    // check first whether there is anything to decode at all, so a device without a log session yet (API-10 not
    // implemented, or simply never logged) needs no ELF
    let Some((mut frames, end)) = resume_from_start(&client).await? else {
        return Ok(LogReport::default());
    };
    let mut report = LogReport::default();
    if end == 0 {
        return Ok(report);
    }
    let (table, warning) = resolve_table(&client, elf, force).await?;
    report.warning = warning;
    let mut decoder = table.new_stream_decoder();
    let mut covered = 0u64;
    while covered < end {
        let event = match tokio::time::timeout(IDLE, frames.recv_event()).await {
            Ok(Ok(event)) => event,
            Ok(Err(e)) => {
                report.lines.push(format!("--- log stream ended: {e} ---"));
                break;
            }
            Err(_) => {
                report
                    .lines
                    .push("--- the device stopped sending its log ---".to_string());
                break;
            }
        };
        match event {
            StreamEvent::Item { value, units, .. } => {
                decode_item(value.bytes.as_slice(), &mut *decoder, &mut report.lines);
                covered += units;
            }
            StreamEvent::Gap { units, cause, .. } => {
                report.lost += units;
                report
                    .lines
                    .push(format!("--- {units} log messages lost ({cause:?}) ---"));
                covered += units;
            }
            StreamEvent::Closed | StreamEvent::Disconnected => break,
            _ => {}
        }
    }
    Ok(report)
}

/// Like [dump], then keeps decoding new frames as they arrive and hands each line to `on_line`, until the link
/// drops or the device closes the log (`Ctrl+C` is how a CLI leaves this). `on_warning` runs once first, if the
/// ELF couldn't be checked or doesn't match (`--force`).
pub async fn follow(
    client: DonguruApiDeviceClient<'_>,
    elf: Option<&Path>,
    force: bool,
    mut on_line: impl FnMut(&str),
    on_warning: impl FnOnce(&str),
) -> Result<()> {
    let Some((mut frames, _end)) = resume_from_start(&client).await? else {
        return Ok(());
    };
    let (table, warning) = resolve_table(&client, elf, force).await?;
    if let Some(w) = &warning {
        on_warning(w);
    }
    let mut decoder = table.new_stream_decoder();
    loop {
        let event = frames
            .recv_event()
            .await
            .map_err(|e| Error::Link(format!("log stream: {e}")))?;
        match event {
            StreamEvent::Item { value, .. } => {
                let mut lines = vec![];
                decode_item(value.bytes.as_slice(), &mut *decoder, &mut lines);
                for line in &lines {
                    on_line(line);
                }
            }
            StreamEvent::Gap { units, cause, .. } => {
                on_line(&format!("--- {units} log messages lost ({cause:?}) ---"));
            }
            StreamEvent::Closed | StreamEvent::Disconnected => return Ok(()),
            _ => {}
        }
    }
}
