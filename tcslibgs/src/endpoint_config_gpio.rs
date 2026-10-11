//! GPIO payloads: discrete lines, as events.
//!
//! **Experimental, and not yet supported.** The terms parse and reach a
//! handler, and the handler watches the lines; nothing has been run against
//! hardware, only against the kernel's simulated chip.
//!
//! A payload is not always a stream of data. A deployment switch, an arming
//! line, a thermostat's output, a payload's own "I am alive" line: each is one
//! wire that is high or low, and what the ground wants to know is when it
//! changed. Linux offers those through a GPIO chip's character device, where a
//! program asks for a set of lines with edge detection and then reads events.
//!
//! So this is the one kind whose payload data is made rather than carried. A
//! conduit moves bytes, and a line's change is not bytes, so each event
//! becomes a record of a fixed size: when it happened, which line, and what it
//! became. The size is [`EVENT_BYTES`], and a payload's `packet_size` must be
//! a whole number of them -- a packet that was part of a record would be a
//! reading nobody could use.
//!
//! What a GPIO payload cannot be is triggered. These are inputs: there is
//! nothing to write a request to, and writing would mean driving an output,
//! which is commanding a payload rather than asking it for data.

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, reject_stream, require, EndpointConfigResult,
    GroupKind, LinkTerms, Scalar,
};

/// How many bytes one line event takes as payload data.
///
/// Sixteen: the time in milliseconds since the epoch as eight, the line's
/// offset as four, and what it became as four. The time is in the unit every
/// other time in this project is in, so a line's change and a telemetry
/// response can be read against each other; the offset is there because one
/// payload may watch several lines, and without it an event says only that
/// something moved.
pub const EVENT_BYTES: usize = 16;

/// The greatest number of lines one request may name, which is the kernel's
/// own limit on a line request.
pub const MOST_LINES: usize = 64;

/// Which changes of a line are worth an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    /// Low to high only.
    Rising,
    /// High to low only.
    Falling,
    /// Both, which is the default: a discrete that is read for its state is
    /// read for both of the states it has.
    Both,
}

/// What holds a line that nothing is driving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bias {
    /// Nothing: the line is left as the board wired it, which is the default
    /// and what a line with its own pull resistor wants.
    None,
    /// Held high until something pulls it down.
    PullUp,
    /// Held low until something pulls it up.
    PullDown,
}

/// What a GPIO payload's lines are set to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpioParams {
    /// The offsets of the lines this payload watches, in the order the file
    /// gave them.
    pub lines: Vec<u32>,
    /// Which changes are events.
    pub edge: Edge,
    /// What holds the lines.
    pub bias: Bias,
    /// How long a line must be steady before a change counts, in
    /// microseconds, where the file asks for it.
    ///
    /// Applied by the kernel, which does the debouncing: a mechanical switch
    /// closes several times in a few milliseconds, and a handler that
    /// reported each of them would be reporting the switch's bounce as the
    /// payload's state.
    pub debounce_us: Option<u32>,
}

/// Read a list of line offsets as a file writes one: `0, 1, 5` or `0 1 5`.
///
/// Commas or spaces, decimal or hexadecimal, because a file written for a
/// board commonly copies the offsets out of a schematic in whichever form the
/// schematic used.
pub fn lines_of(name: &str, stated: &Scalar) -> EndpointConfigResult<Vec<u32>> {
    let mut lines = Vec::new();

    for word in stated
        .as_str()
        .split([',', ' ', '\t'])
        .filter(|word| !word.trim().is_empty())
    {
        let offset = parse_u32(name, "lines", &Scalar::of(word.trim()))?;
        if lines.contains(&offset) {
            return Err(bad(
                name,
                &format!(
                    "lines names offset {offset} twice: a line watched twice would \
                     report every change of it twice"
                ),
            ));
        }
        lines.push(offset);
    }

    if lines.is_empty() {
        return Err(bad(
            name,
            "lines must name at least one line: a payload watching no lines has \
             nothing to report",
        ));
    }

    if lines.len() > MOST_LINES {
        return Err(bad(
            name,
            &format!(
                "lines names {} lines, and one request may name {MOST_LINES}",
                lines.len()
            ),
        ));
    }

    Ok(lines)
}

/// What a GPIO payload says about its lines, and what it must say.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(name, "gpio", &g, &["lines", "edge", "bias", "debounce_us"])?;

    // An event is its own length, so there is nothing for a rule about where
    // a read ends to say.
    reject_stream(name, "gpio", g.stream.is_some())?;

    let lines = require(name, "gpio", "lines", g.lines.as_ref())?;
    let lines = lines_of(name, lines)?;

    let edge = match g.edge.as_ref() {
        Some(stated) => match stated.as_str().to_ascii_lowercase().as_str() {
            "rising" => Edge::Rising,
            "falling" => Edge::Falling,
            "both" => Edge::Both,
            other => {
                return Err(bad(
                    name,
                    &format!("edge: \"{other}\" is not rising, falling or both"),
                ))
            }
        },
        None => Edge::Both,
    };

    let bias = match g.bias.as_ref() {
        Some(stated) => match stated.as_str().to_ascii_lowercase().as_str() {
            "none" => Bias::None,
            "pull_up" => Bias::PullUp,
            "pull_down" => Bias::PullDown,
            other => {
                return Err(bad(
                    name,
                    &format!("bias: \"{other}\" is not none, pull_up or pull_down"),
                ))
            }
        },
        None => Bias::None,
    };

    let debounce_us = match g.debounce_us.as_ref() {
        Some(stated) => {
            let us = parse_u32(name, "debounce_us", stated)?;
            if us == 0 {
                return Err(bad(
                    name,
                    "debounce_us must be greater than zero: a line that need not be \
                     steady at all is a line with no debouncing, which is what \
                     stating nothing asks for",
                ));
            }
            Some(us)
        }
        None => None,
    };

    Ok(GroupKind::Gpio(GpioParams {
        lines,
        edge,
        bias,
        debounce_us,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(stated: &[(&str, &str)]) -> LinkTerms {
        let mut g = LinkTerms::default();
        for (field, value) in stated {
            let value = Some(Scalar::of(*value));
            match *field {
                "lines" => g.lines = value,
                "edge" => g.edge = value,
                "bias" => g.bias = value,
                "debounce_us" => g.debounce_us = value,
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    fn params(stated: &[(&str, &str)]) -> GpioParams {
        match group_kind_of("discretes", terms(stated)).expect("lines") {
            GroupKind::Gpio(gpio) => gpio,
            other => panic!("expected GPIO lines, got {other:?}"),
        }
    }

    /// A payload watches the lines it names, both edges, nothing holding
    /// them.
    #[test]
    fn a_payload_watches_the_lines_it_names() {
        assert_eq!(
            params(&[("lines", "0, 1, 5")]),
            GpioParams {
                lines: vec![0, 1, 5],
                edge: Edge::Both,
                bias: Bias::None,
                debounce_us: None,
            }
        );
    }

    /// Offsets may be written as a schematic writes them.
    #[test]
    fn offsets_are_read_in_either_notation() {
        assert_eq!(params(&[("lines", "0 1 17")]).lines, vec![0, 1, 17]);
        assert_eq!(params(&[("lines", "0x0a,0x0b")]).lines, vec![10, 11]);
        assert_eq!(params(&[("lines", " 3 ")]).lines, vec![3]);
    }

    /// A payload that names no lines, or one twice, is refused.
    #[test]
    fn a_payload_names_lines_once_each_and_at_least_one() {
        let said = group_kind_of("discretes", terms(&[("lines", "")]))
            .expect_err("a payload watching nothing");
        assert!(
            format!("{said}").contains("at least one"),
            "the error should say what is wanted: {said}"
        );

        let said = group_kind_of("discretes", terms(&[("lines", "4, 4")]))
            .expect_err("one line watched twice");
        assert!(
            format!("{said}").contains("twice"),
            "the error should say what is wrong: {said}"
        );

        let many: Vec<String> = (0..=MOST_LINES as u32).map(|n| n.to_string()).collect();
        let said = group_kind_of("discretes", terms(&[("lines", &many.join(","))]))
            .expect_err("more lines than a request may name");
        assert!(
            format!("{said}").contains("64"),
            "the error should say the limit: {said}"
        );
    }

    /// The edge, the bias and the debounce are read, and a word that is
    /// neither is refused.
    #[test]
    fn what_a_payload_may_say_about_its_lines() {
        let said = params(&[
            ("lines", "2"),
            ("edge", "falling"),
            ("bias", "pull_up"),
            ("debounce_us", "5000"),
        ]);
        assert_eq!(said.edge, Edge::Falling);
        assert_eq!(said.bias, Bias::PullUp);
        assert_eq!(said.debounce_us, Some(5000));

        for (field, value, expected) in [
            ("edge", "sideways", "rising, falling or both"),
            ("bias", "pull_sideways", "none, pull_up or pull_down"),
        ] {
            let said = group_kind_of("discretes", terms(&[("lines", "0"), (field, value)]))
                .expect_err("a word that is not a value");
            assert!(
                format!("{said}").contains(expected),
                "the error should list the values: {said}"
            );
        }

        let said = group_kind_of("discretes", terms(&[("lines", "0"), ("debounce_us", "0")]))
            .expect_err("no debouncing at all");
        assert!(
            format!("{said}").contains("debounce_us"),
            "the error should name the term: {said}"
        );
    }

    /// A term of another kind of link is refused.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of("discretes", terms(&[("lines", "0"), ("datarate", "9600")]))
            .expect_err("a line's rate on a set of discretes");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }

    /// An event is sixteen bytes: a time, a line and a value.
    #[test]
    fn an_event_is_a_time_a_line_and_a_value() {
        assert_eq!(EVENT_BYTES, 8 + 4 + 4);
    }
}
