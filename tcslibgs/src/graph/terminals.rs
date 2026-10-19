//! What a link may be attached to: a node's data sinks and data sources.
//!
//! A link joins one node's data source to another's data sink. Each end has a
//! name, and the name says two things about the data and nothing about what is
//! at the other end: whose it is, and whether it arrives here or leaves.
//! `payload-` is a payload's own data, on its way to the ground; `moc-` is
//! what the ground sends, on its way to a payload. `-in` is where it arrives
//! and `-out` is where it leaves -- so every `-out` is a data source and every
//! `-in` a data sink, which is the whole of the rule a reader has to remember.
//!
//! So one stream is named twice -- `payload-out` where it leaves a node and
//! `payload-in` where it arrives at the next -- and the two words are a
//! sentence about the same bytes read from either end.
//!
//! Two streams, and every terminal belongs to one of them:
//!
//! | stream      | leaves at     | arrives at   |
//! |-------------|---------------|--------------|
//! | groundward  | `payload-out` | `payload-in` |
//! | vehicleward | `moc-out`     | `moc-in`     |
//!
//! Groundward is a payload's readings on their way to the ground, which is why
//! those two are the `payload-` pair; vehicleward is what the ground sends
//! back -- a command, a trigger, a setpoint -- which is the `moc-` pair.
//!
//! # Which types this knows
//!
//! The types whose terminals are fixed: a payload kind, a `moc`, the `null`
//! and `ping` vehicle nodes, and the five app node types. A computation's sinks are the inputs it declares and a
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
/// `payload-out` row to be got wrong in.
const KNOWN: &[(&str, bool, Stream)] = &[
    // name, whether it is a sink, which stream.
    ("payload-out", false, Stream::Groundward),
    ("payload-in", true, Stream::Groundward),
    ("moc-out", false, Stream::Vehicleward),
    ("moc-in", true, Stream::Vehicleward),
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

/// The terminal at the other end of this one's stream.
///
/// The pairing, as four sentences: a `payload-out` may only be linked to a
/// `payload-in`, a `payload-in` only to a `payload-out`, a `moc-out` only to
/// a `moc-in`, and a `moc-in` only to a `moc-out`. Which is one
/// sentence twice over -- a stream leaves at one terminal and arrives at the
/// other -- and that is why it is derived from `KNOWN` rather than written
/// out again: the two halves of the table cannot drift apart.
///
/// `None` for a name that is not one of the four, which is a computation's own
/// input: what may be linked to one of those is the computation's business.
pub fn answers(terminal: &str) -> Option<&'static str> {
    let (sink, stream) = stream_of(terminal)?;
    KNOWN
        .iter()
        .find(|(_, theirs, carries)| *carries == stream && *theirs != sink)
        .map(|(name, _, _)| *name)
}

/// Whether a link may leave by one terminal and arrive at the other.
///
/// Both ends of one stream, the source end and the sink end. Used by the
/// checking, which refuses a configuration that says otherwise, and by the
/// editor, which will not draw one in the first place: a gesture that cannot
/// make a bad link is better than a bad link reported after the fact.
pub fn pairs_with(leaves: &str, arrives: &str) -> bool {
    answers(leaves) == Some(arrives) && stream_of(leaves).is_some_and(|(sink, _)| !sink)
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
    /// one source needs no names in the file -- `{ from: temp_1, to: moc }`
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
        sinks: &["moc-in"],
        sources: &["payload-out"],
        merges: false,
    };
    const MOC: Terminals = Terminals {
        // And the ground is the other end of both: it receives the readings
        // and sends what asks for them.
        sinks: &["payload-in"],
        sources: &["moc-out"],
        // Every payload's telemetry arrives here, told apart by the payload
        // id its socket carries: see the field.
        merges: true,
    };
    const NULL: Terminals = Terminals {
        // The simplest vehicle node: one terminal at each end of each stream,
        // and nothing done to what passes through. It is what says what a
        // vehicle node *is*, which is why it is the one with all four.
        sinks: &["payload-in", "moc-in"],
        sources: &["payload-out", "moc-out"],
        merges: false,
    };

    // An app node is a program run with one or both of two file interfaces,
    // and its terminals are which of them it has: see `crate::node::app`. The
    // pairing is the point -- on the interface where it reads payload data it
    // writes MOC data -- so the terminals of the first kind are the ground
    // link's, and of the second a payload's.
    const APP_PAYLOAD: Terminals = Terminals {
        // Payload data read, MOC data written: it takes what travels
        // groundward and answers with what travels vehicleward.
        sinks: &["payload-in"],
        sources: &["moc-out"],
        // One source into it, for the reason a vehicle node has one: a packet
        // carries no word for which node it came from, so two sources would be
        // two streams the program could not tell apart. The ground merges
        // because a payload id is in the socket it arrives on.
        merges: false,
    };
    const APP_MOC: Terminals = Terminals {
        // And the mirror of it: MOC data read, payload data written.
        sinks: &["moc-in"],
        sources: &["payload-out"],
        merges: false,
    };
    const APP_ALL: Terminals = Terminals {
        // Both interfaces, so all four terminals -- which is a `null` node's
        // shape, and an app of this kind is what a `null` node is when
        // somebody wants to do something to what passes through.
        sinks: &["payload-in", "moc-in"],
        sources: &["payload-out", "moc-out"],
        merges: false,
    };
    const APP_PL: Terminals = Terminals {
        // A payload that is a program and is never asked: its readings leave
        // and nothing arrives. The one type in this table with no sink at all,
        // which is the whole of what tells it from the payload below: a link
        // into it would be a datum written to a program that does not read.
        sinks: &[],
        sources: &["payload-out"],
        merges: false,
    };

    match node_type {
        "i2c" | "spi" | "serial" | "hdlc" | "can" | "iio" | "rpmsg" | "network" | "device-file"
        | "gpio" => Some(PAYLOAD),
        "moc" => Some(MOC),
        "null" => Some(NULL),
        "app_payload_veh" => Some(APP_PAYLOAD),
        // A node that answers the ground's pings takes what the ground sends
        // and sends its answer groundward, which is an `app_moc_veh`'s shape:
        // the same table, because it is the same fact. See
        // `crate::node::vehicle::ping`.
        "app_moc_veh" | "ping" => Some(APP_MOC),
        "app_all_veh" => Some(APP_ALL),
        "app_pl" => Some(APP_PL),
        // A payload that is a program and is asked, which is a payload: the
        // same table rather than a copy of it, because the two are the same
        // fact about the same shape.
        "app_all_pl" => Some(PAYLOAD),
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
            // None at all, which is not a link that failed to say which: it is
            // a link that cannot be made. An `app_pl` is a payload nothing
            // asks, so nothing may arrive at it, and a complaint asking which
            // of no terminals was meant would send a reader looking for a name
            // to write.
            None if ours.is_empty() => Err(format!(
                "a {node_type} node has no {which} at all, so nothing may be \
                 linked to it there"
            )),
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
        assert_eq!(stream_of("payload-out"), Some((false, Stream::Groundward)));
        assert_eq!(stream_of("payload-in"), Some((true, Stream::Groundward)));
        assert_eq!(stream_of("moc-out"), Some((false, Stream::Vehicleward)));
        assert_eq!(stream_of("moc-in"), Some((true, Stream::Vehicleward)));
        // A computation's own input is not one of them.
        assert_eq!(stream_of("in0"), None);

        // One source and one sink per stream, which is what makes "the
        // groundward stream leaves at payload-out and arrives at payload-in" a
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

    /// Each terminal may be linked to exactly one other, and which.
    ///
    /// The four sentences the editor is held to: a `payload-out` to a
    /// `payload-in`, a `payload-in` to a `payload-out`, a `moc-out` to a
    /// `moc-in`, a `moc-in` to a `moc-out`. Anything else is a link
    /// that would carry one stream into the other.
    #[test]
    fn a_terminal_may_be_linked_to_one_other_terminal() {
        assert_eq!(answers("payload-out"), Some("payload-in"));
        assert_eq!(answers("payload-in"), Some("payload-out"));
        assert_eq!(answers("moc-out"), Some("moc-in"));
        assert_eq!(answers("moc-in"), Some("moc-out"));
        assert_eq!(answers("in0"), None);

        // And the pairing is from the source end to the sink end: a link
        // leaves one and arrives at the other, so the same two names the
        // other way about is a link drawn backwards.
        assert!(pairs_with("payload-out", "payload-in"));
        assert!(!pairs_with("payload-in", "payload-out"));
        assert!(pairs_with("moc-out", "moc-in"));
        assert!(!pairs_with("moc-in", "moc-out"));

        // Nothing else pairs with anything: the two streams never meet.
        let every = ["payload-out", "payload-in", "moc-out", "moc-in"];
        for leaves in every {
            for arrives in every {
                assert_eq!(
                    pairs_with(leaves, arrives),
                    answers(leaves) == Some(arrives)
                        && !stream_of(leaves).expect("one of the four").0,
                    "{leaves} to {arrives}"
                );
            }
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
        assert_eq!(payload.sources, &["payload-out"]);
        assert_eq!(moc.sinks, &["payload-in"]);
        assert_eq!(moc.sources, &["moc-out"]);
        assert_eq!(payload.sinks, &["moc-in"]);
    }

    /// The null vehicle node has all four, which is what makes it the one
    /// that says what a vehicle node is.
    #[test]
    fn the_null_node_has_both_ends_of_both_streams() {
        let null = terminals_of("null").expect("the null node");
        for name in ["payload-in", "moc-in"] {
            assert!(null.sinks.contains(&name), "{name} is not a sink of null");
        }
        for name in ["payload-out", "moc-out"] {
            assert!(
                null.sources.contains(&name),
                "{name} is not a source of null"
            );
        }
    }

    /// A ping node hears the ground and answers it: what the ground sends
    /// arrives, and the answer leaves groundward, to arrive at a `moc` node.
    #[test]
    fn a_ping_node_hears_the_ground_and_answers_it() {
        let ping = terminals_of("ping").expect("the ping node");
        assert_eq!(ping.sinks, &["moc-in"]);
        assert_eq!(ping.sources, &["payload-out"]);
        assert!(!ping.merges);
        assert!(pairs_with("moc-out", ping.sinks[0]));
        assert!(pairs_with(ping.sources[0], "payload-in"));
    }

    /// Each app kind has the terminals its interfaces are.
    ///
    /// The five of them in one test, because the rule is one rule: a program
    /// run with an interface has the pair of terminals that interface carries,
    /// and a payload that is a program has a payload's.
    #[test]
    fn an_app_has_the_terminals_its_interfaces_are() {
        // The vehicle's own three. On the interface where it reads payload
        // data it writes MOC data, so the first wears the ground link's shape
        // and the second a payload's.
        let payload_veh = terminals_of("app_payload_veh").expect("a kind of app");
        assert_eq!(payload_veh.sinks, &["payload-in"]);
        assert_eq!(payload_veh.sources, &["moc-out"]);
        assert_eq!(
            terminals_of("app_moc_veh"),
            terminals_of("i2c"),
            "a node that is asked and answers wears a payload's shape"
        );
        assert_eq!(terminals_of("app_all_veh"), terminals_of("null"));

        // And the two that are payloads. The asked one is a payload, down to
        // the table it reads from; the one that is never asked is a payload
        // with the sink taken away, which is the only type here with none.
        assert_eq!(terminals_of("app_all_pl"), terminals_of("i2c"));
        let pl = terminals_of("app_pl").expect("a kind of app");
        assert_eq!(pl.sources, &["payload-out"]);
        assert!(pl.sinks.is_empty(), "nothing is asked of it");
        assert!(!pl.merges);
    }

    /// A node with no terminal at that end says so, and does not ask which.
    ///
    /// The complaint for an `app_pl` somebody has drawn a line to. "Which of
    /// its none" would send a reader looking for a name to write, where what
    /// they have to know is that there is nowhere for the line to go.
    #[test]
    fn a_link_to_a_node_with_no_sink_is_told_there_is_none() {
        let said = terminal_at("app_pl", None, true).expect_err("nothing may arrive");
        assert!(said.contains("no data sink at all"), "{said}");
        assert!(!said.contains("which of them"), "{said}");

        // Named, and the name is one of the four: still refused, and by the
        // rule that was there already.
        let said = terminal_at("app_pl", Some("moc-in"), true).expect_err("nothing may arrive");
        assert!(said.contains("none at all"), "{said}");

        // What it does have needs no naming.
        assert_eq!(
            terminal_at("app_pl", None, false),
            Ok(Some("payload-out".to_string()))
        );
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
            Ok(Some("payload-out".to_string())),
            "a payload sends on its only source"
        );
        assert_eq!(
            terminal_at("moc", None, true),
            Ok(Some("payload-in".to_string())),
            "the ground receives on its only sink"
        );

        // And a node with two of them has to be told which: the null node is
        // both ends of both streams, so an unnamed link to it could be either.
        let said = terminal_at("null", None, true).expect_err("ambiguous");
        assert!(
            said.contains("payload-in") && said.contains("moc-in"),
            "{said}"
        );
    }

    /// A name the type does not have, and a name at the wrong end.
    ///
    /// Two different mistakes and two different complaints. A name the type
    /// does not have is a typo or a type confusion; a name at the wrong end is
    /// a link drawn backwards, which is worth saying in those words.
    #[test]
    fn a_terminal_is_refused_by_name_and_by_end() {
        let said = terminal_at("i2c", Some("to_ground"), false).expect_err("no such");
        assert!(
            said.contains("to_ground") && said.contains("payload-out"),
            "{said}"
        );

        let said = terminal_at("moc", Some("moc-out"), true).expect_err("wrong end");
        assert!(
            said.contains("moc-out") && said.contains("leaves"),
            "{said}"
        );
        let said = terminal_at("null", Some("moc-in"), false).expect_err("wrong end");
        assert!(
            said.contains("moc-in") && said.contains("arrives"),
            "{said}"
        );
    }
}
