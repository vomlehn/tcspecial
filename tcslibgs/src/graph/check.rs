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
            Problem::BadLog { node, .. } => About::Node(node.clone()),
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
            Problem::BadLog { node, why } => format!("log \"{node}\": {why}"),
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

        problems
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

    /// A graph built here rather than read, so a test can make one wrong.
    fn graph(nodes: Vec<Node>, links: Vec<Link>) -> Graph {
        Graph {
            description: None,
            groups: Vec::new(),
            nodes,
            links,
            commands: Vec::new(),
            files: Vec::new(),
            homes: BTreeMap::new(),
        }
    }

    fn node(name: &str, node_type: &str) -> Node {
        Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes: BTreeMap::new(),
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
