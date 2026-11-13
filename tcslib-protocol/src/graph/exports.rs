//! What of an included file may be joined from outside it.
//!
//! An included file is a page of the configuration -- tcsgui draws it as a
//! box on the top page, opened into a page of its own -- and its `exports`
//! section is the box's edge: for each of the four terminals, the nodes in
//! the file whose terminal of that name a line from another file may reach.
//! A line that crosses into or out of an included file at a node it does not
//! export is refused by [`crate::graph::Graph::check`], saying which list to
//! add the node to.
//!
//! Stated in the file rather than worked out from the lines that happen to
//! cross it, because an edge is something a reader of the file can check: a
//! page whose insides anything outside may reach has no edge at all, and one
//! line drawn to the wrong node across it is a line nothing would question.
//! The links themselves are written as they always were, node to node, a
//! node's name being its own in every file; so nothing aboard reads this, and
//! the flight software runs the same graph whether a file exports or not.
//!
//! One level, as includes are: the top file has nothing around it, and a
//! top file with an `exports` section is refused as it is read.

use serde::{Deserialize, Serialize};

/// An included file's `exports`: by terminal, the nodes whose terminal of
/// that name may be reached from another file.
///
/// ```yaml
/// exports:
///   world-in: [temp1, temp2, temp3]
///   world-out: [ambient]
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Exports {
    /// Nodes whose readings may arrive from another file.
    #[serde(default, rename = "world-in")]
    pub world_in: Vec<String>,
    /// Nodes whose readings may leave for another file.
    #[serde(default, rename = "world-out")]
    pub world_out: Vec<String>,
    /// Nodes another file may send what the ground sends to.
    #[serde(default, rename = "moc-in")]
    pub moc_in: Vec<String>,
    /// Nodes whose share of what the ground sends may leave for another file.
    #[serde(default, rename = "moc-out")]
    pub moc_out: Vec<String>,
}

impl Exports {
    /// The four lists, each with the terminal it is for, in the order the
    /// terminals are named everywhere else.
    ///
    /// # Returns
    ///
    /// Each terminal's name and the nodes exported at it.
    pub fn each(&self) -> [(&'static str, &[String]); 4] {
        [
            ("world-in", &self.world_in),
            ("world-out", &self.world_out),
            ("moc-in", &self.moc_in),
            ("moc-out", &self.moc_out),
        ]
    }

    /// Whether this node's terminal of this name is exported.
    ///
    /// # Arguments
    ///
    /// * `node`     - The node's name.
    /// * `terminal` - One of the four terminals.
    ///
    /// # Returns
    ///
    /// `true` if the list for that terminal names the node; `false` for a
    /// name that is not one of the four.
    pub fn exports(&self, node: &str, terminal: &str) -> bool {
        self.each().iter().any(|(at, nodes)| {
            *at == terminal && nodes.iter().any(|n| n == node)
        })
    }

    /// Whether nothing at all is exported.
    ///
    /// # Returns
    ///
    /// `true` for four empty lists.
    pub fn is_empty(&self) -> bool {
        self.each().iter().all(|(_, nodes)| nodes.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both spellings say the same thing.
    #[test]
    fn exports_read_the_same_in_both_spellings() {
        let yaml: Exports = serde_norway::from_str(
            "world-in: [temp1, temp2]\nworld-out: [ambient]\n",
        )
        .expect("the YAML reads");
        let xml: Exports = quick_xml::de::from_str(
            "<exports><world-in>temp1</world-in><world-in>temp2</world-in>\
             <world-out>ambient</world-out></exports>",
        )
        .expect("the XML reads");
        assert_eq!(yaml, xml);
        assert!(yaml.exports("temp2", "world-in"));
        assert!(!yaml.exports("temp2", "world-out"));
        assert!(!yaml.exports("ambient", "moc-out"));
        assert!(Exports::default().is_empty());
    }

    /// A misspelled terminal is refused, rather than read as a list nothing
    /// looks at.
    #[test]
    fn an_unknown_terminal_is_refused() {
        assert!(
            serde_norway::from_str::<Exports>("world_in: [temp1]\n").is_err()
        );
    }
}
