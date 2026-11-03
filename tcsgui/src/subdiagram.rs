//! Subdiagrams: a part of the configuration kept in a file of its own.
//!
//! A vehicle's configuration gets large, and the useful way to divide a large
//! diagram is by the thing it describes -- the zone's thermostat here, the
//! antenna pointing there. So a **subdiagram** is a separate included file,
//! shown in the diagram that includes it as one box: simplified when closed,
//! and opened in a window of its own to be worked on.
//!
//! Three decisions follow from that, and the awkward one is the second.
//!
//! **One document, many views.** The editor holds a single assembled document
//! whatever it was read from, and every open window -- the top diagram and
//! each open subdiagram -- draws from it. That is what keeps them in step: not
//! a synchronising mechanism that could be wrong, but the absence of anything
//! to synchronise. A node edited in a subdiagram window is the same node the
//! top diagram's box is counting, because there is only one of it.
//!
//! **A link may cross a boundary.** That was asked for, and it has a cost
//! worth writing down: a subdiagram file is then not self-contained. It cannot
//! be checked on its own, and a file included by two vehicles may be linked
//! into differently by each. What it buys is the freedom to connect anything
//! to anything without restructuring the diagram first.
//!
//! **A name is the node's own in every file.** The configuration reader,
//! `tcslibgs::graph::Graph::load`, knows a node by its bare name across every
//! file of a configuration -- no two nodes share one, whichever files they
//! are in -- so a link in one file names a node in another just as it names
//! one beside it, and that is how every file is written. The editor holds a
//! node under `subdiagram/node`, which says which file it is drawn in, but
//! writes only the `node`.
//!
//! **A subdiagram is named by its include.** Its name is the one the top
//! file's include list gives it: the file's path from the top file's
//! directory without an extension, `zone` beside it or
//! `../thermostat-common/thermostat` elsewhere. A node's own name has no `/`
//! in it, so the last `/` of a held name is the one that divides the two.

/// Where a node lives: the top diagram, or one subdiagram.
///
/// A `String` rather than an index, because what a subdiagram is called is its
/// file's name and that is what the configuration says. The top diagram is
/// `None` -- it has no file of its own in the sense that matters here, being
/// the file that includes the others.
pub type Home = Option<String>;

/// A name as a link writes it, split into where and what.
///
/// `in_` is the subdiagram the name reaches into, or `None` for a name that
/// did not qualify itself -- which means the node is in the same file as the
/// link, whichever file that is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualified {
    pub in_: Home,
    pub node: String,
}

impl Qualified {
    /// Reads a name as the editor holds it.
    ///
    /// The last separator divides it: `zone/temp_1` is in `zone`, and
    /// `../common/zone/temp_1` in `../common/zone`, a subdiagram's name being
    /// a path and a node's name having no `/` in it. A name with nothing
    /// either side of its last separator is taken as a plain one. That keeps
    /// the function total -- a name is always readable as something -- and
    /// leaves saying what is wrong with it to the validation, which has
    /// somewhere to report it.
    ///
    /// # Arguments
    ///
    /// * `name` - The name as the editor holds it, such as `zone/temp_1` or
    ///            `temp_1`.
    ///
    /// # Returns
    ///
    /// The subdiagram the name reaches into, if it says one, and the node's
    /// name.
    pub fn parse(name: &str) -> Qualified {
        match name.rsplit_once('/') {
            Some((home, node)) if !home.is_empty() && !node.is_empty() => {
                Qualified {
                    in_: Some(home.to_string()),
                    node: node.to_string(),
                }
            }
            _ => Qualified {
                in_: None,
                node: name.to_string(),
            },
        }
    }

    /// How this name is written in a file.
    ///
    /// The node's own name, in whichever file: the reader knows a node by it
    /// across every file of a configuration, and a name written
    /// `zone/temp_1` would be a node nobody has. See the module's notes.
    ///
    /// # Returns
    ///
    /// The name as a file writes it.
    pub fn written(&self) -> String {
        self.node.clone()
    }

    /// The name this is, seen from outside every file.
    ///
    /// What the editor holds a node under, so that two subdiagrams may both
    /// have a `temp_1` without either of them having to be renamed.
    ///
    /// # Returns
    ///
    /// `home/node` for a node in a subdiagram, or the plain name for one in the
    /// top diagram.
    pub fn absolute(&self) -> String {
        match &self.in_ {
            Some(home) => format!("{home}/{}", self.node),
            None => self.node.clone(),
        }
    }

    /// A name in a particular home.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name within its file.
    /// * `home` - The subdiagram it is in, or `None` for the top diagram.
    ///
    /// # Returns
    ///
    /// The qualified name.
    pub fn of(node: &str, home: &Home) -> Qualified {
        Qualified {
            in_: home.clone(),
            node: node.to_string(),
        }
    }
}

/// The file a subdiagram is kept in, given the name it goes by.
///
/// The name is the file's path from the including diagram's directory without
/// its extension, so `zone` is `zone.ytcs` beside the diagram that includes it,
/// and `../common/zone` is `zone.ytcs` in `common`, a directory beside the
/// diagram's. Which extension is the including file's: a configuration written
/// in XML keeps its subdiagrams in XML, because a set of files half in one
/// language and half in the other is a thing nobody meant and everybody would
/// have to remember.
///
/// # Arguments
///
/// * `name` - The subdiagram's name, which is its include name.
/// * `like` - The path of the including file, whose directory and extension the
///            subdiagram's file takes; YAML if it has no extension.
///
/// # Returns
///
/// The path of the subdiagram's file.
pub fn file_for(name: &str, like: &std::path::Path) -> std::path::PathBuf {
    let extension = like
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("ytcs")
        .to_string();
    let mut path = like.to_path_buf();
    path.set_file_name(format!("{name}.{extension}"));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A name is written as the node's own, whichever file it is in and
    /// whichever file names it, because that is how the reader knows it.
    #[test]
    fn a_name_is_written_as_the_nodes_own() {
        let inside = Qualified::of("temp_1", &Some("zone".to_string()));
        assert_eq!(inside.written(), "temp_1");
        assert_eq!(inside.absolute(), "zone/temp_1");

        let top = Qualified::of("gateway", &None);
        assert_eq!(top.written(), "gateway");
    }

    /// Reading a name back gives what was written.
    #[test]
    fn a_qualified_name_reads_back_as_itself() {
        for (text, in_, node) in [
            ("zone/temp_1", Some("zone"), "temp_1"),
            ("gateway", None, "gateway"),
            // A subdiagram named by a path, as one in another directory is.
            (
                "../thermostat-common/thermostat/temp1",
                Some("../thermostat-common/thermostat"),
                "temp1",
            ),
        ] {
            let read = Qualified::parse(text);
            assert_eq!(read.in_.as_deref(), in_, "{text}");
            assert_eq!(read.node, node, "{text}");
            assert_eq!(read.absolute(), text);
        }
    }

    /// Two subdiagrams may each have a node of the same name.
    ///
    /// The reason names are qualified rather than required to be unique: a
    /// subdiagram is a separate file so that it can be reused, and a rule that
    /// made two copies collide would take the reuse away.
    #[test]
    fn the_same_name_in_two_subdiagrams_is_two_nodes() {
        let one = Qualified::of("temp_1", &Some("zone".to_string()));
        let two = Qualified::of("temp_1", &Some("bay".to_string()));

        assert_ne!(one.absolute(), two.absolute());
        assert_eq!(one.absolute(), "zone/temp_1");
        assert_eq!(two.absolute(), "bay/temp_1");
    }

    /// A name nobody can mean anything by is still readable.
    ///
    /// Total on purpose: saying what is wrong with a name belongs to the
    /// validation, which has a node to attach the complaint to, and a parse
    /// that could fail would have the complaint arrive from somewhere with
    /// nothing to attach it to.
    #[test]
    fn an_unusable_name_is_read_as_a_plain_one() {
        for odd in ["/temp_1", "zone/", "/", ""] {
            let read = Qualified::parse(odd);
            assert_eq!(read.in_, None, "{odd:?}");
            assert_eq!(read.node, odd, "{odd:?}");
        }

        // More than one separator: the last divides it, everything before
        // being the subdiagram's name.
        let read = Qualified::parse("zone/inner/temp");
        assert_eq!(read.in_.as_deref(), Some("zone/inner"));
        assert_eq!(read.node, "temp");
    }

    /// A subdiagram's file is at its name from its parent's directory, in the
    /// parent's format.
    #[test]
    fn a_subdiagram_is_a_file_beside_its_parent() {
        assert_eq!(
            file_for("zone", Path::new("/var/lib/tcs/vehicle.ytcs")),
            Path::new("/var/lib/tcs/zone.ytcs")
        );
        // XML keeps XML: a set of files half in each language is a thing
        // nobody meant.
        assert_eq!(
            file_for("zone", Path::new("/var/lib/tcs/vehicle.xtcs")),
            Path::new("/var/lib/tcs/zone.xtcs")
        );
        // And one named by a path is that path from the parent's directory,
        // written as it is named.
        assert_eq!(
            file_for("../common/zone", Path::new("/var/lib/tcs/vehicle.ytcs")),
            Path::new("/var/lib/tcs/../common/zone.ytcs")
        );
    }
}
