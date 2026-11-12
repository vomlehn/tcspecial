//! How often each node sends, as far as a configuration says: an estimate,
//! for someone adding up what a vehicle sends the ground.
//!
//! A node read on a schedule sends as often as the schedule says: its
//! group's `interval`, its `trigger_interval`, at most once a
//! `poll_interval`, or a set of discrete lines' `state_interval`. Any other
//! node sends at most one datum for each that arrives at it -- a
//! computation answers each arrival, a tee passes each on -- so its rate is
//! at most the sum of what is linked into it. A node that is read on no
//! schedule and has nothing linked into it sends when it has something to
//! send, which no file says, and its rate is not known; nor is the rate of
//! anything fed by it. Nor is a gateway node's, which sends aboard what the
//! ground sends up.
//!
//! An estimate and not a promise: a computation that sends only on a
//! change sends less, and a part that sends on its own as well as when it
//! is asked sends more. What it is for is a sum that shows which sources
//! are most of a link's load, and roughly how much that is.

use std::collections::BTreeMap;
use std::time::Duration;

use super::{Destination, Graph, Node};

/// How often a node sends, where a configuration says.
#[derive(Debug, Clone, PartialEq)]
pub struct Rate {
    /// Data per second: exactly that for a node read on a schedule, at most
    /// that for one fed by others, and `None` where it is not known.
    pub per_second: Option<f64>,
    /// Whether `per_second` is an upper bound rather than the rate.
    pub at_most: bool,
    /// Where the figure comes from, in the configuration's words.
    pub how: String,
}

/// Says how often a node is read on a schedule, where it is.
///
/// # Arguments
///
/// * `graph` - The configuration.
/// * `node`  - The node.
///
/// # Returns
///
/// The interval, whether it is a least time between reads rather than the
/// time between them, and the words that state it; `None` for a node read
/// on no schedule, or one whose interval does not read, which the check
/// says.
pub fn scheduled(
    graph: &Graph,
    node: &Node,
) -> Option<(Duration, bool, String)> {
    let said = |name: &str| {
        node.attributes.get(name).and_then(|value| {
            super::parse_interval(value.as_str().trim())
                .ok()
                .map(|every| (every, value.as_str().trim().to_string()))
        })
    };
    if let Some(group) = node.group.as_deref().and_then(|g| graph.group(g)) {
        let every = group.interval().ok()?;
        return Some((
            every,
            false,
            format!("group {} interval {}", group.name, group.interval),
        ));
    }
    if let Some((every, text)) = said("trigger_interval") {
        return Some((every, false, format!("trigger_interval {text}")));
    }
    if let Some((every, text)) = said("poll_interval") {
        return Some((every, true, format!("poll_interval {text}")));
    }
    if let Some((every, text)) = said("state_interval") {
        return Some((every, false, format!("state_interval {text}")));
    }
    None
}

/// Estimates how often every node sends: see the module's words.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each node's rate, by name.
pub fn of(graph: &Graph) -> BTreeMap<String, Rate> {
    let mut rates = BTreeMap::new();
    for node in &graph.nodes {
        rate_of(graph, &node.name, &mut rates, &mut Vec::new());
    }
    rates
}

/// Data per second from an interval.
///
/// # Arguments
///
/// * `every` - The interval.
///
/// # Returns
///
/// How many times a second it comes round, or `None` for an interval of
/// nought, which is no rate.
fn per_second(every: Duration) -> Option<f64> {
    (!every.is_zero()).then(|| 1.0 / every.as_secs_f64())
}

/// Estimates one node's rate, and those of the nodes it is fed by.
///
/// # Arguments
///
/// * `graph`   - The configuration.
/// * `name`    - The node's name.
/// * `rates`   - The rates found so far, which this adds to.
/// * `walking` - The nodes whose rates are being found, for a loop.
///
/// # Returns
///
/// The node's rate.
fn rate_of(
    graph: &Graph,
    name: &str,
    rates: &mut BTreeMap<String, Rate>,
    walking: &mut Vec<String>,
) -> Rate {
    if let Some(found) = rates.get(name) {
        return found.clone();
    }
    // A loop sends as often as whatever starts it, which nothing here
    // can say.
    if walking.iter().any(|n| n == name) {
        return Rate {
            per_second: None,
            at_most: false,
            how: "fed by a loop".to_string(),
        };
    }
    let Some(node) = graph.node(name) else {
        return Rate {
            per_second: None,
            at_most: false,
            how: "not in the configuration".to_string(),
        };
    };
    let rate = if crate::gateway::is_gateway(&node.node_type) {
        // What leaves a gateway node aboard is what the ground sends up,
        // and not what is linked into it, which goes down.
        Rate {
            per_second: None,
            at_most: false,
            how: "what the ground sends".to_string(),
        }
    } else if let Some((every, at_most, how)) = scheduled(graph, node) {
        Rate {
            per_second: per_second(every),
            at_most,
            how,
        }
    } else {
        let mut feeds: Vec<String> = graph
            .links
            .iter()
            .filter(|link| {
                link.to
                    .each()
                    .iter()
                    .any(|to| Destination::parse(to).node == name)
            })
            .map(|link| link.origin().node)
            .collect();
        feeds.sort();
        feeds.dedup();
        if feeds.is_empty() {
            Rate {
                per_second: None,
                at_most: false,
                how: "sends when it has something: no schedule".to_string(),
            }
        } else {
            walking.push(name.to_string());
            let mut sum = Some(0.0);
            for feed in &feeds {
                let fed = rate_of(graph, feed, rates, walking);
                sum = sum.zip(fed.per_second).map(|(a, b)| a + b);
            }
            walking.pop();
            Rate {
                per_second: sum,
                at_most: true,
                how: format!("one per datum from {}", feeds.join(", ")),
            }
        }
    };
    rates.insert(name.to_string(), rate.clone());
    rate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The thermostat's sensors are read once a second in their group, each
    /// conversion sends one for each reading, and the vote one for each of
    /// the three.
    #[test]
    fn the_thermostat_adds_up() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        let graph = Graph::load(root.join("thermostat/thermostat.ytcs"))
            .expect("the example loads");
        let rates = of(&graph);
        assert_eq!(rates["temp_1"].per_second, Some(1.0));
        assert!(!rates["temp_1"].at_most);
        assert!(rates["temp_1"].how.contains("zone_temps"));
        assert_eq!(rates["to_k1"].per_second, Some(1.0));
        assert!(rates["to_k1"].at_most);
        assert_eq!(rates["consensus_temp"].per_second, Some(3.0));
        assert_eq!(rates["gpio-out-tmass1"].per_second, Some(1.0));
    }
}
