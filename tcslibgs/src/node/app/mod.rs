//! What an app node says, and what is wrong with what it says.
//!
//! An app node is a node whose work is a program of its own, run by tcspecial,
//! that takes part in the graph: data reaches it, it answers, and what it
//! answers goes on to wherever the configuration says. What a built-in node
//! cannot be -- a part of the arrangement written in something else, or
//! changed without rebuilding the flight software.
//!
//! Five node types, which differ in which of the two data interfaces the
//! program is run with and on which side of the graph the node stands. Three
//! are the vehicle's own, parts of the telemetry and control system itself:
//!
//! * `app_payload_veh` reads payload data and writes MOC data, so it is a sink
//!   for what travels groundward and a source of what travels vehicleward. The
//!   same shape the ground link has, which is the thing an app of this kind
//!   stands in for: something that consumes readings and answers with
//!   commands.
//! * `app_moc_veh` is the mirror of it: MOC data in, payload data out.
//! * `app_all_veh` has both, which makes it the one kind that can carry a
//!   stream *through* -- payload data in at the one interface and out at the
//!   other -- and so the one kind that can stand where a `null` node stands.
//!
//! And two are payloads: what the vehicle carries this flight, where what it
//! carries is a program rather than a device on a bus. A payload sends its
//! readings and is sent what asks for them, so neither of these is run with
//! the payload interface: payload data is what they *write*.
//!
//! * `app_pl` writes payload data and reads nothing. A payload that is never
//!   asked -- it reports of its own accord -- so its one terminal is a
//!   `payload-out` and nothing may be linked into it.
//! * `app_all_pl` reads MOC data and writes payload data, which is the shape
//!   every other payload has: it is asked for a reading, by a trigger or a
//!   command, and answers with one.
//!
//! One module for the five because what a file may say about one is what it
//! may say about any: a program to run, and what to do when it stops. The kind
//! is a field rather than five structs, so that a rule about app nodes is
//! written once -- and the module sits beside the two families rather than in
//! one of them for the same reason. Which family a given kind belongs to is
//! [`Kind::is_payload`], which `crate::graph::section_of` reads.
//!
//! What the interfaces are, what a packet is, and what the program itself
//! holds is `tcsapp`. None of that is here: this is what a configuration says,
//! which is read by the editor and the ground tool as well as by the flight
//! software.
//!
//! Flexibility is last in this project's ranking and this is a piece that buys
//! it, so the arrangement is strict in the way
//! [`crate::node::vehicle::compute`]'s `process` is: the program is named in
//! the configuration and started when it is loaded, so one that cannot be
//! executed is a fault found on the ground; and `restart` is declared, bounded
//! and `never` by default, because a part of the control system that restarts
//! under load is worse than one that stays stopped and says so.

use std::collections::BTreeMap;

use super::vehicle::Restart;
use crate::endpoint_config::Scalar;

/// Which kind of app node this is: which interfaces, and which side.
///
/// The two questions are one enumeration because an app node is both at once
/// -- a program run with certain interfaces, standing on one side of the graph
/// -- and a configuration names the pair of them with a single word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Payload data is read and MOC data written, by a node of the vehicle's
    /// own: the node type `app_payload_veh`.
    PayloadVeh,
    /// MOC data is read and payload data written: `app_moc_veh`.
    MocVeh,
    /// Both: `app_all_veh`.
    AllVeh,
    /// Payload data is written and nothing is read, by a payload: `app_pl`.
    ///
    /// A payload that is never asked. It reports of its own accord, so the
    /// interface it is run with is one it only writes.
    Pl,
    /// MOC data is read and payload data written, by a payload: `app_all_pl`.
    ///
    /// Which is the shape every other payload has: asked for a reading, and
    /// answering with one.
    AllPl,
}

impl Kind {
    /// The configuration's own word for this kind.
    pub fn node_type(self) -> &'static str {
        match self {
            Kind::PayloadVeh => "app_payload_veh",
            Kind::MocVeh => "app_moc_veh",
            Kind::AllVeh => "app_all_veh",
            Kind::Pl => "app_pl",
            Kind::AllPl => "app_all_pl",
        }
    }

    /// Which kind of app this node type is, or `None` for a type that is not
    /// an app at all.
    ///
    /// The one statement of which types are apps, read by the checking, by the
    /// flight software and by anything else that has to tell one: three
    /// separate lists of five words would be three places to add a sixth.
    pub fn of(node_type: &str) -> Option<Kind> {
        match node_type {
            "app_payload_veh" => Some(Kind::PayloadVeh),
            "app_moc_veh" => Some(Kind::MocVeh),
            "app_all_veh" => Some(Kind::AllVeh),
            "app_pl" => Some(Kind::Pl),
            "app_all_pl" => Some(Kind::AllPl),
            _ => None,
        }
    }

    /// Every kind there is, for a sentence listing them and for the tests.
    pub const ALL: [Kind; 5] = [
        Kind::PayloadVeh,
        Kind::MocVeh,
        Kind::AllVeh,
        Kind::Pl,
        Kind::AllPl,
    ];

    /// Whether a node of this kind is a payload: something the vehicle
    /// carries rather than part of the system itself.
    ///
    /// Which section of a configuration it belongs in, which pane of the
    /// editor it is placed on, and which of `tcspecial::node`'s two families
    /// opens it. `crate::graph::Node::is_payload_type` reads this rather than
    /// listing the app types again.
    pub fn is_payload(self) -> bool {
        match self {
            Kind::Pl | Kind::AllPl => true,
            Kind::PayloadVeh | Kind::MocVeh | Kind::AllVeh => false,
        }
    }

    /// Whether the payload interface is opened for a program of this kind:
    /// payload data read and MOC data written, at descriptor 0.
    ///
    /// No payload-side kind has it. A payload's readings are what it writes,
    /// and the interface that writes payload data is the other one.
    pub fn has_payload_interface(self) -> bool {
        matches!(self, Kind::PayloadVeh | Kind::AllVeh)
    }

    /// And whether the MOC interface is: MOC data read and payload data
    /// written, at descriptor 1.
    ///
    /// True of a `Pl` as well, which writes payload data there and never
    /// reads: an interface it holds one way about is still the interface it is
    /// run with, and tcspecial opens the descriptor either way.
    pub fn has_moc_interface(self) -> bool {
        matches!(self, Kind::MocVeh | Kind::AllVeh | Kind::Pl | Kind::AllPl)
    }

    /// Whether a program of this kind is read from at all.
    ///
    /// Every kind but `Pl`, that one being a payload nothing asks: a link into
    /// it would be a datum written to a program that does not read, which is
    /// why it has no data sink for one to arrive at.
    pub fn is_given_data(self) -> bool {
        !matches!(self, Kind::Pl)
    }
}

/// What an app node says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppParams {
    /// Which of the three it is, and so which interfaces its program is run
    /// with.
    pub kind: Kind,
    /// The program to run.
    pub program: String,
    /// What to do when it stops.
    pub restart: Restart,
}

/// What an app node may be told, and whether it must be.
///
/// The one list of them, as `graph::parameters::ATTRIBUTES` is for the
/// tcspecial section: the checking refuses anything else by it and the editor
/// builds its window from it, so a third attribute is added here and appears
/// in both.
pub const ATTRIBUTES: &[(&str, bool)] = &[("program", true), ("restart", false)];

/// What this app node says, or what is wrong with what it says.
///
/// Every attribute is checked, including the ones that are not an app's: one
/// that nothing reads is refused rather than ignored, because an app node
/// whose `progam` was misspelled would be a node with no program at all --
/// refused for the absence and never for the typo, and a reader comparing the
/// two words is the only way anybody would find it.
pub fn params_of(
    what: &str,
    node_type: &str,
    attributes: &BTreeMap<String, Scalar>,
) -> Result<AppParams, String> {
    let Some(kind) = Kind::of(node_type) else {
        return Err(format!(
            "\"{what}\": \"{node_type}\" is not an app node type. They are {}",
            Kind::ALL
                .iter()
                .map(|kind| kind.node_type())
                .collect::<Vec<&str>>()
                .join(", ")
        ));
    };
    let named = |why: String| format!("\"{what}\" ({node_type}): {why}");

    for attribute in attributes.keys() {
        if !ATTRIBUTES.iter().any(|(known, _)| known == attribute) {
            return Err(named(format!(
                "there is nothing called \"{attribute}\" on an app node. It takes {}",
                ATTRIBUTES
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<&str>>()
                    .join(" and ")
            )));
        }
    }

    // Required, with no default: an app node is the program it names, and
    // there is no program this software could reasonably run instead.
    let program = attributes
        .get("program")
        .map(|program| program.as_str().trim().to_string())
        .filter(|program| !program.is_empty())
        .ok_or_else(|| {
            named("program is not set: an app node is the program it names".to_string())
        })?;

    // The same two spellings a computation's restart has, read the same way:
    // see `super::compute`, where this attribute was first written.
    let restart = match attributes.get("restart").map(Scalar::as_str) {
        None | Some("never") => Restart::Never,
        Some(other) => crate::whole::whole_u32(other)
            .map(Restart::UpTo)
            .map_err(|_| {
                named(format!(
                    "restart: \"{other}\" is neither \"never\" nor a number of times"
                ))
            })?,
    };

    Ok(AppParams {
        kind,
        program,
        restart,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a node says, as a graph holds it.
    fn said(attributes: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        attributes
            .iter()
            .map(|(name, value)| (name.to_string(), Scalar::of(*value)))
            .collect()
    }

    /// The five types are the five kinds, and nothing else is one.
    #[test]
    fn each_type_is_a_kind_of_app() {
        for kind in Kind::ALL {
            assert_eq!(Kind::of(kind.node_type()), Some(kind));
        }
        for other in [
            "compute", "log", "null", "i2c", "moc", "app", "app_", "app_pl_",
        ] {
            assert_eq!(Kind::of(other), None, "{other}");
        }

        // Which interfaces each is run with: the payload one, the MOC one, or
        // both, and every kind has at least one.
        assert!(Kind::PayloadVeh.has_payload_interface() && !Kind::PayloadVeh.has_moc_interface());
        assert!(Kind::MocVeh.has_moc_interface() && !Kind::MocVeh.has_payload_interface());
        assert!(Kind::AllVeh.has_payload_interface() && Kind::AllVeh.has_moc_interface());
        // A payload writes its readings, and the interface payload data is
        // written on is the MOC one: neither payload kind is run with the
        // other, whatever else it does with the one it has.
        assert!(Kind::Pl.has_moc_interface() && !Kind::Pl.has_payload_interface());
        assert!(Kind::AllPl.has_moc_interface() && !Kind::AllPl.has_payload_interface());

        // And which side each stands on, which is which section of a file
        // holds it.
        for kind in Kind::ALL {
            assert_eq!(
                kind.is_payload(),
                kind.node_type().ends_with("_pl"),
                "{}",
                kind.node_type()
            );
        }

        // The one kind nothing is given: a payload that is never asked.
        assert!(!Kind::Pl.is_given_data());
        for kind in Kind::ALL.iter().filter(|kind| **kind != Kind::Pl) {
            assert!(kind.is_given_data(), "{}", kind.node_type());
        }
    }

    /// A program is what an app node is, so it says one.
    #[test]
    fn an_app_node_names_a_program() {
        let params = params_of(
            "fore",
            "app_payload_veh",
            &said(&[("program", "/usr/libexec/tcs/fore")]),
        )
        .expect("a node that names a program");
        assert_eq!(params.kind, Kind::PayloadVeh);
        assert_eq!(params.program, "/usr/libexec/tcs/fore");
        // Nothing said about restarting is "never", which is the answer that
        // leaves a stopped program stopped.
        assert_eq!(params.restart, Restart::Never);

        let why = params_of("fore", "app_payload_veh", &said(&[])).expect_err("no program");
        assert!(why.contains("program is not set"), "{why}");
        assert!(why.contains("fore"), "{why}");

        // And a program named as nothing at all is no program.
        let why = params_of("fore", "app_moc_veh", &said(&[("program", "  ")]))
            .expect_err("a program that is blank");
        assert!(why.contains("program is not set"), "{why}");

        // A payload that is a program says the same thing the same way, which
        // is what one module for the five is for.
        let params = params_of(
            "bay",
            "app_pl",
            &said(&[("program", "/usr/libexec/tcs/bay")]),
        )
        .expect("a payload that names a program");
        assert_eq!(params.kind, Kind::Pl);
        assert!(params.kind.is_payload());
        assert_eq!(params.program, "/usr/libexec/tcs/bay");
    }

    /// Restarting is declared, in the two spellings a computation uses.
    #[test]
    fn restarting_is_never_or_a_number_of_times() {
        let of = |restart: &str| {
            params_of(
                "fore",
                "app_all_veh",
                &said(&[("program", "/bin/true"), ("restart", restart)]),
            )
            .map(|params| params.restart)
        };

        assert_eq!(of("never").expect("never"), Restart::Never);
        assert_eq!(of("3").expect("three times"), Restart::UpTo(3));
        // Every integer a configuration writes is written the same way: see
        // `crate::whole`.
        assert_eq!(of("0x3").expect("three times"), Restart::UpTo(3));

        let why = of("sometimes").expect_err("a word that is neither");
        assert!(why.contains("neither"), "{why}");
    }

    /// An attribute belonging to nothing is refused by name.
    #[test]
    fn an_attribute_that_is_not_an_apps_is_refused() {
        let why = params_of(
            "fore",
            "app_payload_veh",
            &said(&[("program", "/bin/true"), ("progam", "/bin/false")]),
        )
        .expect_err("a misspelled attribute");
        assert!(why.contains("progam"), "{why}");
        assert!(why.contains("program and restart"), "{why}");
    }

    /// And a type that is not an app's is not read as one.
    #[test]
    fn a_type_that_is_not_an_apps_is_refused() {
        let why = params_of("k", "compute", &said(&[("program", "/bin/true")]))
            .expect_err("a type that is not an app");
        assert!(why.contains("not an app node type"), "{why}");
        assert!(
            why.contains("app_payload_veh, app_moc_veh, app_all_veh, app_pl, app_all_pl"),
            "{why}"
        );
    }
}
