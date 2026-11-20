//! `donguru pd` - the USB PD source on the plug (`plug.pd`, API-30): start and stop it, show where it stands, log
//! its messages (`plug.pd.messages`) as `pd_log` JSON lines (LOG-1), decoded on screen the same way `pd-log print`
//! does.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;
use clap::Subcommand;
use donguru_core::api::{CcPolarity, PdEventOwned, PdRecordOwned, PdSource, PdState};
use donguru_core::client::observe::StreamEvent;
use std::time::Duration;
use ww_si::{Ampere, Second};

#[derive(Debug, Subcommand)]
pub enum Pd {
    /// Show the PD source's setting and where it stands
    Status,
    /// Start or stop the PD source on the plug
    #[command(subcommand)]
    Source(SourceCmd),
    /// Log the source's messages as `pd_log` JSON lines (LOG-1), decoded on screen the same way `pd-log print`
    /// does; `pd-log merge` lines this up with a rePort sink's log by host time
    Log {
        /// Keep logging until interrupted (Ctrl-C), instead of stopping after `--for`
        #[arg(long)]
        follow: bool,
        /// Write `pd_log` JSON lines to this file as they come in, in addition to the screen
        #[arg(short = 'o', long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Stop the running source and start it again, to see a negotiation from the attach on
        #[arg(long)]
        restart: bool,
        /// Stop after this many seconds
        #[arg(long = "for", value_name = "SECONDS")]
        for_s: Option<f64>,
    },
}

#[derive(Debug, Subcommand)]
pub enum SourceCmd {
    /// Offer 5 V to the DUT over USB PD; plug VBUS comes on once a sink attaches
    On {
        /// Current the 5 V PDO offers, in A (0.1 to 3); no more than the computer side can give
        #[arg(long, default_value_t = 0.5)]
        current: f64,
        /// SenderResponseTimer in ms (24 to 1000) instead of the spec's 30 ms: out of spec, for a slow sink
        #[arg(long)]
        sender_response_ms: Option<u32>,
    },
    /// Stop the PD source: CC open, plug VBUS off
    Off,
}

fn cc(c: CcPolarity) -> &'static str {
    match c {
        CcPolarity::Cc1 => "CC1",
        CcPolarity::Cc2 => "CC2",
        _ => "unknown",
    }
}

struct Status {
    source: PdSource,
    state: PdState,
}

impl Status {
    fn source_text(&self) -> String {
        match &self.source {
            PdSource::Off => "off".into(),
            PdSource::Source {
                max_current,
                sender_response,
            } => {
                let mut s = format!("5 V at {:.2} A", amps(max_current));
                if let Some(t) = sender_response {
                    s += &format!(
                        ", SenderResponseTimer {:.0} ms (out of spec)",
                        t.to_f64() * 1000.0
                    );
                }
                s
            }
            _ => "unknown".into(),
        }
    }

    fn state_text(&self) -> (&'static str, String) {
        match &self.state {
            PdState::Off => ("off", String::new()),
            PdState::Unattached => ("unattached", "waiting for a sink".into()),
            PdState::Attached { cc: c } => ("attached", format!("{}, negotiating", cc(*c))),
            PdState::Contract { cc: c, contract } => (
                "contract",
                format!(
                    "{}, PDO {}, {:.2} V, {:.2} A (max {:.2} A)",
                    cc(*c),
                    contract.object_position,
                    contract.voltage.to_f64(),
                    amps(&contract.operating_current),
                    amps(&contract.max_current)
                ),
            ),
            PdState::NoPd { cc: c } => ("no-pd", format!("{}, a sink without PD, 5 V", cc(*c))),
            PdState::Failed { cc: c } => (
                "failed",
                format!("{}, VBUS off until the sink detaches", cc(*c)),
            ),
            _ => ("unknown", String::new()),
        }
    }
}

fn amps(a: &Ampere) -> f64 {
    a.to_f64()
}

impl Render for Status {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(
            out,
            "{}: {}",
            th.key("source"),
            th.value(&self.source_text())
        )?;
        let (state, detail) = self.state_text();
        let state = match self.state {
            PdState::Contract { .. } => th.ok(state),
            PdState::Failed { .. } => th.warn(state),
            _ => th.value(state),
        };
        if detail.is_empty() {
            writeln!(out, "{}: {}", th.key("state"), state)
        } else {
            writeln!(out, "{}: {} ({})", th.key("state"), state, detail)
        }
    }

    fn json(&self) -> serde_json::Value {
        let (state, _) = self.state_text();
        let mut v = serde_json::json!({ "state": state });
        match &self.source {
            PdSource::Source {
                max_current,
                sender_response,
            } => {
                v["source"] = serde_json::json!({
                    "max_current_a": amps(max_current),
                    "sender_response_ms": sender_response.as_ref().map(|t: &Second| t.to_f64() * 1000.0),
                });
            }
            _ => v["source"] = serde_json::Value::Null,
        }
        match &self.state {
            PdState::Attached { cc: c } | PdState::NoPd { cc: c } | PdState::Failed { cc: c } => {
                v["cc"] = cc(*c).into()
            }
            PdState::Contract { cc: c, contract } => {
                v["cc"] = cc(*c).into();
                v["contract"] = serde_json::json!({
                    "object_position": contract.object_position,
                    "voltage_v": contract.voltage.to_f64(),
                    "operating_current_a": amps(&contract.operating_current),
                    "max_current_a": amps(&contract.max_current),
                });
            }
            _ => {}
        }
        v
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// One `plug.pd.messages` record, as logged: a [`pd_log::Line`] for an actual PD message (decoded the same way
/// `pd-log print` does), or the plain event text for attach, detach and VBUS switching, which `pd_log` doesn't
/// model.
enum LogItem {
    Message(pd_log::Line),
    Event {
        time_s: f64,
        label: &'static str,
        detail: String,
    },
}

/// `record` as a [`LogItem`]: a [`pd_log::Line`] when it's a PD message (also returned, for `-o FILE`), decoded
/// with `decoder`; `None` of that for an attach/detach/VBUS event.
fn log_item(
    record: PdRecordOwned,
    host_time_us: u64,
    decoder: &mut pd_log::Decoder,
) -> (LogItem, Option<pd_log::Line>) {
    if let Some(log_record) = donguru_core::pd::log_record(host_time_us, &record) {
        let line = pd_log::Line::decode(log_record, decoder);
        return (LogItem::Message(line.clone()), Some(line));
    }
    let time_s = record.time.to_f64();
    let (label, detail) = match record.event {
        PdEventOwned::Attached { cc: c } => ("attached", cc(c).to_string()),
        PdEventOwned::Detached => ("detached", String::new()),
        PdEventOwned::Vbus { on } => ("vbus", if on { "on" } else { "off" }.to_string()),
        _ => ("unknown", String::new()),
    };
    (
        LogItem::Event {
            time_s,
            label,
            detail,
        },
        None,
    )
}

impl Render for LogItem {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        match self {
            LogItem::Event {
                time_s,
                label,
                detail,
            } => writeln!(out, "{:12.6}  {:<13} {}", time_s, th.value(label), detail),
            LogItem::Message(line) => {
                let r = &line.record;
                let dir = match r.direction {
                    pd_log::Direction::Rx => "rx",
                    pd_log::Direction::Tx => "tx",
                };
                let outcome = format!("{:?}", r.outcome);
                let outcome = match r.outcome {
                    pd_log::Outcome::Ok => th.value(outcome),
                    pd_log::Outcome::Discarded => th.warn(outcome),
                    pd_log::Outcome::RetryFail | pd_log::Outcome::HardReset => th.bad(outcome),
                };
                let what = match &line.message {
                    Some(m) => format!("{:?}", m.header.message_type()),
                    None => "undecoded".to_string(),
                };
                let device_s = r.device_time_us.map(|us| us as f64 / 1e6).unwrap_or(0.0);
                write!(
                    out,
                    "{device_s:12.6}  {:<9} {dir:<2} {outcome:<12} {what}",
                    th.value(r.source)
                )?;
                match r.raw.as_slice() {
                    [] => writeln!(out),
                    bytes => writeln!(out, "  [{}]", hex(bytes)),
                }
            }
        }
    }

    fn json(&self) -> serde_json::Value {
        match self {
            LogItem::Event {
                time_s,
                label,
                detail,
            } => serde_json::json!({ "t_s": time_s, "event": label, "detail": detail }),
            LogItem::Message(line) => serde_json::to_value(line).unwrap_or_default(),
        }
    }
}

fn log(
    ctx: &Context,
    follow: bool,
    out: Option<PathBuf>,
    restart: bool,
    for_s: Option<f64>,
) -> anyhow::Result<ExitCode> {
    if matches!(ctx.format, Format::Json) {
        anyhow::bail!("`pd log` streams data: use `--format text` or `--format jsonl`");
    }
    if !follow && for_s.is_none() {
        anyhow::bail!("`pd log` needs `--follow` (until Ctrl-C) or `--for SECONDS`");
    }
    let mut file = out
        .as_deref()
        .map(|path| -> anyhow::Result<_> {
            Ok(BufWriter::new(File::create(path).map_err(|e| {
                anyhow::anyhow!("creating {}: {e}", path.display())
            })?))
        })
        .transpose()?;
    let mut text_out = Output::stdout(ctx);
    let selector = super::selector(ctx)?;
    super::runtime()?.block_on(async {
        let dongle = donguru_core::Dongle::open(selector.as_ref()).await?;
        let plug = dongle.plug();
        let mut messages = plug.pd_messages().await?;
        let source = plug.pd_source().await?;
        if restart {
            if matches!(source, PdSource::Off) {
                anyhow::bail!("the PD source is off: `donguru pd source on` first");
            }
            plug.set_pd_source(PdSource::Off).await?;
            plug.set_pd_source(source.clone()).await?;
        }
        if let PdSource::Source {
            sender_response: Some(t),
            ..
        } = &source
        {
            // `writeln!`, not `eprintln!`: a closed stderr is no reason to stop the capture
            let _ = writeln!(
                io::stderr(),
                "donguru: SenderResponseTimer overridden to {:.0} ms (out of spec)",
                t.to_f64() * 1000.0
            );
        }
        let mut decoder = pd_log::Decoder::new();
        // With a file the capture is what counts: a screen that goes away (`| head`) ends the screen output, not the
        // capture (FW-56).
        let mut screen = true;
        let end = for_s.map(|s| tokio::time::Instant::now() + Duration::from_secs_f64(s));
        loop {
            let event = async {
                match end {
                    Some(end) => tokio::time::timeout_at(end, messages.recv_event())
                        .await
                        .ok(),
                    None => Some(messages.recv_event().await),
                }
            };
            tokio::select! {
                event = event => match event {
                    None => break,
                    Some(Ok(StreamEvent::Item { value, .. })) => {
                        let (item, to_write) = log_item(value, donguru_core::pd::host_time_us(), &mut decoder);
                        if let (Some(file), Some(line)) = (&mut file, &to_write) {
                            pd_log::write_line(file, line)?;
                        }
                        if screen && let Err(e) = text_out.record(&item) {
                            if file.is_none() || !crate::exit::is_broken_pipe(&e) {
                                return Err(e);
                            }
                            screen = false;
                        }
                    }
                    Some(Ok(StreamEvent::Gap { units, .. })) => {
                        let _ = writeln!(io::stderr(), "donguru: {units} record(s) lost");
                    }
                    Some(Ok(StreamEvent::Closed | StreamEvent::Disconnected)) => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => anyhow::bail!("PD log stream: {e}"),
                },
                _ = tokio::signal::ctrl_c() => break,
            }
        }
        if let Some(mut file) = file {
            file.flush()?;
        }
        dongle.close().await?;
        Ok(ExitCode::Success)
    })
}

pub fn run(ctx: &Context, command: Pd) -> anyhow::Result<ExitCode> {
    if let Pd::Log {
        follow,
        out,
        restart,
        for_s,
    } = command
    {
        return log(ctx, follow, out, restart, for_s);
    }
    let status = super::with_dongle(ctx, async |dongle| {
        let plug = dongle.plug();
        match command {
            Pd::Status | Pd::Log { .. } => {}
            Pd::Source(SourceCmd::On {
                current,
                sender_response_ms,
            }) => {
                plug.set_pd_source(donguru_core::pd::source(current, sender_response_ms))
                    .await?
            }
            Pd::Source(SourceCmd::Off) => plug.set_pd_source(PdSource::Off).await?,
        }
        Ok(Status {
            source: plug.pd_source().await?,
            state: plug.pd_state().await?,
        })
    })?;
    Output::stdout(ctx).emit(&status)?;
    Ok(ExitCode::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use donguru_core::mock::MockDevice;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn source_on_and_off() {
        let mock = MockDevice::start("cli/pd_source");
        let dongle = mock.open().await.unwrap();
        let plug = dongle.plug();
        assert_eq!(plug.pd_state().await.unwrap(), PdState::Off);
        plug.set_pd_source(donguru_core::pd::source(0.5, Some(100)))
            .await
            .unwrap();
        assert_eq!(
            plug.pd_source().await.unwrap(),
            donguru_core::pd::source(0.5, Some(100))
        );
        assert_eq!(plug.pd_state().await.unwrap(), PdState::Unattached);
        plug.set_pd_source(PdSource::Off).await.unwrap();
        assert_eq!(plug.pd_source().await.unwrap(), PdSource::Off);
        dongle.close().await.unwrap();
    }

    fn record(event: PdEventOwned) -> PdRecordOwned {
        PdRecordOwned {
            time: Second::micro(1_500_000),
            event,
        }
    }

    #[test]
    fn messages_decode_like_pd_log_print() {
        let mut decoder = pd_log::Decoder::new();
        let (item, to_write) = log_item(
            record(PdEventOwned::Tx {
                sop: donguru_core::api::PdSop::Sop,
                outcome: donguru_core::api::PdTxOutcome::Sent,
                bytes: wire_weaver::prelude::MaxLen::new(vec![0x63, 0x03]).unwrap(),
            }),
            42,
            &mut decoder,
        );
        let json = item.json();
        assert_eq!(json["direction"], "tx");
        assert_eq!(json["sop"], "sop");
        assert_eq!(json["outcome"], "ok");
        assert_eq!(json["raw"], "6303");
        assert_eq!(json["device_time_us"], 1_500_000);
        assert_eq!(json["host_time_us"], 42);
        // decoded with usbpd's real parser, not a guess: `None` only when the bytes don't parse
        assert!(!json["message"].is_null());
        let line = to_write.expect("a PD message writes a pd_log line");
        assert_eq!(line.record.host_time_us, 42);
        let mut text = Vec::new();
        item.text(&mut text, &Theme::plain()).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("tx"), "{text:?}");
        assert!(text.contains("Accept"), "{text:?}");
        assert!(text.contains("6303"), "{text:?}");
    }

    #[test]
    fn non_protocol_events_are_not_written_to_the_log() {
        let mut decoder = pd_log::Decoder::new();
        let (item, to_write) = log_item(record(PdEventOwned::Vbus { on: true }), 42, &mut decoder);
        assert!(to_write.is_none());
        let json = item.json();
        assert_eq!(json["event"], "vbus");
        assert_eq!(json["detail"], "on");
        assert_eq!(json["t_s"], 1.5);
    }
}
