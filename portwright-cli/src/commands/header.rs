//! `donguru header` - header J101's own switched VBUS (Q102, PD1) and the isolated relay (U101) (CLI-16). It is
//! not the computer's regulated 5 V: it is the same receptacle VBUS as `donguru power`, routed to the header, so
//! a PD charger above 5 V reaches it too. See `docs/user/power.md`.

use std::io::{self, Write};

use clap::Subcommand;

use super::power::{self, VbusOnArgs};
use crate::context::Context;
use crate::exit::ExitCode;
use crate::output::{Output, Render};
use crate::theme::Theme;
use donguru_core::api::{VbusMode, VbusState};

#[derive(Debug, Subcommand)]
pub enum HeaderVbus {
    /// Switch state and mode
    Status,
    /// Turn VBUS_SW on (`ForceOn`, or `Auto` with `--trip`)
    ///
    /// VBUS_SW carries the computer's own VBUS, not a regulated 5 V supply: above 5.5 V (a PD charger
    /// renegotiated), pass `--allow-voltage` to confirm the higher limit.
    On(VbusOnArgs),
    /// Turn VBUS_SW off
    Off,
}

#[derive(Debug, Subcommand)]
pub enum HeaderRelay {
    /// Contact state
    Status,
    /// Close the relay contact
    On,
    /// Open the relay contact
    Off,
}

#[derive(Debug, Subcommand)]
pub enum Header {
    /// Header VBUS_SW (J101 pin 10)
    #[command(subcommand)]
    Vbus(HeaderVbus),
    /// The isolated relay contact (J101 pins 11/12)
    #[command(subcommand)]
    Relay(HeaderRelay),
}

pub fn run(ctx: &Context, command: Header) -> anyhow::Result<ExitCode> {
    match command {
        Header::Vbus(HeaderVbus::Status) => vbus_status(ctx),
        Header::Vbus(HeaderVbus::On(args)) => vbus_on(ctx, args),
        Header::Vbus(HeaderVbus::Off) => vbus_off(ctx),
        Header::Relay(HeaderRelay::Status) => relay_status(ctx),
        Header::Relay(HeaderRelay::On) => relay_set(ctx, true),
        Header::Relay(HeaderRelay::Off) => relay_set(ctx, false),
    }
}

fn vbus_status(ctx: &Context) -> anyhow::Result<ExitCode> {
    let (state, mode) = super::with_dongle(ctx, async |dongle| {
        let header = dongle.header();
        Ok((header.vbus_sw().await?, header.vbus_sw_mode().await?))
    })?;
    Output::stdout(ctx).emit(&VbusStatus { state, mode })?;
    Ok(ExitCode::Success)
}

/// `header vbus status`.
struct VbusStatus {
    state: VbusState,
    mode: VbusMode,
}

impl Render for VbusStatus {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(
            out,
            "{}: {}",
            th.key("vbus"),
            power::vbus_state_text(th, self.state)
        )?;
        writeln!(
            out,
            "{}: {}",
            th.key("mode"),
            th.value(power::fmt_vbus_mode(&self.mode))
        )
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "vbus": power::state_json(self.state), "mode": power::mode_json(&self.mode) })
    }
}

/// CLI-16: the present VBUS, and a warning that the header's VBUS_SW is not a regulated 5 V supply, before
/// switching it on.
fn warn_not_regulated(ctx: &Context, present_v: Option<f64>) {
    if ctx.quiet {
        return;
    }
    match present_v {
        Some(v) => eprintln!(
            "donguru: warning: header VBUS_SW is the computer's own VBUS, not a regulated 5 V supply (measured {v:.2} V)"
        ),
        None => eprintln!(
            "donguru: warning: header VBUS_SW is the computer's own VBUS, not a regulated 5 V supply"
        ),
    }
}

fn vbus_on(ctx: &Context, args: VbusOnArgs) -> anyhow::Result<ExitCode> {
    let mode = power::mode_from_args(&args);
    let (present, state) = super::with_dongle(ctx, async |dongle| {
        let present = super::optional(dongle.meter().reading().await)?.map(|r| r.vbus_in.to_f64());
        dongle.header().set_vbus_sw_mode(mode).await?;
        Ok((present, dongle.header().vbus_sw().await?))
    })?;
    warn_not_regulated(ctx, present);
    power::report_vbus_state(ctx, "header", state, true)
}

fn vbus_off(ctx: &Context) -> anyhow::Result<ExitCode> {
    let state = super::with_dongle(ctx, async |dongle| {
        dongle.header().set_vbus_sw_mode(VbusMode::ForceOff).await?;
        dongle.header().vbus_sw().await
    })?;
    power::report_vbus_state(ctx, "header", state, false)
}

fn fmt_relay(closed: bool) -> &'static str {
    if closed { "closed" } else { "open" }
}

fn relay_status(ctx: &Context) -> anyhow::Result<ExitCode> {
    let closed = super::with_dongle(ctx, async |dongle| dongle.header().relay().await)?;
    Output::stdout(ctx).emit(&Relay(closed))?;
    Ok(ExitCode::Success)
}

fn relay_set(ctx: &Context, closed: bool) -> anyhow::Result<ExitCode> {
    let got = super::with_dongle(ctx, async |dongle| {
        dongle.header().set_relay(closed).await?;
        dongle.header().relay().await
    })?;
    Output::stdout(ctx).emit(&Relay(got))?;
    Ok(if got == closed {
        ExitCode::Success
    } else {
        ExitCode::Failure
    })
}

/// The relay contact: closed or open.
struct Relay(bool);

impl Render for Relay {
    fn text(&self, out: &mut dyn Write, th: &Theme) -> io::Result<()> {
        writeln!(out, "{}: {}", th.key("relay"), th.value(fmt_relay(self.0)))
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({ "relay": self.0 })
    }
}

#[cfg(test)]
mod tests {
    use donguru_core::api::{VbusMode, VbusState};
    use donguru_core::mock::MockDevice;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn vbus_on_off_and_relay() {
        let mock = MockDevice::start("cli/header");
        let dongle = mock.open().await.unwrap();
        assert_eq!(dongle.header().vbus_sw().await.unwrap(), VbusState::Off);

        dongle
            .header()
            .set_vbus_sw_mode(VbusMode::ForceOn {
                max_voltage: donguru_core::api::Volt::milli(5_500i32),
            })
            .await
            .unwrap();
        assert_eq!(dongle.header().vbus_sw().await.unwrap(), VbusState::On);

        dongle.header().set_relay(true).await.unwrap();
        assert!(dongle.header().relay().await.unwrap());
        dongle.header().set_relay(false).await.unwrap();
        assert!(!dongle.header().relay().await.unwrap());

        dongle
            .header()
            .set_vbus_sw_mode(VbusMode::ForceOff)
            .await
            .unwrap();
        assert_eq!(dongle.header().vbus_sw().await.unwrap(), VbusState::Off);
        dongle.close().await.unwrap();
    }
}
