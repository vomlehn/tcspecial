//! What a node's entry says about its own data sinks.
//!
//! A line between two nodes is written on the node it leaves, under the data
//! source it leaves by, with the node it reaches named by the entry's
//! `target`. The node it arrives at says so too, under the sink it arrives
//! at: `from_payload: true`. The first is what the configuration means --
//! [`NodeFile::links`](super::NodeFile::links) derives the graph from it and
//! the flight software reads nothing else. The second is a record of the same
//! fact at the other end, so that a node's own entry says it is connected
//! without the rest of the file having to be read against it.
//!
//! A flag and not a name. It named the node the data came from for a while,
//! which is a second copy of the link and reads as "and nothing else" the
//! moment a second line arrives at the same sink -- the ground's does, every
//! payload aboard reporting to it. What a node can say about its own sink
//! without repeating the graph is whether anything arrives there at all, and
//! that is what this is.
//!
//! Two statements of one fact can still disagree, and the answer to that is
//! not to believe the second one. The flag is checked against the links it is
//! a record of where the configuration is read, and a disagreement is refused
//! saying which two entries say different things. Nothing downstream ever
//! reads it.
//!
//! It is optional. A file somebody wrote by hand may say nothing about its
//! sinks and be complete -- there is nothing here the links do not already say
//! -- and every file written before this existed stays readable.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::error::{TcsError, TcsResult};

use super::terminals::{answers, stream_of, terminal_at};
use super::{Destination, Graph, NodeFile};

/// What the files said about their nodes' data sinks.
///
/// Gathered as the files are read and thrown away once it has been checked:
/// see [`Mirror::check`]. It is not part of [`Graph`] because it is not part
/// of the configuration -- it is a second statement of part of it, and a
/// second statement that anything went on to read would be a second answer to
/// the question of what is linked to what.
#[derive(Debug, Default)]
pub struct Mirror {
    /// By the node that said it and the sink it was said of: whether a line
    /// arrives there, and the file that said so.
    said: BTreeMap<(String, String), Recorded>,
}

/// One sink's flag, and where it was read from.
#[derive(Debug)]
struct Recorded {
    /// The file it is in, so that a complaint can say which one to open. Not
    /// necessarily the file the link itself is written in: a link may cross
    /// from an included file into the top one.
    from: PathBuf,
    /// What it said: a line arrives here, or none does.
    arrives: bool,
}

impl Mirror {
    /// Take in what one node's entry said about its sinks.
    pub fn record(&mut self, from: &Path, said: &NodeFile) {
        for (sink, arrives) in said.arriving() {
            let Some(arrives) = arrives else { continue };
            self.said.insert(
                (said.node.name.clone(), sink.to_string()),
                Recorded {
                    from: from.to_path_buf(),
                    arrives,
                },
            );
        }
    }

    /// Whether every flag agrees with the links it is a record of.
    ///
    /// Both ways round, and each is a sentence about two entries rather than
    /// about one: a sink that says a line arrives where none does, and one
    /// that says none does where one is written. The configuration is refused
    /// for either, because a file that contradicts itself about its own graph
    /// is a file whose other half may be the wrong one.
    pub fn check(&self, graph: &Graph) -> TcsResult<()> {
        let arriving = arrivals(graph);

        for ((node, sink), recorded) in &self.said {
            let at = recorded.from.display();
            // A sink the type does not have, said as what it is before
            // anything is compared: "a payload has no data sink called
            // from_payload" is the useful complaint, not "nothing arrives
            // there".
            if let Some(said) = graph.node(node) {
                if let Err(why) = terminal_at(&said.node_type, Some(sink), true) {
                    return Err(TcsError::Config(format!(
                        "{at}: \"{node}\" says {sink}: {why}"
                    )));
                }
            }

            let links = arriving.get(&(node.clone(), sink.clone()));
            let sources: Vec<&str> = links
                .map(|links| links.iter().map(String::as_str).collect())
                .unwrap_or_default();

            match (recorded.arrives, sources.as_slice()) {
                (true, []) => {
                    return Err(TcsError::Config(format!(
                        "{at}: \"{node}\" says a line arrives at its {sink}, \
                         and none does. A line is recorded at both of its ends \
                         and the two must agree: the link itself is written on \
                         the node it leaves, as an entry under the data source \
                         it leaves by"
                    )))
                }
                (false, [first, ..]) => {
                    return Err(TcsError::Config(format!(
                        "{at}: \"{node}\" says no line arrives at its {sink}, \
                         and \"{first}\" sends one there"
                    )))
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// What arrives at each sink: by node and sink, the nodes that send there.
///
/// Derived from the links, which are the configuration. The sink a link
/// arrives at is the one the destination named, where it named one of the
/// four; otherwise it is the one at the other end of the stream it leaves by
/// -- a `to_moc` arrives at a `from_payload` and nowhere else, so a file that
/// did not say it still said it.
///
/// The names are here for the complaint rather than for the comparison: what
/// a flag is checked against is whether this is empty, and what a person needs
/// told is which node they forgot.
fn arrivals(graph: &Graph) -> BTreeMap<(String, String), BTreeSet<String>> {
    let mut arriving: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for link in &graph.links {
        let origin = link.origin();
        // The source terminal, which every version 2 link has: a link is
        // written under the terminal it leaves by, so the name is how it was
        // found at all.
        let Some(leaves) = origin.input.as_deref() else {
            continue;
        };
        for destination in link.to.each() {
            let said = Destination::parse(destination);
            let sink = match said.input.as_deref() {
                Some(named) if stream_of(named).is_some() => named.to_string(),
                // A computation's own input, or nothing named: either way the
                // stream says where it arrives.
                _ => match answers(leaves) {
                    Some(sink) => sink.to_string(),
                    None => continue,
                },
            };
            arriving
                .entry((said.node.clone(), sink))
                .or_default()
                .insert(origin.node.clone());
        }
    }
    arriving
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A configuration of one payload and the ground, with whatever the two
    /// entries are given to say about their own data sinks.
    ///
    /// The groundward link is the payload's `to_moc: moc` and the vehicleward
    /// one is the ground's `to_payload: payload`, so the flags that agree with
    /// them are `from_payload: true` on the ground and `from_moc: true` on the
    /// payload.
    fn pair(moc_says: &str, payload_says: &str) -> TcsResult<Graph> {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
            format!(
                "version: \"2.0\"
tcspecial:
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
    interval_ms: 5000
    send: \"0.0.0.0:5550\"
    recv: \"127.0.0.1:5650\"
moc:
  - name: moc
    type: moc
    address: \"10.0.0.1\"
    port: \"5000\"
    to_payload:
      - target: payload
{moc_says}
payloads:
  - name: payload
    type: i2c
    path: \"/dev/i2c-1\"
    address: \"0x48\"
    read_size: \"2\"
    to_moc:
      - target: moc
{payload_says}
"
            ),
        )
        .unwrap();
        Graph::load(&path)
    }

    /// Both sinks saying a line arrives, where one does, is a configuration
    /// that reads.
    #[test]
    fn a_flag_that_agrees_with_the_links_is_read() {
        let graph = pair("    from_payload: true", "    from_moc: true")
            .expect("both ends saying the same thing");
        assert_eq!(graph.links.len(), 2, "the flags are not links");
    }

    /// Saying nothing is a file that is still complete.
    ///
    /// Every configuration written before a sink could say anything, and every
    /// one somebody writes by hand without bothering: there is nothing in a
    /// flag that the links do not already say.
    #[test]
    fn a_file_that_says_nothing_about_its_sinks_is_read() {
        let graph = pair("", "").expect("the links are the configuration");
        assert_eq!(graph.links.len(), 2);
    }

    /// A sink that claims a line nothing sends is refused.
    #[test]
    fn a_sink_that_claims_a_line_nothing_sends_is_refused() {
        let said = format!(
            "{}",
            pair("    from_payload: true", "    from_moc: true\n    from_payload: true")
                .expect_err("a payload has no from_payload")
        );
        assert!(said.contains("no data sink called \"from_payload\""), "{said}");

        // And on a node that does have the sink, with nothing arriving at it:
        // a null node carries both streams, and this one is sent neither.
        let said = format!(
            "{}",
            pair(
                "    from_payload: true",
                "    from_moc: true\nvehicle:\n  - name: through\n    type: null\n    \
                 from_moc: true",
            )
            .expect_err("nothing sends the null node anything")
        );
        assert!(said.contains("none does"), "{said}");
        assert!(said.contains("\"through\""), "{said}");
    }

    /// And one that denies a line somebody sends it.
    ///
    /// The direction that matters more: a sink saying `false` is a sink
    /// somebody has read as unconnected, and the link says otherwise.
    #[test]
    fn a_sink_that_denies_a_line_it_is_sent_is_refused() {
        let said = format!(
            "{}",
            pair("    from_payload: false", "    from_moc: true")
                .expect_err("the payload sends its readings there")
        );
        assert!(said.contains("\"payload\" sends one there"), "{said}");
    }

    /// `false` where nothing arrives is true, and reads.
    #[test]
    fn a_sink_that_denies_a_line_nobody_sends_is_read() {
        pair("    from_payload: true", "    from_moc: false")
            .expect_err("the ground sends the payload what it asks with");
        pair("    from_payload: true", "    from_moc: true")
            .expect("and says so when it says so");
    }

    /// A word that is neither true nor false is refused, saying what the
    /// field means.
    ///
    /// It named the node the data came from for a while. A file written
    /// against that is refused rather than half understood -- the name read as
    /// a word nobody knows would be a sink quietly saying nothing.
    #[test]
    fn a_sink_given_a_name_instead_of_a_flag_is_refused() {
        let said = format!(
            "{}",
            pair("    from_payload: payload", "    from_moc: true")
                .expect_err("a name is not a flag")
        );
        assert!(said.contains("is not true or false"), "{said}");
        assert!(said.contains("written on that node"), "{said}");
    }
}
