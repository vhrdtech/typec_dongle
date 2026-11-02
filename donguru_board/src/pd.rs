//! The PD source on the plug CC (UCPD2, `plug.pd`, API-30): what the firmware's policy-engine glue
//! (`fw/donguru/src/pd.rs`) decides without hardware, unit-tested on the host.
//!
//! - [`Offer`]: a `plug.pd.source` setting checked and turned into the PDO, the Rp, the guard limits of plug VBUS and
//!   the SenderResponseTimer.
//! - [`Offer::accepts`]: the answer to a sink's Request.
//! - [`Ring`]: the message log, raw records with the device time until the server sends them.

use donguru_api::{
    CcPolarity, PdContract, PdEvent, PdRecord, PdSop, PdSource, PdState, PdTxOutcome, VbusMode,
};
use wire_weaver::prelude::{MaxLen, RefVec};
use ww_si::ww_numeric::NumericValue;
use ww_si::{Ampere, Prefix, Second, Watt};

use crate::power::{milli, volt};

/// The most the source offers, mA: 3 A, the Type-C limit at 5 V without an e-marked cable. The receptacle side has
/// no current limit towards the DUT (U601 limits only plug -> receptacle, hw_b153a.md §3), so what the computer can
/// give is the host's to know: rev A presents a fixed Rd and reads no Rp on the receptacle (hw_b153a.md §0).
pub const MAX_CURRENT_MA: i32 = 3000;
/// The least: below USB 2.0's 100 mA unit load no sink works.
pub const MIN_CURRENT_MA: i32 = 100;
/// usbpd's SenderResponseTimer (PD 3.2 Tab. 6.68: 27-33 ms), and its tPSHardReset, the other 30 ms timer.
pub const SPEC_SENDER_RESPONSE_MS: u64 = 30;
/// Range of the sender-response knob, ms: down to the spec's minimum (24 ms, PD 2.0), up to 1 s.
pub const SENDER_RESPONSE_MS: core::ops::RangeInclusive<i32> = 24..=1000;
/// vSafe5V's upper limit, mV: the guard cuts plug VBUS above it.
pub const VSAFE5V_MAX_MV: i32 = 5500;
/// tSrcRecover's upper limit, ms (PD 3.2 Tab. 7.24: 660-1000 ms): VBUS stays off this long in a hard reset, and
/// before it comes back on after a detach. Rev A cannot read plug VBUS (hw_b153a.md §3, FW-35), so vSafe0V is waited
/// for, not measured.
pub const SRC_RECOVER_MS: u64 = 1000;
/// tCCDebounce (100-200 ms): CC must hold still this long before an attach counts.
pub const CC_DEBOUNCE_MS: u64 = 150;
/// The longest message the log keeps: header, extended header and one chunk (2 + 2 + 26). Longer ones are cut.
pub const MAX_MESSAGE: usize = 30;

/// The Rp the source presents, by the current it offers before a contract (Type-C 4.6.2.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Rp {
    /// Default USB power (0.5 / 0.9 A).
    DefaultUsb,
    A1_5,
    A3_0,
}

/// Why a `plug.pd.source` setting is refused.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OfferError {
    /// The current is no number we read, or out of [`MIN_CURRENT_MA`]..=[`MAX_CURRENT_MA`].
    Current,
    /// The sender-response time is no number we read, or out of [`SENDER_RESPONSE_MS`].
    SenderResponse,
    /// A setting of a newer host.
    Unknown,
}

/// A checked `plug.pd.source` setting: one fixed 5 V PDO.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Offer {
    /// The PDO's maximum current, mA, a multiple of 10 (the PDO's unit).
    pub max_ma: i32,
    /// SenderResponseTimer, ms.
    pub sender_response_ms: u64,
}

impl Offer {
    /// `None` for `Off`.
    pub fn from_setting(source: &PdSource) -> Result<Option<Offer>, OfferError> {
        match source {
            PdSource::Off => Ok(None),
            PdSource::Source {
                max_current,
                sender_response,
            } => {
                let ma = milli(max_current.prefix, &max_current.value)
                    .filter(|ma| (MIN_CURRENT_MA..=MAX_CURRENT_MA).contains(ma))
                    .ok_or(OfferError::Current)?;
                let sender_response_ms = match sender_response {
                    None => SPEC_SENDER_RESPONSE_MS,
                    Some(t) => milli(t.prefix, &t.value)
                        .filter(|ms| SENDER_RESPONSE_MS.contains(ms))
                        .ok_or(OfferError::SenderResponse)? as u64,
                };
                Ok(Some(Offer {
                    // the PDO counts in 10 mA: round down, never offer more than asked
                    max_ma: ma / 10 * 10,
                    sender_response_ms,
                }))
            }
            #[allow(unreachable_patterns)] // a setting of a newer host
            _ => Err(OfferError::Unknown),
        }
    }

    /// The setting this offer was made from, for reading `plug.pd.source` back.
    pub fn setting(&self) -> PdSource {
        PdSource::Source {
            max_current: Ampere::new(Prefix::Milli, NumericValue::I32(self.max_ma)),
            sender_response: (self.sender_response_ms != SPEC_SENDER_RESPONSE_MS).then(|| {
                Second::new(
                    Prefix::Milli,
                    NumericValue::U32(self.sender_response_ms as u32),
                )
            }),
        }
    }

    /// The PDO's current field, in its 10 mA unit.
    pub fn max_10ma(&self) -> u16 {
        (self.max_ma / 10) as u16
    }

    /// The Rp for this offer: never more than it offers.
    pub fn rp(&self) -> Rp {
        if self.max_ma < 1500 {
            Rp::DefaultUsb
        } else if self.max_ma < 3000 {
            Rp::A1_5
        } else {
            Rp::A3_0
        }
    }

    /// The guard of plug VBUS while the source runs: vSafe5V's maximum, the offered current with a quarter on top
    /// (a sink's load steps and the meter's ~15 ms window), the power of both.
    pub fn vbus_mode(&self) -> VbusMode {
        let ma = self.max_ma + self.max_ma / 4;
        VbusMode::Auto {
            max_voltage: volt(VSAFE5V_MAX_MV),
            max_current: Ampere::new(Prefix::Milli, NumericValue::I32(ma)),
            max_power: Watt::new(
                Prefix::Milli,
                NumericValue::I32(VSAFE5V_MAX_MV / 10 * ma / 100),
            ),
        }
    }

    /// A sink's Request for a fixed supply, in the RDO's 10 mA units: accepted for the one PDO (position 1) when the
    /// operating current fits the offer. A higher maximum alone is no reason to reject (the sink sets Capability
    /// Mismatch for it, PD 3.2 6.4.2).
    pub fn accepts(&self, object_position: u8, operating_10ma: u16) -> bool {
        object_position == 1 && i32::from(operating_10ma) * 10 <= self.max_ma
    }

    /// The time for one of usbpd's timers of `ms`: its 30 ms ones (SenderResponseTimer and tPSHardReset) take the
    /// knob, all others stay.
    pub fn timer_ms(&self, ms: u64) -> u64 {
        if ms == SPEC_SENDER_RESPONSE_MS {
            self.sender_response_ms
        } else {
            ms
        }
    }
}

/// Where the source stands, as the PD task keeps it: [`PdState`] without the quantities. `cc2`: the sink is on CC2.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Phase {
    Off,
    Unattached,
    Attached {
        cc2: bool,
    },
    /// The Request's fields, in its 10 mA units.
    Contract {
        cc2: bool,
        object_position: u8,
        operating_10ma: u16,
        max_10ma: u16,
    },
    NoPd {
        cc2: bool,
    },
    Failed {
        cc2: bool,
    },
}

impl Phase {
    pub fn to_api(self) -> PdState {
        let cc = |cc2| {
            if cc2 {
                CcPolarity::Cc2
            } else {
                CcPolarity::Cc1
            }
        };
        let ma = |v: u16| Ampere::new(Prefix::Milli, NumericValue::I32(i32::from(v) * 10));
        match self {
            Phase::Off => PdState::Off,
            Phase::Unattached => PdState::Unattached,
            Phase::Attached { cc2 } => PdState::Attached { cc: cc(cc2) },
            Phase::Contract {
                cc2,
                object_position,
                operating_10ma,
                max_10ma,
            } => PdState::Contract {
                cc: cc(cc2),
                contract: PdContract {
                    object_position,
                    // the one PDO there is
                    voltage: volt(5000),
                    operating_current: ma(operating_10ma),
                    max_current: ma(max_10ma),
                },
            },
            Phase::NoPd { cc2 } => PdState::NoPd { cc: cc(cc2) },
            Phase::Failed { cc2 } => PdState::Failed { cc: cc(cc2) },
        }
    }
}

/// What a [`Record`] records: [`PdEvent`] without the borrow.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Kind {
    Attached { cc2: bool },
    Detached,
    Vbus { on: bool },
    Rx,
    Tx { outcome: TxOutcome },
    RxDiscarded,
    HardResetRx,
    HardResetTx,
}

/// How a transmission ended, see [`PdTxOutcome`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TxOutcome {
    Sent,
    Discarded,
    HardReset,
}

/// One entry of the message log, fixed size.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// Device time, µs.
    pub time_us: u64,
    pub kind: Kind,
    /// Bytes of a message in `bytes`, 0 for an event.
    pub len: u8,
    pub bytes: [u8; MAX_MESSAGE],
}

impl Record {
    pub const EMPTY: Record = Record {
        time_us: 0,
        kind: Kind::Detached,
        len: 0,
        bytes: [0; MAX_MESSAGE],
    };

    /// An event without a message.
    pub fn event(time_us: u64, kind: Kind) -> Record {
        Record {
            time_us,
            kind,
            ..Record::EMPTY
        }
    }

    /// A message: `data` beyond [`MAX_MESSAGE`] is cut.
    pub fn message(time_us: u64, kind: Kind, data: &[u8]) -> Record {
        let n = data.len().min(MAX_MESSAGE);
        let mut r = Record::event(time_us, kind);
        r.bytes[..n].copy_from_slice(&data[..n]);
        r.len = n as u8;
        r
    }

    /// The message bytes kept.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// As the API sends it.
    pub fn to_api(&self) -> PdRecord<'_> {
        let bytes = || MaxLen::new_unchecked(RefVec::new_bytes(self.data()));
        let event = match self.kind {
            Kind::Attached { cc2 } => PdEvent::Attached {
                cc: if cc2 {
                    CcPolarity::Cc2
                } else {
                    CcPolarity::Cc1
                },
            },
            Kind::Detached => PdEvent::Detached,
            Kind::Vbus { on } => PdEvent::Vbus { on },
            Kind::Rx => PdEvent::Rx {
                sop: PdSop::Sop,
                bytes: bytes(),
            },
            Kind::Tx { outcome } => PdEvent::Tx {
                sop: PdSop::Sop,
                outcome: match outcome {
                    TxOutcome::Sent => PdTxOutcome::Sent,
                    TxOutcome::Discarded => PdTxOutcome::Discarded,
                    TxOutcome::HardReset => PdTxOutcome::HardReset,
                },
                bytes: bytes(),
            },
            Kind::RxDiscarded => PdEvent::RxDiscarded,
            Kind::HardResetRx => PdEvent::HardResetRx,
            Kind::HardResetTx => PdEvent::HardResetTx,
        };
        PdRecord {
            // µs in 32 bits, as the firmware's numbers go (no `u64` in ww_si there): wraps every 71.6 min, the host
            // unwraps it against when it received the record
            time: Second::micro(self.time_us as u32),
            event,
        }
    }
}

/// The log between the PD task and the server: a ring of `N` records. A full ring drops the oldest and counts it,
/// so the stream's `lost` says what a host missed.
pub struct Ring<const N: usize> {
    records: [Record; N],
    /// Index of the oldest record.
    head: usize,
    len: usize,
    lost: u32,
}

impl<const N: usize> Ring<N> {
    pub const fn new() -> Self {
        Ring {
            records: [Record::EMPTY; N],
            head: 0,
            len: 0,
            lost: 0,
        }
    }

    pub fn push(&mut self, r: Record) {
        if self.len == N {
            self.head = (self.head + 1) % N;
            self.len -= 1;
            self.lost = self.lost.saturating_add(1);
        }
        self.records[(self.head + self.len) % N] = r;
        self.len += 1;
    }

    /// The oldest record, with the records dropped before it since the last call.
    pub fn pop(&mut self) -> Option<(Record, u32)> {
        if self.len == 0 {
            return None;
        }
        let r = self.records[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some((r, core::mem::take(&mut self.lost)))
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<const N: usize> Default for Ring<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(ma: i32, sender_response_ms: Option<u32>) -> PdSource {
        PdSource::Source {
            max_current: Ampere::new(Prefix::Milli, NumericValue::I32(ma)),
            sender_response: sender_response_ms
                .map(|ms| Second::new(Prefix::Milli, NumericValue::U32(ms))),
        }
    }

    #[test]
    fn off_is_no_offer() {
        assert_eq!(Offer::from_setting(&PdSource::Off), Ok(None));
    }

    #[test]
    fn offer_checks_current_and_knob() {
        let o = Offer::from_setting(&source(1505, None)).unwrap().unwrap();
        assert_eq!(o.max_ma, 1500);
        assert_eq!(o.max_10ma(), 150);
        assert_eq!(o.sender_response_ms, SPEC_SENDER_RESPONSE_MS);
        for ma in [0, 99, 3001, -500] {
            assert_eq!(
                Offer::from_setting(&source(ma, None)),
                Err(OfferError::Current)
            );
        }
        for ms in [0, 23, 1001] {
            assert_eq!(
                Offer::from_setting(&source(500, Some(ms))),
                Err(OfferError::SenderResponse)
            );
        }
        let slow = Offer::from_setting(&source(500, Some(100)))
            .unwrap()
            .unwrap();
        assert_eq!(slow.sender_response_ms, 100);
        // 3 A in amperes reads the same
        let a = PdSource::Source {
            max_current: Ampere::new(Prefix::Unit, NumericValue::U8(3)),
            sender_response: None,
        };
        assert_eq!(Offer::from_setting(&a).unwrap().unwrap().max_ma, 3000);
    }

    #[test]
    fn setting_reads_back() {
        for s in [source(500, None), source(3000, Some(200))] {
            let o = Offer::from_setting(&s).unwrap().unwrap();
            assert_eq!(Offer::from_setting(&o.setting()).unwrap(), Some(o));
        }
    }

    #[test]
    fn rp_never_offers_more() {
        let rp = |ma| {
            Offer::from_setting(&source(ma, None))
                .unwrap()
                .unwrap()
                .rp()
        };
        assert_eq!(rp(500), Rp::DefaultUsb);
        assert_eq!(rp(1490), Rp::DefaultUsb);
        assert_eq!(rp(1500), Rp::A1_5);
        assert_eq!(rp(2990), Rp::A1_5);
        assert_eq!(rp(3000), Rp::A3_0);
    }

    #[test]
    fn guard_follows_the_offer() {
        let o = Offer::from_setting(&source(2000, None)).unwrap().unwrap();
        let limits = crate::power::Limits::from_mode(&o.vbus_mode())
            .unwrap()
            .unwrap();
        assert_eq!(limits.max_mv, VSAFE5V_MAX_MV);
        assert_eq!(limits.max_ma, Some(2500));
        assert_eq!(limits.max_mw, Some(13_750));
    }

    #[test]
    fn requests() {
        let o = Offer::from_setting(&source(900, None)).unwrap().unwrap();
        assert!(o.accepts(1, 90));
        assert!(o.accepts(1, 10));
        assert!(!o.accepts(1, 91));
        assert!(!o.accepts(2, 10));
        assert!(!o.accepts(0, 10));
    }

    #[test]
    fn knob_takes_only_the_30ms_timers() {
        let o = Offer::from_setting(&source(500, Some(120)))
            .unwrap()
            .unwrap();
        assert_eq!(o.timer_ms(30), 120);
        assert_eq!(o.timer_ms(27), 27);
        assert_eq!(o.timer_ms(150), 150);
    }

    #[test]
    fn phase_converts() {
        let c = Phase::Contract {
            cc2: true,
            object_position: 1,
            operating_10ma: 50,
            max_10ma: 90,
        };
        let PdState::Contract { cc, contract } = c.to_api() else {
            panic!("not a contract");
        };
        assert_eq!(cc, CcPolarity::Cc2);
        assert_eq!(contract.object_position, 1);
        assert_eq!(
            milli(
                contract.operating_current.prefix,
                &contract.operating_current.value
            ),
            Some(500)
        );
        assert_eq!(
            milli(contract.max_current.prefix, &contract.max_current.value),
            Some(900)
        );
        assert_eq!(
            Phase::NoPd { cc2: false }.to_api(),
            PdState::NoPd {
                cc: CcPolarity::Cc1
            }
        );
    }

    #[test]
    fn ring_drops_the_oldest_and_counts() {
        let mut ring = Ring::<3>::new();
        assert!(ring.pop().is_none());
        for t in 0..5 {
            ring.push(Record::event(t, Kind::Detached));
        }
        assert_eq!(ring.len(), 3);
        let (r, lost) = ring.pop().unwrap();
        assert_eq!((r.time_us, lost), (2, 2));
        let (r, lost) = ring.pop().unwrap();
        assert_eq!((r.time_us, lost), (3, 0));
        ring.push(Record::event(9, Kind::Detached));
        assert_eq!(ring.pop().unwrap().0.time_us, 4);
        assert_eq!(ring.pop().unwrap().0.time_us, 9);
        assert!(ring.is_empty());
    }

    #[test]
    fn records_cut_long_messages_and_convert() {
        let long = [0xAA; 40];
        let r = Record::message(5_000_000_123, Kind::Rx, &long);
        assert_eq!(r.data().len(), MAX_MESSAGE);
        let api = r.to_api();
        assert_eq!(
            api.time,
            Second::micro((5_000_000_123u64 & 0xFFFF_FFFF) as u32)
        );
        assert!(
            matches!(api.event, PdEvent::Rx { sop: PdSop::Sop, ref bytes } if bytes.len() == MAX_MESSAGE)
        );
        let tx = Record::message(
            1,
            Kind::Tx {
                outcome: TxOutcome::Discarded,
            },
            &[0x61, 0x11],
        );
        assert!(matches!(
            tx.to_api().event,
            PdEvent::Tx { outcome: PdTxOutcome::Discarded, ref bytes, .. } if bytes.len() == 2
        ));
        let a = Record::event(1, Kind::Attached { cc2: true });
        assert_eq!(
            a.to_api().event,
            PdEvent::Attached {
                cc: CcPolarity::Cc2
            }
        );
    }
}
