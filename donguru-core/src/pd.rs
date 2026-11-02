//! `plug.pd` settings from plain numbers (API-30), as [`crate::vbus`] does for the VBUS modes: integer milliamps and
//! milliseconds, never floats, as the firmware reads them.

use donguru_api::PdSource;
use ww_si::{Ampere, Second};

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
