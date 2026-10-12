//! A session: the document, its history, and keeping both on disk.
//!
//! Every edit is written as it is made, which is what was asked for and is
//! also the only arrangement that does not have to ask a person whether they
//! want to lose their work when something goes wrong. What that costs is a
//! write per edit, which for a configuration file is nothing.
//!
//! So saving is never something to remember, and [`Editor::save`] exists
//! anyway. It writes every file of the configuration now, which is the three
//! things the ordinary case does not cover: retrying a write that failed,
//! putting right a set of files left half new by a failure between two of
//! them, and answering somebody who has been taught by every other program to
//! save their work. The way out of the program does the same for anything the
//! files do not have -- see [`Editor::unsaved`].
//!
//! Two files are written, and both matter:
//!
//! * the **document**, which is the configuration a vehicle flies. This is the
//!   file that must never be damaged, and [`crate::save`] is how.
//! * the **journal** beside it, which is the history. Without it a session
//!   recovered after a crash would come back with nothing to undo -- the work
//!   would be there and the ability to take it back would not.
//!
//! The document is written first and the journal second. If the process dies
//! between the two, the journal is missing its last edit: recovery then shows
//! a document one edit ahead of its history, which costs one undo and loses no
//! work. The other order would write a journal claiming an edit the document
//! does not have, and recovery would replay it onto a document that already
//! had it. Of the two, losing one undo is the better failure, so that is the
//! order.

use std::path::{Path, PathBuf};

use crate::document::{Document, Fault};
use crate::journal::{Edit, Journal};
use crate::save::{write_atomically, SaveError};

/// What the editor has open.
pub struct Editor {
    document: Document,
    journal: Journal,
    path: PathBuf,
    /// Whether the document has changes the files do not have.
    ///
    /// Normally false, and that is the point: every edit is written as it is
    /// made, so the files are current. It goes true when a write fails -- a
    /// full device, a directory gone read-only, a name whose extension says
    /// nothing -- and stays true until one succeeds. What it is for is the
    /// Save button and the write on the way out, which are both "try that
    /// again now": the first when somebody has fixed whatever was wrong, the
    /// second so that a session is not left with work only in memory.
    unsaved: bool,
    /// Why the last write failed, if it did.
    ///
    /// Held rather than returned and forgotten, because the window shows it --
    /// in red, which is what red is for -- until a write succeeds. An editor
    /// that reported a failed save once, in a message that has since gone, is
    /// an editor that lets a person keep working into a file that is not being
    /// written.
    trouble: Option<String>,
}

impl Editor {
    /// Start a session on `path`, which need not exist yet.
    pub fn new(path: &Path) -> Self {
        Editor {
            document: Document::new(),
            journal: Journal::new(),
            path: path.to_path_buf(),
            unsaved: false,
            trouble: None,
        }
    }

    /// Open a configuration: the document from the file, the history from
    /// beside it.
    ///
    /// Two files and two different questions. *What is the configuration* is
    /// answered by the file, always, because the file is current: every edit is
    /// written as it is made, so the file already holds the result of every
    /// edit the journal describes. *What can be taken back* is answered by the
    /// journal, which is how undo reaches past the end of the last session and
    /// past a crash.
    ///
    /// It used to replay the journal instead, onto an empty document, and
    /// return that -- which was right while the editor could not read a file at
    /// all, because then the journal was the only record of anything. It became
    /// wrong the moment the editor could open a configuration somebody else had
    /// written: a session that moved one node of a twelve-node graph left a
    /// journal holding one edit, and reopening the file rebuilt the document
    /// from that one edit. Eleven nodes and every link went missing, and the
    /// next edit wrote the loss to disk.
    ///
    /// Replaying the journal *on top of* the file would be wrong in the other
    /// direction: the file is written after every edit, so each edit would be
    /// applied a second time and a session that added a node would reopen with
    /// two of it.
    ///
    /// What the split costs is the one case where the two disagree: a process
    /// that died between writing the document and writing the journal leaves
    /// the journal one edit short, so that edit is in the configuration and
    /// cannot be undone. That is the trade the write order was chosen for --
    /// see the top of this file -- and losing one undo is the better failure.
    ///
    /// A file that cannot be read is said so and opened as nothing: an editor
    /// that refused to start on a damaged file would be an editor that cannot
    /// be used to fix one.
    pub fn open(path: &Path) -> Self {
        let mut editor = Editor::new(path);

        match crate::read::read(path) {
            Ok(document) => editor.document = document,
            Err(why) => {
                editor.trouble =
                    Some(format!("{} cannot be read: {why}", path.display()));
                // No history either: a journal belongs to the document it
                // describes, and the document is not the one that was meant.
                return editor;
            }
        }

        // A journal that cannot be read costs the undo and not the work, so it
        // is passed over in silence rather than reported: there is nothing the
        // person at the keyboard can do about it, and the thing they came to
        // edit is open and correct.
        if let Ok(text) = std::fs::read_to_string(journal_beside(path)) {
            if let Ok(journal) = serde_yaml::from_str::<Journal>(&text) {
                editor.journal = journal;
            }
        }
        editor
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn faults(&self) -> Vec<Fault> {
        self.document.faults()
    }

    /// What the window shows in red, if anything.
    pub fn trouble(&self) -> Option<&str> {
        self.trouble.as_deref()
    }

    /// Whether anything has been changed that the files do not have.
    pub fn unsaved(&self) -> bool {
        self.unsaved
    }

    /// Where the history is kept, for saying so when the configuration itself
    /// cannot be written.
    pub fn journal_path(&self) -> PathBuf {
        journal_beside(&self.path)
    }

    /// Write every file of the configuration now.
    ///
    /// There is nothing to do here in the ordinary case -- every edit is
    /// written as it is made -- and it is worth having all the same, for three
    /// things that the ordinary case does not cover.
    ///
    /// A write that failed can be retried. A full device or a directory
    /// somebody had made read-only leaves the document in memory and the
    /// reason in red; once that is put right, this is how the work reaches the
    /// disk without having to make another edit.
    ///
    /// A configuration written as several files can be left half new by a
    /// power failure between two of them. Nothing can undo that afterwards, so
    /// what puts it right is writing them all again.
    ///
    /// And a person who has been taught by every other program to save their
    /// work can save it, and be told plainly that it is saved. "There is no
    /// Save command" is a true description of what the editor does and a poor
    /// answer to somebody who wants to be sure.
    pub fn save(&mut self) {
        self.persist();
    }

    /// Apply an edit, record it, and write both files.
    ///
    /// The document is changed whether or not the write succeeds: refusing the
    /// edit would mean a full device makes the editor unusable rather than
    /// unable to save, and the person at the keyboard is better served by
    /// being told plainly and allowed to carry on until they can fix it.
    pub fn edit(&mut self, edit: Edit) {
        self.document.apply(&edit);
        self.journal.record(edit);
        self.persist();
    }

    /// Take back the last edit.
    pub fn undo(&mut self) -> bool {
        match self.journal.undo() {
            Some(inverse) => {
                self.document.apply(&inverse);
                self.persist();
                true
            }
            None => false,
        }
    }

    /// Put back an edit that was taken off.
    pub fn redo(&mut self) -> bool {
        match self.journal.redo() {
            Some(edit) => {
                self.document.apply(&edit);
                self.persist();
                true
            }
            None => false,
        }
    }

    /// How deep the undo currently goes, for the window to show.
    pub fn undo_depth(&self) -> usize {
        self.journal.depth()
    }

    /// Write the document and then the journal. See the module comment for
    /// why that order.
    fn persist(&mut self) {
        self.trouble = None;
        // Until a write succeeds, there is something in the document that no
        // file has. Set before the writing rather than after a failure, so
        // that a path which returns early still leaves it true.
        self.unsaved = true;

        // Every file the document is written as: the top diagram and one per
        // subdiagram, each in whichever format its name asks for.
        let files = match self.document.files(&self.path) {
            Ok(files) => files,
            Err(why) => {
                self.trouble = Some(why);
                return;
            }
        };

        // Each is written atomically, so none can be left half-written. What
        // cannot be promised across several files is that they all change
        // together: a power failure between two of them leaves one new and one
        // old. Nothing can undo that afterwards -- the journal is history and
        // not a copy of the document -- so what puts it right is writing them
        // all again, which the next edit does and the Save button does on
        // demand. A failure here stops before the journal is touched, so the
        // history never gets ahead of the configuration.
        for (path, text) in &files {
            if let Err(why) = write_atomically(path, text.as_bytes()) {
                self.trouble = Some(describe(&why));
                return;
            }
        }
        // The configuration is on the disk. Whatever happens to the history
        // below, there is nothing left in the document that no file has.
        self.unsaved = false;

        match serde_yaml::to_string(&self.journal) {
            Ok(text) => {
                if let Err(why) = write_atomically(&journal_beside(&self.path), text.as_bytes()) {
                    // The configuration is safely written; what is lost is the
                    // ability to undo past a crash. Worth saying and not worth
                    // treating as a failed save.
                    self.trouble = Some(format!(
                        "the configuration is saved, but its history is not: {}",
                        describe(&why)
                    ));
                }
            }
            Err(why) => {
                self.trouble = Some(format!(
                    "the configuration is saved, but its history could not be \
                     written down: {why}"
                ));
            }
        }
    }
}

/// What the window says about a failed write.
fn describe(why: &SaveError) -> String {
    format!("{why}")
}

/// The journal's name, which is the document's with a suffix.
///
/// Beside the document rather than in a state directory somewhere: a
/// configuration copied to another machine should take its history with it,
/// and a person who finds the pair in a directory can see that they belong
/// together.
fn journal_beside(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tcsgui-history");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(name: &str, node_type: &str) -> Edit {
        Edit::AddNode {
            name: name.to_string(),
            node_type: node_type.to_string(),
            home: None,
        }
    }

    /// Every edit reaches the disk as it is made.
    #[test]
    fn an_edit_is_on_disk_before_the_next_one_is_made() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.yaml");
        let mut editor = Editor::new(&path);

        editor.edit(add("temp_fore", "i2c"));
        let after_one = std::fs::read_to_string(&path).expect("the file exists already");
        assert!(after_one.contains("temp_fore"), "{after_one}");
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        editor.edit(add("temp_aft", "i2c"));
        let after_two = std::fs::read_to_string(&path).unwrap();
        assert!(after_two.contains("temp_aft"), "{after_two}");
    }

    /// Saving when there is nothing to save writes the files anyway.
    ///
    /// Which is the point of the button: it is what retries a write that
    /// failed, and what puts right a set of files left half new by a failure
    /// between two of them. Nothing to save is the ordinary case, because
    /// every edit is written as it is made.
    #[test]
    fn saving_writes_every_file_whether_or_not_anything_is_owed() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        let mut editor = Editor::new(&path);

        editor.edit(add("temp_fore", "i2c"));
        assert!(!editor.unsaved(), "an edit that was written owes nothing");

        // Taken away behind the editor's back, as a half-written set of files
        // would be.
        std::fs::remove_file(&path).expect("the file goes");
        editor.save();
        assert!(path.exists(), "Save did not write the file");
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());
        assert!(std::fs::read_to_string(&path).unwrap().contains("temp_fore"));
    }

    /// A write that failed is owed, and a later save settles it.
    ///
    /// The one way this editor could lose a session's work: a device that
    /// could not be written to, and nobody told. What it does instead is hold
    /// the work, say so in red, and leave a way to write it -- the button, or
    /// the way out of the program.
    #[test]
    fn work_the_files_do_not_have_is_owed_until_it_is_written() {
        use std::os::unix::fs::PermissionsExt;

        let where_ = tempfile::tempdir().expect("a directory");
        let locked = where_.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let path = locked.join("vehicle.yaml");
        let mut editor = Editor::new(&path);

        let shut = |mode: u32| {
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(mode);
            std::fs::set_permissions(&locked, permissions).unwrap();
        };

        shut(0o500);
        editor.edit(add("temp_fore", "i2c"));
        assert!(editor.trouble().is_some(), "a failed write should be said");
        assert!(
            editor.unsaved(),
            "work the files do not have should be owed"
        );
        // And the work is still there: the person at the keyboard has lost
        // nothing but the saving.
        assert_eq!(editor.document().nodes.len(), 1);

        // Put right, and saved without another edit being made -- which is
        // what the button is for, and what the way out of the program does.
        shut(0o700);
        editor.save();
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());
        assert!(!editor.unsaved());
        assert!(std::fs::read_to_string(&path).unwrap().contains("temp_fore"));
    }

    /// A group is written out, and read back.
    ///
    /// A group is one of the four things a configuration is made of, and the
    /// one the editor could only read: a group made in the window had to reach
    /// the file or the nodes in it would name something nothing declared.
    #[test]
    fn a_group_made_in_the_editor_is_written_and_read_back() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        let mut editor = Editor::new(&path);

        editor.edit(Edit::AddGroup {
            name: "cabin_temps".into(),
            interval: "3s".into(),
        });
        editor.edit(add("temp_fore", "i2c"));
        for (attribute, value) in [("group", "cabin_temps"), ("path", "/dev/i2c-1"), ("address", "0x48")] {
            editor.edit(Edit::SetAttribute {
                node: "temp_fore".into(),
                attribute: attribute.into(),
                was: None,
                now: Some(value.into()),
            });
        }
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("cabin_temps"), "{written}");
        assert!(written.contains("3s"), "{written}");

        let reopened = Editor::open(&path);
        assert_eq!(
            reopened.document().groups.get("cabin_temps").map(String::as_str),
            Some("3s"),
            "the group did not survive a round trip"
        );
        assert_eq!(reopened.document().payloads_of("cabin_temps").len(), 1);
    }

    /// A session recovered after a crash has the work *and* the undo.
    ///
    /// Which is the whole reason the journal is written as well as the
    /// document. Simulated by starting a second editor on the same path, which
    /// is what happens when the first one dies.
    #[test]
    fn a_recovered_session_can_still_undo_past_the_crash() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.yaml");

        {
            let mut editor = Editor::new(&path);
            editor.edit(add("a", "i2c"));
            editor.edit(add("b", "spi"));
            editor.edit(add("c", "compute"));
            assert_eq!(editor.undo_depth(), 3);
            // And the process dies here, with no chance to tidy up.
        }

        let mut recovered = Editor::open(&path);
        assert_eq!(
            recovered.document().nodes.len(),
            3,
            "the work did not come back"
        );
        assert_eq!(
            recovered.undo_depth(),
            3,
            "the work came back but the history did not"
        );

        assert!(recovered.undo());
        assert_eq!(recovered.document().nodes.len(), 2);
        assert!(recovered.undo());
        assert!(recovered.undo());
        assert_eq!(recovered.document().nodes.len(), 0);
        assert!(!recovered.undo(), "undo went past the start of the session");
    }

    /// Undo and redo are written as they happen, like any other change.
    #[test]
    fn an_undo_reaches_the_disk_too() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.yaml");
        let mut editor = Editor::new(&path);

        editor.edit(add("a", "i2c"));
        editor.edit(add("b", "spi"));
        editor.undo();

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("name: a"), "{saved}");
        assert!(
            !saved.contains("name: b"),
            "the undone node is still in the file: {saved}"
        );

        editor.redo();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("name: b"), "the redo was not saved: {saved}");
    }

    /// A write that fails is said so, and the session carries on.
    ///
    /// A full device must not cost the person at the keyboard their work or
    /// their ability to keep going; it must cost them nothing but the
    /// knowledge that saving is not happening, which they are given.
    #[test]
    fn a_failed_write_is_reported_and_the_session_survives() {
        let where_ = tempfile::tempdir().expect("a directory");
        let locked = where_.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let path = locked.join("graph.yaml");

        let mut editor = Editor::new(&path);
        editor.edit(add("a", "i2c"));
        assert!(editor.trouble().is_none());

        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o500);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }

        editor.edit(add("b", "spi"));
        let said = editor.trouble().expect("a failed write should be reported");
        assert!(
            said.contains("unchanged"),
            "the report should say the file on disk survived: {said}"
        );

        // And the edit is in the document, so nothing was lost but the save.
        assert_eq!(editor.document().nodes.len(), 2);
        assert_eq!(editor.undo_depth(), 2);

        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }

        // And once it can be written, the trouble clears.
        editor.edit(add("c", "compute"));
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());
        let saved = std::fs::read_to_string(&path).unwrap();
        for name in ["a", "b", "c"] {
            assert!(saved.contains(&format!("name: {name}")), "{saved}");
        }
    }

    /// A journal that is ahead of its document is never written.
    ///
    /// The ordering the module comment argues for, checked rather than
    /// described: if the document write fails, the journal must not record the
    /// edit, or recovery would replay an edit the document already lacks.
    #[test]
    fn a_failed_document_write_does_not_leave_a_journal_ahead_of_it() {
        let where_ = tempfile::tempdir().expect("a directory");
        let locked = where_.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let path = locked.join("graph.yaml");

        let mut editor = Editor::new(&path);
        editor.edit(add("a", "i2c"));

        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o500);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }
        editor.edit(add("b", "spi"));
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }

        // What a recovery would find: one node and one edit of history, which
        // agree with each other. The second edit is lost, which is what a
        // device that could not be written to costs.
        let recovered = Editor::open(&path);
        assert_eq!(recovered.document().nodes.len(), 1);
        assert_eq!(recovered.undo_depth(), 1);
    }

    /// A subdiagram is written to a file of its own, beside the top diagram.
    #[test]
    fn a_subdiagram_is_written_to_its_own_file() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.yaml");
        let mut editor = Editor::new(&path);

        editor.edit(Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });
        editor.edit(Edit::AddNode {
            name: "moc".into(),
            node_type: "moc".into(),
            home: None,
        });
        // A link that crosses the boundary, which was asked for.
        editor.edit(Edit::AddLink {
            link: crate::journal::Link::new("cabin/temp_fore", "moc", None),
        });
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        let top = std::fs::read_to_string(&path).expect("the top diagram");
        let inner =
            std::fs::read_to_string(where_.path().join("cabin.yaml")).expect("the subdiagram");

        // The top file names what it includes and holds its own nodes.
        assert!(top.contains("include:\n  - cabin"), "{top}");
        assert!(top.contains("name: moc"), "{top}");
        assert!(
            !top.contains("name: temp_fore"),
            "the subdiagram's node is in the top file too: {top}"
        );

        // The subdiagram holds its node, and the crossing link is written
        // where its source lives -- qualified at the far end only.
        assert!(inner.contains("name: temp_fore"), "{inner}");
        assert!(
            inner.contains("{ from: temp_fore, to: moc }"),
            "the crossing link should be plain at home and plain at the top, \
             since moc has no subdiagram to qualify it with: {inner}"
        );
        assert!(
            !top.contains("to: moc }"),
            "the crossing link is written twice: {top}"
        );
    }

    /// A subdiagram's own files are written whatever format the parent is.
    #[test]
    fn subdiagrams_follow_the_parent_into_xml() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.xml");
        let mut editor = Editor::new(&path);

        editor.edit(Edit::AddNode {
            name: "cabin".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "temp_fore".into(),
            node_type: "i2c".into(),
            home: Some("cabin".into()),
        });

        let top = std::fs::read_to_string(&path).expect("the top diagram");
        let inner = std::fs::read_to_string(where_.path().join("cabin.xml"))
            .expect("the subdiagram, in XML as its parent is");
        assert!(top.contains("<include>cabin</include>"), "{top}");
        assert!(inner.starts_with("<?xml"), "{inner}");
        assert!(inner.contains("<name>temp_fore</name>"), "{inner}");
    }

    /// An XML name gets an XML file, and the history still works.
    #[test]
    fn a_configuration_named_xml_is_written_as_xml() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.xml");
        let mut editor = Editor::new(&path);

        editor.edit(add("temp_fore", "i2c"));
        let written = std::fs::read_to_string(&path).expect("the file exists");
        assert!(written.starts_with("<?xml"), "{written}");
        assert!(written.contains("<type>i2c</type>"), "{written}");
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        // And the history is beside it as it is for YAML, so undo reaches past
        // a crash whichever format the configuration is written in.
        editor.edit(add("temp_aft", "i2c"));
        let recovered = Editor::open(&path);
        assert_eq!(recovered.undo_depth(), 2);
        assert_eq!(recovered.document().nodes.len(), 2);
    }

    /// A name that says nothing about its format is refused, and said so.
    #[test]
    fn a_file_with_no_usable_extension_is_reported() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.json");
        let mut editor = Editor::new(&path);

        editor.edit(add("a", "i2c"));
        let said = editor.trouble().expect("this should be refused");
        assert!(said.contains(".yaml"), "{said}");
        // The edit is still in the document: the person at the keyboard has
        // lost nothing but the saving, and can say where to put it.
        assert_eq!(editor.document().nodes.len(), 1);
    }

    /// The journal sits beside the document it belongs to.
    #[test]
    fn the_history_is_named_after_the_document() {
        assert_eq!(
            journal_beside(Path::new("/var/lib/tcs/graph.yaml")),
            PathBuf::from("/var/lib/tcs/graph.yaml.tcsgui-history")
        );
    }
}
