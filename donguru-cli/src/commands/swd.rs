//! `donguru swd` - the SWD-host pins on the plug SBU lines (FW-15): cable orientation and the Hi-Z release used
//! by bench power-cut tests. CMSIS-DAP itself is not a `donguru` subcommand: probe-rs talks to it directly over
//! USB (see the hardware checklist in `docs/development/index.md`).

use crate::context::Context;
use crate::exit::ExitCode;
use clap::{Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Orientation {
    /// SBU1 = SWCLK, SBU2 = SWDIO
    Normal,
    /// SBU1 = SWDIO, SBU2 = SWCLK
    Swapped,
}

#[derive(Debug, Subcommand)]
pub enum Swd {
    /// Show or set the cable orientation used for SWD-over-SBU
    ///
    /// Takes effect on the next CMSIS-DAP `DAP_Connect`; does not affect an already connected probe-rs session.
    Orientation {
        /// New orientation; omit to show the current one
        #[arg(value_enum)]
        orientation: Option<Orientation>,
    },

    /// Release the SWD-host pins to Hi-Z (SWCLK, SWDIO and the SBU level shifter), independent of any
    /// CMSIS-DAP session
    ///
    /// For a test harness to call right before cutting the DUT's power, so nothing on the plug SBU lines can
    /// back-power it.
    Release,
}

pub fn run(_ctx: &Context, cmd: Swd) -> anyhow::Result<ExitCode> {
    match cmd {
        Swd::Orientation { orientation: None } => super::stub("swd orientation", ExitCode::Failure),
        Swd::Orientation {
            orientation: Some(orientation),
        } => super::stub(
            &format!("swd orientation {orientation:?}"),
            ExitCode::Failure,
        ),
        Swd::Release => super::stub("swd release", ExitCode::Failure),
    }
}
