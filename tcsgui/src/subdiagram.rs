//! Subdiagrams: a part of the configuration kept in a file of its own.
//!
//! A vehicle's configuration gets large, and the useful way to divide a large
//! diagram is by the thing it describes -- the cabin's thermostat here, the
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
//! **A name that crosses is qualified.** Since a link may name a node in
//! another file, names have to be unambiguous across the whole assembly -- and
//! requiring them to be globally unique would make including one subdiagram
//! twice impossible, which is exactly the reuse a separate file is for. So a
//! node is named plainly within its own file and `subdiagram/node` from
//! outside it. A name is local until it has to travel.

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
    /// Reads a name as a link wrote it.
    ///
    /// One separator and no more: `cabin/temp_1` reaches into `cabin`, and
    /// a name with two separators in it is a name this cannot mean anything
    /// by, so the whole of it after the first is taken as the node's. That
    /// keeps the function total -- a name is always readable as something --
    /// and leaves saying so to the validation, which has somewhere to report
    /// it.
    ///
    /// # Arguments
    ///
    /// * `name` - The name as a link wrote it, such as `cabin/temp_1` or
    ///            `temp_1`.
    ///
    /// # Returns
    ///
    /// The subdiagram the name reaches into, if it says one, and the node's
    /// name.
    pub fn parse(name: &str) -> Qualified {
        match name.split_once('/') {
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

    /// How this name is written in a link that lives in `from`.
    ///
    /// Plain where the two are in the same file, qualified where they are not.
    /// This is the half that keeps a subdiagram's own links readable: inside
    /// `cabin.yaml` a link says `temp_1`, and only a link reaching out of
    /// it says `cabin/temp_1`.
    ///
    /// # Arguments
    ///
    /// * `from` - The subdiagram the link lives in, or `None` for the top
    ///            diagram.
    ///
    /// # Returns
    ///
    /// The name as the link should write it.
    pub fn written_from(&self, from: &Home) -> String {
        match (&self.in_, from) {
            (a, b) if a == b => self.node.clone(),
            (Some(home), _) => format!("{home}/{}", self.node),
            (None, _) => self.node.clone(),
        }
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
/// The name is the file's stem, so `cabin` is `cabin.yaml` beside the diagram
/// that includes it. Which extension is the including file's: a configuration
/// written in XML keeps its subdiagrams in XML, because a set of files half in
/// one language and half in the other is a thing nobody meant and everybody
/// would have to remember.
///
/// # Arguments
///
/// * `name` - The subdiagram's name, which is the file's stem.
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
        .unwrap_or("yaml")
        .to_string();
    let mut path = like.to_path_buf();
    path.set_file_name(format!("{name}.{extension}"));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A name is local until it has to travel.
    #[test]
    fn a_name_is_plain_at_home_and_qualified_away_from_it() {
        let inside = Qualified::of("temp_1", &Some("cabin".to_string()));

        // Written from inside the same subdiagram: plain.
        assert_eq!(inside.written_from(&Some("cabin".to_string())), "temp_1");

        // Written from the top diagram, or from another subdiagram: qualified.
        assert_eq!(inside.written_from(&None), "cabin/temp_1");
        assert_eq!(
            inside.written_from(&Some("antenna".to_string())),
            "cabin/temp_1"
        );

        // And a node of the top diagram is plain from anywhere, there being no
        // file name to qualify it with.
        let top = Qualified::of("moc", &None);
        assert_eq!(top.written_from(&None), "moc");
        assert_eq!(top.written_from(&Some("cabin".to_string())), "moc");
    }

    /// Reading a name back gives what was written.
    #[test]
    fn a_qualified_name_reads_back_as_itself() {
        for (text, in_, node) in [
            ("cabin/temp_1", Some("cabin"), "temp_1"),
            ("moc", None, "moc"),
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
        let one = Qualified::of("temp_1", &Some("cabin".to_string()));
        let two = Qualified::of("temp_1", &Some("bay".to_string()));

        assert_ne!(one.absolute(), two.absolute());
        assert_eq!(one.absolute(), "cabin/temp_1");
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
        for odd in ["/temp_1", "cabin/", "/", ""] {
            let read = Qualified::parse(odd);
            assert_eq!(read.in_, None, "{odd:?}");
            assert_eq!(read.node, odd, "{odd:?}");
        }

        // More than one separator: everything after the first is the node's,
        // which is a name no node has, so the link dangles and is reported.
        let read = Qualified::parse("cabin/inner/temp");
        assert_eq!(read.in_.as_deref(), Some("cabin"));
        assert_eq!(read.node, "inner/temp");
    }

    /// A subdiagram's file sits beside its parent, in the parent's format.
    #[test]
    fn a_subdiagram_is_a_file_beside_its_parent() {
        assert_eq!(
            file_for("cabin", Path::new("/var/lib/tcs/vehicle.yaml")),
            Path::new("/var/lib/tcs/cabin.yaml")
        );
        // XML keeps XML: a set of files half in each language is a thing
        // nobody meant.
        assert_eq!(
            file_for("cabin", Path::new("/var/lib/tcs/vehicle.xml")),
            Path::new("/var/lib/tcs/cabin.xml")
        );
    }
}
