//! `plug.pd` settings from plain numbers (API-30), as [`crate::vbus`] does for the VBUS modes: integer milliamps and
//! milliseconds, never floats, as the firmware reads them. Also `plug.pd.messages` as `pd_log::Record`s (LOG-1), for
//! `dg pd log`'s JSON lines file and its screen decode, shared with rePort's sink log.

use donguru_api::{PdEventOwned, PdRecordOwned, PdSop, PdSource, PdTxOutcome};
use ww_si::{Ampere, Overflow, Prefix, Rounding, Second};

/// This device's label in a `pd_log` record ([`pd_log::Source`]).
pub const LOG_SOURCE: &str = "donguru";

/// `plug.pd` device time (µs, wraps every 71.6 min) as a plain integer for [`pd_log::Record::device_time_us`]: a
/// pass-through of the firmware's own counter, not unwrapped (one log file is one capture, not a stream spanning a
/// wrap).
fn device_time_us(time: Second) -> u64 {
    time.to_in::<u32>(Prefix::Micro, Rounding::Truncate, Overflow::Checked)
        .unwrap_or(0) as u64
}

fn log_sop(sop: PdSop) -> pd_log::Sop {
    match sop {
        PdSop::Sop => pd_log::Sop::Sop,
        PdSop::SopPrime => pd_log::Sop::Sop1,
        PdSop::SopDoublePrime => pd_log::Sop::Sop2,
        // a future firmware's SOP* this host doesn't know yet: logged as plain SOP rather than lost
        _ => pd_log::Sop::Sop,
    }
}

/// One `plug.pd.messages` record as a [`pd_log::Record`] (LOG-1): `None` for the events `pd_log` doesn't model
/// (attach, detach, VBUS switching) since they are informational, not a PD message on the wire. `host_time_us` is
/// the host's own UTC clock when the record was logged (see [`host_time_us`]), not a back-computed mapping of
/// `record`'s device time: `pd-log merge` only needs the two logs lined up roughly, not to the µs.
pub fn log_record(host_time_us: u64, record: &PdRecordOwned) -> Option<pd_log::Record> {
    let (direction, sop, outcome, bytes) = match &record.event {
        PdEventOwned::Rx { sop, bytes } => (
            pd_log::Direction::Rx,
            log_sop(*sop),
            pd_log::Outcome::Ok,
            bytes.as_slice(),
        ),
        PdEventOwned::Tx {
            sop,
            outcome,
            bytes,
        } => (
            pd_log::Direction::Tx,
            log_sop(*sop),
            match outcome {
                PdTxOutcome::Sent => pd_log::Outcome::Ok,
                PdTxOutcome::HardReset => pd_log::Outcome::HardReset,
                // `Discarded` (the line was busy) and an unknown future outcome: `pd_log` has one "didn't go
                // through" outcome for a Tx, not the finer firmware-side reason
                _ => pd_log::Outcome::RetryFail,
            },
            bytes.as_slice(),
        ),
        PdEventOwned::RxDiscarded => (
            pd_log::Direction::Rx,
            pd_log::Sop::Sop,
            pd_log::Outcome::Discarded,
            [].as_slice(),
        ),
        PdEventOwned::HardResetRx => (
            pd_log::Direction::Rx,
            pd_log::Sop::Sop,
            pd_log::Outcome::HardReset,
            [].as_slice(),
        ),
        PdEventOwned::HardResetTx => (
            pd_log::Direction::Tx,
            pd_log::Sop::Sop,
            pd_log::Outcome::HardReset,
            [].as_slice(),
        ),
        _ => return None,
    };
    Some(pd_log::Record {
        source: pd_log::Source::new(LOG_SOURCE),
        direction,
        sop,
        device_time_us: Some(device_time_us(record.time)),
        host_time_us,
        outcome,
        // `bytes` comes off the wire as `MaxLen<_, 30>`, already within `pd_log::MAX_PAYLOAD`
        raw: pd_log::RawFrame::new(bytes)
            .expect("plug.pd messages are at most 30 bytes (MaxLen<_, 30>)"),
    })
}

/// Host UTC now, in microseconds since the Unix epoch, for [`pd_log::Record::host_time_us`]: the moment the CLI
/// logged the message, not `record`'s device time mapped through the clock sync.
pub fn host_time_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

/// The PD source offering 5 V at `max_current_a`, with the spec's SenderResponseTimer unless `sender_response_ms`
/// stretches it (an out-of-spec test knob, 24 to 1000 ms).
pub fn source(max_current_a: f64, sender_response_ms: Option<u32>) -> PdSource {
    PdSource::Source {
        max_current: Ampere::milli((max_current_a * 1000.0).round() as i32),
        sender_response: sender_response_ms.map(|ms| Second::milli(ms as i32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(b: &[u8]) -> wire_weaver::prelude::MaxLen<Vec<u8>, 30> {
        wire_weaver::prelude::MaxLen::new(b.to_vec()).unwrap()
    }

    #[test]
    fn rx_message_is_ok() {
        let record = PdRecordOwned {
            time: Second::micro(2_000_000u32),
            event: PdEventOwned::Rx {
                sop: PdSop::Sop,
                bytes: bytes(&[0x63, 0x03]),
            },
        };
        let line = log_record(99, &record).unwrap();
        assert_eq!(line.direction, pd_log::Direction::Rx);
        assert_eq!(line.sop, pd_log::Sop::Sop);
        assert_eq!(line.outcome, pd_log::Outcome::Ok);
        assert_eq!(line.device_time_us, Some(2_000_000));
        assert_eq!(line.host_time_us, 99);
        assert_eq!(line.raw.as_slice(), &[0x63, 0x03]);
        assert_eq!(line.source.as_str(), LOG_SOURCE);
    }

    #[test]
    fn tx_discarded_has_no_good_crc_either() {
        let record = PdRecordOwned {
            time: Second::micro(0u32),
            event: PdEventOwned::Tx {
                sop: PdSop::SopPrime,
                outcome: PdTxOutcome::Discarded,
                bytes: bytes(&[0x01, 0x02]),
            },
        };
        let line = log_record(0, &record).unwrap();
        assert_eq!(line.direction, pd_log::Direction::Tx);
        assert_eq!(line.sop, pd_log::Sop::Sop1);
        assert_eq!(line.outcome, pd_log::Outcome::RetryFail);
    }

    #[test]
    fn hard_resets_and_discards_carry_no_bytes() {
        let record = |event| PdRecordOwned {
            time: Second::micro(0u32),
            event,
        };
        for event in [
            PdEventOwned::RxDiscarded,
            PdEventOwned::HardResetRx,
            PdEventOwned::HardResetTx,
        ] {
            let line = log_record(0, &record(event)).unwrap();
            assert!(line.raw.as_slice().is_empty());
        }
    }

    #[test]
    fn attach_detach_and_vbus_are_not_pd_messages() {
        let record = |event| PdRecordOwned {
            time: Second::micro(0u32),
            event,
        };
        for event in [
            PdEventOwned::Attached {
                cc: donguru_api::CcPolarity::Cc1,
            },
            PdEventOwned::Detached,
            PdEventOwned::Vbus { on: true },
        ] {
            assert!(log_record(0, &record(event)).is_none());
        }
    }

    #[test]
    fn host_time_us_is_roughly_now() {
        let us = host_time_us();
        // sometime after this file was written (2 Nov 2026) and before the year is silly
        assert!(us > 1_793_000_000_000_000, "{us}");
        assert!(us < 4_000_000_000_000_000, "{us}");
    }

    #[test]
    fn builds_in_milli_units() {
        let PdSource::Source {
            max_current,
            sender_response,
        } = source(1.5, Some(100))
        else {
            panic!("not a source");
        };
        assert_eq!(max_current, Ampere::milli(1500));
        assert_eq!(sender_response, Some(Second::milli(100)));
        assert!(matches!(
            source(0.5, None),
            PdSource::Source {
                sender_response: None,
                ..
            }
        ));
    }
}
