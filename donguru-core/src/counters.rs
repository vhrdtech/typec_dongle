//! `device.counters` (`ww_counters::Counters`), decoded with the firmware's ELF found by its build id (CLI-19,
//! `cnt_core`, the same crate `ww counters` uses).

use std::path::Path;

use cnt_core::{Counters, Source, Storage};
use serde::Serialize;
use ww_counters::CounterBuffer;

use crate::client::generated::donguru_api_donguru_api::donguru_api_device::DonguruApiDeviceClient;
use crate::levels::{call, read};
use crate::{Error, Result};

/// One decoded counter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CounterValue {
    /// `RAM` or `BKP`.
    pub buffer: &'static str,
    /// `group/name`.
    pub name: String,
    pub value: u64,
    pub unit: String,
    pub severity: String,
}

/// What `device counters` shows.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CountersReport {
    /// ELF used to decode the buffers.
    pub elf: String,
    /// Build id the device reported, hex, empty when it has none.
    pub build_id: String,
    pub counters: Vec<CounterValue>,
    /// Something worth telling the user: the ELF couldn't be checked, or doesn't match (`--force`).
    pub warning: Option<String>,
}

/// All the words of `buffer`, read a page at a time.
async fn read_buffer(
    client: &DonguruApiDeviceClient<'_>,
    buffer: CounterBuffer,
    used_words: u16,
) -> Result<Vec<u32>> {
    let mut words = Vec::with_capacity(used_words as usize);
    while (words.len() as u16) < used_words {
        let mut counters = client.counters();
        let page = call::<_, ww_counters::CountersErrorOwned>(
            "Counters::read",
            counters.read(buffer, words.len() as u16),
        )
        .await?;
        let page: Vec<u32> = page.into_inner().iter().map(|w| w.0).collect();
        if page.is_empty() {
            break;
        }
        words.extend(page);
    }
    words.truncate(used_words as usize);
    Ok(words)
}

/// Buffers already read from the device, so [cnt_core::Counters::read] needs no I/O of its own.
struct Preloaded {
    ram: Vec<u32>,
    bkp: Option<Vec<u32>>,
}

impl Source for Preloaded {
    fn read(&mut self, storage: Storage, offset: u64, words: &mut [u32]) -> anyhow::Result<()> {
        let buf = match storage {
            Storage::Ram => &self.ram,
            Storage::Bkp => self
                .bkp
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("the device has no backup counter buffer"))?,
        };
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let end = offset.saturating_add(words.len()).min(buf.len());
        let available = end.saturating_sub(offset.min(buf.len()));
        words[..available].copy_from_slice(&buf[offset..offset + available]);
        words[available..].fill(0);
        Ok(())
    }

    fn reset(&mut self, _storage: Storage) -> anyhow::Result<()> {
        // `device counters` only reads; `reset` isn't in the v1 command tree (CLI-19 scope).
        Err(anyhow::anyhow!("resetting counters isn't supported yet"))
    }
}

/// Reads and decodes every counter the device has (CLI-19 `device counters`).
pub async fn report(
    client: DonguruApiDeviceClient<'_>,
    elf: Option<&Path>,
    force: bool,
) -> Result<CountersReport> {
    let info = read("Counters::INFO", client.counters().read_INFO()).await?;
    let build_id: Vec<u8> = info.build_id.as_slice().to_vec();
    if info.ram.used_words == 0 && info.backup.is_none_or(|b| b.used_words == 0) {
        // nothing to decode: don't even ask for an ELF (the device may not have linked one with `cnt` yet)
        return Ok(CountersReport {
            elf: String::new(),
            build_id: cnt_core::elf::hex(&build_id),
            counters: vec![],
            warning: None,
        });
    }
    let resolved = cnt_core::elf::resolve(elf, Some(&build_id), force)
        .map_err(|e| Error::Link(format!("resolving the firmware ELF: {e:#}")))?;
    let mut counters = Counters::load_elf(&resolved.path)
        .map_err(|e| Error::Link(format!("{}: {e:#}", resolved.path.display())))?;

    let ram = read_buffer(&client, CounterBuffer::Ram, info.ram.used_words).await?;
    let bkp = match info.backup {
        Some(b) => Some(read_buffer(&client, CounterBuffer::Backup, b.used_words).await?),
        None => None,
    };
    let mut source = Preloaded { ram, bkp };
    counters
        .read(&mut source)
        .map_err(|e| Error::Link(format!("decoding counters: {e:#}")))?;

    let mut out = vec![];
    for block in counters.blocks() {
        let buffer = match block.storage() {
            Storage::Ram => "RAM",
            Storage::Bkp => "BKP",
        };
        for (c, value) in block.values() {
            out.push(CounterValue {
                buffer,
                name: c.qualified_name(),
                value: value.to_u64(),
                unit: c.unit.clone(),
                severity: format!("{:?}", c.severity).to_lowercase(),
            });
        }
    }
    Ok(CountersReport {
        elf: resolved.path.display().to_string(),
        build_id: cnt_core::elf::hex(&build_id),
        counters: out,
        warning: resolved.warning,
    })
}
