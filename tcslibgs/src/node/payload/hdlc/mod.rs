//! HDLC nodes: a synchronous serial line, which Linux presents as a network
//! interface.
//!
//! **Experimental, and not yet supported.** The terms parse and reach an
//! endpoint; nothing has been run against a card, and unlike every other type
//! here there is nothing in the kernel to run it against -- see "No stand-in"
//! below.
//!
//! # What Linux calls it
//!
//! Generic HDLC. A synchronous line is not a device file: the kernel drives it
//! as a network interface -- `hdlc0` and not `/dev/ttyS1` -- and the terms of
//! the line are set with one ioctl on it, `SIOCWANDEV`, carrying the
//! structures in `linux/hdlc/ioctl.h`. There are two halves to the settings,
//! and the attributes here are those two halves:
//!
//! * the **physical** side, `sync_serial_settings`: which electrical standard
//!   the port is wired as, whose clock the bits are on, the rate of that clock
//!   where this end generates it, and whether the port is looped back.
//! * the **framing** side, `raw_hdlc_proto`: how the bits are encoded on the
//!   wire, and what checks a frame.
//!
//! The values are the kernel's own, word for word: `CLOCK_EXT`, `CLOCK_INT`,
//! `CLOCK_TXINT` and `CLOCK_TXFROMRX`; `ENCODING_NRZ` through
//! `ENCODING_MANCHESTER`; and the CRCs the kernel calls a line's `parity`,
//! which this calls `frame_check` because on a synchronous line that is what
//! it is.
//!
//! # Why it is a type of its own
//!
//! It used to be the other half of the `serial` type, reached by saying
//! `asynchronous: false`. Almost nothing about the two is the same: this one
//! is located by an interface rather than a path, its data is frames rather
//! than a stream, its terms are set with an ioctl on a socket rather than with
//! termios on a file descriptor, and its rate is a clock this end may or may
//! not generate. Two types with one word between them was the arrangement that
//! let a file state a clock nothing applied.
//!
//! # No stand-in
//!
//! Every other experimental type has something in the kernel to run it
//! against: `vcan` for a CAN bus, `i2c-stub` for a bus device, `gpio-sim` for
//! discrete lines, `iio_dummy` for a sensor. There is no virtual HDLC
//! interface, so this type needs a card -- a `wanxl`, a `pci200syn`, a
//! `pc300too` or a FarSync -- and the simulator says so where it is asked to
//! stand in for one.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_flag, parse_u32, reject_foreign_fields, reject_stream, require,
    EndpointConfigResult, GroupKind, LinkTerms,
};

/// Which electrical standard the port is wired as.
///
/// The kernel's `IF_IFACE_*` choices for a synchronous serial port. V.35 is
/// what a telecom link commonly is, V.24 is RS-232's own synchronous mode and
/// X.21 the X-series one; `x21d` is the dual-clocking variant a FarSync card
/// offers.
///
/// `t1` and `e1` are deliberately absent. They are telco framings rather than
/// a port's wiring, and the kernel takes a different structure for them --
/// `te1_settings`, with a map of which timeslots this link owns -- which
/// nothing here states. A file asking for one is told that rather than being
/// given a port set up as something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standard {
    V35,
    V24,
    X21,
    X21D,
}

impl Standard {
    /// How this value is spelled in a configuration file.
    pub fn as_str(&self) -> &'static str {
        match self {
            Standard::V35 => "v35",
            Standard::V24 => "v24",
            Standard::X21 => "x21",
            Standard::X21D => "x21d",
        }
    }

    /// Which standard a file's word names, if it names one.
    pub fn from_spelling(text: &str) -> Option<Standard> {
        [Standard::V35, Standard::V24, Standard::X21, Standard::X21D]
            .into_iter()
            .find(|standard| standard.as_str() == text)
    }

    /// Every spelling, for an error to list.
    pub fn spellings() -> &'static str {
        "v35, v24, x21 or x21d"
    }
}

impl fmt::Display for Standard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whose clock a synchronous line's bits are on.
///
/// The kernel's four, from `linux/hdlc/ioctl.h`: a device link is commonly
/// `external`, the equipment at the far end driving the clock, and
/// `internal` where this end drives it. The two mixed settings exist because
/// some links take their transmit clock from one place and their receive
/// clock from another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockType {
    External,
    Internal,
    TxInternal,
    TxFromRx,
}

impl ClockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClockType::External => "external",
            ClockType::Internal => "internal",
            ClockType::TxInternal => "tx_internal",
            ClockType::TxFromRx => "tx_from_rx",
        }
    }

    pub fn from_spelling(text: &str) -> Option<ClockType> {
        [
            ClockType::External,
            ClockType::Internal,
            ClockType::TxInternal,
            ClockType::TxFromRx,
        ]
        .into_iter()
        .find(|clock| clock.as_str() == text)
    }

    /// Whether this end generates the clock the data is sent on.
    ///
    /// Which decides what the data rate means: a rate this end generates, or
    /// a rate recorded against what the far end is expected to clock.
    pub fn is_ours(&self) -> bool {
        matches!(self, ClockType::Internal | ClockType::TxInternal)
    }
}

impl fmt::Display for ClockType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a synchronous line's bits are carried on the wire.
///
/// The kernel's five. NRZ is the plain one; NRZI and the FM codings carry
/// their own clock recovery, which a long cable needs and a short one does
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    Nrz,
    Nrzi,
    FmMark,
    FmSpace,
    Manchester,
}

impl Encoding {
    pub fn as_str(&self) -> &'static str {
        match self {
            Encoding::Nrz => "nrz",
            Encoding::Nrzi => "nrzi",
            Encoding::FmMark => "fm_mark",
            Encoding::FmSpace => "fm_space",
            Encoding::Manchester => "manchester",
        }
    }

    pub fn from_spelling(text: &str) -> Option<Encoding> {
        [
            Encoding::Nrz,
            Encoding::Nrzi,
            Encoding::FmMark,
            Encoding::FmSpace,
            Encoding::Manchester,
        ]
        .into_iter()
        .find(|encoding| encoding.as_str() == text)
    }
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What checks a frame on a synchronous line.
///
/// The kernel calls this a line's parity too, and for a synchronous line it
/// is a frame check rather than a bit per character: the variants are the
/// CRCs `linux/hdlc/ioctl.h` offers, which differ in width, in the value the
/// register starts at, and in whether the ITU-T polynomial is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameCheck {
    None,
    Crc16Pr0,
    Crc16Pr1,
    Crc16Pr0Ccitt,
    Crc16Pr1Ccitt,
    Crc32Pr0Ccitt,
    Crc32Pr1Ccitt,
}

impl FrameCheck {
    pub fn as_str(&self) -> &'static str {
        match self {
            FrameCheck::None => "none",
            FrameCheck::Crc16Pr0 => "crc16_pr0",
            FrameCheck::Crc16Pr1 => "crc16_pr1",
            FrameCheck::Crc16Pr0Ccitt => "crc16_pr0_ccitt",
            FrameCheck::Crc16Pr1Ccitt => "crc16_pr1_ccitt",
            FrameCheck::Crc32Pr0Ccitt => "crc32_pr0_ccitt",
            FrameCheck::Crc32Pr1Ccitt => "crc32_pr1_ccitt",
        }
    }

    pub fn from_spelling(text: &str) -> Option<FrameCheck> {
        [
            FrameCheck::None,
            FrameCheck::Crc16Pr0,
            FrameCheck::Crc16Pr1,
            FrameCheck::Crc16Pr0Ccitt,
            FrameCheck::Crc16Pr1Ccitt,
            FrameCheck::Crc32Pr0Ccitt,
            FrameCheck::Crc32Pr1Ccitt,
        ]
        .into_iter()
        .find(|check| check.as_str() == text)
    }
}

impl fmt::Display for FrameCheck {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What an HDLC node says about its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HdlcParams {
    /// Which electrical standard the port is wired as.
    pub standard: Standard,
    /// Whose clock the bits are on.
    pub clock_type: ClockType,
    /// The clock rate in bits per second, where this end generates the clock.
    ///
    /// `None` where it does not: a rate this end cannot set is a rate it has
    /// no business stating, and the far end's clock is whatever the far end
    /// makes it. See [`ClockType::is_ours`].
    pub clock_rate: Option<u32>,
    /// Whether the port is put in loopback, which is for proving a card
    /// without a line attached.
    pub loopback: bool,
    /// How the bits are carried on the wire.
    pub encoding: Encoding,
    /// What checks a frame: the kernel's own `parity` for raw HDLC.
    pub frame_check: FrameCheck,
}

/// What an HDLC node says about its line, and what it must say.
///
/// A line carries no negotiation, so both ends must be told the same thing:
/// the standard and the clock are required, and a line read with the wrong one
/// of either delivers noise rather than nothing. The encoding and the frame
/// check have the plain defaults the kernel's own are, since a short cable
/// commonly wants neither clock recovery nor a CRC.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(
        name,
        "hdlc",
        &g,
        &[
            "standard",
            "clock_type",
            "clock_rate",
            "loopback",
            "encoding",
            "frame_check",
        ],
    )?;

    // A frame is its own length -- the card delivers whole frames or nothing
    // -- so there is nothing for a rule about where a read ends to say.
    reject_stream(name, "hdlc", g.stream.is_some())?;

    let standard = require(name, "hdlc", "standard", g.standard.as_ref())?;
    let standard =
        Standard::from_spelling(&standard.as_str().to_ascii_lowercase()).ok_or_else(|| {
            bad(
                name,
                &format!(
                    "standard: \"{}\" is not one this knows. There are {}",
                    standard.as_str(),
                    Standard::spellings()
                ),
            )
        })?;

    let clock_type = require(name, "hdlc", "clock_type", g.clock_type.as_ref())?;
    let clock_type = ClockType::from_spelling(&clock_type.as_str().to_ascii_lowercase())
        .ok_or_else(|| {
            bad(
                name,
                &format!(
                    "clock_type: \"{}\" is not one this knows. There are external, \
                     internal, tx_internal and tx_from_rx",
                    clock_type.as_str()
                ),
            )
        })?;

    // The rate belongs to whoever generates the clock. Required where that is
    // this end, and refused where it is not: a rate written against a clock
    // the far end drives is a number nothing here could apply, which is the
    // rule this whole type was rewritten under.
    let clock_rate = match (clock_type.is_ours(), g.clock_rate.as_ref()) {
        (true, Some(stated)) => {
            let rate = parse_u32(name, "clock_rate", stated)?;
            if rate == 0 {
                return Err(bad(
                    name,
                    "clock_rate must be greater than zero: a clock of no bits a \
                     second is a line that carries nothing",
                ));
            }
            Some(rate)
        }
        (true, None) => {
            return Err(bad(
                name,
                &format!(
                    "clock_rate is required with clock_type: {clock_type}, which is \
                     this end generating the clock -- so what rate it generates has \
                     to be said"
                ),
            ))
        }
        (false, Some(_)) => {
            return Err(bad(
                name,
                &format!(
                    "clock_rate does not apply with clock_type: {clock_type}, which \
                     is the far end generating the clock: this end cannot set a rate \
                     it does not drive"
                ),
            ))
        }
        (false, None) => None,
    };

    let loopback = parse_flag(name, "loopback", g.loopback.as_ref())?.unwrap_or(false);

    let encoding = match g.encoding.as_ref() {
        Some(stated) => {
            Encoding::from_spelling(&stated.as_str().to_ascii_lowercase()).ok_or_else(|| {
                bad(
                    name,
                    &format!(
                        "encoding: \"{}\" is not one this knows. There are nrz, nrzi, \
                         fm_mark, fm_space and manchester",
                        stated.as_str()
                    ),
                )
            })?
        }
        None => Encoding::Nrz,
    };

    let frame_check = match g.frame_check.as_ref() {
        Some(stated) => FrameCheck::from_spelling(&stated.as_str().to_ascii_lowercase())
            .ok_or_else(|| {
                bad(
                    name,
                    &format!(
                        "frame_check: \"{}\" is not one this knows. There are none, \
                         crc16_pr0, crc16_pr1, crc16_pr0_ccitt, crc16_pr1_ccitt, \
                         crc32_pr0_ccitt and crc32_pr1_ccitt",
                        stated.as_str()
                    ),
                )
            })?,
        None => FrameCheck::None,
    };

    Ok(GroupKind::Hdlc(HdlcParams {
        standard,
        clock_type,
        clock_rate,
        loopback,
        encoding,
        frame_check,
    }))
}

/// Whether `interface` is a stand-in, which a synchronous line never is:
/// there is no virtual HDLC interface, so such a payload needs a card. See
/// [`crate::simulate`].
pub fn judged(interface: &str) -> crate::simulate::Judged {
    crate::simulate::Judged::hardware(
        format!("{interface} is a synchronous line, which nothing stands in for"),
        "interface",
        "leave the hdlc node out of a run that must exercise no hardware",
    )
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
                "standard" => g.standard = value,
                "clock_type" => g.clock_type = value,
                "clock_rate" => g.clock_rate = value,
                "loopback" => g.loopback = value,
                "encoding" => g.encoding = value,
                "frame_check" => g.frame_check = value,
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    fn params(stated: &[(&str, &str)]) -> HdlcParams {
        match group_kind_of("link", terms(stated)).expect("a line") {
            GroupKind::Hdlc(hdlc) => hdlc,
            other => panic!("expected an HDLC line, got {other:?}"),
        }
    }

    /// A line says how it is wired and whose clock it is on, and the rest has
    /// the kernel's own defaults.
    #[test]
    fn a_line_says_its_standard_and_its_clock() {
        assert_eq!(
            params(&[("standard", "v35"), ("clock_type", "external")]),
            HdlcParams {
                standard: Standard::V35,
                clock_type: ClockType::External,
                clock_rate: None,
                loopback: false,
                encoding: Encoding::Nrz,
                frame_check: FrameCheck::None,
            }
        );

        // However the file capitalised them, as every other word of a link is
        // read.
        assert_eq!(
            params(&[("standard", "X21"), ("clock_type", "TX_FROM_RX")]).standard,
            Standard::X21
        );
    }

    /// Every term is read, and each has the kernel's own spelling.
    #[test]
    fn what_a_line_may_say_about_itself() {
        assert_eq!(
            params(&[
                ("standard", "v24"),
                ("clock_type", "internal"),
                ("clock_rate", "2048000"),
                ("loopback", "true"),
                ("encoding", "nrzi"),
                ("frame_check", "crc16_pr1_ccitt"),
            ]),
            HdlcParams {
                standard: Standard::V24,
                clock_type: ClockType::Internal,
                clock_rate: Some(2_048_000),
                loopback: true,
                encoding: Encoding::Nrzi,
                frame_check: FrameCheck::Crc16Pr1Ccitt,
            }
        );
    }

    /// What a line cannot do without.
    #[test]
    fn a_line_that_says_too_little_is_refused() {
        let said = group_kind_of("link", terms(&[("clock_type", "external")]))
            .expect_err("a line that does not say how it is wired");
        assert!(format!("{said}").contains("standard"), "{said}");

        let said = group_kind_of("link", terms(&[("standard", "v35")]))
            .expect_err("a line that does not say whose clock it is on");
        assert!(format!("{said}").contains("clock_type"), "{said}");
    }

    /// The clock rate belongs to whoever generates the clock.
    #[test]
    fn the_rate_belongs_to_whoever_drives_the_clock() {
        // This end drives it, so the rate is required.
        for ours in ["internal", "tx_internal"] {
            let said = group_kind_of("link", terms(&[("standard", "v35"), ("clock_type", ours)]))
                .expect_err("a clock this end drives at no stated rate");
            assert!(
                format!("{said}").contains("clock_rate is required"),
                "{ours}: {said}"
            );
            assert_eq!(
                params(&[
                    ("standard", "v35"),
                    ("clock_type", ours),
                    ("clock_rate", "64000"),
                ])
                .clock_rate,
                Some(64_000)
            );
        }

        // The far end drives it, so the rate is refused: this end cannot set
        // what it does not drive.
        for theirs in ["external", "tx_from_rx"] {
            let said = group_kind_of(
                "link",
                terms(&[
                    ("standard", "v35"),
                    ("clock_type", theirs),
                    ("clock_rate", "64000"),
                ]),
            )
            .expect_err("a rate on a clock the far end drives");
            assert!(
                format!("{said}").contains("does not apply"),
                "{theirs}: {said}"
            );
        }

        // And nought is not a rate.
        let said = group_kind_of(
            "link",
            terms(&[
                ("standard", "v35"),
                ("clock_type", "internal"),
                ("clock_rate", "0"),
            ]),
        )
        .expect_err("a clock of no bits a second");
        assert!(format!("{said}").contains("clock_rate"), "{said}");
    }

    /// A word that is not one of the values is refused, listing them.
    #[test]
    fn a_word_that_is_not_a_value_is_refused() {
        for (field, value, wanted) in [
            ("standard", "v36", "v35, v24, x21 or x21d"),
            ("clock_type", "sideways", "tx_from_rx"),
            ("encoding", "nrzz", "manchester"),
            ("frame_check", "crc8", "crc32_pr1_ccitt"),
        ] {
            let said = group_kind_of(
                "link",
                terms(&[
                    ("standard", "v35"),
                    ("clock_type", "external"),
                    (field, value),
                ]),
            )
            .expect_err("a word that is not a value");
            assert!(
                format!("{said}").contains(wanted),
                "{field}: the error should list the values: {said}"
            );
        }
    }

    /// A term of another type of link is refused, and so is a stream rule.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of(
            "link",
            terms(&[
                ("standard", "v35"),
                ("clock_type", "external"),
                ("datarate", "9600"),
            ]),
        )
        .expect_err("a start-stop line's rate on a synchronous one");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );

        let mut g = terms(&[("standard", "v35"), ("clock_type", "external")]);
        g.stream = Some(crate::endpoint_config::StreamTerms {
            max_length: Some(Scalar::of("512")),
            timeout: None,
            terminators: None,
            length_prefix: None,
        });
        let said = group_kind_of("link", g).expect_err("a stream rule on frames");
        assert!(format!("{said}").contains("stream"), "{said}");
    }
}
