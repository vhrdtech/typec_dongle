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

/// Name of a PD message from its 16-bit header (little endian on the wire), for a log line: the message type (USB PD
/// 3.1 tables 6-5 and 6-6), `MessageID` and the number of data objects. `None` when `bytes` is shorter than a header.
pub fn describe(bytes: &[u8]) -> Option<String> {
    let [low, high, ..] = *bytes else {
        return None;
    };
    let header = u16::from_le_bytes([low, high]);
    let kind = (header & 0x1f) as u8;
    let objects = (header >> 12) & 7;
    let id = (header >> 9) & 7;
    let name = if objects == 0 {
        match kind {
            1 => "GoodCRC",
            2 => "GotoMin",
            3 => "Accept",
            4 => "Reject",
            5 => "Ping",
            6 => "PS_RDY",
            7 => "Get_Source_Cap",
            8 => "Get_Sink_Cap",
            9 => "DR_Swap",
            10 => "PR_Swap",
            11 => "VCONN_Swap",
            12 => "Wait",
            13 => "Soft_Reset",
            16 => "Not_Supported",
            17 => "Get_Source_Cap_Extended",
            18 => "Get_Status",
            19 => "FR_Swap",
            20 => "Get_PPS_Status",
            _ => return Some(format!("control {kind} id {id}")),
        }
    } else {
        match kind {
            1 => "Source_Capabilities",
            2 => "Request",
            3 => "BIST",
            4 => "Sink_Capabilities",
            5 => "Battery_Status",
            6 => "Alert",
            7 => "Get_Country_Info",
            15 => "Vendor_Defined",
            _ => return Some(format!("data {kind} id {id}, {objects} objects")),
        }
    };
    Some(if objects == 0 {
        format!("{name} id {id}")
    } else {
        format!("{name} id {id}, {objects} objects")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_messages_by_header() {
        // Source_Capabilities, one object, id 2: NumDO 1, id 2, type 1
        assert_eq!(
            describe(&[0x01, 0x14, 0, 0, 0, 0]).unwrap(),
            "Source_Capabilities id 2, 1 objects"
        );
        assert_eq!(describe(&[0x41, 0x02]).unwrap(), "GoodCRC id 1");
        assert_eq!(describe(&[0x1f, 0x00]).unwrap(), "control 31 id 0");
        assert_eq!(describe(&[0x01]), None);
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
