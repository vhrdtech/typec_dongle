//! `donguru pd` - the USB PD source on the plug (`plug.pd`, API-30): start and stop it, show where it stands, follow
//! its message log (`plug.pd.messages`).

use std::io::{self, Write};

use crate::cli::Format;
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;
use clap::Subcommand;
use donguru_core::api::{
    CcPolarity, PdEventOwned, PdRecordOwned, PdSop, PdSource, PdState, PdTxOutcome,
};
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
    /// Follow the source's messages and events until interrupted (Ctrl-C), device time in seconds
    Log {
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

struct Line(PdRecordOwned);

fn sop(s: PdSop) -> &'static str {
    match s {
        PdSop::Sop => "SOP",
        PdSop::SopPrime => "SOP'",
        PdSop::SopDoublePrime => "SOP''",
        _ => "SOP?",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Line {
    /// What happened, with its details and the raw bytes of a message.
    fn parts(&self) -> (&'static str, String, Option<&[u8]>) {
        let named = |bytes: &[u8]| donguru_core::pd::describe(bytes).unwrap_or_default();
        match &self.0.event {
            PdEventOwned::Attached { cc: c } => ("attached", cc(*c).into(), None),
            PdEventOwned::Detached => ("detached", String::new(), None),
            PdEventOwned::Vbus { on } => ("vbus", if *on { "on" } else { "off" }.to_string(), None),
            PdEventOwned::Rx { sop: s, bytes } => (
                "rx",
                format!("{} {}", sop(*s), named(bytes.as_slice())),
                Some(bytes.as_slice()),
            ),
            PdEventOwned::Tx {
                sop: s,
                outcome,
                bytes,
            } => {
                let end = match outcome {
                    PdTxOutcome::Sent => "",
                    PdTxOutcome::Discarded => " (discarded)",
                    PdTxOutcome::HardReset => " (cut by a hard reset)",
                    _ => " (unknown outcome)",
                };
                (
                    "tx",
                    format!("{} {}{end}", sop(*s), named(bytes.as_slice())),
                    Some(bytes.as_slice()),
                )
            }
            PdEventOwned::RxDiscarded => ("rx-discarded", String::new(), None),
            PdEventOwned::HardResetRx => ("hard-reset-rx", String::new(), None),
            PdEventOwned::HardResetTx => ("hard-reset-tx", String::new(), None),
            _ => ("unknown", String::new(), None),
        }
    }
}

impl Render for Line {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        let (event, detail, bytes) = self.parts();
        write!(
            out,
            "{:12.6}  {:<13} {}",
            self.0.time.to_f64(),
            th.value(event),
            detail
        )?;
        match bytes {
            Some(b) => writeln!(out, "  [{}]", hex(b)),
            None => writeln!(out),
        }
    }

    fn json(&self) -> serde_json::Value {
        let (event, detail, bytes) = self.parts();
        serde_json::json!({
            "t_s": self.0.time.to_f64(),
            "event": event,
            "detail": detail,
            "bytes": bytes.map(hex),
        })
    }
}

fn log(ctx: &Context, restart: bool, for_s: Option<f64>) -> anyhow::Result<ExitCode> {
    if matches!(ctx.format, Format::Json) {
        anyhow::bail!("`pd log` streams data: use `--format text` or `--format jsonl`");
    }
    let mut out = Output::stdout(ctx);
    let selector = super::selector(ctx)?;
    super::runtime()?.block_on(async {
        let dongle = donguru_core::Dongle::open(selector.as_ref()).await?;
        let plug = dongle.plug();
        let mut messages = plug.pd_messages().await?;
        if restart {
            let source = plug.pd_source().await?;
            if matches!(source, PdSource::Off) {
                anyhow::bail!("the PD source is off: `donguru pd source on` first");
            }
            plug.set_pd_source(PdSource::Off).await?;
            plug.set_pd_source(source).await?;
        }
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
                    Some(Ok(StreamEvent::Item { value, .. })) => out.record(&Line(value))?,
                    Some(Ok(StreamEvent::Gap { units, .. })) => {
                        eprintln!("donguru: {units} record(s) lost");
                    }
                    Some(Ok(StreamEvent::Closed | StreamEvent::Disconnected)) => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => anyhow::bail!("PD log stream: {e}"),
                },
                _ = tokio::signal::ctrl_c() => break,
            }
        }
        dongle.close().await?;
        Ok(ExitCode::Success)
    })
}

pub fn run(ctx: &Context, command: Pd) -> anyhow::Result<ExitCode> {
    if let Pd::Log { restart, for_s } = command {
        return log(ctx, restart, for_s);
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

    fn line(event: PdEventOwned) -> Line {
        Line(PdRecordOwned {
            time: Second::micro(1_500_000),
            event,
        })
    }

    #[test]
    fn log_lines_name_the_messages() {
        let tx = line(PdEventOwned::Tx {
            sop: PdSop::Sop,
            outcome: PdTxOutcome::Sent,
            bytes: wire_weaver::prelude::MaxLen::new(vec![0x63, 0x03]).unwrap(),
        });
        assert_eq!(tx.json()["detail"], "SOP Accept id 1");
        assert_eq!(tx.json()["bytes"], "6303");
        assert_eq!(tx.json()["t_s"], 1.5);
        let mut text = Vec::new();
        tx.text(&mut text, &Theme::plain()).unwrap();
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "    1.500000  tx            SOP Accept id 1  [6303]\n"
        );
        let vbus = line(PdEventOwned::Vbus { on: true });
        assert_eq!(vbus.json()["event"], "vbus");
        assert_eq!(vbus.json()["detail"], "on");
    }
}
