//! The memtest over a real probe (feature `probe-rs`): open a probe at a given SWD speed, attach, halt the core,
//! save the region, run patterns, put the region back and let the core run on.

use super::{Pattern, PatternResult, Region, WordMemory, run_pattern};
use probe_rs::probe::{DebugProbeSelector, list::Lister};
use probe_rs::{Core, MemoryInterface, Permissions, Session};
use std::time::Duration;

/// Donguru's CMSIS-DAP v2 interface (FW-15).
pub const DONGURU_PROBE: &str = "c0de:cafe";

impl WordMemory for Core<'_> {
    type Error = probe_rs::Error;

    fn read_words(&mut self, addr: u64, buf: &mut [u32]) -> Result<(), probe_rs::Error> {
        self.read_32(addr, buf)
    }

    fn write_words(&mut self, addr: u64, data: &[u32]) -> Result<(), probe_rs::Error> {
        self.write_32(addr, data)
    }

    fn write_bytes(&mut self, addr: u64, data: &[u8]) -> Result<(), probe_rs::Error> {
        self.write_8(addr, data)
    }
}

/// An attached target, at the SWD speed the probe accepted.
pub struct Target {
    session: Session,
    pub speed_khz: u32,
}

impl Target {
    /// Opens `probe` (`VID:PID[:SERIAL]`), sets the SWD speed and attaches to `chip` (a probe-rs target name).
    pub fn attach(probe: &str, chip: &str, speed_khz: u32) -> anyhow::Result<Self> {
        let selector: DebugProbeSelector = probe.parse()?;
        let mut probe = Lister::new().open(selector)?;
        let speed_khz = probe.set_speed(speed_khz)?;
        let session = probe.attach(chip, Permissions::default())?;
        Ok(Target { session, speed_khz })
    }

    /// The largest RAM region in the target's memory map, as a test region.
    pub fn largest_ram(&self) -> Option<Region> {
        self.session
            .target()
            .memory_map
            .iter()
            .filter_map(|r| r.as_ram_region())
            .max_by_key(|r| r.range.end - r.range.start)
            .map(|r| Region {
                base: r.range.start,
                words: ((r.range.end - r.range.start) / 4) as usize,
            })
    }

    /// Halts core 0, saves `region`, runs `patterns` (calling `on_result` after each), restores the region and
    /// resumes the core unless `restore` is false (then it stays halted with test data in RAM).
    pub fn run(
        &mut self,
        region: Region,
        patterns: &[Pattern],
        seed: u64,
        restore: bool,
        mut on_result: impl FnMut(&PatternResult),
    ) -> anyhow::Result<Vec<PatternResult>> {
        let mut core = self.session.core(0)?;
        core.halt(Duration::from_millis(500))?;
        let mut saved = vec![0u32; if restore { region.words } else { 0 }];
        if restore {
            core.read_32(region.base, &mut saved)?;
        }
        let mut results = Vec::with_capacity(patterns.len());
        let outcome = patterns.iter().try_for_each(|&p| {
            let r = run_pattern(&mut core, region, p, seed)?;
            on_result(&r);
            results.push(r);
            Ok::<_, probe_rs::Error>(())
        });
        if restore {
            core.write_32(region.base, &saved)?;
            core.run()?;
        }
        outcome?;
        Ok(results)
    }
}
