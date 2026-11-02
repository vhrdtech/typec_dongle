//! `donguru pd` - the USB PD source on the plug (`plug.pd`, API-30): start and stop it, show where it stands. The
//! message log (`plug.pd.messages`) gets its own command with the log format (the PD plan's `dg pd log`).

use std::io::{self, Write};

use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;
use clap::Subcommand;
use donguru_core::api::{CcPolarity, PdSource, PdState};
use ww_si::{Ampere, Second};

#[derive(Debug, Subcommand)]
pub enum Pd {
    /// Show the PD source's setting and where it stands
    Status,
    /// Start or stop the PD source on the plug
    #[command(subcommand)]
    Source(SourceCmd),
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

pub fn run(ctx: &Context, command: Pd) -> anyhow::Result<ExitCode> {
    let status = super::with_dongle(ctx, async |dongle| {
        let plug = dongle.plug();
        match command {
            Pd::Status => {}
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
