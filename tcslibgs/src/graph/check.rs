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

use super::{Destination, Graph};
use crate::verify::About;

/// Something wrong with a configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Two nodes of one name. Links name nodes, so this makes a link
    /// ambiguous as well as a node.
    DuplicateNode { name: String },
    /// Two groups of one name.
    DuplicateGroup { name: String },
    /// A node with no name, which nothing can link to.
    NamelessNode,
    /// A link naming a node that is not in the configuration.
    NoSuchNode { link: String, missing: String },
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
            Problem::BadInterval { group, .. } => About::NodeGroup(group.clone()),
            // A link's problem is reported on the node it leaves, which is
            // where a reader looks: the links section says `from: a`, and `a`
            // is what they will search for.
            Problem::NoSuchNode { .. } => About::Section("links"),
            Problem::SelfLink { node } => About::Node(node.clone()),
            Problem::NoSuchGroup { node, .. } => About::Node(node.clone()),
            Problem::IntervalInAGroup { node, .. } => About::Node(node.clone()),
            Problem::InputBoundTwice { node, .. } => About::Node(node.clone()),
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

    /// What to print, one problem per line.
    pub fn describe(&self) -> String {
        match self {
            Problem::DuplicateNode { name } => {
                format!("two nodes are called \"{name}\"")
            }
            Problem::DuplicateGroup { name } => {
                format!("two groups are called \"{name}\"")
            }
            Problem::NamelessNode => "a node has no name".to_string(),
            Problem::NoSuchNode { link, missing } => {
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
            // crate::node::compute::params_of.
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

        // Links: both ends must be nodes, and no input twice.
        let mut bound: BTreeMap<(String, String), usize> = BTreeMap::new();
        for link in &self.links {
            let described = |to: &str| format!("\"{}\" to \"{to}\"", link.from);

            if self.node(&link.from).is_none() {
                problems.push(Problem::NoSuchNode {
                    link: described(&link.to.each().join(", ")),
                    missing: link.from.clone(),
                });
            }

            for destination in link.to.each() {
                let Destination { node, input } = Destination::parse(destination);
                if node == link.from {
                    problems.push(Problem::SelfLink { node });
                    continue;
                }
                if self.node(&node).is_none() {
                    problems.push(Problem::NoSuchNode {
                        link: described(destination),
                        missing: node,
                    });
                    continue;
                }
                if let Some(input) = input {
                    *bound.entry((node, input)).or_insert(0) += 1;
                }
            }
        }
        for ((node, input), count) in bound {
            if count > 1 {
                problems.push(Problem::InputBoundTwice { node, input });
            }
        }

        // A node nobody reaches and nobody reads.
        let mut touched: BTreeSet<&str> = BTreeSet::new();
        for link in &self.links {
            touched.insert(link.from.as_str());
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

        for node in &self.nodes {
            if !node.name.trim().is_empty() && !touched.contains(node.name.as_str()) {
                problems.push(Problem::Unconnected {
                    node: node.name.clone(),
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
            let what = match crate::node::compute::params_of(&node.name, &node.attributes) {
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
            if let crate::node::compute::Computation::Mean(mean) = &what {
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

        // The logs, and whether what links into each of them fits the shape it
        // writes. Settled here and not when a record is written: a log that
        // would have to refuse a record in flight loses telemetry at the
        // moment telemetry is worth most.
        for node in self.nodes.iter().filter(|n| n.node_type == "log") {
            let params = match crate::node::log::params_of(&node.name, &node.attributes) {
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
                // parameters.rs; what the values have to be -- a multicast
                // beacon address, a protocol that exists, two ports that are
                // not one socket -- is in the struct the command interpreter
                // itself reads.
                match crate::graph::parameters::ci_config_of(said) {
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
        // Only the nodes that are opened. A computation and a log have their
        // own rules below, and a node whose type nothing recognises is
        // refused by name already -- a second complaint about its attributes
        // would bury the one that says what is actually wrong.
        for node in self
            .nodes
            .iter()
            .filter(|node| node.is_payload() || node.node_type == "moc")
        {
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
        for node in self.nodes.iter().filter(|n| n.is_payload()) {
            if node.attributes.contains_key("trigger") {
                continue;
            }
            let read = self.links.iter().any(|link| link.from == node.name);
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
    fn inputs_bound_at(&self, node: &str) -> Vec<String> {
        let mut inputs = Vec::new();
        for link in &self.links {
            for destination in link.to.each() {
                let Destination { node: to, input } = Destination::parse(destination);
                if to == node {
                    if let Some(input) = input {
                        inputs.push(input);
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
    fn sources_of(&self, node: &str) -> Vec<&str> {
        let mut sources: Vec<&str> = Vec::new();
        for link in &self.links {
            let arrives = link.to.each().iter().any(|destination| {
                destination.split('.').next().unwrap_or(destination) == node
            });
            if arrives && !sources.contains(&link.from.as_str()) {
                sources.push(link.from.as_str());
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
    fn tcspecial() -> BTreeMap<String, Scalar> {
        [
            ("tcs_cmd_recv", "0.0.0.0:4000"),
            ("moc_cmd_send", "127.0.0.1:4100"),
            ("tcs_tc_base", "0.0.0.0:6000"),
            ("moc_tc_base", "127.0.0.1:6100"),
            ("payload_interface", "0.0.0.0:4001"),
            ("beacon_interval_ms", "5000"),
            ("beacon_send", "239.255.0.1:5550"),
            ("beacon_interface", "127.0.0.1"),
            ("log_dir", "/var/log/tcspecial"),
            ("log_segment_bytes", "65536"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_string(), Scalar::of(value)))
        .collect()
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
            "moc" => &[("address", "10.0.0.1"), ("port", "5000")],
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
            editor: None,
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
            editor: None,
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
            // A name belonging to nothing.
            ("beacon_adress", "239.255.0.1:5550", "beacon_adress"),
            // A command interface that is only half stated, which is the way
            // one attribute carrying two things goes wrong.
            ("tcs_cmd_recv", "0.0.0.0", "address:port"),
            // A port that is not one.
            ("payload_port", "nought", "payload_port"),
            // And a beacon address that is not a multicast group, which is
            // the rule the command interpreter's own struct knows.
            ("beacon_send", "10.0.0.1:5550", "multicast"),
        ];

        for (attribute, value, expected) in wrong {
            let mut g = graph(vec![node("a", "i2c"), node("c", "moc")], vec![link("a", &["c"])]);
            g.tcspecial
                .as_mut()
                .unwrap()
                .insert(attribute.to_string(), Scalar::of(value));

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

    /// The rule is about payloads and nothing else.
    ///
    /// A computation, a log and the ground link all take inputs and none of
    /// them is a thing the vehicle carries: a trigger would mean nothing on
    /// any of them, and asking for one would make every configuration
    /// unwritable.
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

    /// A node nothing links to or from is named.
    ///
    /// More likely a mistake than an intention, and the kind a reader of the
    /// file would not notice: a sensor placed and never connected produces
    /// data nobody reads.
    #[test]
    fn a_node_with_no_links_is_named() {
        let g = graph(vec![node("lonely", "i2c")], Vec::new());
        let problems = g.check();
        assert!(
            problems
                .iter()
                .any(|p| matches!(p, Problem::Unconnected { node } if node == "lonely")),
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
        let problems = computation(&[("function", "process"), ("program", "/bin/cat")]);
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

    /// Every problem can be shown to a person.
    #[test]
    fn every_problem_describes_itself() {
        let problems = [
            Problem::DuplicateNode { name: "a".into() },
            Problem::DuplicateGroup { name: "g".into() },
            Problem::NamelessNode,
            Problem::NoSuchNode {
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
