//! What is checked before a configuration is allowed to run.
//!
//! The reliability argument for the whole architecture is that a graph is
//! checkable, and this is the check. `tcsverify` runs it on the ground and
//! tcspecial runs the same code again aboard, so a configuration that is
//! refused is refused in the same words at a desk and in flight.
//!
//! Every problem names the thing it is about, and all of them are reported
//! rather than the first: somebody fixing a configuration wants the list, not
//! one item of it at a time.

use std::collections::{BTreeMap, BTreeSet};

use super::{terminals, Destination, Graph};
use crate::verify::About;

/// How much a problem matters to somebody editing the configuration.
///
/// Two kinds, and the difference is whether the configuration says something
/// wrong or has not finished saying it:
///
/// * an **error** is a configuration that contradicts itself or the rules. Two
///   nodes of one name, a link to a node that is not there, a link carrying
///   one stream into the other, two payloads claiming one socket. Reading it
///   again will not make it true.
/// * a **warning** is a configuration that is not finished. A node whose
///   attributes are not all in yet -- no `path` on a sensor, no `inputs` on a
///   mean -- and a node nothing is linked to. That is what a half-built graph
///   looks like, and it is the state an editor leaves one in between one edit
///   and the next.
///
/// This changes no rule. [`Graph::check`](super::Graph::check) hands back
/// every problem either way and the flight software refuses a configuration
/// with any of them: a sensor whose device nobody named cannot be opened, so
/// it is not a question of degree aboard. What the split is for is the person:
/// a graph halfway through being drawn should not bury the mistakes in it under
/// the things they have not typed yet. `tcsverify` says which is which and
/// exits nought where only warnings were found; tcsgui refuses to generate
/// code while any remain, warnings included, because code generated from an
/// unfinished configuration would not compile into anything that flies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The configuration says something wrong.
    Error,
    /// The configuration has not finished saying it.
    Warning,
}

impl Severity {
    /// What this is called in a report.
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// How many problems are listed before the rest are summed up.
///
/// A person reading a report acts on the first few; the twentieth is scrolled
/// past, and in a half-built graph most of them are the same unfinished node
/// said a second way. So the first few are listed and the rest are counted.
pub const LISTED: usize = 5;

/// What follows the ones that were listed, where there were more.
///
/// One phrase, used by the verifier's output and by the editor's refusal to
/// generate code, so that a person meets the same shape of report in both.
pub const AND_OTHERS: &str = "and others";

/// Something wrong with a configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Two nodes of one name. Links name nodes, so this makes a link
    /// ambiguous as well as a node.
    DuplicateNode { name: String },
    /// Two groups of one name.
    DuplicateGroup { name: String },
    /// A group nothing is in, which reads nothing on its interval.
    ///
    /// What a group whose name a node misspelled looks like from the group's
    /// end, as version 1 says of a payload group nobody names. tcsgui marked
    /// this as a fault long before anything aboard refused it, which made
    /// the editor and the flight software disagree about the one file.
    EmptyGroup { group: String },
    /// A node with no name, which nothing can link to.
    NamelessNode,
    /// A link naming a node that is not in the configuration.
    ///
    /// `from` is the node the link leaves, which is where the link is written:
    /// a link is an attribute of that node, so a complaint about one belongs
    /// on its line.
    NoSuchNode {
        from: String,
        link: String,
        missing: String,
    },
    /// A link from a node to itself.
    SelfLink { node: String },
    /// A node in a group the configuration does not have.
    NoSuchGroup { node: String, group: String },
    /// A group whose interval cannot be read.
    BadInterval { group: String, why: String },
    /// A node that states an interval of its own while in a group.
    ///
    /// Two sensors in one group with intervals of their own is a
    /// configuration whose own description disagrees with itself.
    IntervalInAGroup { node: String, group: String },
    /// Two links arriving at one input of one node.
    InputBoundTwice { node: String, input: String },
    /// A link end naming a terminal its node does not have, or naming one at
    /// the wrong end, or naming none where the node has several to choose
    /// between. See [`crate::graph::terminals`].
    BadTerminal {
        from: String,
        link: String,
        why: String,
    },
    /// A link whose two ends carry different streams: one of them is drawn
    /// backwards.
    CrossedStreams {
        from: String,
        link: String,
        leaves: String,
        arrives: String,
    },
    /// A payload or a ground link with no link on it at all.
    NodeOnNoLink { node: String, node_type: String },
    /// One link written twice: the same data source to the same data sink.
    ///
    /// Two deliveries of every datum, and nothing in the drawing to show it:
    /// two lines between one pair of terminals lie on top of each other. The
    /// editor will not draw the second; a file may still say it.
    DuplicateLink { from: String, to: String },
    /// A command binding naming a node that is not there.
    NoSuchTarget { sets: String },
    /// A command binding that does not name an attribute.
    NoAttributeNamed { sets: String },
    /// A command binding arriving on something that is not a MOC link.
    NotAMocLink { from: String },
    /// A log node whose attributes are not a log's.
    BadLog { node: String, why: String },
    /// A compute node whose attributes are not its computation's.
    BadCompute { node: String, why: String },
    /// A mean whose declared inputs are not the ones the links bind.
    ///
    /// `declared` is what the node says it is over and `bound` what arrives:
    /// the two are listed so that a reader can see which is missing.
    MeanInputs {
        node: String,
        declared: Vec<String>,
        bound: Vec<String>,
    },
    /// A source whose records a fixed-length log cannot hold.
    ///
    /// `produces` is what the source's records are, where the configuration
    /// says: `None` is a source whose record length nothing states, which a
    /// fixed-length log cannot take either.
    LogSourceMismatch {
        log: String,
        source: String,
        wanted: u32,
        produces: Option<u32>,
    },
    /// A node that does not say what its type needs, or says something its
    /// type does not know.
    ///
    /// `why` is the reason in the words of the rules that have been tested
    /// against real buses -- see [`crate::graph::reach::reach_of`] -- so that
    /// the ground check refuses a node for exactly what the flight software
    /// would refuse it for, and says so with the file and the line.
    ///
    /// This check is why the editor can be lazy. An editor that required every
    /// field before it would let you leave the keyboard is an editor that
    /// fights a person working out what they are describing; what it owes
    /// instead is that whatever *is* entered is valid. Requiring that it all be
    /// entered belongs here, where the complaint can name the file, the line
    /// and the attribute.
    BadNode { node: String, why: String },
    /// A configuration with no `tcspecial` section.
    ///
    /// Required, and required rather than defaulted for the reason the beacon
    /// attributes inside it are: where commands are taken and where beacons go
    /// are not questions to be answered by whatever a file was silently given.
    /// A configuration that has not been asked them has not answered them.
    NoTcspecial,
    /// A `tcspecial` section that is there and does not hold together.
    ///
    /// `why` is the reason in the words of the one piece of code that knows
    /// them -- see [`crate::types::CIConfigJson::to_ci_config`] -- so that the
    /// ground check, the editor and the flight software all refuse the same
    /// file for the same stated reason.
    BadTcspecial { why: String },
    /// A payload that is read and also written to, and says nothing about
    /// what the writing is for.
    ///
    /// `from` is the node whose data arrives. A payload that is read is
    /// written to in order to ask it for a reading, and the bytes that ask are
    /// the `trigger`: a payload with none is one the configuration describes
    /// as only read, so a link delivering into it has nowhere to go.
    UntriggeredInput { node: String, from: String },
    /// A node nothing reads and nothing writes.
    ///
    /// More likely a mistake than an intention: a node with no link at all
    /// does nothing, and a configuration is read by people who want to know
    /// what it does.
    Unconnected { node: String },
}

impl Problem {
    /// What this problem is about, for working out which line to report it on.
    ///
    /// The same `About` version 1's problems use, because the question is the
    /// same: where in the file is the thing called this. `tcsverify` scans the
    /// text for it -- a line number cannot be had from a parsed document, and
    /// carrying one through every struct here would put the file's shape into
    /// types that exist so two formats can describe one thing.
    pub fn about(&self) -> About {
        match self {
            Problem::DuplicateNode { name } => About::Node(name.clone()),
            Problem::NamelessNode => About::Section("nodes"),
            Problem::DuplicateGroup { name } => About::NodeGroup(name.clone()),
            Problem::EmptyGroup { group } => About::NodeGroup(group.clone()),
            Problem::BadInterval { group, .. } => About::NodeGroup(group.clone()),
            // A link's problem is reported on the node it leaves, which is
            // where the link is written: a link is an attribute of that node.
            Problem::NoSuchNode { from, .. } => About::Node(from.clone()),
            Problem::SelfLink { node } => About::Node(node.clone()),
            Problem::NoSuchGroup { node, .. } => About::Node(node.clone()),
            Problem::IntervalInAGroup { node, .. } => About::Node(node.clone()),
            Problem::InputBoundTwice { node, .. } => About::Node(node.clone()),
            // And so is a terminal's problem, and a crossed link's.
            Problem::BadTerminal { from, .. } => About::Node(from.clone()),
            Problem::CrossedStreams { from, .. } => About::Node(from.clone()),
            Problem::DuplicateLink { from, .. } => About::Node(from.clone()),
            Problem::NodeOnNoLink { node, .. } => About::Node(node.clone()),
            Problem::NoSuchTarget { .. } | Problem::NoAttributeNamed { .. } => {
                About::Section("commands")
            }
            Problem::NotAMocLink { from } => About::Node(from.clone()),
            Problem::Unconnected { node } => About::Node(node.clone()),
            // On the payload and not on the source: the source is right
            // about where it sends, and the payload is where the trigger
            // would have to be written.
            Problem::UntriggeredInput { node, .. } => About::Node(node.clone()),
            // The section, where there is one to point at. A missing section
            // has no line of its own and is reported on the file, which is
            // where it would have to be written.
            Problem::NoTcspecial | Problem::BadTcspecial { .. } => {
                About::Section("tcspecial")
            }
            Problem::BadNode { node, .. } => About::Node(node.clone()),
            Problem::BadLog { node, .. } => About::Node(node.clone()),
            Problem::BadCompute { node, .. } => About::Node(node.clone()),
            Problem::MeanInputs { node, .. } => About::Node(node.clone()),
            // Reported on the source and not on the log: the log is right
            // about what it wants, and the source is the thing a reader will
            // have to change or unlink.
            Problem::LogSourceMismatch { source, .. } => About::Node(source.clone()),
        }
    }

    /// Whether this is a configuration that is wrong or one that is
    /// unfinished. See [`Severity`], which has the argument.
    pub fn severity(&self) -> Severity {
        match self {
            // The attributes of a node: not all in yet is what an editor
            // leaves between one edit and the next. A value that is there and
            // wrong is counted with them rather than apart, because what says
            // which is a sentence written by whichever node type knows -- and
            // a reader of either has the same thing to do, which is open that
            // node and look at its words.
            Problem::BadNode { .. }
            | Problem::BadCompute { .. }
            | Problem::BadLog { .. }
            // And a node with nothing joined to it, which is a line somebody
            // has not drawn yet.
            | Problem::NodeOnNoLink { .. }
            | Problem::Unconnected { .. } => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// What to print, one problem per line.
    pub fn describe(&self) -> String {
        match self {
            Problem::DuplicateNode { name } => {
                format!("two nodes are called \"{name}\"")
            }
            Problem::DuplicateGroup { name } => {
                format!("two groups are called \"{name}\"")
            }
            Problem::EmptyGroup { group } => {
                format!("group \"{group}\" has no nodes in it")
            }
            Problem::NamelessNode => "a node has no name".to_string(),
            Problem::NoSuchNode { link, missing, .. } => {
                format!("{link}: there is no node called \"{missing}\"")
            }
            Problem::SelfLink { node } => {
                format!("\"{node}\" is linked to itself")
            }
            Problem::NoSuchGroup { node, group } => {
                format!("\"{node}\" is in group \"{group}\", which is not there")
            }
            Problem::BadInterval { group, why } => {
                format!("group \"{group}\": {why}")
            }
            Problem::IntervalInAGroup { node, group } => format!(
                "\"{node}\" states an interval of its own and is in group \
                 \"{group}\", which states one for it"
            ),
            Problem::InputBoundTwice { node, input } => {
                format!("two links arrive at \"{node}.{input}\"")
            }
            Problem::BadTerminal { link, why, .. } => format!("{link}: {why}"),
            Problem::CrossedStreams {
                link,
                leaves,
                arrives,
                ..
            } => format!(
                "{link}: it leaves by \"{leaves}\" and arrives at \"{arrives}\", \
                 which are opposite ways for the data to be going -- one end of \
                 this link is drawn backwards"
            ),
            Problem::NodeOnNoLink { node, node_type } => format!(
                "\"{node}\" is a {node_type} node with no link on it: a payload \
                 or a ground link needs a data sink or a data source, or there \
                 is nothing for it to do"
            ),
            Problem::DuplicateLink { from, to } => format!(
                "\"{from}\" is linked to \"{to}\" twice: one link is one \
                 delivery, and the second is a datum sent again with nothing in \
                 the drawing to show it"
            ),
            Problem::NoSuchTarget { sets } => {
                format!("a command sets \"{sets}\", and there is no such node")
            }
            Problem::NoAttributeNamed { sets } => format!(
                "a command sets \"{sets}\", which names no attribute: a binding \
                 is written node.attribute"
            ),
            Problem::NotAMocLink { from } => format!(
                "a command arrives on \"{from}\", which is not a moc node -- \
                 commands come from the ground"
            ),
            Problem::Unconnected { node } => format!(
                "\"{node}\" has no link to or from it, so nothing reaches it and \
                 nothing reads it"
            ),
            // The reason already names the node: see reach_of, whose
            // complaints are written to be read on their own.
            Problem::BadNode { why, .. } => why.clone(),
            Problem::NoTcspecial => "this configuration has no tcspecial \
                 section, so nothing says where commands are taken, where \
                 beacons go or where the telemetry log is written"
                .to_string(),
            Problem::BadTcspecial { why } => format!("tcspecial: {why}"),
            Problem::UntriggeredInput { node, from } => format!(
                "\"{from}\" is delivered to \"{node}\", which states no trigger: \
                 what is written to a payload that is read is what asks it for \
                 a reading, so a payload with no trigger is one nothing may be \
                 delivered to"
            ),
            Problem::BadLog { node, why } => format!("log \"{node}\": {why}"),
            // The reason already names the node and the computation: see
            // crate::node::vehicle::compute::params_of.
            Problem::BadCompute { why, .. } => why.clone(),
            Problem::MeanInputs {
                node,
                declared,
                bound,
            } => format!(
                "\"{node}\" is a mean over {} and the links bind {}: a mean whose \
                 inputs are not the ones that arrive is a mean over the wrong \
                 readings",
                said(declared),
                said(bound)
            ),
            Problem::LogSourceMismatch {
                log,
                source,
                wanted,
                produces: Some(produces),
            } => format!(
                "\"{source}\" produces records of {produces} bytes and log \"{log}\" \
                 holds records of exactly {wanted}: a fixed-length log cannot hold \
                 both"
            ),
            Problem::LogSourceMismatch {
                log,
                source,
                wanted,
                produces: None,
            } => format!(
                "log \"{log}\" holds records of exactly {wanted} bytes, and nothing \
                 says how long \"{source}\"'s records are: a fixed-length log takes \
                 only sources whose length the configuration states, which a \
                 computation's does not"
            ),
        }
    }
}

/// A list of names, as a complaint says one.
///
/// "in0, in1 and in2", and "nothing" for an empty list: a sentence that said
/// "the links bind " and stopped would read as a sentence that had lost a
/// word.
fn said(names: &[String]) -> String {
    match names.len() {
        0 => "nothing".to_string(),
        1 => format!("\"{}\"", names[0]),
        _ => {
            let (last, rest) = names.split_last().expect("more than one");
            format!(
                "{} and \"{last}\"",
                rest.iter()
                    .map(|name| format!("\"{name}\""))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        }
    }
}

impl Graph {
    /// Everything wrong with this configuration.
    ///
    /// Empty means it can be flown as far as this can tell. What it cannot
    /// tell is whatever depends on a node type's own rules -- whether a log's
    /// sources suit its format, whether a compute function has the parameters
    /// it wants -- which belong to the type and are checked where the type is.
    pub fn check(&self) -> Vec<Problem> {
        let mut problems = Vec::new();

        // Names first: everything after this assumes a name means one node.
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for node in &self.nodes {
            if node.name.trim().is_empty() {
                problems.push(Problem::NamelessNode);
                continue;
            }
            *seen.entry(node.name.as_str()).or_insert(0) += 1;
        }
        for (name, count) in &seen {
            if *count > 1 {
                problems.push(Problem::DuplicateNode {
                    name: (*name).to_string(),
                });
            }
        }

        let mut groups: BTreeMap<&str, usize> = BTreeMap::new();
        for group in &self.groups {
            *groups.entry(group.name.as_str()).or_insert(0) += 1;
            if let Err(why) = group.interval() {
                problems.push(Problem::BadInterval {
                    group: group.name.clone(),
                    why,
                });
            }
        }
        for (name, count) in &groups {
            if *count > 1 {
                problems.push(Problem::DuplicateGroup {
                    name: (*name).to_string(),
                });
            }
        }

        // A declared group must hold something.
        for group in &self.groups {
            if !self
                .nodes
                .iter()
                .any(|node| node.group.as_deref() == Some(group.name.as_str()))
            {
                problems.push(Problem::EmptyGroup {
                    group: group.name.clone(),
                });
            }
        }

        // A node's group must exist, and the group owns the interval.
        for node in &self.nodes {
            let Some(group) = &node.group else { continue };
            if self.group(group).is_none() {
                problems.push(Problem::NoSuchGroup {
                    node: node.name.clone(),
                    group: group.clone(),
                });
            }
            if node.attributes.contains_key("interval") {
                problems.push(Problem::IntervalInAGroup {
                    node: node.name.clone(),
                    group: group.clone(),
                });
            }
        }

        // Links: both ends must be nodes, each end must be a terminal that
        // node has, the two ends must carry the same stream, and no sink
        // twice.
        let mut bound: BTreeMap<(String, String), usize> = BTreeMap::new();
        // Every link, as the pair of ends it joins: a link written twice is
        // two deliveries of every datum, and two lines in the drawing lying on
        // top of each other.
        let mut drawn: BTreeSet<(String, String)> = BTreeSet::new();
        // Which nodes have a link on a sink and which on a source, for the
        // rule below that a payload and the ground need one or the other.
        let mut linked: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
        for link in &self.links {
            let described = |to: &str| format!("\"{}\" to \"{to}\"", link.from);
            let origin = link.origin();

            let leaves = match self.node(&origin.node) {
                None => {
                    problems.push(Problem::NoSuchNode {
                        from: origin.node.clone(),
                        link: described(&link.to.each().join(", ")),
                        missing: origin.node.clone(),
                    });
                    continue;
                }
                Some(node) => {
                    linked.entry(node.name.as_str()).or_default().1 = true;
                    // Which of its data sources this link leaves by, and what
                    // stream that carries.
                    match terminals::terminal_at(
                        &node.node_type,
                        origin.input.as_deref(),
                        false,
                    ) {
                        Ok(terminal) => terminal,
                        Err(why) => {
                            problems.push(Problem::BadTerminal {
                                from: origin.node.clone(),
                                link: described(&link.to.each().join(", ")),
                                why,
                            });
                            continue;
                        }
                    }
                }
            };

            for destination in link.to.each() {
                if !drawn.insert((link.from.clone(), destination.to_string())) {
                    problems.push(Problem::DuplicateLink {
                        from: origin.node.clone(),
                        to: destination.to_string(),
                    });
                    continue;
                }
                let Destination { node, input } = Destination::parse(destination);
                if node == origin.node {
                    problems.push(Problem::SelfLink { node });
                    continue;
                }
                let arrives = match self.node(&node) {
                    None => {
                        problems.push(Problem::NoSuchNode {
                            from: origin.node.clone(),
                            link: described(destination),
                            missing: node,
                        });
                        continue;
                    }
                    Some(at) => {
                        linked.entry(at.name.as_str()).or_default().0 = true;
                        match terminals::terminal_at(
                            &at.node_type,
                            input.as_deref(),
                            true,
                        ) {
                            // Carried with the terminal: whether this one is
                            // held to a single source, which is the node
                            // type's business and not the link's.
                            Ok(terminal) => {
                                terminal.map(|terminal| {
                                    (terminal, terminals::merges(&at.node_type))
                                })
                            }
                            Err(why) => {
                                problems.push(Problem::BadTerminal {
                                    from: origin.node.clone(),
                                    link: described(destination),
                                    why,
                                });
                                continue;
                            }
                        }
                    }
                };

                // The same stream at both ends. A link from a source carrying
                // the readings into a sink that takes commands is a link drawn
                // backwards, and it would be found at run time as a payload
                // that answers nothing.
                if let (Some(leaves), Some((arrives, _))) = (&leaves, &arrives) {
                    match (
                        terminals::stream_of(leaves),
                        terminals::stream_of(arrives),
                    ) {
                        (Some((_, one)), Some((_, two))) if one != two => {
                            problems.push(Problem::CrossedStreams {
                                from: origin.node.clone(),
                                link: described(destination),
                                leaves: leaves.clone(),
                                arrives: arrives.clone(),
                            });
                            continue;
                        }
                        _ => {}
                    }
                }

                // A sink takes one source: two into one would be two streams
                // interleaved, and nothing downstream could tell them apart.
                // Except the ground's, which can: every payload's telemetry
                // arrives at the ground labelled with the payload's id, so
                // one moc node is where the whole vehicle reports. See
                // `terminals::Terminals::merges`.
                if let Some((arrives, merges)) = arrives {
                    if !merges {
                        *bound.entry((node, arrives)).or_insert(0) += 1;
                    }
                }
            }
        }
        for ((node, input), count) in bound {
            if count > 1 {
                problems.push(Problem::InputBoundTwice { node, input });
            }
        }

        // A payload and the ground link have to be joined to something.
        //
        // A payload with neither a sink nor a source linked is a payload the
        // configuration describes and nothing reads or drives -- a sensor
        // nobody looks at, which is a line somebody meant to connect. The
        // ground link is the same the other way about: one with nothing on it
        // receives no telemetry and sends no commands.
        //
        // Either will do. A payload that is only read has a source and no
        // sink; one that is only driven -- a heater, a valve, a latch -- has a
        // sink and no source; and both are configurations somebody means.
        //
        // And a payload whose telemetry goes to the ground is joined to the
        // ground, by a link the `tcspecial` section describes rather than one
        // drawn: see `crate::graph::telemetry`. A section that does not read
        // is complained about where the section is checked, and leaves every
        // payload judged by its drawn links alone.
        let telemetered: BTreeSet<String> = crate::graph::telemetry::of(self)
            .map(|links| {
                links
                    .into_iter()
                    .filter(|l| l.via == crate::graph::telemetry::Via::PayloadTelemetry)
                    .map(|l| l.source)
                    .collect()
            })
            .unwrap_or_default();
        for node in self
            .nodes
            .iter()
            .filter(|node| node.is_payload() || node.node_type == "moc")
            .filter(|node| !telemetered.contains(&node.name))
        {
            let (sink, source) = linked.get(node.name.as_str()).copied().unwrap_or_default();
            if !sink && !source {
                problems.push(Problem::NodeOnNoLink {
                    node: node.name.clone(),
                    node_type: node.node_type.clone(),
                });
            }
        }

        // A node nobody reaches and nobody reads.
        let mut touched: BTreeSet<&str> = BTreeSet::new();
        for link in &self.links {
            // The borrow has to outlive the loop, so the node is found in the
            // graph rather than parsed into a String here.
            let name = link.from.split('.').next().unwrap_or(&link.from);
            if let Some(node) = self.node(name) {
                touched.insert(node.name.as_str());
            }
            for destination in link.to.each() {
                // The borrow has to outlive the loop, so the node name is
                // found in the graph rather than parsed into a String here.
                let name = destination.split('.').next().unwrap_or(destination);
                if let Some(node) = self.node(name) {
                    touched.insert(node.name.as_str());
                }
            }
        }
        // A command binding touches both ends of itself. A MOC link exists to
        // carry commands as much as to carry telemetry, and a thermostat whose
        // setpoint is the only thing the ground sets is reached by the binding
        // and by nothing else -- so a configuration judged by its links alone
        // would call both of them unconnected and refuse something correct.
        for binding in &self.commands {
            if let Some(node) = self.node(&binding.from) {
                touched.insert(node.name.as_str());
            }
            let target = binding.target();
            if let Some(node) = self.node(&target.node) {
                touched.insert(node.name.as_str());
            }
        }

        // A payload reporting to the ground is reached by its telemetry link,
        // as above.
        for name in &telemetered {
            if let Some(node) = self.node(name) {
                touched.insert(node.name.as_str());
            }
        }

        for node in &self.nodes {
            if !node.name.trim().is_empty() && !touched.contains(node.name.as_str()) {
                problems.push(Problem::Unconnected {
                    node: node.name.clone(),
                });
            }
        }

        // Every link to the ground has a port of its own that exists: a base
        // at 65500 and sixty nodes counted from it is a configuration that
        // reads and sockets that cannot be, and two sources on one port would
        // arrive interleaved. A section that does not read is complained
        // about where the section is checked, and not again here.
        if let Err(refused) = crate::graph::telemetry::of(self) {
            match refused.node {
                Some(node) => problems.push(Problem::BadNode {
                    node,
                    why: refused.why,
                }),
                None if !problems
                    .iter()
                    .any(|p| matches!(p, Problem::BadTcspecial { .. })) =>
                {
                    problems.push(Problem::BadTcspecial { why: refused.why })
                }
                None => {}
            }
        }

        // A ground link that receives -- commands, or a line out of it --
        // says where aboard the ground sends them: both its address and its
        // port. One that only sends to its ground_base binds nothing.
        for node in self.nodes.iter().filter(|n| n.node_type == "moc") {
            let commanded = self.commands.iter().any(|b| b.from == node.name);
            let feeds = self
                .links
                .iter()
                .any(|l| l.origin().node == node.name);
            if (commanded || feeds) && !crate::graph::reach::binds(node) {
                problems.push(Problem::BadNode {
                    node: node.name.clone(),
                    why: format!(
                        "\"{}\" receives from the ground -- {} -- so it needs an \
                         address and port for the ground to send to: say address \
                         and port",
                        node.name,
                        if commanded {
                            "a command binding comes from it"
                        } else {
                            "a line leaves it"
                        }
                    ),
                });
            }
        }

        // Command bindings: from the ground, to an attribute of a real node.
        for binding in &self.commands {
            match self.node(&binding.from) {
                Some(node) if node.node_type == "moc" => {}
                _ => problems.push(Problem::NotAMocLink {
                    from: binding.from.clone(),
                }),
            }

            let target = binding.target();
            if target.input.is_none() {
                problems.push(Problem::NoAttributeNamed {
                    sets: binding.sets.clone(),
                });
                continue;
            }
            if self.node(&target.node).is_none() {
                problems.push(Problem::NoSuchTarget {
                    sets: binding.sets.clone(),
                });
            }
        }

        // The computations: what each says it computes, and what that
        // computation must be told. Checked here so that a thermostat with no
        // band or a program with no deadline is a fault found on a desk --
        // these rules used to live in the flight software, where this tool
        // could not reach them.
        for node in self.nodes.iter().filter(|n| n.node_type == "compute") {
            let what = match crate::node::vehicle::compute::params_of(&node.name, &node.attributes) {
                Ok(what) => what,
                Err(why) => {
                    problems.push(Problem::BadCompute {
                        node: node.name.clone(),
                        why,
                    });
                    continue;
                }
            };

            // A mean says which inputs it is over, and the links say which
            // arrive. Both are checked, and against each other: a mean over
            // three sensors whose third link was deleted has two inputs, each
            // bound exactly once, and would otherwise quietly become a mean
            // over two.
            if let crate::node::vehicle::compute::Computation::Mean(mean) = &what {
                let mut bound = self.inputs_bound_at(&node.name);
                let mut declared = mean.inputs.clone();
                bound.sort();
                declared.sort();
                if bound != declared {
                    problems.push(Problem::MeanInputs {
                        node: node.name.clone(),
                        declared,
                        bound,
                    });
                }
            }
        }

        // A `null` node says nothing about itself.
        //
        // It has nothing to say: it is not opened, it has no parameters, and
        // what it does is carry what arrives through. So an attribute on one
        // is a word nothing reads -- most likely a node whose type was changed
        // and whose attributes were left behind -- and it is refused by name
        // rather than ignored, which is the rule every other type is held to.
        for node in self.nodes.iter().filter(|n| n.node_type == "null") {
            for attribute in node.attributes.keys() {
                problems.push(Problem::BadNode {
                    node: node.name.clone(),
                    why: format!(
                        "\"{}\": \"{attribute}\" is not an attribute of a null                          node: a null node carries what arrives through and has                          nothing to be told",
                        node.name
                    ),
                });
            }
        }

        // The apps: each is the program it names, and what else it may be told
        // is `node::app`'s to say -- one rule for all five kinds, the three
        // that are the vehicle's own and the two that are payloads. Checked
        // here for the reason a computation's parameters are: a program nobody
        // named is a node the vehicle cannot start, and a desk is where that is
        // worth finding out.
        for node in self
            .nodes
            .iter()
            .filter(|n| crate::node::app::Kind::of(&n.node_type).is_some())
        {
            if let Err(why) = crate::node::app::params_of(
                &node.name,
                &node.node_type,
                &node.attributes,
            ) {
                problems.push(Problem::BadNode {
                    node: node.name.clone(),
                    why,
                });
            }
        }

        // The logs, and whether what links into each of them fits the shape it
        // writes. Settled here and not when a record is written: a log that
        // would have to refuse a record in flight loses telemetry at the
        // moment telemetry is worth most.
        for node in self.nodes.iter().filter(|n| n.node_type == "log") {
            let params = match crate::node::vehicle::log::params_of(&node.name, &node.attributes) {
                Ok(params) => params,
                Err(why) => {
                    problems.push(Problem::BadLog {
                        node: node.name.clone(),
                        // The name is in the problem already; the reason need
                        // not carry it twice.
                        why: why
                            .strip_prefix(&format!("\"{}\": ", node.name))
                            .unwrap_or(&why)
                            .to_string(),
                    });
                    continue;
                }
            };

            // Only a fixed-length log is particular: the two variable formats
            // take a record of any length, so any source suits them.
            let Some(wanted) = params.format.exactly() else {
                continue;
            };

            for source in self.sources_of(&node.name) {
                let produces = self.record_bytes_of(source);
                if produces != Some(wanted) {
                    problems.push(Problem::LogSourceMismatch {
                        log: node.name.clone(),
                        source: source.to_string(),
                        wanted,
                        produces,
                    });
                }
            }
        }

        // What tcspecial itself is configured with.
        //
        // Required: a graph describes what the vehicle carries and what is
        // done with it, and none of that reaches the ground unless something
        // says where commands are taken and where beacons go. Checked with
        // the same code the payload sets are checked with, so that one
        // mistake is one complaint wherever the section is written.
        match &self.tcspecial {
            None => problems.push(Problem::NoTcspecial),
            Some(said) => {
                // Two rounds of rules, and both of them somebody else's. What
                // the section may say and whether it says it is in
                // parameters.rs; what the values have to be -- an address
                // and a port wherever one is named, two links that are not
                // one socket -- is in the struct the command interpreter
                // itself reads.
                // Flat, prefixes and all, which is how the rules and the
                // complaints name an attribute: the file's shape is the
                // file's business. See `parameters::flatten`.
                let flat = crate::graph::parameters::flatten(said);
                match crate::graph::parameters::ci_config_of(&flat) {
                    Err(why) => problems.push(Problem::BadTcspecial { why }),
                    Ok(section) => {
                        if let Err(why) = section.to_ci_config() {
                            problems.push(Problem::BadTcspecial { why });
                        }
                    }
                }
            }
        }

        // Every node that is opened says what opening it takes.
        //
        // The rules are the ones that have been tested against real buses,
        // reached rather than restated: a bus and an address for a device on
        // one, a protocol and an address and a port for a network link, a
        // path and a set of lines for discrete lines. What this moves is a
        // whole class of error from flight time to desk time -- a sensor with
        // no address used to be found when the vehicle tried to open it.
        //
        // Only the nodes that are reached over an interface. A computation and
        // a log have their own rules below, and a node whose type nothing
        // recognises is refused by name already -- a second complaint about
        // its attributes would bury the one that says what is actually wrong.
        //
        // An app node is not reached over an interface at all, and two of them
        // are payloads: a payload that is a program is reached by running it,
        // which is why it names a program and no bus. Asking this of one would
        // refuse every `app_pl` in every file for want of an address it has no
        // use for.
        for node in self.nodes.iter().filter(|node| {
            (node.is_payload() && crate::node::app::Kind::of(&node.node_type).is_none())
                || node.node_type == "moc"
        }) {
            if let Err(why) = crate::graph::reach::reach_of(node) {
                problems.push(Problem::BadNode {
                    node: node.name.clone(),
                    why,
                });
            }
        }

        // What may be written to a payload.
        //
        // A payload that is read is asked for its readings, and the bytes
        // that ask are its trigger -- so a payload that is read, is written
        // to, and states no trigger is a configuration delivering data to
        // something it also describes as answering nothing. The usual cause
        // is a link drawn to the wrong node.
        //
        // Only a payload that is read. A payload that is written and never
        // read -- a heater, a valve, a latch -- is driven rather than asked,
        // and there is nothing for a trigger to do on one: the bytes would be
        // written before a read that never happens. Requiring one there would
        // be requiring an attribute that means nothing, which is how a rule
        // gets worked around rather than obeyed.
        //
        // And not of a payload that is a program. What asks an `app_all_pl`
        // for a reading is the MOC data arriving on its interface -- that
        // being the whole of what its program is given -- so the bytes that
        // ask are the datum itself and there is no trigger to state. An
        // `app_pl` is asked nothing at all and has no sink for a link to
        // arrive at, so nothing reaches this rule by that road either.
        for node in self.nodes.iter().filter(|n| {
            n.is_payload() && crate::node::app::Kind::of(&n.node_type).is_none()
        }) {
            if node.attributes.contains_key("trigger") {
                continue;
            }
            // Whatever terminal it leaves by: a payload read through its
            // `payload-out` is a payload that is read.
            let read = self.links.iter().any(|link| {
                link.from.split('.').next().unwrap_or(&link.from) == node.name
            });
            if !read {
                continue;
            }
            for from in self.sources_of(&node.name) {
                problems.push(Problem::UntriggeredInput {
                    node: node.name.clone(),
                    from: from.to_string(),
                });
            }
        }

        problems
    }

    /// Every named input of this node that a link binds.
    ///
    /// A terminal is not one of them. `in0` is which *of several* ways into a
    /// computation and a mean is built on knowing; `moc-in` is which *way*
    /// the data is going, and a link may name it on a computation as it does on
    /// any other type. Counting one as an input would make a mean over `in0`
    /// and `in1` look like a mean over three things the moment somebody drew a
    /// line from the ground to it.
    fn inputs_bound_at(&self, node: &str) -> Vec<String> {
        let mut inputs = Vec::new();
        for link in &self.links {
            for destination in link.to.each() {
                let Destination { node: to, input } = Destination::parse(destination);
                if to == node {
                    if let Some(input) = input {
                        if terminals::stream_of(&input).is_none() {
                            inputs.push(input);
                        }
                    }
                }
            }
        }
        inputs
    }

    /// Every node whose output arrives at this one.
    ///
    /// Each once: a node delivering to two inputs of one destination is one
    /// source of it, and naming it twice would read as two.
    ///
    /// Named as the node, whatever terminal the link left by: a complaint that
    /// said `"n.payload-out" produces records of three bytes` would be naming a
    /// thing the file has no entry for.
    fn sources_of(&self, node: &str) -> Vec<&str> {
        let mut sources: Vec<&str> = Vec::new();
        for link in &self.links {
            let arrives = link.to.each().iter().any(|destination| {
                destination.split('.').next().unwrap_or(destination) == node
            });
            let from = link.from.split('.').next().unwrap_or(&link.from);
            if arrives && !sources.contains(&from) {
                sources.push(from);
            }
        }
        sources
    }

    /// How long this node's records are, where the configuration says.
    ///
    /// `read_size` and nothing else. A device read in fixed-size packets says
    /// so and is the case a fixed-length log exists for; a computation's
    /// result is a number written out as text, whose width depends on the
    /// number, and nothing in the file could say otherwise. `None` is "not
    /// knowable from this configuration", which is different from a length
    /// that disagrees and is reported differently.
    fn record_bytes_of(&self, node: &str) -> Option<u32> {
        let node = self.node(node)?;
        if node.node_type == "compute" || node.node_type == "log" {
            return None;
        }
        let stated = node.attributes.get("read_size")?;
        stated.as_str().parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{CommandBinding, Destinations, Group, Link, Node};
    use crate::Scalar;
    use std::collections::BTreeMap;

    /// What tcspecial is configured with, for a graph that is otherwise
    /// sound.
    ///
    /// Required of every configuration, so every test that is about something
    /// else has to have one: a graph without it is refused, and a test that
    /// read `check() == []` would be reading the wrong refusal. The values are
    /// the shipped examples'.
    fn tcspecial() -> BTreeMap<String, crate::graph::parameters::Parameter> {
        // Written flat and nested here, which is the arrangement everything
        // but the file itself works in: `parameters::nest` is what puts the
        // subsections back.
        crate::graph::parameters::nest(
            &[
                ("tcs_cmd.recv", "0.0.0.0:4000"),
                ("tcs_cmd.send", "127.0.0.1:4100"),
                ("payload_telemetry.source_base", "0.0.0.0:6000"),
                ("payload_telemetry.dest_base", "127.0.0.1:6100"),
                ("payload_command.source_base", "127.0.0.1:4300"),
                ("payload_command.dest_base", "0.0.0.0:4200"),
                ("beacon.interval", "5s"),
                ("beacon.send", "0.0.0.0:5550"),
                ("beacon.recv", "127.0.0.1:5650"),
                ("log.directory", "/var/log/tcspecial"),
                ("log.segment_size", "65536"),
            ]
            .into_iter()
            .map(|(name, value)| (name.to_string(), Scalar::of(value)))
            .collect(),
        )
    }

    /// A graph built here rather than read, so a test can make one wrong.
    fn graph(nodes: Vec<Node>, links: Vec<Link>) -> Graph {
        Graph {
            description: None,
            tcspecial: Some(tcspecial()),
            groups: Vec::new(),
            nodes,
            links,
            commands: Vec::new(),
            files: Vec::new(),
            homes: BTreeMap::new(),
            placed: BTreeMap::new(),
            set_aside: Default::default(),
        }
    }

    /// A node of this type, with whatever its type cannot do without.
    ///
    /// Every type is given the minimum it is opened with, because a node that
    /// says less than that is refused here now -- and these tests are about
    /// the shape of a graph rather than about which bus anything is on. A
    /// compute node gets a conversion for the same reason, and a conversion
    /// rather than a mean: a mean's inputs are checked against the links,
    /// which is the one computation whose attributes depend on the shape
    /// around it.
    fn node(name: &str, node_type: &str) -> Node {
        let mut attributes = BTreeMap::new();
        let minimum: &[(&str, &str)] = match node_type {
            "i2c" => &[("path", "/dev/i2c-1"), ("address", "0x48")],
            "spi" => &[
                ("path", "/dev/spidev0.0"),
                ("max_speed", "1000000"),
                ("spi_mode", "0"),
            ],
            "serial" => &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("byte_length", "8"),
                ("stop_bits", "1"),
            ],
            "can" => &[("interface", "can0"), ("can_id", "0x100")],
            "iio" => &[("path", "/dev/iio:device0")],
            "rpmsg" => &[("path", "/dev/rpmsg0"), ("service", "telemetry")],
            "gpio" => &[("path", "/dev/gpiochip0"), ("lines", "3,4,5")],
            "device-file" => &[("path", "/dev/urandom")],
            "network" => &[
                ("protocol", "udp"),
                ("address", "127.0.0.1"),
                ("port", "5100"),
            ],
            "moc" => &[
                ("address", "10.0.0.1"),
                ("port", "5000"),
                ("ground_base", "10.0.0.2:5100"),
            ],
            // An app node is the program it names, and the argument for each
            // stream its kind handles: see `crate::node::app`.
            "app_payload_veh" | "app_pl" => {
                &[("executable", "/bin/cat"), ("app-payload-arg", "--readings")]
            }
            "app_moc_veh" => &[("executable", "/bin/cat"), ("app-moc-arg", "--commands")],
            "app_all_veh" | "app_all_pl" => &[
                ("executable", "/bin/cat"),
                ("app-payload-arg", "--readings"),
                ("app-moc-arg", "--commands"),
            ],
            "compute" => &[
                ("function", "to_kelvin"),
                ("format", "i16_be"),
                ("scale", "1.0"),
                ("offset", "0.0"),
            ],
            _ => &[],
        };
        for (attribute, value) in minimum {
            attributes.insert(attribute.to_string(), Scalar::of(*value));
        }

        Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes,
        }
    }

    fn link(from: &str, to: &[&str]) -> Link {
        Link {
            from: from.to_string(),
            to: Destinations::of(to.iter().map(|s| s.to_string()).collect()),
            comment: None,
        }
    }

    /// A configuration with nothing wrong with it is accepted.
    #[test]
    fn a_sound_graph_has_nothing_said_about_it() {
        let g = graph(
            vec![node("a", "i2c"), node("b", "compute"), node("c", "moc")],
            vec![link("a", &["b"]), link("b", &["c"])],
        );
        assert_eq!(g.check(), Vec::new());
    }

    /// Every node that is opened says what opening it takes.
    ///
    /// The check the editor's laziness rests on. tcsgui requires nothing to be
    /// entered -- an editor that did would fight a person working out what
    /// they are describing -- so this is where a configuration is held to
    /// saying it all, and it says which node and what to add. A sensor with no
    /// address used to be found when the vehicle tried to open it.
    #[test]
    fn every_node_that_is_opened_says_what_that_takes() {
        // A graph of nodes that say nothing.
        let bare = |name: &str, node_type: &str| Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes: BTreeMap::new(),
        };

        let g = graph(
            vec![bare("sensor", "i2c"), bare("ground", "moc")],
            vec![link("sensor", &["ground"])],
        );
        let problems = g.check();

        for (node, expected) in [("sensor", "path"), ("ground", "address")] {
            let said = problems
                .iter()
                .find_map(|p| match p {
                    Problem::BadNode { node: named, why } if named == node => {
                        Some(why.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{node}: {problems:?}"));
            // The complaint names the node and the attribute to add, so that
            // tcsverify can put a line number on it and a reader knows what to
            // type.
            assert!(said.contains(node), "{said}");
            assert!(said.contains(expected), "{said}");
        }

        // Reported on the node, which is the line a reader will look for.
        let said = problems
            .iter()
            .find(|p| matches!(p, Problem::BadNode { .. }))
            .unwrap();
        assert!(matches!(said.about(), About::Node(_)));

        // A computation and a log are not opened and are not asked: they have
        // rules of their own, and this one would tell them they are not
        // interfaces.
        let g = graph(
            vec![node("k", "compute"), node("c", "moc")],
            vec![link("k", &["c"])],
        );
        assert!(
            !g.check()
                .iter()
                .any(|p| matches!(p, Problem::BadNode { .. })),
            "{:?}",
            g.check()
        );

        // And what a node says has to be a thing its type knows. A misspelled
        // attribute is otherwise a value silently ignored -- a peripheral left
        // clocked at whatever the last program set.
        let mut said_wrong = node("sensor", "i2c");
        said_wrong
            .attributes
            .insert("max_speeed".to_string(), Scalar::of("1000000"));
        let g = graph(
            vec![said_wrong, node("c", "moc")],
            vec![link("sensor", &["c"])],
        );
        let complaint = g
            .check()
            .into_iter()
            .find_map(|p| match p {
                Problem::BadNode { why, .. } => Some(why),
                _ => None,
            })
            .expect("a misspelled attribute should be refused");
        assert!(complaint.contains("max_speeed"), "{complaint}");
    }

    /// An app node is held to what an app node may say.
    ///
    /// The rules are `crate::node::app`'s and are tested there; what
    /// is asked here is that this reaches them, and that what it says names
    /// the node -- an operator reading a report wants to know which of forty
    /// it is about.
    ///
    /// The graph is sound first, which is worth as much as the refusal: it is
    /// the statement that the three kinds can be linked the way their
    /// interfaces say. `through` stands where a `null` node would, carrying
    /// both streams; `fore` takes a sensor's readings and answers with
    /// commands, which is what the ground does; and `back` is asked and
    /// answers, which is what a payload does.
    #[test]
    fn an_app_node_is_held_to_what_it_may_say() {
        let mut sensor = node("a", "i2c");
        let mut answers = node("b", "i2c");
        for one in [&mut sensor, &mut answers] {
            // Written to, so each says what asks it for a reading.
            one.attributes
                .insert("trigger".to_string(), Scalar::of("55"));
        }

        let sound = graph(
            vec![
                sensor,
                answers,
                node("through", "app_all_veh"),
                node("fore", "app_payload_veh"),
                node("back", "app_moc_veh"),
                node("c", "moc"),
            ],
            vec![
                link("a.payload-out", &["through.payload-in"]),
                link("through.payload-out", &["c.payload-in"]),
                link("c.moc-out", &["through.moc-in", "back.moc-in"]),
                link("through.moc-out", &["a.moc-in"]),
                link("b.payload-out", &["fore.payload-in", "c.payload-in"]),
                link("fore.moc-out", &["b.moc-in"]),
                link("back.payload-out", &["c.payload-in"]),
            ],
        );
        assert_eq!(
            sound
                .check()
                .iter()
                .map(|p| p.describe())
                .collect::<Vec<String>>(),
            Vec::<String>::new()
        );

        // A node of this type that names no program is a node the vehicle
        // cannot start, and it is said here rather than at startup.
        let mut g = sound.clone();
        g.nodes
            .iter_mut()
            .find(|n| n.name == "fore")
            .expect("the node")
            .attributes
            .clear();
        let said = g
            .check()
            .iter()
            .find_map(|p| match p {
                Problem::BadNode { node, why } if node == "fore" => Some(why.clone()),
                _ => None,
            })
            .expect("a node with no program");
        assert!(said.contains("executable is not set"), "{said}");
        assert!(said.contains("app_payload_veh"), "{said}");
    }

    /// A configuration must say what tcspecial itself is configured with.
    ///
    /// A graph describes what the vehicle carries and what is done with it,
    /// and none of that reaches the ground until something says where commands
    /// are taken and where beacons go. Required rather than defaulted for the
    /// reason the attributes inside it are: a configuration that has not been
    /// asked the question has not answered it.
    #[test]
    fn a_configuration_says_what_tcspecial_is_configured_with() {
        let mut g = graph(vec![node("a", "i2c"), node("c", "moc")], vec![link("a", &["c"])]);
        assert_eq!(g.check(), Vec::new(), "this one is sound");

        g.tcspecial = None;
        let problems = g.check();
        assert!(
            problems.contains(&Problem::NoTcspecial),
            "{problems:?}"
        );
        let said = problems
            .iter()
            .find(|p| **p == Problem::NoTcspecial)
            .unwrap();
        // Reported on the section, which is where it would have to be
        // written, and the complaint says what the section is for.
        assert_eq!(said.about(), About::Section("tcspecial"));
        for word in ["tcspecial", "commands", "beacons"] {
            assert!(said.describe().contains(word), "{}", said.describe());
        }
    }

    /// A section that is there and does not hold together is refused by its
    /// own words.
    ///
    /// Two sets of rules behind this, and neither of them written twice: what
    /// the section may say is `graph::parameters`, and what the values have to
    /// be is the struct the command interpreter itself reads. A mistake is one
    /// complaint wherever the section is written.
    #[test]
    fn a_tcspecial_section_that_does_not_hold_together_is_refused() {
        let wrong = [
            // A name belonging to nothing, at the top of the section and
            // under a subsection: both are a value silently ignored if they
            // are not refused.
            ("beacon_adress", "0.0.0.0:5550", "beacon_adress"),
            ("beacon.adress", "0.0.0.0:5550", "beacon.adress"),
            // A command interface that is only half stated, which is the way
            // one attribute carrying two things goes wrong.
            ("tcs_cmd.recv", "0.0.0.0", "address:port"),
            // A port that is not one.
            ("payload_port", "nought", "payload_port"),
            // And a beacon address with no port, which is the rule the
            // command interpreter's own struct knows: beacons go from one
            // socket to another, each written address:port.
            ("beacon.send", "10.0.0.1", "address:port"),
            ("beacon.recv", "127.0.0.1:0", "port 0"),
        ];

        for (attribute, value, expected) in wrong {
            let mut g = graph(vec![node("a", "i2c"), node("c", "moc")], vec![link("a", &["c"])]);
            let mut flat = crate::graph::parameters::flatten(g.tcspecial.as_ref().unwrap());
            flat.insert(attribute.to_string(), Scalar::of(value));
            g.tcspecial = Some(crate::graph::parameters::nest(&flat));

            let problems = g.check();
            let said = problems
                .iter()
                .find_map(|p| match p {
                    Problem::BadTcspecial { why } => Some(why.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{attribute} = {value}: {problems:?}"));
            assert!(
                said.contains(expected),
                "{attribute} = {value} was refused as {said:?}"
            );
        }
    }

    /// A payload that is read takes nothing unless it says what asks it.
    ///
    /// The rule: what is written to a payload that is read is what asks it for
    /// a reading, and the trigger is the asking. A link delivering into a
    /// payload with no trigger has nowhere to go, and the usual cause is a
    /// link drawn to the wrong node -- which is a mistake nothing else in the
    /// configuration would reveal.
    #[test]
    fn a_payload_that_is_read_takes_nothing_without_a_trigger() {
        // A sensor read into a computation, with something delivered back
        // into it.
        let g = graph(
            vec![
                node("sensor", "i2c"),
                node("k", "compute"),
                node("moc", "moc"),
            ],
            vec![
                link("sensor", &["k"]),
                link("k", &["moc"]),
                link("moc", &["sensor"]),
            ],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::UntriggeredInput { node, from } if node == "sensor" && from == "moc"
            )),
            "{problems:?}"
        );
        // Reported on the payload, which is where a trigger would be written.
        let said = problems
            .iter()
            .find(|p| matches!(p, Problem::UntriggeredInput { .. }))
            .unwrap();
        assert_eq!(said.about(), About::Node("sensor".to_string()));
        assert!(said.describe().contains("trigger"), "{}", said.describe());

        // And with a trigger it is allowed: the payload answers what it is
        // asked, which is what a trigger says.
        let mut nodes = vec![
            node("sensor", "i2c"),
            node("k", "compute"),
            node("moc", "moc"),
        ];
        nodes[0]
            .attributes
            .insert("trigger".to_string(), Scalar::of("01"));
        let g = graph(
            nodes,
            vec![
                link("sensor", &["k"]),
                link("k", &["moc"]),
                link("moc", &["sensor"]),
            ],
        );
        assert!(
            !g.check()
                .iter()
                .any(|p| matches!(p, Problem::UntriggeredInput { .. })),
            "{:?}",
            g.check()
        );
    }

    /// A payload that is only driven needs no trigger.
    ///
    /// A heater, a valve, a latch: nothing is read from it, so there is
    /// nothing to ask it for, and a trigger on one would be bytes written
    /// before a read that never happens. Requiring one would be requiring an
    /// attribute that means nothing -- which is how a rule gets worked around
    /// instead of obeyed, and the shipped thermostat is exactly this shape.
    #[test]
    fn a_payload_that_is_only_driven_needs_no_trigger() {
        let g = graph(
            vec![
                node("sensor", "i2c"),
                node("thermostat", "compute"),
                node("heaters", "gpio"),
            ],
            vec![
                link("sensor", &["thermostat"]),
                link("thermostat", &["heaters"]),
            ],
        );
        assert!(
            !g.check()
                .iter()
                .any(|p| matches!(p, Problem::UntriggeredInput { .. })),
            "{:?}",
            g.check()
        );
    }

    /// The rule is about payloads that are devices and nothing else.
    ///
    /// A computation, a log and the ground link all take inputs and none of
    /// them is a thing the vehicle carries: a trigger would mean nothing on
    /// any of them, and asking for one would make every configuration
    /// unwritable. A payload that is a program is a payload and is still not
    /// asked, for the reason the rule itself gives: what asks it is the datum
    /// that arrives, there being nothing else it is given.
    #[test]
    fn only_a_payload_is_asked_for_a_trigger() {
        for node_type in ["compute", "log", "moc"] {
            assert!(!node("x", node_type).is_payload(), "{node_type}");
        }
        for node_type in [
            "i2c",
            "spi",
            "serial",
            "hdlc",
            "can",
            "iio",
            "rpmsg",
            "network",
            "device-file",
            "gpio",
        ] {
            assert!(node("x", node_type).is_payload(), "{node_type}");
        }
        // The two payloads that are programs: payloads, and asked for no
        // trigger. One of them cannot even be written to.
        for node_type in ["app_pl", "app_all_pl"] {
            assert!(node("x", node_type).is_payload(), "{node_type}");
        }
        let g = graph(
            vec![node("bay", "app_all_pl"), node("moc", "moc")],
            vec![link("bay", &["moc"]), link("moc", &["bay"])],
        );
        assert!(
            !g.check()
                .iter()
                .any(|p| matches!(p, Problem::UntriggeredInput { .. })),
            "{:?}",
            g.check()
        );

        // And a type nothing recognises is not a payload: it is refused by
        // name already, and a second complaint about a trigger would bury the
        // one that says what is actually wrong.
        assert!(!node("x", "contraption").is_payload());
    }

    /// A link naming a node that is not there is refused, saying which.
    ///
    /// The first check the architecture promises, and the one that makes a
    /// configuration checkable at all.
    #[test]
    fn a_link_to_nowhere_is_refused() {
        let g = graph(vec![node("a", "i2c")], vec![link("a", &["nowhere"])]);
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::NoSuchNode { missing, .. } if missing == "nowhere"
            )),
            "{problems:?}"
        );
        // And the complaint names the link, so a file with twenty of them can
        // be read.
        let said = problems[0].describe();
        assert!(said.contains("nowhere"), "{said}");
    }

    /// Two links into one input are refused.
    ///
    /// The check that keeps a mean from averaging one reading with itself
    /// while another never arrives.
    #[test]
    fn one_input_may_be_bound_once() {
        let g = graph(
            vec![node("a", "i2c"), node("b", "i2c"), node("m", "compute")],
            vec![link("a", &["m.in0"]), link("b", &["m.in0"])],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::InputBoundTwice { node, input } if node == "m" && input == "in0"
            )),
            "{problems:?}"
        );

        // Different inputs of the same node are fine, which is what a mean
        // over three sensors is.
        let g = graph(
            vec![node("a", "i2c"), node("b", "i2c"), node("m", "compute")],
            vec![link("a", &["m.in0"]), link("b", &["m.in1"])],
        );
        assert!(!g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::InputBoundTwice { .. })));
    }

    /// A link with several destinations is the "T", and is not a fault.
    #[test]
    fn one_link_may_reach_several_places() {
        let g = graph(
            vec![
                node("k", "compute"),
                node("moc", "moc"),
                node("m", "compute"),
            ],
            vec![link("k", &["moc", "m.in0"])],
        );
        assert_eq!(g.check(), Vec::new(), "a fan-out was refused");
    }

    /// Two nodes of one name are refused.
    #[test]
    fn a_name_belongs_to_one_node() {
        let g = graph(vec![node("a", "i2c"), node("a", "spi")], Vec::new());
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::DuplicateNode { .. })));
    }

    /// A group must exist, must read on a readable interval, and owns it.
    #[test]
    fn a_group_owns_the_interval_of_everything_in_it() {
        let mut g = graph(vec![node("a", "i2c"), node("b", "moc")], vec![link("a", &["b"])]);
        g.nodes[0].group = Some("temps".to_string());

        // Named and not declared.
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::NoSuchGroup { .. })));

        g.groups.push(Group {
            name: "temps".to_string(),
            interval: "3s".to_string(),
        });
        assert_eq!(g.check(), Vec::new());

        // A member may not state an interval of its own: two sensors in one
        // group with intervals each is a configuration disagreeing with
        // itself.
        g.nodes[0]
            .attributes
            .insert("interval".to_string(), crate::endpoint_config::Scalar::of("1s"));
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::IntervalInAGroup { .. })));

        // And an interval that cannot be read is said so.
        let mut g = graph(Vec::new(), Vec::new());
        g.groups.push(Group {
            name: "temps".to_string(),
            interval: "soon".to_string(),
        });
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::BadInterval { .. })));
    }

    /// A group nothing is in is refused, as tcsgui has always marked it.
    #[test]
    fn a_group_with_nothing_in_it_is_refused() {
        let mut g = graph(vec![node("a", "i2c"), node("b", "moc")], vec![link("a", &["b"])]);
        g.groups.push(Group {
            name: "temps".to_string(),
            interval: "3s".to_string(),
        });
        let problems = g.check();
        assert_eq!(
            problems,
            vec![Problem::EmptyGroup {
                group: "temps".to_string()
            }]
        );
        assert_eq!(problems[0].severity(), Severity::Error);

        g.nodes[0].group = Some("temps".to_string());
        assert_eq!(g.check(), Vec::new());
    }

    /// A node nothing links to or from is named.
    ///
    /// More likely a mistake than an intention, and the kind a reader of the
    /// file would not notice: a vehicle node placed and never connected is
    /// given nothing and gives nothing to anyone.
    #[test]
    fn a_node_with_no_links_is_named() {
        let g = graph(vec![node("lonely", "null")], Vec::new());
        let problems = g.check();
        assert!(
            problems
                .iter()
                .any(|p| matches!(p, Problem::Unconnected { node } if node == "lonely")),
            "{problems:?}"
        );
    }

    /// A payload with no links is not: its telemetry goes to the ground over
    /// the link the `tcspecial` section describes. See
    /// `crate::graph::telemetry`.
    #[test]
    fn a_payload_with_no_links_reports_to_the_ground() {
        let g = graph(vec![node("sensor", "i2c")], Vec::new());
        let problems = g.check();
        assert!(
            !problems.iter().any(|p| matches!(
                p,
                Problem::Unconnected { .. } | Problem::NodeOnNoLink { .. }
            )),
            "{problems:?}"
        );
    }

    /// Bases with no room for the payloads counted from them are refused.
    #[test]
    fn a_telemetry_base_with_no_room_is_refused() {
        // Two payloads nothing in the drawing touches, so both report over
        // the section's links: the second is counted on by one.
        let mut g = graph(vec![node("a", "i2c"), node("b", "i2c")], Vec::new());
        let section = g.tcspecial.as_mut().expect("the helper's section");
        let mut flat = crate::graph::parameters::flatten(section);
        flat.insert(
            "payload_telemetry.dest_base".to_string(),
            Scalar::of("127.0.0.1:65535"),
        );
        *section = crate::graph::parameters::nest(&flat);
        let problems = g.check();
        assert!(
            problems
                .iter()
                .any(|p| matches!(p, Problem::BadTcspecial { why } if why.contains("65535"))),
            "{problems:?}"
        );
    }

    /// A ground link that only sends needs no address of its own, and one
    /// that receives does.
    #[test]
    fn a_ground_link_that_receives_says_where() {
        let mut sends = node("moc", "moc");
        sends.attributes.remove("address");
        sends.attributes.remove("port");
        let g = graph(vec![node("a", "i2c"), sends.clone()], vec![link("a", &["moc"])]);
        assert_eq!(g.check(), Vec::new());

        let mut g = graph(
            vec![sends, node("thermostat", "compute")],
            Vec::new(),
        );
        g.commands = vec![CommandBinding {
            from: "moc".to_string(),
            sets: "thermostat.setpoint".to_string(),
        }];
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadNode { node, why } if node == "moc" && why.contains("address and port")
            )),
            "{problems:?}"
        );
    }

    /// A command binding counts as a connection, at both of its ends.
    ///
    /// A MOC link exists to carry commands as much as telemetry, and a
    /// computation whose setpoint is the only thing the ground sets is reached
    /// by the binding and by nothing else. Judged by links alone both would be
    /// called unconnected, and a correct configuration refused.
    #[test]
    fn a_command_binding_connects_both_of_its_ends() {
        let mut g = graph(
            vec![node("moc", "moc"), node("thermostat", "compute")],
            Vec::new(),
        );
        g.commands = vec![CommandBinding {
            from: "moc".to_string(),
            sets: "thermostat.setpoint".to_string(),
        }];

        let problems = g.check();
        assert!(
            !problems
                .iter()
                .any(|p| matches!(p, Problem::Unconnected { .. })),
            "neither end of a binding is unconnected: {problems:?}"
        );
    }

    /// A link from a node to itself is refused.
    #[test]
    fn a_node_may_not_link_to_itself() {
        let g = graph(vec![node("a", "compute")], vec![link("a", &["a"])]);
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::SelfLink { .. })));
    }

    /// A command binding comes from the ground and names an attribute.
    #[test]
    fn a_command_sets_an_attribute_of_a_real_node_from_the_ground() {
        let mut g = graph(
            vec![node("moc", "moc"), node("t", "compute")],
            vec![link("t", &["moc"])],
        );
        g.commands.push(CommandBinding {
            from: "moc".to_string(),
            sets: "t.setpoint".to_string(),
        });
        assert_eq!(g.check(), Vec::new());

        // Naming no attribute.
        g.commands[0].sets = "t".to_string();
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::NoAttributeNamed { .. })));

        // Naming a node that is not there.
        g.commands[0].sets = "nobody.setpoint".to_string();
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::NoSuchTarget { .. })));

        // Arriving on something that is not a MOC link: commands come from
        // the ground, and a binding on a sensor would be a sensor commanding
        // the vehicle.
        g.commands[0] = CommandBinding {
            from: "t".to_string(),
            sets: "t.setpoint".to_string(),
        };
        assert!(g
            .check()
            .iter()
            .any(|p| matches!(p, Problem::NotAMocLink { .. })));
    }

    /// A computation is checked here, where the ground tool can see it.
    #[test]
    fn a_computation_that_cannot_run_is_refused_on_the_ground() {
        let computation = |pairs: &[(&str, &str)]| {
            let mut thermostat = node("thermostat", "compute");
            thermostat.attributes.clear();
            for (attribute, value) in pairs {
                thermostat
                    .attributes
                    .insert(attribute.to_string(), Scalar::of(*value));
            }
            let g = graph(
                vec![node("sensor", "i2c"), thermostat],
                vec![link("sensor", &["thermostat"])],
            );
            g.check()
        };

        // A thermostat with no band, which used to pass here and be refused
        // when the vehicle started.
        let problems = computation(&[
            ("function", "hysteresis"),
            ("setpoint", "293.15"),
            ("above", "2.0"),
        ]);
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadCompute { node, why } if node == "thermostat" && why.contains("below")
            )),
            "{problems:?}"
        );

        // A program with no deadline.
        let problems = computation(&[("function", "process"), ("executable", "/bin/cat")]);
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadCompute { why, .. } if why.contains("timeout")
            )),
            "{problems:?}"
        );

        // And one that is right, which says nothing.
        let problems = computation(&[
            ("function", "hysteresis"),
            ("setpoint", "293.15"),
            ("above", "2.0"),
            ("below", "2.0"),
        ]);
        assert!(
            !problems
                .iter()
                .any(|p| matches!(p, Problem::BadCompute { .. })),
            "{problems:?}"
        );
    }

    /// A mean is over the inputs the links bind, and says which they are.
    ///
    /// The fault this catches has no other symptom: a mean over three sensors
    /// whose third link was deleted has two inputs, each bound exactly once,
    /// and would otherwise quietly become a mean over two.
    #[test]
    fn a_mean_is_over_the_inputs_that_arrive() {
        let mean = |declared: &str, links: Vec<Link>| {
            let mut average = node("average", "compute");
            average.attributes.clear();
            average
                .attributes
                .insert("function".to_string(), Scalar::of("mean"));
            average
                .attributes
                .insert("inputs".to_string(), Scalar::of(declared));
            let g = graph(
                vec![node("a", "i2c"), node("b", "i2c"), average],
                links,
            );
            g.check()
        };

        let both = || {
            vec![
                link("a", &["average.in0"]),
                link("b", &["average.in1"]),
            ]
        };

        // Three declared, two bound.
        let problems = mean("in0,in1,in2", both());
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::MeanInputs { node, declared, bound }
                    if node == "average" && declared.len() == 3 && bound.len() == 2
            )),
            "{problems:?}"
        );

        // Two declared and two bound, under other names: the names have to be
        // the same ones, not merely the same number -- a mean told in1 and in2
        // and linked in0 and in1 is waiting for a reading nothing delivers.
        let problems = mean("in1,in2", both());
        assert!(
            problems
                .iter()
                .any(|p| matches!(p, Problem::MeanInputs { .. })),
            "{problems:?}"
        );

        // And the two agreeing, which says nothing.
        let problems = mean("in0,in1", both());
        assert!(
            !problems
                .iter()
                .any(|p| matches!(p, Problem::MeanInputs { .. })),
            "{problems:?}"
        );
    }

    /// A terminal named on a computation is not one of its inputs.
    ///
    /// A mean says which inputs it is over and the links say which arrive, and
    /// the two are held against each other. A link that names a *terminal* --
    /// which is what the editor writes when somebody draws a line to the
    /// corner of a computation where what the ground sends arrives -- is not an
    /// input, and counting it as one made a mean over two readings look like a
    /// mean over three things.
    #[test]
    fn a_terminal_on_a_computation_is_not_an_input() {
        let mut average = node("average", "compute");
        average.attributes.clear();
        average
            .attributes
            .insert("function".to_string(), Scalar::of("mean"));
        average
            .attributes
            .insert("inputs".to_string(), Scalar::of("in0,in1"));

        let g = graph(
            vec![
                node("a", "i2c"),
                node("b", "i2c"),
                average,
                node("moc", "moc"),
            ],
            vec![
                link("a", &["average.in0"]),
                link("b", &["average.in1"]),
                link("average", &["moc"]),
                // And the ground reaching it, drawn to the corner where what
                // the ground sends arrives.
                link("moc.moc-out", &["average.moc-in"]),
            ],
        );
        let problems = g.check();
        assert!(
            !problems
                .iter()
                .any(|p| matches!(p, Problem::MeanInputs { .. })),
            "{problems:?}"
        );
    }

    /// A log says what a log says, and is refused for the rest.
    #[test]
    fn a_log_whose_attributes_are_not_a_logs_is_refused() {
        let mut log = node("housekeeping", "log");
        log.attributes
            .insert("format".to_string(), Scalar::of("compressed"));
        log.attributes
            .insert("directory".to_string(), Scalar::of("/var/log/tcspecial"));

        let g = graph(
            vec![node("sensor", "i2c"), log],
            vec![link("sensor", &["housekeeping"])],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadLog { node, why } if node == "housekeeping" && why.contains("compressed")
            )),
            "{problems:?}"
        );
    }

    /// A fixed-length log takes only sources of exactly that length.
    ///
    /// Settled on the ground, which is the point: a log that would have to
    /// refuse a record in flight loses telemetry at the moment telemetry is
    /// worth most.
    #[test]
    fn a_fixed_length_log_takes_only_what_fits() {
        let fixed_log = |format: &str| {
            let mut log = node("housekeeping", "log");
            log.attributes
                .insert("format".to_string(), Scalar::of(format));
            log.attributes
                .insert("directory".to_string(), Scalar::of("/var/log/tcspecial"));
            log
        };
        let sensor = |name: &str, read_size: Option<&str>| {
            let mut sensor = node(name, "i2c");
            if let Some(size) = read_size {
                sensor
                    .attributes
                    .insert("read_size".to_string(), Scalar::of(size));
            }
            sensor
        };

        // Two sensors read in two-byte packets, into a two-byte log: nothing
        // to say.
        let g = graph(
            vec![
                sensor("temp_fore", Some("2")),
                sensor("temp_aft", Some("2")),
                fixed_log("fixed:2"),
            ],
            vec![
                link("temp_fore", &["housekeeping"]),
                link("temp_aft", &["housekeeping"]),
            ],
        );
        assert!(
            !g.check()
                .iter()
                .any(|p| matches!(p, Problem::LogSourceMismatch { .. })),
            "{:?}",
            g.check()
        );

        // One of them read in three, which the log cannot hold: reported on
        // the source, naming both lengths.
        let g = graph(
            vec![
                sensor("temp_fore", Some("2")),
                sensor("temp_bay", Some("3")),
                fixed_log("fixed:2"),
            ],
            vec![
                link("temp_fore", &["housekeeping"]),
                link("temp_bay", &["housekeeping"]),
            ],
        );
        let problems = g.check();
        let mismatches: Vec<&Problem> = problems
            .iter()
            .filter(|p| matches!(p, Problem::LogSourceMismatch { .. }))
            .collect();
        assert_eq!(mismatches.len(), 1, "{problems:?}");
        assert!(matches!(
            mismatches[0],
            Problem::LogSourceMismatch { source, wanted: 2, produces: Some(3), .. }
                if source == "temp_bay"
        ));
        assert!(
            mismatches[0].describe().contains("3 bytes")
                && mismatches[0].describe().contains("exactly 2"),
            "{}",
            mismatches[0].describe()
        );

        // A source whose length nothing states, which is every computation:
        // refused too, and told why rather than being told a number.
        let g = graph(
            vec![
                node("thermostat", "compute"),
                sensor("temp_fore", None),
                fixed_log("fixed:2"),
            ],
            vec![
                link("thermostat", &["housekeeping"]),
                link("temp_fore", &["housekeeping"]),
            ],
        );
        let problems = g.check();
        assert_eq!(
            problems
                .iter()
                .filter(|p| matches!(
                    p,
                    Problem::LogSourceMismatch { produces: None, .. }
                ))
                .count(),
            2,
            "{problems:?}"
        );

        // And the two variable formats take all of them: a record of any
        // length fits a log that writes its length beside it.
        for format in ["variable_simple", "variable_ts_rc"] {
            let g = graph(
                vec![
                    node("thermostat", "compute"),
                    sensor("temp_bay", Some("3")),
                    fixed_log(format),
                ],
                vec![
                    link("thermostat", &["housekeeping"]),
                    link("temp_bay", &["housekeeping"]),
                ],
            );
            assert!(
                !g.check()
                    .iter()
                    .any(|p| matches!(p, Problem::LogSourceMismatch { .. })),
                "{format}: {:?}",
                g.check()
            );
        }
    }

    /// A link end naming a terminal its node does not have is refused.
    ///
    /// The names are the four of them and nothing else -- see
    /// [`crate::graph::terminals`] -- so a link drawn to `moc.telemetry` is a
    /// link to a terminal nothing will bind, which used to be a configuration
    /// that passed here and delivered nowhere.
    #[test]
    fn a_link_end_names_a_terminal_its_node_has() {
        let g = graph(
            vec![node("a", "i2c"), node("c", "moc")],
            vec![link("a", &["c.telemetry"])],
        );
        let problems = g.check();
        let why = problems
            .iter()
            .find_map(|p| match p {
                Problem::BadTerminal { why, .. } => Some(why.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{problems:?}"));
        // Saying what it does have, which is the one thing a reader needs.
        assert!(why.contains("telemetry") && why.contains("payload-in"), "{why}");
        // Reported on the node the link leaves, which is where the link is
        // written: a link is an attribute of that node.
        let said = problems
            .iter()
            .find(|p| matches!(p, Problem::BadTerminal { .. }))
            .unwrap();
        assert_eq!(said.about(), About::Node("a".to_string()));

        // A name at the wrong end: the right word, the wrong half of the
        // link, and worth saying in those words rather than as a name nothing
        // knows.
        let g = graph(
            vec![node("a", "i2c"), node("c", "moc")],
            vec![link("a", &["c.moc-out"])],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadTerminal { why, .. } if why.contains("leaves")
            )),
            "{problems:?}"
        );

        // And a node with two sinks has to be told which. The null node is
        // both ends of both streams, so an unnamed link into it could be
        // either, and guessing one would be guessing which stream it carries.
        let g = graph(
            vec![node("a", "i2c"), node("n", "null"), node("c", "moc")],
            vec![link("a", &["n"]), link("n.payload-out", &["c"])],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadTerminal { why, .. }
                    if why.contains("payload-in") && why.contains("moc-in")
            )),
            "{problems:?}"
        );
    }

    /// A link whose ends carry opposite streams is refused.
    ///
    /// One end of it is drawn backwards, and nothing else in the
    /// configuration would show it: the file reads as a link, both nodes
    /// exist, both terminals exist, and in flight it is a payload that
    /// answers nothing.
    #[test]
    fn a_link_may_not_carry_two_streams() {
        let g = graph(
            vec![node("n", "null"), node("c", "moc")],
            // A data source at one end and a data sink at the other, so the
            // ends themselves are right -- and the stream is vehicleward
            // where it leaves and groundward where it arrives.
            vec![link("n.moc-out", &["c.payload-in"])],
        );
        let problems = g.check();
        let said = problems
            .iter()
            .find(|p| matches!(p, Problem::CrossedStreams { .. }))
            .unwrap_or_else(|| panic!("{problems:?}"));
        assert!(
            said.describe().contains("backwards"),
            "{}",
            said.describe()
        );
    }

    /// The null node passes the rest of itself: all four terminals, each used.
    ///
    /// What a generic vehicle node is: a sink and a source at each end of each
    /// stream, with nothing done to what goes through. A configuration built
    /// of one is the smallest thing that exercises every terminal at once.
    #[test]
    fn a_null_vehicle_node_carries_both_streams_through() {
        let mut sensor = node("a", "i2c");
        // Written to in order to be read, which is what the trigger says.
        sensor
            .attributes
            .insert("trigger".to_string(), Scalar::of("\"go\\n\""));
        let g = graph(
            vec![sensor, node("n", "null"), node("c", "moc")],
            vec![
                link("a.payload-out", &["n.payload-in"]),
                link("n.payload-out", &["c.payload-in"]),
                link("c.moc-out", &["n.moc-in"]),
                link("n.moc-out", &["a.moc-in"]),
            ],
        );
        assert_eq!(g.check(), Vec::new());

        // And it has nothing to be told: an attribute on one is a word
        // nothing reads, most likely left behind by a type that was changed.
        let mut said_something = node("n", "null");
        said_something
            .attributes
            .insert("address".to_string(), Scalar::of("0x48"));
        let g = graph(
            vec![node("a", "i2c"), said_something, node("c", "moc")],
            vec![link("a", &["n.payload-in"]), link("n.payload-out", &["c"])],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::BadNode { node, why } if node == "n" && why.contains("address")
            )),
            "{problems:?}"
        );
    }

    /// Every payload aboard may report to the one ground link.
    ///
    /// A sink takes one source, and the ground's is the exception the whole
    /// architecture rests on: each payload's telemetry arrives at its own
    /// socket, `source_base` plus the payload's id, so the ground can tell
    /// apart what a vehicle node could not. Four sensors into one moc node is
    /// the shipped thermostat.
    #[test]
    fn the_ground_takes_every_payload_at_once() {
        let g = graph(
            vec![
                node("a", "i2c"),
                node("b", "i2c"),
                node("c", "i2c"),
                node("moc", "moc"),
            ],
            vec![
                link("a", &["moc"]),
                link("b", &["moc"]),
                link("c", &["moc"]),
            ],
        );
        assert_eq!(g.check(), Vec::new());

        // And a vehicle node's sink is not: two streams into one link, in
        // whatever order they were sent, with nothing to say which was which.
        let g = graph(
            vec![node("a", "i2c"), node("b", "i2c"), node("n", "null")],
            vec![
                link("a", &["n.payload-in"]),
                link("b", &["n.payload-in"]),
            ],
        );
        let problems = g.check();
        assert!(
            problems.iter().any(|p| matches!(
                p,
                Problem::InputBoundTwice { node, input }
                    if node == "n" && input == "payload-in"
            )),
            "{problems:?}"
        );
    }

    /// A payload or a ground link with no link on it at all is refused.
    ///
    /// Either will do -- a payload that is only read has a source and no sink,
    /// one that is only driven has a sink and no source -- but neither is a
    /// payload the configuration describes and nothing reads or drives.
    #[test]
    fn a_payload_and_the_ground_need_a_link() {
        // Reached by a command binding, so the older `Unconnected` rule is
        // satisfied and this one is what is left to catch it: a ground link
        // that commands a computation and carries no telemetry at all.
        let mut g = graph(
            vec![node("moc", "moc"), node("t", "compute")],
            Vec::new(),
        );
        g.commands.push(CommandBinding {
            from: "moc".to_string(),
            sets: "t.setpoint".to_string(),
        });
        let problems = g.check();
        let said = problems
            .iter()
            .find(|p| matches!(p, Problem::NodeOnNoLink { node, .. } if node == "moc"))
            .unwrap_or_else(|| panic!("{problems:?}"));
        assert_eq!(said.about(), About::Node("moc".to_string()));
        assert!(
            said.describe().contains("data sink"),
            "{}",
            said.describe()
        );

        // A payload that is only driven has a sink and nothing else, and is
        // allowed: a heater is a configuration somebody means.
        let g = graph(
            vec![node("t", "compute"), node("heaters", "gpio"), node("moc", "moc")],
            vec![link("t", &["heaters"]), link("t", &["moc"])],
        );
        assert!(
            !g.check()
                .iter()
                .any(|p| matches!(p, Problem::NodeOnNoLink { .. })),
            "{:?}",
            g.check()
        );
    }

    /// Every problem can be shown to a person.
    #[test]
    fn every_problem_describes_itself() {
        let problems = [
            Problem::DuplicateNode { name: "a".into() },
            Problem::DuplicateGroup { name: "g".into() },
            Problem::EmptyGroup { group: "g".into() },
            Problem::NamelessNode,
            Problem::NoSuchNode {
                from: "a".into(),
                link: "\"a\" to \"b\"".into(),
                missing: "b".into(),
            },
            Problem::SelfLink { node: "a".into() },
            Problem::NoSuchGroup {
                node: "a".into(),
                group: "g".into(),
            },
            Problem::BadInterval {
                group: "g".into(),
                why: "no unit".into(),
            },
            Problem::IntervalInAGroup {
                node: "a".into(),
                group: "g".into(),
            },
            Problem::InputBoundTwice {
                node: "m".into(),
                input: "in0".into(),
            },
            Problem::BadTerminal {
                from: "a".into(),
                link: "\"a\" to \"c.telemetry\"".into(),
                why: "a moc node has no data sink called \"telemetry\"".into(),
            },
            Problem::CrossedStreams {
                from: "n".into(),
                link: "\"n\" to \"c\"".into(),
                leaves: "payload-out".into(),
                arrives: "moc-out".into(),
            },
            Problem::DuplicateLink {
                from: "a".into(),
                to: "c".into(),
            },
            Problem::NodeOnNoLink {
                node: "a".into(),
                node_type: "i2c".into(),
            },
            Problem::NoSuchTarget { sets: "x.y".into() },
            Problem::NoAttributeNamed { sets: "x".into() },
            Problem::NotAMocLink { from: "a".into() },
            Problem::Unconnected { node: "a".into() },
            Problem::BadCompute {
                node: "c".into(),
                why: "\"c\" (mean): inputs is not set".into(),
            },
            Problem::MeanInputs {
                node: "c".into(),
                declared: vec!["in0".into(), "in1".into(), "in2".into()],
                bound: vec!["in0".into()],
            },
            Problem::MeanInputs {
                node: "c".into(),
                declared: vec!["in0".into()],
                bound: vec![],
            },
            Problem::BadLog {
                node: "l".into(),
                why: "format: \"compressed\" is not a format a log knows".into(),
            },
            Problem::LogSourceMismatch {
                log: "l".into(),
                source: "s".into(),
                wanted: 2,
                produces: Some(3),
            },
            Problem::LogSourceMismatch {
                log: "l".into(),
                source: "s".into(),
                wanted: 2,
                produces: None,
            },
        ];
        for problem in problems {
            assert!(!problem.describe().is_empty(), "{problem:?} says nothing");
        }
    }
}
