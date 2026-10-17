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
use std::path::PathBuf;

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

    /// Whether this kind handles payload data, and so takes the argument for
    /// it.
    ///
    /// The question the node type's own name answers: a type that names
    /// payload handles payload data, one that names MOC handles MOC data, and
    /// an `all` handles both. Not the same question as
    /// [`Kind::has_payload_interface`], which is about descriptors: a `Pl`
    /// handles payload data -- writing it is handling it -- on the interface
    /// at descriptor 1.
    ///
    /// What it is for is `app-payload-arg`: see [`Kind::attributes`] and
    /// [`AppParams::arguments`].
    pub fn handles_payload_data(self) -> bool {
        matches!(
            self,
            Kind::PayloadVeh | Kind::AllVeh | Kind::Pl | Kind::AllPl
        )
    }

    /// And whether it handles MOC data, and so takes `app-moc-arg`.
    pub fn handles_moc_data(self) -> bool {
        matches!(self, Kind::MocVeh | Kind::AllVeh | Kind::AllPl)
    }

    /// What a node of this kind may be told, and whether it must be.
    ///
    /// Per kind and not one list for the five, because the arguments are: a
    /// kind that handles payload data must state `app-payload-arg` and a kind
    /// that does not has nothing to say with it. The checking refuses anything
    /// else by this and the editor builds a node's window from it, so an
    /// attribute added here appears in both.
    ///
    /// Required ones first, which is the order a window asks in: the executable,
    /// then the argument for each stream the kind handles, then what to do
    /// when it stops.
    pub fn attributes(self) -> &'static [(&'static str, bool)] {
        match self {
            Kind::PayloadVeh | Kind::Pl => &[
                ("executable", true),
                ("app-payload-arg", true),
                ("restart", false),
            ],
            Kind::MocVeh => &[("executable", true), ("app-moc-arg", true), ("restart", false)],
            Kind::AllVeh | Kind::AllPl => &[
                ("executable", true),
                ("app-payload-arg", true),
                ("app-moc-arg", true),
                ("restart", false),
            ],
        }
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
    /// What the node is called in the configuration.
    ///
    /// Held because the program is told it: it is the first argument of every
    /// app, whatever its kind. See [`AppParams::arguments`].
    pub name: String,
    /// Which of the five it is, and so which interfaces its program is run
    /// with and which arguments it is given.
    pub kind: Kind,
    /// Where the application is, as the configuration wrote it.
    ///
    /// Kept as written, `~` and `$HOME` and all: what those stand for is
    /// settled where the program is executed, which is aboard the vehicle --
    /// see [`AppParams::executable_path`]. A ground tool expanding them against
    /// its own home directory would answer a question about a workstation.
    pub executable: String,
    /// What the program is told about the payload stream, where its kind
    /// handles payload data.
    ///
    /// `Some` exactly when [`Kind::handles_payload_data`], the attribute being
    /// required of those kinds and refused of the others.
    pub payload_arg: Option<String>,
    /// And what it is told about the MOC stream, where its kind handles MOC
    /// data.
    pub moc_arg: Option<String>,
    /// What to do when it stops.
    pub restart: Restart,
}

impl AppParams {
    /// Where the application is, with `~` and `$HOME` expanded.
    ///
    /// What tcspecial executes. Called when it goes to start the program and
    /// not when the configuration is read, because what `$HOME` stands for is
    /// a property of the process that runs the application: the vehicle's.
    ///
    /// The expansion is [`expanded`]'s, which is a shell's for the two forms
    /// it covers and nothing else's.
    pub fn executable_path(&self) -> Result<PathBuf, String> {
        let home = std::env::var("HOME").ok();
        expanded(&self.executable, home.as_deref())
            .map(PathBuf::from)
            .map_err(|why| format!("\"{}\": executable: {why}", self.name))
    }

    /// The arguments the program is run with, in order.
    ///
    /// The node's name first, for every kind: a program that is three nodes of
    /// one configuration is three processes which otherwise could not say
    /// which of them each is -- not in a complaint, not in a log of its own,
    /// not in a file it writes.
    ///
    /// Then the argument for each stream the kind handles, payload before MOC.
    /// Fixed by the kind rather than by what the file happened to state, which
    /// is why both are required where the kind takes them: an argument list
    /// whose meaning depends on which attributes somebody wrote is a list a
    /// program has to count before it can read, and it would count wrongly on
    /// the day it mattered.
    pub fn arguments(&self) -> Vec<String> {
        let mut said = vec![self.name.clone()];
        said.extend(self.payload_arg.clone());
        said.extend(self.moc_arg.clone());
        said
    }
}

/// `executable` as a path, with `~` and `$HOME` expanded as a shell would.
///
/// `home` is what both stand for, which is `HOME` in the environment of
/// whatever is about to run the program. `None` is an environment that has
/// none, and a value needing one is then refused rather than guessed at: a
/// relative path made from an empty home is a path to somewhere else, and an
/// app node started from it would be a program nobody chose.
///
/// Two forms, which is what a configuration needs and is where this stops:
///
/// * a leading `~`, alone or before a `/`, is the home directory;
/// * `$HOME` and `${HOME}` anywhere in the value are the same thing.
///
/// Nothing else is expanded. A `~` that is not the first character is a
/// character in a file name, as it is in a shell; `~somebody` is refused
/// rather than read as either a home directory or a directory called
/// `~somebody`, there being no way to tell which was meant; and no other
/// variable is looked up, a configuration that wanted one being a
/// configuration that would behave differently on two vehicles.
pub fn expanded(program: &str, home: Option<&str>) -> Result<String, String> {
    let wants_home = program == "~"
        || program.starts_with("~/")
        || program.contains("$HOME")
        || program.contains("${HOME}");

    if program.starts_with('~') && !(program == "~" || program.starts_with("~/")) {
        return Err(format!(
            "\"{program}\" starts with \"~\" and a name. A home directory other \
             than this vehicle's is not expanded here: write the path out"
        ));
    }

    let home = match (wants_home, home) {
        (false, _) => return Ok(program.to_string()),
        (true, Some(home)) if !home.is_empty() => home,
        (true, _) => {
            return Err(format!(
                "\"{program}\" needs HOME, and HOME is not set where the program \
                 would be run: a path made from an empty home is a path to \
                 somewhere else"
            ))
        }
    };

    let said = if program == "~" {
        home.to_string()
    } else if let Some(rest) = program.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else {
        program.to_string()
    };
    Ok(said.replace("${HOME}", home).replace("$HOME", home))
}

/// What an app node of each kind may be told, and whether it must be.
///
/// [`Kind::attributes`] by another name, for a caller that has the node type
/// and not the kind. A type that is not an app's has nothing to say here and
/// answers `None`.
pub fn attributes_of(node_type: &str) -> Option<&'static [(&'static str, bool)]> {
    Kind::of(node_type).map(Kind::attributes)
}

/// What this app node says, or what is wrong with what it says.
///
/// Every attribute is checked, including the ones that are not this kind's:
/// one that nothing reads is refused rather than ignored, because an app node
/// whose `exectuable` was misspelled would be a node with no program at all --
/// refused for the absence and never for the typo, and a reader comparing the
/// two words is the only way anybody would find it. An `app-moc-arg` on a kind
/// that handles no MOC data is the same mistake with a different word: an
/// argument written for a program that will not be given it.
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
    let known = kind.attributes();
    let listed = |which: &[(&str, bool)]| {
        which
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<&str>>()
            .join(", ")
    };

    for attribute in attributes.keys() {
        if !known.iter().any(|(name, _)| name == attribute) {
            return Err(named(format!(
                "there is nothing called \"{attribute}\" on an {node_type} node. \
                 It takes {}",
                listed(known)
            )));
        }
    }

    // What the program is told, and the one thing every kind of app is told:
    // the name of the node it is. Required of the configuration in the sense
    // that a node has a name already -- this is where it is kept, for
    // `AppParams::arguments` to hand over.
    let name = what.to_string();

    // Required, with no default: an app node is the program it names, and
    // there is no program this software could reasonably run instead.
    let executable = attributes
        .get("executable")
        .map(|executable| executable.as_str().trim().to_string())
        .filter(|executable| !executable.is_empty())
        .ok_or_else(|| {
            named(
                "executable is not set: an app node is the program it names".to_string(),
            )
        })?;

    // The argument for each stream the kind handles, which is required of the
    // kinds that handle one: a program is run with a fixed list, so a node
    // that said nothing would shift what the rest of the list means. Taken as
    // written, trimmed of nothing: an argument is the mission's own word for
    // something, and a space in it may be what it meant.
    let argument = |attribute: &str, handled: bool| -> Result<Option<String>, String> {
        match (attributes.get(attribute), handled) {
            (Some(said), true) => Ok(Some(said.as_str().to_string())),
            (None, true) => Err(named(format!(
                "{attribute} is not set, and an {node_type} node is run with it: \
                 it is the argument this kind of app is given for the stream it \
                 handles, and the list a program is run with is a fixed one"
            ))),
            // Refused above, where every attribute is held against the kind's
            // own list. Here for the reader: a kind that handles no data of
            // that stream has nothing to say with the argument for it.
            (_, false) => Ok(None),
        }
    };
    let payload_arg = argument("app-payload-arg", kind.handles_payload_data())?;
    let moc_arg = argument("app-moc-arg", kind.handles_moc_data())?;

    // The same two spellings a computation's restart has, read in the one
    // place they are read: see `Restart::of`.
    let restart = Restart::of(attributes.get("restart").map(Scalar::as_str)).map_err(named)?;

    Ok(AppParams {
        name,
        kind,
        executable,
        payload_arg,
        moc_arg,
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

        // Which streams each handles, which is the question its own name
        // answers and is not the question about descriptors above: a `Pl`
        // handles payload data and holds the MOC interface to write it on.
        assert!(Kind::PayloadVeh.handles_payload_data() && !Kind::PayloadVeh.handles_moc_data());
        assert!(Kind::MocVeh.handles_moc_data() && !Kind::MocVeh.handles_payload_data());
        assert!(Kind::AllVeh.handles_payload_data() && Kind::AllVeh.handles_moc_data());
        assert!(Kind::Pl.handles_payload_data() && !Kind::Pl.handles_moc_data());
        assert!(Kind::AllPl.handles_payload_data() && Kind::AllPl.handles_moc_data());

        // And so which arguments each takes. Every kind says a program and
        // may say a restart; the argument for a stream is listed by exactly
        // the kinds that handle that stream, and is required of them.
        for kind in Kind::ALL {
            let attributes = kind.attributes();
            let says = |name: &str| attributes.iter().find(|(known, _)| *known == name);
            assert_eq!(
                says("executable"),
                Some(&("executable", true)),
                "{}",
                kind.node_type()
            );
            assert_eq!(
                says("restart"),
                Some(&("restart", false)),
                "{}",
                kind.node_type()
            );
            assert_eq!(
                says("app-payload-arg"),
                kind.handles_payload_data()
                    .then_some(&("app-payload-arg", true)),
                "{}",
                kind.node_type()
            );
            assert_eq!(
                says("app-moc-arg"),
                kind.handles_moc_data().then_some(&("app-moc-arg", true)),
                "{}",
                kind.node_type()
            );
            assert_eq!(attributes_of(kind.node_type()), Some(attributes));
        }
        assert_eq!(attributes_of("i2c"), None);
    }

    /// The program is run with the node's name, then an argument per stream.
    ///
    /// The order is the whole of what makes the list readable from the program
    /// end: the name first, always, and payload before MOC where a kind has
    /// both. A program counting its arguments to find out which it was given
    /// would count wrongly on the day somebody changed the node's type.
    #[test]
    fn a_program_is_run_with_the_name_and_an_argument_per_stream() {
        let of = |what: &str, node_type: &str, attributes: &[(&str, &str)]| {
            params_of(what, node_type, &said(attributes)).map(|params| params.arguments())
        };

        // Both streams: the name, then payload, then MOC.
        assert_eq!(
            of(
                "through",
                "app_all_veh",
                &[
                    ("executable", "/bin/true"),
                    ("app-payload-arg", "--readings"),
                    ("app-moc-arg", "--commands"),
                ]
            )
            .expect("a node that states both"),
            vec!["through", "--readings", "--commands"]
        );

        // One stream apiece, and the argument is the one its kind handles.
        assert_eq!(
            of(
                "fore",
                "app_payload_veh",
                &[("executable", "/bin/true"), ("app-payload-arg", "--readings")]
            )
            .expect("a node that states its one"),
            vec!["fore", "--readings"]
        );
        assert_eq!(
            of(
                "back",
                "app_moc_veh",
                &[("executable", "/bin/true"), ("app-moc-arg", "--commands")]
            )
            .expect("a node that states its one"),
            vec!["back", "--commands"]
        );

        // A payload that is a program is told the same way, and the one that
        // is never asked is told about the one stream it has.
        assert_eq!(
            of(
                "bay",
                "app_pl",
                &[("executable", "/bin/true"), ("app-payload-arg", "--bay-2")]
            )
            .expect("a payload that states its one"),
            vec!["bay", "--bay-2"]
        );
        assert_eq!(
            of(
                "bay",
                "app_all_pl",
                &[
                    ("executable", "/bin/true"),
                    ("app-payload-arg", "--bay-2"),
                    ("app-moc-arg", "--bay-2-commands"),
                ]
            )
            .expect("a payload that states both"),
            vec!["bay", "--bay-2", "--bay-2-commands"]
        );

        // An argument with a space in it is one argument: it is the mission's
        // own word for something and is handed over as written.
        let params = params_of(
            "fore",
            "app_payload_veh",
            &said(&[("executable", "/bin/true"), ("app-payload-arg", "two words")]),
        )
        .expect("an argument as written");
        assert_eq!(params.arguments(), vec!["fore", "two words"]);
    }

    /// An argument a kind is run with has to be stated.
    ///
    /// Required for the reason the order exists: the list is fixed by the kind,
    /// so a node that said nothing would hand its program a shorter list and
    /// the program would read the wrong word as the first argument.
    #[test]
    fn an_argument_the_kind_is_run_with_is_required() {
        let why = params_of(
            "fore",
            "app_payload_veh",
            &said(&[("executable", "/bin/true")]),
        )
        .expect_err("a kind that handles payload data and said nothing");
        assert!(why.contains("app-payload-arg is not set"), "{why}");
        assert!(why.contains("fore"), "{why}");

        let why = params_of("back", "app_moc_veh", &said(&[("executable", "/bin/true")]))
            .expect_err("a kind that handles MOC data and said nothing");
        assert!(why.contains("app-moc-arg is not set"), "{why}");

        // And both of them, where the kind handles both: the first missing one
        // is the complaint, payload being the first argument.
        let why = params_of(
            "through",
            "app_all_veh",
            &said(&[("executable", "/bin/true"), ("app-moc-arg", "--commands")]),
        )
        .expect_err("a kind that handles both and stated one");
        assert!(why.contains("app-payload-arg is not set"), "{why}");
    }

    /// Where the application is, with what a shell would expand expanded.
    ///
    /// Two forms and no others, and a value that needs a home directory where
    /// there is none is refused: a path made from an empty home is a path to
    /// somewhere else, and the thing at the end of it is whatever happens to
    /// be there.
    #[test]
    fn a_programs_path_expands_a_tilde_and_home() {
        let home = Some("/home/flight");

        // A leading tilde, alone and before a path.
        assert_eq!(expanded("~", home).expect("a home"), "/home/flight");
        assert_eq!(
            expanded("~/libexec/tcs/bay-2", home).expect("a home"),
            "/home/flight/libexec/tcs/bay-2"
        );

        // The variable, in either spelling and wherever it appears.
        assert_eq!(
            expanded("$HOME/libexec/bay-2", home).expect("a home"),
            "/home/flight/libexec/bay-2"
        );
        assert_eq!(
            expanded("${HOME}/libexec/bay-2", home).expect("a home"),
            "/home/flight/libexec/bay-2"
        );
        assert_eq!(
            expanded("/opt/$HOME/bay-2", home).expect("a home"),
            "/opt//home/flight/bay-2"
        );

        // An absolute path, which is what a flight configuration writes, is
        // what it says and needs no home at all.
        assert_eq!(
            expanded("/usr/libexec/tcs/bay-2", None).expect("no home needed"),
            "/usr/libexec/tcs/bay-2"
        );

        // A tilde that is not the first character is a character in a name, as
        // it is in a shell.
        assert_eq!(
            expanded("/usr/libexec/bay~2", None).expect("no home needed"),
            "/usr/libexec/bay~2"
        );

        // Somebody else's home is refused rather than guessed at, either way
        // round: as a home directory this cannot look up, or as a directory
        // whose name begins with a tilde.
        let why = expanded("~flight/bay-2", home).expect_err("another home");
        assert!(why.contains("~flight/bay-2"), "{why}");
        assert!(why.contains("write the path out"), "{why}");

        // And a value that needs a home where there is none.
        for needs in ["~", "~/bay-2", "$HOME/bay-2", "${HOME}/bay-2"] {
            let why = expanded(needs, None).expect_err("no home");
            assert!(why.contains("HOME is not set"), "{why}");
            let why = expanded(needs, Some("")).expect_err("an empty home");
            assert!(why.contains("HOME is not set"), "{why}");
        }
    }

    /// A node keeps its program as written and expands it where it is run.
    ///
    /// The two halves of the arrangement: the configuration holds the word
    /// somebody wrote, and `executable_path` answers with what it stands for in
    /// the environment of whatever is about to run it. A complaint from it
    /// names the node, a report about one of forty app nodes being no use
    /// without that.
    #[test]
    fn a_nodes_program_is_expanded_where_it_is_run() {
        let params = params_of(
            "bay",
            "app_pl",
            &said(&[
                ("executable", "~/libexec/tcs/bay-2"),
                ("app-payload-arg", "--bay-2"),
            ]),
        )
        .expect("a payload that names a program");

        // Kept as written: a ground tool reading the file sees what the file
        // says, whatever its own home directory is.
        assert_eq!(params.executable, "~/libexec/tcs/bay-2");

        // And expanded against the home of this process, which aboard the
        // vehicle is the one the program will be run from.
        match std::env::var("HOME") {
            Ok(home) if !home.is_empty() => assert_eq!(
                params.executable_path().expect("a home"),
                PathBuf::from(format!("{home}/libexec/tcs/bay-2"))
            ),
            // Run where there is no home: refused, and the complaint says so
            // rather than offering a path made from nothing.
            _ => {
                let why = params.executable_path().expect_err("no home");
                assert!(why.contains("HOME is not set"), "{why}");
                assert!(why.contains("bay"), "{why}");
            }
        }

        // A path that needs nothing expanded is the path it is.
        let params = params_of(
            "fore",
            "app_payload_veh",
            &said(&[
                ("executable", "/usr/libexec/tcs/fore"),
                ("app-payload-arg", "--readings"),
            ]),
        )
        .expect("a node that names a program");
        assert_eq!(
            params.executable_path().expect("nothing to expand"),
            PathBuf::from("/usr/libexec/tcs/fore")
        );
    }

    /// And one the kind is not run with is refused by name.
    ///
    /// An `app-moc-arg` on a kind that handles no MOC data is an argument
    /// written for a program that will not be given it -- most likely a node
    /// whose type was changed and whose attributes were left behind.
    #[test]
    fn an_argument_the_kind_is_not_run_with_is_refused() {
        let why = params_of(
            "fore",
            "app_payload_veh",
            &said(&[
                ("executable", "/bin/true"),
                ("app-payload-arg", "--readings"),
                ("app-moc-arg", "--commands"),
            ]),
        )
        .expect_err("an argument this kind is not run with");
        assert!(why.contains("app-moc-arg"), "{why}");
        assert!(why.contains("app_payload_veh"), "{why}");
        assert!(why.contains("executable, app-payload-arg, restart"), "{why}");

        // The mirror of it: the payload argument on the one vehicle kind that
        // handles no payload data.
        let why = params_of(
            "back",
            "app_moc_veh",
            &said(&[
                ("executable", "/bin/true"),
                ("app-moc-arg", "--commands"),
                ("app-payload-arg", "--readings"),
            ]),
        )
        .expect_err("an argument this kind is not run with");
        assert!(why.contains("app-payload-arg"), "{why}");

        // And on a payload that is never asked, which handles no MOC data.
        let why = params_of(
            "bay",
            "app_pl",
            &said(&[
                ("executable", "/bin/true"),
                ("app-payload-arg", "--bay-2"),
                ("app-moc-arg", "--commands"),
            ]),
        )
        .expect_err("an argument this kind is not run with");
        assert!(why.contains("app-moc-arg"), "{why}");
    }

    /// A program is what an app node is, so it says one.
    #[test]
    fn an_app_node_names_a_program() {
        let params = params_of(
            "fore",
            "app_payload_veh",
            &said(&[
                ("executable", "/usr/libexec/tcs/fore"),
                ("app-payload-arg", "--readings"),
            ]),
        )
        .expect("a node that names a program");
        assert_eq!(params.kind, Kind::PayloadVeh);
        assert_eq!(params.executable, "/usr/libexec/tcs/fore");
        // Nothing said about restarting is "never", which is the answer that
        // leaves a stopped program stopped.
        assert_eq!(params.restart, Restart::Never);

        let why = params_of("fore", "app_payload_veh", &said(&[])).expect_err("no program");
        assert!(why.contains("executable is not set"), "{why}");
        assert!(why.contains("fore"), "{why}");

        // And a program named as nothing at all is no program.
        let why = params_of(
            "fore",
            "app_moc_veh",
            &said(&[("executable", "  "), ("app-moc-arg", "--commands")]),
        )
        .expect_err("a program that is blank");
        assert!(why.contains("executable is not set"), "{why}");

        // A payload that is a program says the same thing the same way, which
        // is what one module for the five is for.
        let params = params_of(
            "bay",
            "app_pl",
            &said(&[
                ("executable", "/usr/libexec/tcs/bay"),
                ("app-payload-arg", "--bay-2"),
            ]),
        )
        .expect("a payload that names a program");
        assert_eq!(params.kind, Kind::Pl);
        assert!(params.kind.is_payload());
        assert_eq!(params.executable, "/usr/libexec/tcs/bay");
        assert_eq!(params.name, "bay");
        assert_eq!(params.payload_arg.as_deref(), Some("--bay-2"));
        assert_eq!(params.moc_arg, None);
    }

    /// Restarting is declared, in the two spellings a computation uses.
    #[test]
    fn restarting_is_true_or_false() {
        let of = |restart: &str| {
            params_of(
                "fore",
                "app_all_veh",
                &said(&[
                    ("executable", "/bin/true"),
                    ("app-payload-arg", "--readings"),
                    ("app-moc-arg", "--commands"),
                    ("restart", restart),
                ]),
            )
            .map(|params| params.restart)
        };

        assert_eq!(of("false").expect("false"), Restart::Never);
        assert_eq!(of("true").expect("true"), Restart::WhenItStops);
        // An empty box is a file that says nothing, which is what a node says
        // before anybody answers this: see `Restart::of`.
        assert_eq!(of("").expect("nothing said"), Restart::Never);

        // And nothing else, a number of times included: see `Restart::of` for
        // why a bound is not something a configuration chooses.
        for neither in ["never", "3", "0x3", "sometimes", "True"] {
            let why = of(neither).expect_err("a word that is neither");
            assert!(why.contains("neither"), "{why}");
            assert!(why.contains(neither), "{why}");
        }
    }

    /// And a node that says nothing about it is a node left stopped.
    #[test]
    fn restarting_is_never_unless_it_is_asked_for() {
        let params = params_of(
            "fore",
            "app_payload_veh",
            &said(&[("executable", "/bin/true"), ("app-payload-arg", "--readings")]),
        )
        .expect("a node that says nothing about restarting");
        assert_eq!(params.restart, Restart::Never);
    }

    /// An attribute belonging to nothing is refused by name.
    #[test]
    fn an_attribute_that_is_not_an_apps_is_refused() {
        let why = params_of(
            "fore",
            "app_payload_veh",
            &said(&[
                ("executable", "/bin/true"),
                ("app-payload-arg", "--readings"),
                ("progam", "/bin/false"),
            ]),
        )
        .expect_err("a misspelled attribute");
        assert!(why.contains("progam"), "{why}");
        assert!(why.contains("executable, app-payload-arg, restart"), "{why}");
    }

    /// And a type that is not an app's is not read as one.
    #[test]
    fn a_type_that_is_not_an_apps_is_refused() {
        let why = params_of("k", "compute", &said(&[("executable", "/bin/true")]))
            .expect_err("a type that is not an app");
        assert!(why.contains("not an app node type"), "{why}");
        assert!(
            why.contains("app_payload_veh, app_moc_veh, app_all_veh, app_pl, app_all_pl"),
            "{why}"
        );
    }
}
