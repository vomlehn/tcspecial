//! Where each node's output goes, worked out once.
//!
//! A link is written from a node to one or more destinations, which is the
//! shape a person wants to write. It is not the shape the running software
//! wants: when a node produces bytes, the question is "where do these go",
//! and answering it by walking every link each time would be a scan per
//! datum.
//!
//! So the links are turned into a table, once, at startup: node name to the
//! list of places its output is delivered. And the other way as well -- which
//! link fills each named input of a node -- because a node that computes over
//! several inputs has to know which arrival is which, and the configuration
//! says so by name rather than by order.
//!
//! Nothing here opens anything or moves any bytes. It is the arithmetic of
//! the graph, which is worth having on its own: it can be tested against a
//! configuration without a vehicle, and a fault in it would otherwise look
//! like a fault in a device.

use std::collections::BTreeMap;

use tcslibgs::graph::{Destination, Graph};

/// What a command from the ground sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commanded {
    /// The node whose attribute it sets.
    pub node: String,
    /// Which attribute: a setpoint, a rate, a limit.
    pub attribute: String,
}

/// One place a node's output is delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The node it goes to.
    pub to: String,
    /// Which of that node's inputs, where it names them.
    ///
    /// `None` is a node that takes one input and does not name it, which is
    /// most of them: a device, a MOC link, a log.
    pub input: Option<String>,
}

/// The table: what goes where.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Routes {
    /// Where each node's output goes, by node name. A node whose output goes
    /// nowhere is absent rather than present with an empty list.
    deliveries: BTreeMap<String, Vec<Delivery>>,
    /// Which node fills each named input, by (node, input).
    ///
    /// The reverse of the above, and not merely for convenience: a node that
    /// computes a mean has to be told that its three inputs are bound before
    /// it starts, and has to know which arrival is `in0`.
    sources: BTreeMap<(String, String), String>,
    /// What a command arriving on each MOC link sets, by the link's name.
    ///
    /// Kept here with the links because it is the same question in the same
    /// table: where does what arrives at this node go. A command is not a
    /// datum on a link -- it sets an attribute rather than being passed on --
    /// but a MOC link that commands arrive on is a node something is read
    /// from, and nothing else in the configuration says so. A thermostat
    /// whose setpoint comes from the ground has a `commands` entry and no
    /// link out of the ground at all.
    commands: BTreeMap<String, Vec<Commanded>>,
}

impl Routes {
    /// Work out the table for this configuration.
    ///
    /// The graph is assumed checked: `Graph::check` has already said that
    /// every name is a node and no input is bound twice, so nothing here
    /// reports anything. Building a table from an unchecked graph would give
    /// a table with holes in it, which is why the order matters and is
    /// written down here.
    pub fn of(graph: &Graph) -> Routes {
        let mut routes = Routes::default();

        for link in &graph.links {
            for destination in link.to.each() {
                let Destination { node, input } = Destination::parse(destination);

                routes
                    .deliveries
                    .entry(link.from.clone())
                    .or_default()
                    .push(Delivery {
                        to: node.clone(),
                        input: input.clone(),
                    });

                if let Some(input) = input {
                    routes.sources.insert((node, input), link.from.clone());
                }
            }
        }

        for binding in &graph.commands {
            let Destination { node, input } = binding.target();
            // An attribute is what a command sets, and a binding that named
            // none is what the checking refuses the configuration for. Left
            // out rather than guessed at here.
            if let Some(attribute) = input {
                routes
                    .commands
                    .entry(binding.from.clone())
                    .or_default()
                    .push(Commanded { node, attribute });
            }
        }

        routes
    }

    /// Where this node's output goes.
    ///
    /// An empty slice for a node whose output goes nowhere, which a checked
    /// configuration does not have -- but a slice is what a caller wants to
    /// iterate either way, and an `Option` here would put a `match` at every
    /// delivery.
    pub fn from(&self, node: &str) -> &[Delivery] {
        self.deliveries.get(node).map_or(&[], Vec::as_slice)
    }

    /// Which node fills this input.
    pub fn source_of(&self, node: &str, input: &str) -> Option<&str> {
        self.sources
            .get(&(node.to_string(), input.to_string()))
            .map(String::as_str)
    }

    /// Every named input of this node that something fills, in order.
    ///
    /// In order because a computation over several inputs is written with
    /// them named in0, in1, in2, and a reader of the telemetry expects the
    /// third number to be the third sensor. Sorted by name rather than by the
    /// order the links happen to be written in, which is the file's business
    /// and not the computation's.
    pub fn inputs_of(&self, node: &str) -> Vec<&str> {
        self.sources
            .keys()
            .filter(|(to, _)| to == node)
            .map(|(_, input)| input.as_str())
            .collect()
    }

    /// How many places take this node's output.
    pub fn fan_out(&self, node: &str) -> usize {
        self.from(node).len()
    }

    /// What a command arriving on this node sets.
    ///
    /// Empty for every node but a MOC link, and for a MOC link that nothing
    /// is commanded through.
    pub fn commands_on(&self, node: &str) -> &[Commanded] {
        self.commands.get(node).map_or(&[], Vec::as_slice)
    }

    /// Every node whose output arrives here, in order and once each.
    ///
    /// The question a node asks about itself: is anything delivered to me.
    /// A device that nothing delivers to is one that is only read, and that
    /// decides which half of its endpoint the running software uses -- see
    /// [`crate::runtime::port::Use`]. Walked rather than tabulated because it
    /// is asked once per node at startup, where [`Routes::from`] is asked once
    /// per datum.
    pub fn arriving_at(&self, node: &str) -> Vec<&str> {
        // Each source once, which the table gives for nothing: it is keyed by
        // the node the output leaves, so a node delivering here twice -- two
        // legs of one fan-out into two inputs of this node -- is one key and
        // one answer.
        self.deliveries
            .iter()
            .filter(|(_, deliveries)| deliveries.iter().any(|d| d.to == node))
            .map(|(from, _)| from.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The shipped thermostat, as the routing sees it.
    fn thermostat() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads")
    }

    /// A link with several destinations delivers to all of them.
    ///
    /// The "T" of the example: a converted reading goes to the ground, into
    /// the mean, and into the log. A table that kept one of the three would
    /// lose telemetry silently, which is the failure this is here to prevent.
    #[test]
    fn one_output_reaches_every_destination_of_its_link() {
        let routes = Routes::of(&thermostat());

        let from_k_fore = routes.from("k_fore");
        assert_eq!(
            from_k_fore.len(),
            3,
            "the T should deliver three ways: {from_k_fore:?}"
        );

        let places: Vec<&str> = from_k_fore.iter().map(|d| d.to.as_str()).collect();
        assert!(places.contains(&"moc"), "{places:?}");
        assert!(places.contains(&"mean_temp"), "{places:?}");
        assert!(places.contains(&"housekeeping"), "{places:?}");

        // And the one into the mean knows which input it fills, where the
        // other two do not name one.
        let into_mean = from_k_fore
            .iter()
            .find(|d| d.to == "mean_temp")
            .expect("a delivery into the mean");
        assert_eq!(into_mean.input.as_deref(), Some("in0"));
        let to_moc = from_k_fore.iter().find(|d| d.to == "moc").unwrap();
        assert_eq!(to_moc.input, None);
    }

    /// A node that computes over several inputs knows which is which.
    #[test]
    fn a_fan_in_is_filled_by_name_and_not_by_order() {
        let routes = Routes::of(&thermostat());

        assert_eq!(routes.inputs_of("mean_temp"), vec!["in0", "in1", "in2"]);
        assert_eq!(routes.source_of("mean_temp", "in0"), Some("k_fore"));
        assert_eq!(routes.source_of("mean_temp", "in1"), Some("k_aft"));
        assert_eq!(routes.source_of("mean_temp", "in2"), Some("k_bay"));

        // An input nothing fills has no source, which is what the checking
        // refuses a configuration for -- here it is simply absent.
        assert_eq!(routes.source_of("mean_temp", "in3"), None);
    }

    /// A node whose output goes one place has one delivery.
    #[test]
    fn an_ordinary_link_delivers_once() {
        let routes = Routes::of(&thermostat());

        let from_sensor = routes.from("temp_fore");
        assert_eq!(from_sensor.len(), 1);
        assert_eq!(from_sensor[0].to, "k_fore");
        assert_eq!(from_sensor[0].input, None);
        assert_eq!(routes.fan_out("temp_fore"), 1);
    }

    /// The table answers the other direction too: what arrives here.
    ///
    /// Which is what decides whether a node's endpoint is opened to be read,
    /// to be driven, or both -- see [`crate::runtime::port::Use`].
    #[test]
    fn what_arrives_at_a_node_is_asked_the_other_way_round() {
        let routes = Routes::of(&thermostat());

        // The three converted readings and the thermostat's decision all go
        // into the log, each named once.
        let mut into_log = routes.arriving_at("housekeeping");
        into_log.sort();
        assert_eq!(into_log, vec!["k_aft", "k_bay", "k_fore", "thermostat"]);

        // A sensor is the start of a path and nothing arrives at it.
        assert!(routes.arriving_at("temp_fore").is_empty());
        // And a name the configuration does not have.
        assert!(routes.arriving_at("nobody").is_empty());

        // Three links into three named inputs are three sources.
        let into_mean = routes.arriving_at("mean_temp");
        assert_eq!(into_mean, vec!["k_aft", "k_bay", "k_fore"], "{into_mean:?}");
    }

    /// A node nothing leaves has no deliveries, and asking is cheap.
    #[test]
    fn a_node_with_no_output_has_an_empty_table() {
        let routes = Routes::of(&thermostat());

        // The heaters are driven and produce nothing.
        assert!(routes.from("heaters").is_empty());
        assert_eq!(routes.fan_out("heaters"), 0);
        // And a name that is not in the configuration at all.
        assert!(routes.from("nobody").is_empty());
    }

    /// Every link of the configuration is in the table, and nothing else is.
    ///
    /// The table is what the running software believes about the graph, so a
    /// link missing from it is a link that silently does not happen.
    #[test]
    fn the_table_holds_every_link_and_invents_none() {
        let graph = thermostat();
        let routes = Routes::of(&graph);

        let expected: usize = graph.links.iter().map(|l| l.to.each().len()).sum();
        let held: usize = graph
            .nodes
            .iter()
            .map(|n| routes.from(&n.name).len())
            .sum();
        assert_eq!(held, expected, "the table and the file disagree");
    }
}
