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
//! And the links to the ground, which are where what goes to the ground is
//! actually delivered. A line drawn into a `moc` node is delivered to that
//! source's own link to the `moc` node's `ground_base`, at the port plus the
//! source's device id; and a payload nothing touches is delivered to its link
//! over the `tcspecial` section's `payload_telemetry`. They are in the same
//! table because they are the same question -- where does this reading go --
//! and each is named for what it carries: see `tcslibgs::graph::telemetry`.
//!
//! Nothing here opens anything or moves any bytes. It is the arithmetic of
//! the graph, which is worth having on its own: it can be tested against a
//! configuration without a vehicle, and a fault in it would otherwise look
//! like a fault in a device.

use std::collections::BTreeMap;

use tcslibgs::graph::telemetry::{self, Telemetered, Via};
use tcslibgs::graph::terminals::{self, Stream};
use tcslibgs::graph::{Destination, Graph, Link};

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
    /// Which of the two streams this link carries.
    ///
    /// A link carries one: both of its ends are terminals of the same stream,
    /// which is what [`tcslibgs::graph::check::Problem::CrossedStreams`]
    /// refuses a configuration for. So it is a property of the link and is
    /// settled here, once, from the terminals the file names or from the only
    /// terminal the type at either end has -- see [`stream_of`].
    ///
    /// `None` is a link whose stream nothing settles, which a checked
    /// configuration does not have: it is carried as "wherever its links go",
    /// which is what every delivery was before the streams were kept apart.
    pub stream: Option<Stream>,
}

/// Which stream a link carries, where the configuration settles it.
///
/// Three places to look, in the order a reader of the file would:
///
/// * the terminal the link leaves by, which is where a file says it --
///   `n.payload-out` is groundward and says so;
/// * failing that, the terminal it arrives at, for a link written from a node
///   with one source into a node with several sinks;
/// * and failing both, the only terminal the type at either end has. A payload
///   has one source and one sink, so a link from one is groundward whether or
///   not the word was written -- which is why most links in most files need
///   none of this.
///
/// `None` where nothing settles it, which is a link whose two ends both have a
/// choice and whose file named neither: `Graph::check` refuses that as a
/// `BadTerminal`, so nothing downstream depends on it -- a `None` is carried
/// as "wherever its links go", which is what every delivery was before.
fn stream_of(graph: &Graph, link: &Link) -> Option<Stream> {
    let named = |terminal: Option<&str>| {
        terminal
            .and_then(terminals::stream_of)
            .map(|(_, stream)| stream)
    };
    // The only terminal of one end of one node, where it has one.
    let only = |node: &str, sink: bool| {
        let node_type = &graph.node(node)?.node_type;
        let terminals = terminals::terminals_of(node_type)?;
        match if sink { terminals.sinks } else { terminals.sources } {
            [one] => named(Some(one)),
            _ => None,
        }
    };

    let origin = link.origin();
    let first = link
        .to
        .each()
        .first()
        .map(|destination| Destination::parse(destination));

    named(origin.input.as_deref())
        .or_else(|| first.as_ref().and_then(|to| named(to.input.as_deref())))
        .or_else(|| only(&origin.node, false))
        .or_else(|| first.as_ref().and_then(|to| only(&to.node, true)))
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
    /// Every link to the ground, each delivered to under its own name.
    ///
    /// The names are not names a configuration can use, so they cannot
    /// collide with a node: a node's name is a word, and these have spaces in
    /// them. They are opened from what is here.
    ground_links: Vec<Telemetered>,
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
        // The links to the ground first, because a line into a moc node is
        // delivered to one of them. A configuration whose links to the ground
        // do not work out gives none: `Graph::check` refuses it, saying why,
        // and this is the table for a checked graph.
        let mut routes = Routes {
            ground_links: telemetry::of(graph).unwrap_or_default(),
            ..Routes::default()
        };
        let to_the_ground: BTreeMap<(String, String), String> = routes
            .ground_links
            .iter()
            .filter_map(|link| match &link.via {
                Via::Moc(moc) => Some(((link.source.clone(), moc.clone()), link.name.clone())),
                Via::PayloadTelemetry => None,
            })
            .collect();

        for link in &graph.links {
            // The node it leaves, whatever terminal it leaves by. The routing
            // question is which node, so `n.payload-out` and `n` are one
            // source here -- and which way the data is going is the link's own
            // and is settled once, below.
            let from = link.origin().node;
            let stream = stream_of(graph, link);

            for destination in link.to.each() {
                let Destination { node, input } = Destination::parse(destination);
                // And the same at the other end, with one difference: a
                // computation's named input is which *of several* ways in, and
                // the table is built on it. A terminal is not one of those, so
                // it is dropped here rather than carried to a port that would
                // refuse it as an input a device does not have.
                let input = input.filter(|named| {
                    tcslibgs::graph::terminals::stream_of(named).is_none()
                });

                // A line into a moc node goes to this source's own link to
                // that node's ground_base.
                let to = to_the_ground
                    .get(&(from.clone(), node.clone()))
                    .cloned()
                    .unwrap_or_else(|| node.clone());

                routes
                    .deliveries
                    .entry(from.clone())
                    .or_default()
                    .push(Delivery {
                        to,
                        input: input.clone(),
                        stream,
                    });

                if let Some(input) = input {
                    routes.sources.insert((node, input), from.clone());
                }
            }
        }

        // A payload nothing touches, to its link over payload_telemetry.
        for link in routes
            .ground_links
            .iter()
            .filter(|link| link.via == Via::PayloadTelemetry)
        {
            routes
                .deliveries
                .entry(link.source.clone())
                .or_default()
                .push(Delivery {
                    to: link.name.clone(),
                    input: None,
                    // What a payload reports, and only that: an app payload's
                    // other stream is what the ground sent it.
                    stream: Some(Stream::Groundward),
                });
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

    /// Each payload's link to the ground, for the payloads that send one.
    pub fn ground_links(&self) -> &[Telemetered] {
        &self.ground_links
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

    /// Whether the configuration lets the ground set this attribute.
    ///
    /// The question a command asks on arrival. An attribute no binding
    /// mentions cannot be set from the ground: what is changeable in flight is
    /// written down, which is what makes it checkable before it flies.
    pub fn is_commanded(&self, node: &str, attribute: &str) -> bool {
        self.commands.values().flatten().any(|commanded| {
            commanded.node == node && commanded.attribute == attribute
        })
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

        let from_k_fore = routes.from("k_1");
        assert_eq!(
            from_k_fore.len(),
            3,
            "the T should deliver three ways: {from_k_fore:?}"
        );

        // The one to the ground goes to k_1's own link to the moc node's
        // ground_base: see `a_line_into_a_moc_node_goes_to_its_own_link`.
        let places: Vec<&str> = from_k_fore.iter().map(|d| d.to.as_str()).collect();
        assert!(places.contains(&"k_1 to moc"), "{places:?}");
        assert!(places.contains(&"cabin_temp"), "{places:?}");
        assert!(places.contains(&"housekeeping"), "{places:?}");

        // And the one into the mean knows which input it fills, where the
        // other two do not name one.
        let into_mean = from_k_fore
            .iter()
            .find(|d| d.to == "cabin_temp")
            .expect("a delivery into the mean");
        assert_eq!(into_mean.input.as_deref(), Some("in0"));
        let payload_out = from_k_fore.iter().find(|d| d.to == "k_1 to moc").unwrap();
        assert_eq!(payload_out.input, None);
    }

    /// A link that names its terminals routes as one that does not.
    ///
    /// A terminal says which way the data is going and not which of several
    /// ways in -- see `tcslibgs::graph::terminals` -- so `a.payload-out` to
    /// `c.payload-in` is the same delivery as `a` to `c`. It matters here
    /// and not only in the checking: a terminal carried through as a named
    /// input would reach a device's port, which refuses an input a device does
    /// not have, and a configuration that passed the ground check would stop
    /// delivering the moment it flew.
    #[test]
    fn a_terminal_on_a_link_is_not_a_named_input() {
        use tcslibgs::graph::Destinations;

        // The nodes are named and told nothing: a node's own attributes are
        // no part of the routing. Their *types* are, and only for one
        // question -- which stream a link carries, where the file left the
        // terminals out and the type is what settles it. See `stream_of`.
        let node = |name: &str, node_type: &str| tcslibgs::graph::Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes: std::collections::BTreeMap::new(),
        };
        let one_link = |from: &str, to: &str| Graph {
            description: None,
            tcspecial: None,
            groups: Vec::new(),
            nodes: vec![node("a", "i2c"), node("c", "moc"), node("m", "compute")],
            links: vec![tcslibgs::graph::Link {
                from: from.to_string(),
                to: Destinations::of(vec![to.to_string()]),
                comment: None,
            }],
            commands: Vec::new(),
            files: Vec::new(),
            homes: std::collections::BTreeMap::new(),
            placed: std::collections::BTreeMap::new(),
            set_aside: Default::default(),
        };

        let plain = Routes::of(&one_link("a", "c"));
        let named = Routes::of(&one_link("a.payload-out", "c.payload-in"));

        for routes in [&plain, &named] {
            let deliveries = routes.from("a");
            assert_eq!(deliveries.len(), 1, "{deliveries:?}");
            assert_eq!(deliveries[0].to, "c");
            // Not an input: a device and a ground link have one way in and do
            // not name it.
            assert_eq!(deliveries[0].input, None);
            assert!(routes.inputs_of("c").is_empty());
        }
        assert_eq!(plain.from("a"), named.from("a"));

        // Including the stream, which both settle the same way: the file says
        // `payload-out` in the one and an i2c node has no other source in the
        // other, so the link is groundward either way it was written.
        assert_eq!(plain.from("a")[0].stream, Some(Stream::Groundward));

        // And a computation's named input is still carried, which is the
        // distinction: `in0` is which of several ways in, and the mean is
        // built on knowing.
        let into_mean = Routes::of(&one_link("a.payload-out", "m.in0"));
        assert_eq!(into_mean.from("a")[0].input.as_deref(), Some("in0"));
        assert_eq!(into_mean.source_of("m", "in0"), Some("a"));
    }

    /// A node that computes over several inputs knows which is which.
    #[test]
    fn a_fan_in_is_filled_by_name_and_not_by_order() {
        let routes = Routes::of(&thermostat());

        assert_eq!(routes.inputs_of("cabin_temp"), vec!["in0", "in1", "in2"]);
        assert_eq!(routes.source_of("cabin_temp", "in0"), Some("k_1"));
        assert_eq!(routes.source_of("cabin_temp", "in1"), Some("k_2"));
        assert_eq!(routes.source_of("cabin_temp", "in2"), Some("k_3"));

        // An input nothing fills has no source, which is what the checking
        // refuses a configuration for -- here it is simply absent.
        assert_eq!(routes.source_of("cabin_temp", "in3"), None);
    }

    /// A node whose output goes one place has one delivery.
    #[test]
    fn an_ordinary_link_delivers_once() {
        let routes = Routes::of(&thermostat());

        let from_sensor = routes.from("temp_1");
        assert_eq!(from_sensor.len(), 1, "{from_sensor:?}");
        assert_eq!(from_sensor[0].to, "k_1");
        assert_eq!(from_sensor[0].input, None);
        assert_eq!(routes.fan_out("temp_1"), 1);
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
        assert_eq!(into_log, vec!["k_1", "k_2", "k_3", "thermostat"]);

        // A sensor is the start of a path and nothing arrives at it.
        assert!(routes.arriving_at("temp_1").is_empty());
        // And a name the configuration does not have.
        assert!(routes.arriving_at("nobody").is_empty());

        // Three links into three named inputs are three sources.
        let into_vote = routes.arriving_at("cabin_temp");
        assert_eq!(into_vote, vec!["k_1", "k_2", "k_3"], "{into_vote:?}");
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

    /// Every link of the configuration is in the table, and nothing else is
    /// -- bar each payload's link to the ground, which the `tcspecial` section
    /// describes rather than a line.
    ///
    /// The table is what the running software believes about the graph, so a
    /// link missing from it is a link that silently does not happen.
    #[test]
    fn the_table_holds_every_link_and_invents_none() {
        let graph = thermostat();
        let routes = Routes::of(&graph);

        let drawn: usize = graph.links.iter().map(|l| l.to.each().len()).sum();
        let untouched = routes
            .ground_links()
            .iter()
            .filter(|l| l.via == Via::PayloadTelemetry)
            .count();
        let expected = drawn + untouched;
        let held: usize = graph
            .nodes
            .iter()
            .map(|n| routes.from(&n.name).len())
            .sum();
        assert_eq!(held, expected, "the table and the file disagree");
    }

    /// A payload nothing in the drawing touches reports to the ground over
    /// its own link, and one the drawing does touch goes only where it is
    /// drawn: see `tcslibgs::graph::telemetry`.
    #[test]
    fn only_an_undrawn_payload_reports_over_its_own_link() {
        let mut graph = thermostat();
        graph.links.retain(|link| link.origin().node != "temp_2");
        let routes = Routes::of(&graph);

        let to: Vec<&str> = routes.from("temp_2").iter().map(|d| d.to.as_str()).collect();
        assert_eq!(to, vec![telemetry::payload_link_name("temp_2").as_str()]);
        for drawn in ["temp_1", "temp_3", "heaters"] {
            assert!(
                !routes
                    .from(drawn)
                    .iter()
                    .any(|d| d.to == telemetry::payload_link_name(drawn)),
                "{drawn} is drawn and should go only where it is drawn"
            );
        }
        let linked: Vec<&str> = routes
            .ground_links()
            .iter()
            .filter(|l| l.via == Via::PayloadTelemetry)
            .map(|l| l.source.as_str())
            .collect();
        assert_eq!(linked, vec!["temp_2"]);
    }

    /// A line into a moc node goes to the source's own link to the ground,
    /// at the moc node's ground_base plus the source's device id -- and
    /// nothing is delivered to the moc node itself, which only receives.
    #[test]
    fn a_line_into_a_moc_node_goes_to_its_own_link() {
        let routes = Routes::of(&thermostat());
        assert!(routes.arriving_at("moc").is_empty());

        let mean: Vec<&str> = routes.from("cabin_temp").iter().map(|d| d.to.as_str()).collect();
        assert!(mean.contains(&"cabin_temp to moc"), "{mean:?}");

        let link = routes
            .ground_links()
            .iter()
            .find(|l| l.name == "cabin_temp to moc")
            .expect("cabin_temp's link to the ground");
        assert_eq!(link.via, Via::Moc("moc".to_string()));
        // The thermostat's mean is its fifth node, id 4, and its moc node's
        // ground_base is 10.0.0.2:5100.
        assert_eq!(
            (link.ground.address.as_str(), link.ground.port),
            ("10.0.0.2", 5104)
        );
    }
}
