//! The call chain of the last reset's crash as `file:line` frames (`ww_report::crash`, wire_weaver CLT-59): the
//! panic site and its callers, or a fault's place, decoded with the ELF of the firmware that crashed. Release
//! firmware keeps no `file:line` in its panic messages (`-Zlocation-detail=none`, FW-63), so this is where they
//! come from; the record holds the return addresses (embedded_bedrock ROB-13).

use std::path::Path;

use ww_firmware_info::ResetRecordOwned;
use ww_report::crash::{CrashAddresses, Symbolizer};

pub use ww_report::crash::{CrashFrames, Frame, FrameKind};

use crate::{Error, Result};

/// A decoded call chain, and a warning worth telling the user (ELF not checked, or forced).
#[derive(Debug, Clone)]
pub struct Decoded {
    pub frames: CrashFrames,
    pub warning: Option<String>,
}

/// The call chain of `record`, decoded with `elf` or the ELF found by the record's build id (checked against it
/// unless `force`). `Ok(None)` when the record holds no addresses.
pub fn frames(
    record: &ResetRecordOwned,
    elf: Option<&Path>,
    force: bool,
) -> Result<Option<Decoded>> {
    let addresses = CrashAddresses::from_record(record);
    if addresses.is_empty() {
        return Ok(None);
    }
    let build_id = record.build_id.as_slice();
    if build_id.is_empty() && elf.is_none() {
        return Err(Error::Elf(
            "the crash record has no build id, so its ELF can't be found: pass --elf".to_string(),
        ));
    }
    let resolved = cnt_core::elf::resolve(elf, Some(build_id), force)
        .map_err(|e| Error::Elf(format!("{e:#}")))?;
    let symbolizer = Symbolizer::load(&resolved.path)
        .map_err(|e| Error::Elf(format!("{}: {e:#}", resolved.path.display())))?;
    Ok(Some(Decoded {
        frames: symbolizer.symbolize(&addresses),
        warning: resolved.warning,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wire_weaver::shrink_wrap::MaxLen;
    use ww_firmware_info::ResetCause;

    fn record(stack: Vec<u32>, build_id: Vec<u8>) -> ResetRecordOwned {
        ResetRecordOwned {
            cause: ResetCause::Panic,
            build_id: MaxLen::new_unchecked(build_id),
            uptime: None,
            fault: None,
            stack: MaxLen::new_unchecked(stack),
            message: MaxLen::new_unchecked("index out of bounds".to_string()),
            boots_since_crash: 0,
        }
    }

    #[test]
    fn no_addresses_nothing_to_decode() {
        assert!(
            frames(&record(vec![], vec![1, 2]), None, false)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn no_build_id_asks_for_the_elf() {
        let e = frames(&record(vec![0x0800_0101], vec![]), None, false).unwrap_err();
        assert!(e.to_string().contains("--elf"), "{e}");
    }
}
