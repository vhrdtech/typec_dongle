//! UCPD1 (receptacle, to the computer) and UCPD2 (plug, to the DUT) for the `debug` level (`ucpd`, `ucpd_role`,
//! API-18): which CC role a port may take on this board, and the UCPD_CR fields of a role (RM0444 §34.7.2).
//!
//! Rev A has no Rd of its own on the receptacle and no OVP on CC (hw_b153a.md §0, §4, §14): UCPD1 stays off for
//! normal use and may only be a sink (Rd) for PD tests. An Rp there would present a source to the computer.

use crate::pins::{Denied, Holder};
use donguru_api::{CcRole, UcpdPort};

/// UCPD_CR fields of a CC role.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RoleFields {
    /// ANAMODE: `true` sink (Rd), `false` source (Rp).
    pub sink: bool,
    /// ANASUBMODE: the Rp current, 1 default USB, 2 1.5 A, 3 3.0 A; 0 for a sink.
    pub rp: u8,
    /// CCENABLE both lines; `false` turns the CC PHY off (the role `Off`).
    pub enabled: bool,
}

/// The function that holds a port's CC pins while it has a role.
pub const fn holder(port: UcpdPort) -> Holder {
    match port {
        UcpdPort::Ucpd1 => Holder::Ucpd1,
        UcpdPort::Ucpd2 => Holder::Ucpd2,
    }
}

/// The CR fields of `role` on `port`, or why it is refused (`rev_a`: the board is a B153A).
pub fn role_fields(port: UcpdPort, role: CcRole, rev_a: bool) -> Result<RoleFields, Denied> {
    let fields = match role {
        CcRole::Off => RoleFields {
            sink: false,
            rp: 0,
            enabled: false,
        },
        CcRole::Rd => RoleFields {
            sink: true,
            rp: 0,
            enabled: true,
        },
        CcRole::Rp0A9 | CcRole::Rp1A5 | CcRole::Rp3A0 => {
            if rev_a && port == UcpdPort::Ucpd1 {
                return Err(Denied::Refused(
                    "Rp on UCPD1 presents a source to the computer (rev A)",
                ));
            }
            RoleFields {
                sink: false,
                rp: match role {
                    CcRole::Rp0A9 => 1,
                    CcRole::Rp1A5 => 2,
                    _ => 3,
                },
                enabled: true,
            }
        }
        #[allow(unreachable_patterns)] // a role of a newer host
        _ => {
            return Err(Denied::Invalid {
                arg: 1,
                reason: "unknown CC role",
            });
        }
    };
    Ok(fields)
}

/// The role UCPD_CR holds, `None` for a combination [role_fields] never writes.
pub fn role_of(cr: u32) -> Option<CcRole> {
    let enabled = (cr >> 10) & 3;
    let sink = (cr >> 9) & 1 != 0;
    let rp = (cr >> 7) & 3;
    Some(match (enabled, sink, rp) {
        (0, _, _) => CcRole::Off,
        (3, true, _) => CcRole::Rd,
        (3, false, 1) => CcRole::Rp0A9,
        (3, false, 2) => CcRole::Rp1A5,
        (3, false, 3) => CcRole::Rp3A0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cr(f: RoleFields) -> u32 {
        (if f.enabled { 3 << 10 } else { 0 }) | (f.sink as u32) << 9 | (f.rp as u32) << 7
    }

    #[test]
    fn rev_a_ucpd1_is_off_or_sink() {
        for role in [CcRole::Off, CcRole::Rd] {
            assert!(role_fields(UcpdPort::Ucpd1, role, true).is_ok());
        }
        for role in [CcRole::Rp0A9, CcRole::Rp1A5, CcRole::Rp3A0] {
            assert!(matches!(
                role_fields(UcpdPort::Ucpd1, role, true),
                Err(Denied::Refused(r)) if r.len() <= 64
            ));
            assert!(role_fields(UcpdPort::Ucpd1, role, false).is_ok());
            assert!(role_fields(UcpdPort::Ucpd2, role, true).is_ok());
        }
    }

    #[test]
    fn fields_read_back_as_the_role() {
        for role in [
            CcRole::Off,
            CcRole::Rd,
            CcRole::Rp0A9,
            CcRole::Rp1A5,
            CcRole::Rp3A0,
        ] {
            let f = role_fields(UcpdPort::Ucpd2, role, true).unwrap();
            assert_eq!(role_of(cr(f)), Some(role));
        }
        assert_eq!(role_of(1 << 10), None);
    }
}
