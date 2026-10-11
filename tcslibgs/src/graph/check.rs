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

        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{CommandBinding, Destinations, Group, Link, Node};
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
        ];
        for problem in problems {
            assert!(!problem.describe().is_empty(), "{problem:?} says nothing");
        }
    }
}
