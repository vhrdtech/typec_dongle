//! `donguru info` - the device contract in one read: firmware, board/variant, uid, label, last reset (CLI-7).
//!
//! The same summary as `donguru device info`; this is the short top-level spelling.

use crate::context::Context;
use crate::exit::ExitCode;

pub fn run(ctx: &Context) -> anyhow::Result<ExitCode> {
    let (entry, summary) = super::device::runtime()?.block_on(async {
        let dongle = super::device::open(ctx).await?;
        let summary = dongle.device().summary().await?;
        let entry = dongle.entry().clone();
        dongle.close().await?;
        Ok::<_, anyhow::Error>((entry, summary))
    })?;
    crate::output::Output::stdout(ctx).emit(&super::device::Info::new(&entry, &summary))?;
    Ok(ExitCode::Success)
}
