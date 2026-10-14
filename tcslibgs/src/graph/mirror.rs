//! The arriving end of a link, as a file records it.
//!
//! A line between two nodes is written at both of its ends. On the node it
//! leaves, under the data source it leaves by, where the value names where it
//! goes -- `to_moc: k_fore` -- and on the node it arrives at, under the data
//! sink it arrives at, where the value names where it came from --
//! `from_payload: temp_fore`. The first is what the configuration means:
//! [`NodeFile::links`](super::NodeFile::links) derives the graph from it and
//! the flight software reads nothing else. The second is the same fact
//! recorded at the other end, so that a node's own entry says what it is
//! connected to without the rest of the file having to be read against it.
//!
//! Two statements of one fact can disagree, and the answer to that is not to
//! believe the second one. The record is checked against the links it mirrors
//! where the configuration is read, and a disagreement is refused saying which
//! two entries say different things. Nothing downstream ever reads it.
//!
//! It is optional. A file somebody wrote by hand may record nothing at the
//! arriving ends and be complete without it -- there is nothing here the
//! source side does not already say -- and every file written before this
//! existed stays readable. But a node that records one source at a sink
//! records all of them: a half-written record is worse than none, because it
//! reads as "and nothing else".

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::error::{TcsError, TcsResult};

use super::terminals::{answers, stream_of, terminal_at};
use super::{Destination, Graph, NodeFile};

/// What the files recorded at the arriving ends of their links.
///
/// Gathered as the files are read and thrown away once it has been checked:
/// see [`Mirror::check`]. It is not part of [`Graph`] because it is not part
/// of the configuration -- it is a second copy of part of it, and a second
/// copy that anything went on to read would be a second answer to the
/// question of what is linked to what.
#[derive(Debug, Default)]
pub struct Mirror {
    /// By the node that recorded them and the sink they arrive at: the sources
    /// named there, and the file that named them.
    said: BTreeMap<(String, String), Recorded>,
}

/// One sink's record, and where it was read from.
#[derive(Debug)]
struct Recorded {
    /// The file it is in, so that a complaint can say which one to open. Not
    /// necessarily the file the link itself is written in: a link may cross
    /// from an included file into the top one.
    from: PathBuf,
    /// The sources named, as written: `node` or `node.terminal`.
    sources: Vec<String>,
}

impl Mirror {
    /// Take in what one node's entry recorded.
    pub fn record(&mut self, from: &Path, said: &NodeFile) {
        for (sink, sources) in said.arriving() {
            if sources.is_empty() {
                continue;
            }
            self.said
                .entry((said.node.name.clone(), sink.to_string()))
                .or_insert_with(|| Recorded {
                    from: from.to_path_buf(),
                    sources: Vec::new(),
                })
                .sources
                .extend(sources.iter().cloned());
        }
    }

    /// Whether every record agrees with the links it is a record of.
    ///
    /// Three ways it can fail, and each is a sentence about two entries rather
    /// than about one: a source named that nothing links from, a source named
    /// as arriving by a terminal it does not leave by, and a link left out of
    /// a sink that records its others. The configuration is refused in all
    /// three, because a file that contradicts itself about its own graph is a
    /// file whose other half may be the wrong one.
    pub fn check(&self, graph: &Graph) -> TcsResult<()> {
        let arriving = arrivals(graph);

        for ((node, sink), recorded) in &self.said {
            let at = recorded.from.display();
            // A sink the type does not have, said as what it is before
            // anything is compared: "a payload has no data sink called
            // from_payload" is the useful complaint, not "nothing links
            // there".
            if let Some(said) = graph.node(node) {
                if let Err(why) = terminal_at(&said.node_type, Some(sink), true) {
                    return Err(TcsError::Config(format!(
                        "{at}: \"{node}\" records {sink}: {why}"
                    )));
                }
            }

            let links = arriving.get(&(node.clone(), sink.clone()));
            let mut named: BTreeSet<String> = BTreeSet::new();
            for source in &recorded.sources {
                let said = Destination::parse(source);
                let leaves = links
                    .and_then(|links| links.get(&said.node))
                    .ok_or_else(|| {
                        TcsError::Config(format!(
                            "{at}: \"{node}\" records {sink}: {source}, but \
                             nothing links \"{said}\" to it. A line is \
                             recorded at both of its ends and the two must \
                             agree, and the link itself is the one written on \
                             \"{said}\"",
                            said = said.node
                        ))
                    })?;
                // Where the record named the terminal the data leaves by, it
                // has to be the one it leaves by.
                if let Some(by) = &said.input {
                    if by != leaves {
                        return Err(TcsError::Config(format!(
                            "{at}: \"{node}\" records {sink}: {source}, but \
                             the link from \"{}\" leaves by {leaves}",
                            said.node
                        )));
                    }
                }
                named.insert(said.node.clone());
            }

            // And all of them, since it records any: see the module comment.
            let all: BTreeSet<String> = links
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default();
            if let Some(left_out) = all.difference(&named).next() {
                return Err(TcsError::Config(format!(
                    "{at}: \"{node}\" records what arrives at {sink} and \
                     leaves out \"{left_out}\", which links to it. A sink's \
                     record names every source that arrives there, or none at \
                     all: one that names some of them reads as a list of all \
                     of them"
                )));
            }
        }

        Ok(())
    }
}

/// What arrives at each sink: by node and sink, the source nodes and the
/// terminal each leaves by.
///
/// Derived from the links, which are the configuration. The sink a link
/// arrives at is the one the destination named, where it named one of the
/// four; otherwise it is the one at the other end of the stream it leaves by
/// -- a `to_moc` arrives at a `from_payload` and nowhere else, so a file that
/// did not say it still said it.
fn arrivals(graph: &Graph) -> BTreeMap<(String, String), BTreeMap<String, String>> {
    let mut arriving: BTreeMap<(String, String), BTreeMap<String, String>> = BTreeMap::new();
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
                .insert(origin.node.clone(), leaves.to_string());
        }
    }
    arriving
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A configuration of one payload and the ground, with whatever the two
    /// entries are given to say about what arrives at them.
    ///
    /// The groundward link is the payload's `to_moc: moc` and the vehicleward
    /// one is the ground's `to_payload: payload`, so the records that agree
    /// with them are `from_payload: payload` on the ground and `from_moc: moc`
    /// on the payload.
    fn pair(moc_records: &str, payload_records: &str) -> TcsResult<Graph> {
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
    to_payload: payload
{moc_records}
payloads:
  - name: payload
    type: i2c
    path: \"/dev/i2c-1\"
    address: \"0x48\"
    read_size: \"2\"
    to_moc: moc
{payload_records}
"
            ),
        )
        .unwrap();
        Graph::load(&path)
    }

    /// Both ends recorded, and agreeing, is a configuration that reads.
    #[test]
    fn a_record_that_agrees_with_its_link_is_read() {
        let graph = pair("    from_payload: payload", "    from_moc: moc")
            .expect("both ends saying the same thing");
        assert_eq!(graph.links.len(), 2, "the records are not links");
    }

    /// And the terminal the data leaves by may be named as well.
    #[test]
    fn a_record_may_name_the_terminal_the_data_left_by() {
        pair(
            "    from_payload: payload.to_moc",
            "    from_moc: moc.to_payload",
        )
        .expect("the long spelling of the same thing");
    }

    /// Nothing recorded is a file that is still complete.
    ///
    /// Every configuration written before the arriving end was recorded at
    /// all, and every one somebody writes by hand without bothering: there is
    /// nothing in a record that the link itself does not say.
    #[test]
    fn a_file_that_records_nothing_is_read() {
        let graph = pair("", "").expect("the source side is the configuration");
        assert_eq!(graph.links.len(), 2);
    }

    /// A record of a link that is not there is refused.
    #[test]
    fn a_record_of_a_link_nothing_made_is_refused() {
        let said = format!(
            "{}",
            pair("    from_payload: ghost", "    from_moc: moc")
                .expect_err("a source nothing links from")
        );
        assert!(said.contains("\"ghost\""), "{said}");
        assert!(said.contains("from_payload"), "{said}");
    }

    /// A record that names the wrong terminal at the other end is refused.
    #[test]
    fn a_record_that_names_the_wrong_source_terminal_is_refused() {
        let said = format!(
            "{}",
            pair("    from_payload: payload.to_payload", "    from_moc: moc")
                .expect_err("the payload's link leaves by to_moc")
        );
        assert!(said.contains("leaves by to_moc"), "{said}");
    }

    /// A sink that records some of what arrives there is refused.
    ///
    /// The whole reason to refuse it: a list that is missing one reads as a
    /// list of everything, so a half-written record is a statement that the
    /// link somebody is looking for does not exist.
    #[test]
    fn a_record_that_leaves_out_a_link_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        std::fs::write(
            &path,
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
    from_payload: fore
payloads:
  - { name: fore, type: i2c, path: \"/dev/i2c-1\", address: \"0x48\", \
             read_size: \"2\", to_moc: moc }
  - { name: aft, type: i2c, path: \"/dev/i2c-1\", address: \"0x49\", \
             read_size: \"2\", to_moc: moc }
",
        )
        .unwrap();

        let said = format!("{}", Graph::load(&path).expect_err("aft is left out"));
        assert!(said.contains("\"aft\""), "{said}");
        assert!(said.contains("every source"), "{said}");
    }

    /// A record under a sink the node has not got is said to be that.
    ///
    /// The useful complaint about `from_payload` on a payload is that a
    /// payload has no such sink, not that nothing links there.
    #[test]
    fn a_record_under_a_sink_of_the_wrong_kind_is_refused() {
        let said = format!(
            "{}",
            pair("    from_payload: payload", "    from_payload: moc")
                .expect_err("a payload receives at from_moc")
        );
        assert!(said.contains("no data sink called \"from_payload\""), "{said}");
    }
}
