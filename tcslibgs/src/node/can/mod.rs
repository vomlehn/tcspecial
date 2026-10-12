//! CAN payloads: the interface, the identifier, and the frame.
//!
//! **Experimental, and not yet supported.** The terms parse and reach a
//! handler, and both ends of the link are written; nothing has been run
//! against a real CAN controller, only against the kernel's virtual bus.
//!
//! A CAN payload is not located by a device file. It is on a bus, reached
//! through a network interface -- `can0`, or `vcan0` for the kernel's virtual
//! one -- and addressed by an identifier the frames carry. Several payloads
//! commonly share one bus and differ only in that identifier, which is the
//! arrangement an I2C payload has, so a CAN payload states the interface and
//! the identifier the way an I2C payload states the bus and the address.
//!
//! What a frame can carry is the other thing this kind settles. A classic CAN
//! frame holds eight bytes and a CAN FD frame sixty-four, both exactly: there
//! is no fragmentation and no length beyond the frame's own. A payload whose
//! packets will not fit in one frame is refused here rather than being
//! discovered a frame at a time at run time.
//!
//! One identifier serves both directions. The handler filters on it, so it
//! reads the payload's frames and nothing else on the bus, and sends its
//! triggers with it, so a payload that answers requests hears them. The
//! socket does not receive its own frames, so a handler never reads back the
//! trigger it just sent.

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{bad, parse_flag, EndpointConfigResult, GroupKind, LinkTerms};
use crate::endpoint_config::reject_foreign_fields;
use crate::endpoint_config::reject_stream;

/// What a CAN payload's bus is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanParams {
    /// Whether frames are CAN FD rather than classic CAN.
    ///
    /// Applied: the socket is told to accept and send FD frames, which is
    /// also what lets a packet be longer than eight bytes.
    pub fd: bool,
    /// Whether the controller loops frames back to every socket on the host.
    ///
    /// Applied. On by default in the kernel, which is what lets two programs
    /// on one host talk over a virtual bus -- the arrangement tcssim and
    /// tcspecial are in -- and what a real bus does not need.
    pub loopback: bool,
}

/// The greatest number of bytes one frame carries.
///
/// Both exact, not upper bounds with padding: a classic frame has a
/// four-bit length and carries up to eight bytes, and an FD frame carries up
/// to sixty-four in the sixteen lengths it has codes for.
pub const CLASSIC_FRAME_BYTES: usize = 8;
/// As above, for CAN FD.
pub const FD_FRAME_BYTES: usize = 64;

/// The largest identifier of each kind.
///
/// Eleven bits in a base frame and twenty-nine in an extended one. A file
/// that states an identifier above the base range has stated an extended one,
/// which is read from the value rather than from a flag beside it: the two
/// ranges do not overlap, so there is nothing a flag would settle.
pub const LARGEST_BASE_ID: u32 = 0x7FF;
/// As above, for an extended identifier.
pub const LARGEST_EXTENDED_ID: u32 = 0x1FFF_FFFF;

/// How many bytes a payload's frames may carry, by what kind they are.
pub fn frame_bytes(fd: bool) -> usize {
    if fd {
        FD_FRAME_BYTES
    } else {
        CLASSIC_FRAME_BYTES
    }
}

/// Whether an identifier needs the extended format.
///
/// Read from the value: the base range is eleven bits and the extended one
/// twenty-nine, so an identifier above the base range is extended and one
/// inside it is not. A file could have stated it, and then a file stating
/// both would have two answers to one question.
pub fn is_extended(can_id: u32) -> bool {
    can_id > LARGEST_BASE_ID
}

/// Check an identifier a file stated, by the ranges CAN has.
pub fn check_can_id(name: &str, can_id: u32) -> EndpointConfigResult<u32> {
    if can_id > LARGEST_EXTENDED_ID {
        return Err(bad(
            name,
            &format!(
                "can_id {can_id:#x} is above {LARGEST_EXTENDED_ID:#x}: a CAN \
                 identifier is eleven bits in a base frame and twenty-nine in an \
                 extended one"
            ),
        ));
    }

    Ok(can_id)
}

/// What a CAN payload says about its bus, and what it must say.
///
/// Nothing is required but the kind itself: a bus that states no bitrate is
/// one whose rate is set where it is brought up, which is every bus the
/// simulator uses, and classic frames and loopback are what a bus does
/// unless a file says otherwise.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(name, "can", &g, &["can_fd", "loopback"])?;

    // A frame is its own length, so there is nothing for a stream rule to
    // say: the reason a bus has none either.
    reject_stream(name, "can", g.stream.is_some())?;


    let fd = parse_flag(name, "can_fd", g.can_fd.as_ref())?.unwrap_or(false);

    // On unless a file says otherwise, which is the kernel's own default and
    // what two programs on one host need of a virtual bus.
    let loopback = parse_flag(name, "loopback", g.loopback.as_ref())?.unwrap_or(true);

    Ok(GroupKind::Can(CanParams {
        fd,
        loopback,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_config::Scalar;

    fn terms(stated: &[(&str, &str)]) -> LinkTerms {
        let mut g = LinkTerms::default();
        for (field, value) in stated {
            let value = Some(Scalar::of(*value));
            match *field {
                "can_fd" => g.can_fd = value,
                "loopback" => g.loopback = value,
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    fn params(stated: &[(&str, &str)]) -> CanParams {
        match group_kind_of("payload", terms(stated)).expect("a bus") {
            GroupKind::Can(can) => can,
            other => panic!("expected a CAN bus, got {other:?}"),
        }
    }

    /// A bus that states nothing is classic CAN, looped back.
    #[test]
    fn a_bus_that_states_nothing_is_classic_can() {
        assert_eq!(
            params(&[]),
            CanParams {
                fd: false,
                loopback: true,
            }
        );
    }

    /// Each term a socket can set is read.
    ///
    /// Two of them, and both are applied: the frame format through
    /// `CAN_RAW_FD_FRAMES` and the loopback through `CAN_RAW_LOOPBACK`. The
    /// bitrate used to be a third and is not a term any more -- it belongs to
    /// the interface and is set where it is brought up, so nothing here could
    /// apply what a file said.
    #[test]
    fn what_a_bus_may_say_about_itself() {
        assert_eq!(
            params(&[("can_fd", "true"), ("loopback", "false")]),
            CanParams {
                fd: true,
                loopback: false,
            }
        );

    }

    /// A term of another kind of link is refused here as everywhere.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of("payload", terms(&[("datarate", "9600")]))
            .expect_err("a line's rate on a bus");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }

    /// What a frame carries, and which identifiers are extended.
    #[test]
    fn a_frame_carries_eight_bytes_or_sixty_four() {
        assert_eq!(frame_bytes(false), 8);
        assert_eq!(frame_bytes(true), 64);

        // The ranges do not overlap, so the value says which format it is.
        assert!(!is_extended(0));
        assert!(!is_extended(LARGEST_BASE_ID));
        assert!(is_extended(LARGEST_BASE_ID + 1));
        assert!(is_extended(LARGEST_EXTENDED_ID));

        assert_eq!(
            check_can_id("p", LARGEST_EXTENDED_ID).expect("the largest there is"),
            LARGEST_EXTENDED_ID
        );
        let said = check_can_id("p", LARGEST_EXTENDED_ID + 1)
            .expect_err("an identifier no frame can carry");
        assert!(
            format!("{said}").contains("twenty-nine"),
            "the error should say what a CAN identifier is: {said}"
        );
    }
}
