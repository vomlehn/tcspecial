//! Where a line may arrive at a node, and the lines that do.
//!
//! A line between two nodes is written on the node it leaves, under the data
//! source it leaves by, as the node it reaches and a name: `world-out:
//! [zone_temp.in0]`. The node it reaches lists that name under the sink it
//! arrives at -- `world-in: [in0, in1, in2]` -- which is the node saying where
//! it may be joined. The two are matched here, as the configuration is read:
//! a line to a name its far end does not list is refused, saying which names
//! it does, because a link to a guessed name is a link to whatever the guess
//! happened to hit.
//!
//! A name may be listed with nothing arriving at it: a list is where a node
//! may be joined, not a promise that it is. What has to be joined is the
//! rules' business -- a vote over an input nothing sends is refused as that,
//! in [`crate::graph::check`].
//!
//! Matched, each line is turned into what it means, which is what every rule
//! and the flight software read: the sink it arrives at, and for a vote the
//! input. The names themselves are kept apart, in [`Named`], for the editor,
//! which writes them back; nothing aboard reads them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{TcsError, TcsResult};

use super::terminals::{answers, terminal_at, terminals_of};
use super::{Destination, Destinations, Graph, Named, Node, NodeFile, Sinks};

/// What the files said about where their nodes may be joined.
///
/// Gathered as the files are read and used once all of them have been: a
/// line may cross from an included file into the top one, so a name and the
/// line to it are not necessarily in the same file. See [`Mirror::resolve`].
#[derive(Debug, Default)]
pub struct Mirror {
    /// By node: the file its entry is in, and the names it listed.
    said: BTreeMap<String, (PathBuf, Sinks)>,
}

impl Mirror {
    /// Takes in the names one node's entry listed.
    ///
    /// # Arguments
    ///
    /// * `from` - The file the entry is in.
    /// * `said` - The entry.
    pub fn record(&mut self, from: &Path, said: &NodeFile) {
        self.said.insert(
            said.node.name.clone(),
            (
                from.to_path_buf(),
                Sinks {
                    world_in: said.world_in.clone(),
                    moc_in: said.moc_in.clone(),
                },
            ),
        );
    }

    /// Matches each line to a name its far end lists, and makes it what it
    /// means.
    ///
    /// A node's names first, each once and under a sink its type has; then
    /// each line, which must name its node and a name that node lists under
    /// the sink the line's stream arrives at. A line to a node nothing
    /// declares is left as written, for the checking to say there is no such
    /// node.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration, every file in it read. Its links are
    ///             made what they mean, and its [`Named`] filled in.
    ///
    /// # Returns
    ///
    /// `Ok(())` once every line is matched.
    ///
    /// # Errors
    ///
    /// [`TcsError::Config`], naming the file, for a name listed twice or
    /// under a sink the type does not have, a line naming no name, a name its
    /// far end does not list under the sink it arrives at, and two lines from
    /// one source to one node.
    pub fn resolve(&self, graph: &mut Graph) -> TcsResult<()> {
        for (node, (from, sinks)) in &self.said {
            let at = from.display();
            let mut seen: Vec<&str> = Vec::new();
            for (sink, names) in
                [("world-in", &sinks.world_in), ("moc-in", &sinks.moc_in)]
            {
                for name in names {
                    if seen.contains(&name.as_str()) {
                        return Err(TcsError::Config(format!(
                            "{at}: \"{node}\" lists \"{name}\" twice: a \
                             name is where a line arrives, and two of one \
                             name would leave a line to it meaning either. \
                             List it once, under one sink"
                        )));
                    }
                    seen.push(name);
                }
                if names.is_empty() {
                    continue;
                }
                // A sink the type does not have, said as what it is: "a
                // payload has no data sink called world-in" is the useful
                // complaint, not one about the lines to it.
                if let Some(said) = graph.node(node) {
                    if let Err(why) =
                        terminal_at(&said.node_type, Some(sink), true)
                    {
                        return Err(TcsError::Config(format!(
                            "{at}: \"{node}\" lists names under {sink}: \
                             {why}"
                        )));
                    }
                }
            }
        }

        let mut written = BTreeMap::new();
        let mut links = std::mem::take(&mut graph.links);
        for link in &mut links {
            let origin = link.origin();
            // The source terminal, which every link read from a node has: a
            // link is written under the terminal it leaves by. One that
            // names none, or a terminal of no stream, is the checking's to
            // complain about.
            let Some(leaves) = origin.input.as_deref() else {
                continue;
            };
            let Some(sink) = answers(leaves) else {
                continue;
            };
            let at = graph
                .homes
                .get(&origin.node)
                .map(|home| home.display().to_string())
                .unwrap_or_default();
            let mut meant: Vec<String> = Vec::new();
            for destination in link.to.each() {
                let Destination { node, input } =
                    Destination::parse(destination);
                let (Some((_, sinks)), Some(reached)) =
                    (self.said.get(&node), graph.node(&node))
                else {
                    meant.push(destination.to_string());
                    continue;
                };
                let (ours, theirs, other) = if sink == "world-in" {
                    (&sinks.world_in, &sinks.moc_in, "moc-in")
                } else {
                    (&sinks.moc_in, &sinks.world_in, "world-in")
                };
                let listed = || match ours.as_slice() {
                    [] => format!("it lists none under its {sink}"),
                    names => format!(
                        "it lists {} under its {sink}",
                        names
                            .iter()
                            .map(|name| format!("\"{name}\""))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                let sender = &origin.node;
                let Some(name) = input else {
                    return Err(TcsError::Config(format!(
                        "{at}: \"{sender}\" sends a line to \"{node}\" by \
                         its {leaves} and names no name for it to arrive by: \
                         write it as `{node}.<name>`, with a name \"{node}\" \
                         lists under its {sink}, and {}",
                        listed()
                    )));
                };
                if !ours.contains(&name) {
                    let why = if theirs.contains(&name) {
                        format!(
                            "\"{node}\" lists \"{name}\" under its {other}, \
                             and a line leaving by a {leaves} arrives at a \
                             {sink}"
                        )
                    } else {
                        format!(
                            "\"{node}\" lists no \"{name}\" under its \
                             {sink}: add it there, or send to a name it \
                             lists -- {}",
                            listed()
                        )
                    };
                    return Err(TcsError::Config(format!(
                        "{at}: \"{sender}\" sends a line to \
                         \"{destination}\", and {why}"
                    )));
                }
                let means = meaning(reached, sink, &name);
                if meant.contains(&means) {
                    return Err(TcsError::Config(format!(
                        "{at}: \"{sender}\" sends two lines by its {leaves} \
                         to \"{node}\", and one carries everything it sends \
                         there: take out \"{destination}\""
                    )));
                }
                written.insert(
                    (link.from.clone(), means.clone()),
                    destination.to_string(),
                );
                meant.push(means);
            }
            link.to = Destinations::of(meant);
        }
        graph.links = links;

        graph.named = Named {
            sinks: self
                .said
                .iter()
                .map(|(node, (_, sinks))| (node.clone(), sinks.clone()))
                .collect(),
            written,
        };
        Ok(())
    }
}

/// Says what a line arriving at a node by a name means.
///
/// The sink it arrives at, written the way every rule reads a destination:
/// the node alone where the type has one sink, and the node and the sink
/// where it has two. A vote's world-in names are its inputs, so a line to one
/// of those is to that input. A computation or a log has one sink of each
/// stream, which is the node alone.
///
/// # Arguments
///
/// * `node` - The node the line reaches.
/// * `sink` - The sink it arrives at, `world-in` or `moc-in`.
/// * `name` - The name it arrives by.
///
/// # Returns
///
/// The destination, `node`, `node.<sink>` or `node.<input>`.
fn meaning(node: &Node, sink: &str, name: &str) -> String {
    match terminals_of(&node.node_type) {
        Some(terminals) if terminals.sinks.len() > 1 => {
            format!("{}.{sink}", node.name)
        }
        Some(_) => node.name.clone(),
        None => {
            let votes = node
                .attributes
                .get("function")
                .is_some_and(|function| function.as_str() == "vote");
            if votes && sink == "world-in" {
                format!("{}.{name}", node.name)
            } else {
                node.name.clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads a configuration of one payload and the ground, with whatever the
    /// two entries are given to say about where they may be joined and where
    /// they send.
    ///
    /// # Arguments
    ///
    /// * `gateway_says` - Lines added to the ground's entry.
    /// * `payload_says` - Lines added to the payload's entry.
    ///
    /// # Returns
    ///
    /// The configuration.
    ///
    /// # Errors
    ///
    /// Whatever [`Graph::load`] refuses in it.
    ///
    /// # Panics
    ///
    /// If the file cannot be written.
    fn pair(gateway_says: &str, payload_says: &str) -> TcsResult<Graph> {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        std::fs::write(
            &path,
            format!(
                "tcspecial:
  gateway: radio
  interface: \"c7820f9bfe585feab72247ece0a2261d\"
  state_file: /tmp/tcspecial-test.state
  tcs_cmd:
    recv: 0.0.0.0:4000
    send: 127.0.0.1:4100
  payload_command:
    source_base: 127.0.0.1:4300
    dest_base: 0.0.0.0:4200
  payload_telemetry:
    source_base: 0.0.0.0:6000
    dest_base: 127.0.0.1:6100
  beacon:
    default_interval: 5 s
    reset_interval: 5 s
    send: \"0.0.0.0:5550\"
    recv: \"127.0.0.1:5650\"
gateways:
  - type gateway_radio:
      - gateway: gateway
        address: \"10.0.0.1\"
        port: \"5000\"
{gateway_says}
payloads:
  - type i2c:
      - payload: payload
        path: \"/dev/i2c-1\"
        address: \"0x48\"
        read_size: \"2\"
{payload_says}
"
            ),
        )
        .unwrap();
        Graph::load(&path)
    }

    /// The ground's half: where it may be joined, and what it sends.
    const GATEWAY: &str = "        world-in: [readings]
        moc-out: [payload.asked]";
    /// And the payload's.
    const PAYLOAD: &str = "        moc-in: [asked]
        world-out: [gateway.readings]";

    /// A line to a name its far end lists is read, as what it means, and
    /// the name is kept for the editor.
    #[test]
    fn a_line_to_a_listed_name_is_read() {
        let graph = pair(GATEWAY, PAYLOAD).expect("both ends agree");
        let to = |from: &str| {
            let link = graph
                .links
                .iter()
                .find(|link| link.from == from)
                .expect("the link");
            link.to.each().join(",")
        };
        assert_eq!(to("payload.world-out"), "gateway");
        assert_eq!(to("gateway.moc-out"), "payload");
        assert_eq!(
            graph.named.written
                [&("payload.world-out".to_string(), "gateway".to_string())],
            "gateway.readings"
        );
        assert_eq!(graph.named.sinks["payload"].moc_in, ["asked"]);
    }

    /// A name listed with nothing arriving at it is where the node may be
    /// joined, and reads.
    #[test]
    fn a_name_nothing_arrives_at_is_read() {
        pair("        world-in: [readings, spare]", "")
            .expect("a name may wait for a line");
    }

    /// A line to a name nobody listed is refused, saying which the node
    /// does list.
    #[test]
    fn a_line_to_a_name_not_listed_is_refused() {
        let said = pair(
            GATEWAY,
            "        moc-in: [asked]\n        world-out: [gateway.reading]",
        )
        .expect_err("gateway lists readings, not reading")
        .to_string();
        assert!(said.contains("lists no \"reading\""), "{said}");
        assert!(said.contains("\"readings\""), "{said}");

        // A name under the other sink, said as that.
        let said = pair(
            "        world-in: [readings]\n        moc-in: [x]",
            "        world-out: [gateway.x]",
        )
        .expect_err("a payload sink has no moc-in on the ground");
        assert!(said.to_string().contains("no data sink"), "{said}");

        // And a line naming no name at all.
        let said = pair(
            "        world-in: [readings]",
            "        world-out: [gateway]",
        )
        .expect_err("no name")
        .to_string();
        assert!(said.contains("`gateway.<name>`"), "{said}");
    }

    /// A name listed twice, or under a sink the type does not have, is
    /// refused.
    #[test]
    fn a_name_listed_twice_or_in_the_wrong_place_is_refused() {
        let said = pair("        world-in: [readings, readings]", "")
            .expect_err("twice")
            .to_string();
        assert!(said.contains("twice"), "{said}");

        let said = pair("", "        world-in: [readings]")
            .expect_err("a payload has no world-in")
            .to_string();
        assert!(said.contains("no data sink called \"world-in\""), "{said}");
    }

    /// The flag a sink once was is refused, told what it holds now.
    #[test]
    fn a_sink_written_as_a_flag_is_refused() {
        let said = pair("        world-in: true", "")
            .expect_err("a flag is not a name")
            .to_string();
        assert!(said.contains("list the names"), "{said}");
    }

    /// A vote's world-in names are its inputs, and a line to one is to that
    /// input.
    #[test]
    fn a_vote_is_over_the_names_it_lists() {
        let graph = pair(
            GATEWAY,
            "        moc-in: [asked]\n        world-out: [vote.a]\n\
             vehicle:\n  - type compute:\n      - compute: vote\n        \
             function: vote\n        algorithm: median\n        \
             world-in: [a, b, c]",
        )
        .expect("a vote over three");
        let vote = graph.node("vote").expect("the vote");
        assert_eq!(vote.attributes["inputs"].as_str(), "a,b,c");
        let link = graph
            .links
            .iter()
            .find(|link| link.from == "payload.world-out")
            .expect("the link");
        assert_eq!(link.to.each(), ["vote.a"]);
    }
}
