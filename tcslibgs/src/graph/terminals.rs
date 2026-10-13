//! What a link may be attached to: a node's data sinks and data sources.
//!
//! A link joins one node's data source to another's data sink. Each end has a
//! name, and the name says which way the data is travelling rather than what
//! is at the other end of it: `to_moc` is data leaving for the ground,
//! `from_payload` is data arriving from the payload side. So one stream is
//! named twice -- `to_moc` where it leaves and `from_payload` where it arrives
//! -- and the two words are a sentence about the same bytes read from either
//! end.
//!
//! Two streams, and every terminal belongs to one of them:
//!
//! | stream       | leaves at     | arrives at      |
//! |--------------|---------------|-----------------|
//! | groundward   | `to_moc`      | `from_payload`  |
//! | vehicleward  | `to_payload`  | `from_moc`      |
//!
//! Groundward is a payload's readings on their way to the ground; vehicleward
//! is what the ground sends back -- a command, a trigger, a setpoint.
//!
//! # Which types this knows
//!
//! The types whose terminals are fixed: a payload kind, a `moc`, and the
//! `null` vehicle node. A computation's sinks are the inputs it declares and a
//! log's are whatever is written to it, so their terminals are their own rules'
//! business -- see [`crate::graph::check`] -- and [`terminals_of`] says so by
//! answering `None`.

/// Which way the data on a terminal is travelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// A payload's readings, on their way to the ground.
    Groundward,
    /// What the ground sends back: a command, a trigger, a setpoint.
    Vehicleward,
}

impl Stream {
    /// What this stream is called in a complaint.
    pub fn label(self) -> &'static str {
        match self {
            Stream::Groundward => "groundward",
            Stream::Vehicleward => "vehicleward",
        }
    }
}

/// The four terminal names, and what each one is.
///
/// One table, because the name, the end it belongs at and the stream it
/// carries are one fact about it: three tables would be three places for the
/// `to_moc` row to be got wrong in.
const KNOWN: &[(&str, bool, Stream)] = &[
    // name, whether it is a sink, which stream.
    ("to_moc", false, Stream::Groundward),
    ("from_payload", true, Stream::Groundward),
    ("to_payload", false, Stream::Vehicleward),
    ("from_moc", true, Stream::Vehicleward),
];

/// Which stream a terminal carries, and whether it is a sink.
///
/// `None` for a name that is not one of the four -- a computation's `in0`, for
/// instance, which is a sink its own rules know about.
pub fn stream_of(terminal: &str) -> Option<(bool, Stream)> {
    KNOWN
        .iter()
        .find(|(name, _, _)| *name == terminal)
        .map(|(_, sink, stream)| (*sink, *stream))
}

/// A node type's data sinks and data sources, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terminals {
    /// Where data arrives. A sink is either absent -- nothing linked to it --
    /// or linked to exactly one source: two sources into one sink would be two
    /// streams interleaved, and nothing downstream could tell them apart.
    /// Unless [`merges`](Terminals::merges).
    pub sinks: &'static [&'static str],
    /// Where data leaves. A source is either absent or linked to one sink or
    /// several: one reading reaching the ground and an on-board computation at
    /// once is the ordinary case.
    pub sources: &'static [&'static str],
    /// Whether a sink of this type takes more than one source.
    ///
    /// The ground does, and it is the only one that does. A sink is held to
    /// one source because two would interleave two streams that nothing
    /// downstream could tell apart -- but the ground can tell them apart: each
    /// payload's telemetry arrives at its own socket, `source_base` plus the
    /// payload's id (see [`crate::types::HostPort::at`]), and the id is what
    /// says which panel a datagram belongs on. So every payload aboard
    /// reporting to one `moc` node is the ordinary shape of a configuration,
    /// and it is what the shipped examples are.
    ///
    /// A vehicle node is not the ground and does not merge: two streams into
    /// one of its sinks arrive on one link, in whatever order they were sent,
    /// with nothing to say which was which.
    pub merges: bool,
}

impl Terminals {
    /// The one terminal of this kind, where there is exactly one.
    ///
    /// What a link end that names no terminal means. A node with one sink and
    /// one source needs no names in the file -- `{ from: temp_fore, to: moc }`
    /// is unambiguous -- and one with two of either has to be told which.
    fn only(of: &'static [&'static str]) -> Option<&'static str> {
        match of {
            [one] => Some(one),
            _ => None,
        }
    }
}

/// The sinks and sources of this type, where they are fixed.
///
/// `None` where they are not: a computation's sinks are the inputs it declares
/// and a log's are whatever is written to it, so there is no table to give.
pub fn terminals_of(node_type: &str) -> Option<Terminals> {
    const PAYLOAD: Terminals = Terminals {
        // A payload receives what asks it for a reading -- a trigger, a
        // command, a byte written to a line -- and sends what it read.
        sinks: &["from_moc"],
        sources: &["to_moc"],
        merges: false,
    };
    const MOC: Terminals = Terminals {
        // And the ground is the other end of both: it receives the readings
        // and sends what asks for them.
        sinks: &["from_payload"],
        sources: &["to_payload"],
        // Every payload's telemetry arrives here, told apart by the payload
        // id its socket carries: see the field.
        merges: true,
    };
    const NULL: Terminals = Terminals {
        // The simplest vehicle node: one terminal at each end of each stream,
        // and nothing done to what passes through. It is what says what a
        // vehicle node *is*, which is why it is the one with all four.
        sinks: &["from_payload", "from_moc"],
        sources: &["to_moc", "to_payload"],
        merges: false,
    };

    match node_type {
        "i2c" | "spi" | "serial" | "hdlc" | "can" | "iio" | "rpmsg" | "network"
        | "device-file" | "gpio" => Some(PAYLOAD),
        "moc" => Some(MOC),
        "null" => Some(NULL),
        _ => None,
    }
}

/// Whether a sink of this type takes more than one source.
///
/// The ground does and nothing else does: see [`Terminals::merges`]. A type
/// whose terminals are its own business answers `false`, which is the rule a
/// computation's inputs are held to already -- a mean over one reading and
/// itself is the fault [`crate::graph::check::Problem::InputBoundTwice`]
/// exists for.
pub fn merges(node_type: &str) -> bool {
    terminals_of(node_type).is_some_and(|terminals| terminals.merges)
}

/// Which terminal a link end means, and what is wrong with it if anything.
///
/// `named` is what the file wrote after the dot, where it wrote one. A node
/// whose terminals are fixed is held to them: a name it does not have is
/// refused, a sink named where a source belongs is refused, and an end that
/// names nothing is refused where the type has more than one terminal to
/// choose between. A type whose terminals are its own business answers
/// whatever was written, for its own rules to judge.
///
/// `sink` says which end of the link this is: `true` for where the data
/// arrives, `false` for where it leaves.
pub fn terminal_at(
    node_type: &str,
    named: Option<&str>,
    sink: bool,
) -> Result<Option<String>, String> {
    let Some(terminals) = terminals_of(node_type) else {
        return Ok(named.map(str::to_string));
    };
    let (ours, theirs, which) = if sink {
        (terminals.sinks, terminals.sources, "data sink")
    } else {
        (terminals.sources, terminals.sinks, "data source")
    };

    match named {
        Some(named) if ours.contains(&named) => Ok(Some(named.to_string())),
        // Named, and it is the type's other kind of terminal: said as the
        // mistake it is rather than as a name nothing knows, because the name
        // is right and the end is wrong.
        Some(named) if theirs.contains(&named) => Err(format!(
            "\"{named}\" is not a {which} of a {node_type} node -- it is where \
             data {}, and this is the end where it {}",
            if sink { "leaves" } else { "arrives" },
            if sink { "arrives" } else { "leaves" }
        )),
        Some(named) => Err(format!(
            "a {node_type} node has no {which} called \"{named}\". Its {which}s \
             are {}",
            said(ours)
        )),
        // Nothing named, which is every link in a file written before there
        // were names for these: one terminal needs no naming.
        None => match Terminals::only(ours) {
            Some(only) => Ok(Some(only.to_string())),
            None => Err(format!(
                "a {node_type} node has {} {which}s, so a link has to say \
                 which of them: {}",
                ours.len(),
                said(ours)
            )),
        },
    }
}

/// A list of names, for a complaint.
fn said(names: &[&str]) -> String {
    match names {
        [] => "none at all".to_string(),
        _ => names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four names, each at one end of one stream.
    ///
    /// The table itself, because everything below reads from it: a name in two
    /// streams, or two names at the same end of one, would make the pairing
    /// rule say something that is not true.
    #[test]
    fn each_terminal_is_one_end_of_one_stream() {
        assert_eq!(stream_of("to_moc"), Some((false, Stream::Groundward)));
        assert_eq!(stream_of("from_payload"), Some((true, Stream::Groundward)));
        assert_eq!(stream_of("to_payload"), Some((false, Stream::Vehicleward)));
        assert_eq!(stream_of("from_moc"), Some((true, Stream::Vehicleward)));
        // A computation's own input is not one of them.
        assert_eq!(stream_of("in0"), None);

        // One source and one sink per stream, which is what makes "the
        // groundward stream leaves at to_moc and arrives at from_payload" a
        // sentence rather than a guess.
        for stream in [Stream::Groundward, Stream::Vehicleward] {
            let ends: Vec<bool> = KNOWN
                .iter()
                .filter(|(_, _, theirs)| *theirs == stream)
                .map(|(_, sink, _)| *sink)
                .collect();
            assert_eq!(ends.len(), 2, "{}", stream.label());
            assert!(ends.contains(&true) && ends.contains(&false));
        }
    }

    /// A payload and the ground are the two ends of both streams.
    #[test]
    fn a_payload_and_the_ground_face_each_other() {
        let payload = terminals_of("i2c").expect("a payload kind");
        let moc = terminals_of("moc").expect("the ground link");

        // What the payload sends arrives at the ground, and what the ground
        // sends arrives at the payload: the same two streams, named from each
        // end.
        assert_eq!(payload.sources, &["to_moc"]);
        assert_eq!(moc.sinks, &["from_payload"]);
        assert_eq!(moc.sources, &["to_payload"]);
        assert_eq!(payload.sinks, &["from_moc"]);
    }

    /// The null vehicle node has all four, which is what makes it the one
    /// that says what a vehicle node is.
    #[test]
    fn the_null_node_has_both_ends_of_both_streams() {
        let null = terminals_of("null").expect("the null node");
        for name in ["from_payload", "from_moc"] {
            assert!(null.sinks.contains(&name), "{name} is not a sink of null");
        }
        for name in ["to_moc", "to_payload"] {
            assert!(null.sources.contains(&name), "{name} is not a source of null");
        }
    }

    /// The ground takes several sources into its sink, and nothing else does.
    ///
    /// Not an exception made to let the shipped examples through: the ground
    /// is the one place that can tell interleaved streams apart, because each
    /// payload's telemetry arrives at its own socket and the payload id is
    /// what says whose it is. A vehicle node has one link and no such label.
    #[test]
    fn only_the_ground_takes_several_sources_at_one_sink() {
        assert!(merges("moc"));
        assert!(!merges("i2c"));
        assert!(!merges("null"));
        // And a type with rules of its own is held to one, which is what a
        // mean's inputs are held to.
        assert!(!merges("compute"));
        assert!(!merges("log"));
    }

    /// A type whose terminals are its own business says so.
    #[test]
    fn a_computation_keeps_its_own_terminals() {
        assert_eq!(terminals_of("compute"), None);
        assert_eq!(terminals_of("log"), None);
        assert_eq!(terminals_of("contraption"), None);
        // And a name written on one is answered as written, for its own rules
        // to judge.
        assert_eq!(
            terminal_at("compute", Some("in0"), true),
            Ok(Some("in0".to_string()))
        );
        assert_eq!(terminal_at("compute", None, true), Ok(None));
    }

    /// A link end that names nothing takes the one terminal, where there is
    /// one.
    #[test]
    fn an_unnamed_end_is_the_only_terminal() {
        assert_eq!(
            terminal_at("i2c", None, false),
            Ok(Some("to_moc".to_string())),
            "a payload sends on its only source"
        );
        assert_eq!(
            terminal_at("moc", None, true),
            Ok(Some("from_payload".to_string())),
            "the ground receives on its only sink"
        );

        // And a node with two of them has to be told which: the null node is
        // both ends of both streams, so an unnamed link to it could be either.
        let said = terminal_at("null", None, true).expect_err("ambiguous");
        assert!(said.contains("from_payload") && said.contains("from_moc"), "{said}");
    }

    /// A name the type does not have, and a name at the wrong end.
    ///
    /// Two different mistakes and two different complaints. A name the type
    /// does not have is a typo or a type confusion; a name at the wrong end is
    /// a link drawn backwards, which is worth saying in those words.
    #[test]
    fn a_terminal_is_refused_by_name_and_by_end() {
        let said = terminal_at("i2c", Some("to_ground"), false).expect_err("no such");
        assert!(said.contains("to_ground") && said.contains("to_moc"), "{said}");

        let said = terminal_at("moc", Some("to_payload"), true).expect_err("wrong end");
        assert!(
            said.contains("to_payload") && said.contains("leaves"),
            "{said}"
        );
        let said = terminal_at("null", Some("from_moc"), false).expect_err("wrong end");
        assert!(said.contains("from_moc") && said.contains("arrives"), "{said}");
    }
}
