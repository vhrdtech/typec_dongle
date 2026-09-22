//! `donguru device` - enumerate and inspect dongles.

use crate::context::Context;
use crate::exit::ExitCode;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Device {
    /// List connected dongles
    #[command(alias = "ls")]
    List,
    /// Show identity, serial and firmware version
    Info,
    /// Persist a default device for this project
    Select {
        /// Device selector (serial number, index or USB path)
        selector: String,
    },
}

pub fn run(_ctx: &Context, command: Device) -> anyhow::Result<ExitCode> {
    match command {
        Device::List => super::stub("device list", ExitCode::Failure),
        Device::Info => super::stub("device info", ExitCode::Failure),
        Device::Select { selector } => {
            super::stub(&format!("device select {selector}"), ExitCode::Failure)
        }
    }
}
