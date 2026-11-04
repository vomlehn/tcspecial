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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::document::{Document, Fault};
use crate::faithful::Written;
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
    /// What the last command did, where it did something worth saying.
    ///
    /// Beside `trouble` and not in it: the window has one red text and this is
    /// not red. Generating the Rust writes a file somebody named and nothing
    /// else changes, so silence would leave them wondering whether the menu
    /// item did anything.
    said: Option<String>,
    /// The simulator file this session was given, whether or not it could be
    /// opened.
    ///
    /// Apart from `document.simulator`, which is what was read from it and is
    /// nothing when the file was refused: the file is still the one the
    /// session is about, and it is what `tcsverify` is asked about as the
    /// editor starts -- see [`Editor::files`].
    simulator_path: Option<PathBuf>,
    /// Whether this session may change anything: false for one started with
    /// `--read-only`.
    ///
    /// Held here, where every change is made and every file is written,
    /// rather than only in the window: a window can grey a button, and the
    /// one way to be sure a session started to look writes nothing is for the
    /// thing that writes to refuse. See [`Editor::set_read_only`].
    read_only: bool,
    /// Each file as the session last read or wrote it: what the editor
    /// wrote, or would have written, for it then, and what it held.
    ///
    /// What lets a save change only what was edited: the next text for a
    /// file is the file as it was with the edit merged into it, rather than
    /// the editor's own text throughout. See `crate::faithful`. A file the
    /// session has neither read nor written has nothing here, and is
    /// written as the editor writes it.
    written: BTreeMap<PathBuf, Written>,
}

impl Editor {
    /// Starts a session on `path`, which need not exist yet.
    ///
    /// # Arguments
    ///
    /// * `path` - The configuration file the session writes to.
    ///
    /// # Returns
    ///
    /// The session, with an empty document and history, nothing unsaved and
    /// nothing wrong.
    pub fn new(path: &Path) -> Self {
        Editor {
            document: Document::new(),
            journal: Journal::new(),
            path: path.to_path_buf(),
            unsaved: false,
            trouble: None,
            said: None,
            simulator_path: None,
            read_only: false,
            written: BTreeMap::new(),
        }
    }

    /// Make this session one that looks and does not touch: no edit is made,
    /// no file is written -- not the configuration, not the simulator's, not
    /// the history -- and no history is thrown away.
    ///
    /// What is opened is still read and checked as it always is, and what is
    /// wrong with it is still said: a configuration somebody opened to look at
    /// is one they want to know the state of. What came in is still settled
    /// into its panes on the screen, which changes nothing on the disk.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    /// Opens a configuration: the document from the file, the history from
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
    ///
    /// `dividers` is where the three working panes meet, and it is here
    /// because what is read has to obey the rule the editor enforces: a
    /// payload the file draws in the vehicle working pane is brought into its
    /// own. See [`Document::settle`], which says what that is and is not.
    ///
    /// # Arguments
    ///
    /// * `path`     - The configuration file, which need not exist yet.
    /// * `dividers` - Where the three working panes meet.
    ///
    /// # Returns
    ///
    /// The session. What stopped the file being read, if anything, is in its
    /// trouble.
    pub fn open(path: &Path, dividers: crate::pane::Dividers) -> Self {
        let mut editor = Editor::new(path);

        match crate::read::read(path) {
            Ok(mut document) => {
                document.settle(dividers);
                // What the editor would write for the files as they are,
                // taken after settling, so that a node settling moved is not
                // an edit: its line is written as the file had it until
                // somebody moves the node. Where settling puts a node
                // depends on the window's panes, which are not a fact about
                // the file, and a save writes only what was edited.
                if let Ok(files) = document.files(path) {
                    editor.remember(files);
                }
                editor.document = document;
            }
            Err(why) => {
                editor.trouble =
                    Some(format!("{} cannot be read: {why}", path.display()));
                // No history either: a journal belongs to the document it
                // describes, and the document is not the one that was meant.
                return editor;
            }
        };

        // A journal that cannot be read costs the undo and not the work, so it
        // is passed over in silence rather than reported: there is nothing the
        // person at the keyboard can do about it, and the thing they came to
        // edit is open and correct. See `crate::history`, which is where it
        // lives and how long for.
        if let Some(journal) = crate::history::read(path) {
            editor.journal = journal;
        }
        editor
    }

    /// Also opens the simulator's configuration, from a second file.
    ///
    /// Part of the same session on purpose: the two files are edited
    /// together, checked together and must agree, so one Save writes both and
    /// one Undo takes back whichever of them the last edit touched. A file
    /// that cannot be read is said so and opened as nothing, as the
    /// configuration is: an editor that refused to start on a damaged file
    /// would be an editor that cannot be used to fix one.
    ///
    /// # Arguments
    ///
    /// * `path` - The simulator configuration file, which must be named `.yaml`
    ///            and need not exist yet.
    pub fn open_simulator(&mut self, path: &Path) {
        self.simulator_path = Some(path.to_path_buf());

        // A name that is not YAML is refused outright, and nothing is opened
        // for it: the editor writes a simulator configuration as YAML, so
        // adopting such a name would mean writing YAML to a file called
        // something else at the next Save. The configuration itself is still
        // edited; what is lost is the simulator's half of the session, which
        // is what the message says.
        if let Err(why) = crate::sim::must_be_yaml(path) {
            self.trouble = Some(why);
            self.document.simulator = None;
            return;
        }

        match crate::sim::read(path) {
            Ok(simulator) => {
                // A file not there yet is begun with an entry for each
                // payload: see `crate::sim::Simulator::begun`.
                let simulator = if path.exists() {
                    simulator
                } else {
                    crate::sim::Simulator::begun(
                        path,
                        &self.document.simulated_payloads(),
                    )
                };
                // Every simulated payload has to name a payload of the
                // configuration: that name is the whole of how the two files
                // are joined, so one that names nothing is a set of settings
                // that drives nothing -- which is what a misspelling looks
                // like. Refused rather than adopted, because adopting it
                // means writing the file back on the next edit with the
                // stray entry still in it and nothing said.
                //
                // By each payload's own name, wherever it is drawn: the file
                // names a payload in an included file as it names one in the
                // top file, the reader allowing no two nodes one name across
                // all of them. See `crate::sim::Simulator::key_of`.
                let stray: Vec<&String> = simulator
                    .payloads
                    .keys()
                    .filter(|name| {
                        !self.document.nodes.iter().any(|node| {
                            crate::pane::is_payload(&node.node_type)
                                && &&node.name == name
                        })
                    })
                    .collect();
                if stray.is_empty() {
                    self.document.simulator = Some(simulator);
                    // And what the editor would write for it as it is.
                    let file = self.document.simulator_file();
                    self.remember(file.into_iter().collect());
                } else {
                    self.trouble = Some(format!(
                        "{}: {} names no payload of {}; a simulated payload \
                         names the payload it stands in for, so the two files \
                         are joined by the name and this one joins nothing",
                        path.display(),
                        said_list(&stray),
                        self.path.display()
                    ));
                    self.document.simulator = None;
                }
            }
            Err(trouble) => {
                self.trouble = Some(format!(
                    "{} cannot be read: {}",
                    path.display(),
                    trouble.why()
                ));
                // An empty one in its place where the file cannot be read at
                // all, so that the windows have somewhere to put what
                // somebody types rather than being unreachable for the rest
                // of the session -- and nothing at all where the file parses
                // but says something the two files cannot both mean: that one
                // would be written back with half of itself missing. See
                // `crate::sim::Trouble`.
                self.document.simulator = trouble
                    .leaves_somewhere_to_type()
                    .then(|| crate::sim::Simulator::new(path));
            }
        }
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

    /// The configuration's file, and the simulator's where the session was
    /// given one: the two `tcsverify` would be asked about.
    pub fn files(&self) -> (&Path, Option<&Path>) {
        (&self.path, self.simulator_path.as_deref())
    }

    /// What the window shows in red, if anything.
    pub fn trouble(&self) -> Option<&str> {
        self.trouble.as_deref()
    }

    /// What the last command did, where it said anything.
    pub fn said(&self) -> Option<&str> {
        self.said.as_deref()
    }

    /// Whether anything has been changed that the files do not have.
    pub fn unsaved(&self) -> bool {
        self.unsaved
    }

    /// Where the history is kept, for saying so when the configuration itself
    /// cannot be written.
    ///
    /// In the user's state directory and not beside the configuration: see
    /// `crate::history`. `None` where there is nowhere to keep one at all.
    pub fn journal_path(&self) -> Option<PathBuf> {
        crate::history::journal_for(&self.path)
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

    /// Applies an edit, records it, and writes both files.
    ///
    /// The document is changed whether or not the write succeeds: refusing the
    /// edit would mean a full device makes the editor unusable rather than
    /// unable to save, and the person at the keyboard is better served by
    /// being told plainly and allowed to carry on until they can fix it.
    ///
    /// # Arguments
    ///
    /// * `edit` - The edit to make. Ignored in a read-only session.
    pub fn edit(&mut self, edit: Edit) {
        // A session that may not change anything takes no edit, whatever
        // asked for it: the window greys what would make one, and this is
        // what holds when something it did not grey asks anyway.
        if self.read_only {
            return;
        }
        self.document.apply(&edit);
        self.journal.record(edit);
        self.persist();
    }

    /// Renames the node in this position, as a repair rather than an edit.
    ///
    /// For the one thing that cannot be done by [`Edit::RenameNode`]: telling
    /// two nodes that share a name apart. A name is how an edit says which
    /// node it means, so an edit cannot mean "the second of the two" -- see
    /// [`Document::rename_at`], which is the arithmetic.
    ///
    /// Not recorded in the history, which is the decision worth stating. A
    /// file with two nodes of one name is a file nothing can fly: the editor
    /// cannot save a graph that names a node twice without the next session
    /// facing the same question, and tcsverify refuses it. So putting it right
    /// is not an edit somebody made to a drawing -- it is the file being made
    /// readable -- and an undo that put the duplicate back would write a
    /// broken configuration to the disk, every edit being saved as it is made.
    /// Undo takes back what was drawn; this was not drawn.
    ///
    /// Written at once, for the same reason: the document and the file must
    /// not disagree about a name while the session carries on.
    ///
    /// # Arguments
    ///
    /// * `at`  - The node's index in the document's list of nodes.
    /// * `now` - Its new name, within its own subdiagram.
    pub fn settle_name(&mut self, at: usize, now: &str) {
        if self.read_only {
            return;
        }
        self.document.rename_at(at, now);
        self.persist();
    }

    /// Lays every node out afresh: each at the top of its own pane, nothing
    /// covering anything.
    ///
    /// One edit, so one undo puts the whole layout back -- see
    /// [`Edit::Arrange`] -- and nothing at all where there is nothing to move,
    /// because an entry in the history that changes nothing is an undo that
    /// does nothing visible.
    ///
    /// `dividers` is where the panes meet in the window asking, since that is
    /// what says how wide a pane is.
    ///
    /// # Arguments
    ///
    /// * `dividers` - Where the working panes meet in the window asking.
    pub fn arrange(&mut self, dividers: crate::pane::Dividers) {
        let now = self.document.arranged(dividers);
        if now.is_empty() {
            return;
        }
        let was = self.document.where_each_is(&now);
        self.edit(Edit::Arrange { was, now });
    }

    /// Takes back the last edit.
    ///
    /// # Returns
    ///
    /// Whether an edit was undone: false at the start of the history or in a
    /// read-only session.
    pub fn undo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
        match self.journal.undo() {
            Some(inverse) => {
                self.document.apply(&inverse);
                self.persist();
                true
            }
            None => false,
        }
    }

    /// Puts back an edit that was taken off.
    ///
    /// # Returns
    ///
    /// Whether an edit was redone: false at the end of the history or in a
    /// read-only session.
    pub fn redo(&mut self) -> bool {
        if self.read_only {
            return false;
        }
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
        // Nothing reaches the disk from a session that may not change it --
        // not even a Save asked for, which would rewrite the files as they
        // were read.
        if self.read_only {
            return;
        }
        self.trouble = None;
        // What the last command said is about that command and not about this
        // one: an edit after generating the code is not a second report that
        // the code was written.
        self.said = None;
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
        //
        // Each as the file it was with the edit made to it, so that a save
        // changes only what was edited: see `crate::faithful`.
        for (path, now) in files {
            let text = match self.written.get(&path) {
                Some(written) => crate::faithful::text(&path, written, &now),
                None => now.clone(),
            };
            if let Err(why) = write_atomically(&path, text.as_bytes()) {
                self.trouble = Some(describe(&why));
                return;
            }
            self.written.insert(
                path,
                Written {
                    base: now,
                    kept: text,
                },
            );
        }
        // The configuration is on the disk. Whatever happens to the history
        // below, there is nothing left in the document that no file has.
        self.unsaved = false;

        if let Err(why) = crate::history::write(&self.path, &self.journal) {
            // The configuration is safely written; what is lost is the ability
            // to undo past a crash. Worth saying and not worth treating as a
            // failed save.
            self.trouble = Some(format!(
                "the configuration is saved, but its history is not: {why}"
            ));
        }
    }

    /// Holds what the editor writes for each of some files beside what each
    /// file holds, for the next save to merge an edit into: see
    /// [`Editor::written`].
    ///
    /// A file that cannot be read -- one not there yet among them -- is
    /// left out, and is written as the editor writes it.
    ///
    /// # Arguments
    ///
    /// * `files` - Each file's path, with what the editor writes for it.
    fn remember(&mut self, files: Vec<(PathBuf, String)>) {
        for (path, base) in files {
            if let Ok(kept) = std::fs::read_to_string(&path) {
                self.written.insert(path, Written { base, kept });
            }
        }
    }

    /// Throw the history away, for an exit that was asked for.
    ///
    /// A session that ended tidily has no further use for its undo, and a
    /// directory that only ever grows is a directory somebody has to tidy.
    /// What is kept is the history of a session that ended *badly*, which is
    /// the one somebody will want -- see `crate::history`.
    pub fn forget_history(&self) {
        // Another session's history, as far as a read-only one is concerned:
        // it made none and has no business throwing one away.
        if self.read_only {
            return;
        }
        crate::history::forget(&self.path);
    }
}

/// What the window says about a failed write.
fn describe(why: &SaveError) -> String {
    format!("{why}")
}

/// A list of names as a sentence says them.
///
/// One name is quoted; two are joined by "and"; more are a comma list with an
/// "and" before the last. A message that said `["a", "b"]` would be a message
/// written for a programmer.
///
/// # Arguments
///
/// * `names` - The names, in the order they are to be said.
///
/// # Returns
///
/// The names quoted and joined; empty for no names.
fn said_list(names: &[&String]) -> String {
    let quoted: Vec<String> =
        names.iter().map(|name| format!("\"{name}\"")).collect();
    match quoted.len() {
        0 => String::new(),
        1 => quoted[0].clone(),
        2 => format!("{} and {}", quoted[0], quoted[1]),
        _ => format!(
            "{} and {}",
            quoted[..quoted.len() - 1].join(", "),
            quoted[quoted.len() - 1]
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes the edit that adds a node to the top diagram.
    ///
    /// # Arguments
    ///
    /// * `name`      - The new node's name.
    /// * `node_type` - Its type, as the configuration spells it.
    ///
    /// # Returns
    ///
    /// The edit.
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
        let path = where_.path().join("graph.ytcs");
        let mut editor = Editor::new(&path);

        editor.edit(add("temp_1", "i2c"));
        let after_one =
            std::fs::read_to_string(&path).expect("the file exists already");
        assert!(after_one.contains("temp_1"), "{after_one}");
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        editor.edit(add("temp_2", "i2c"));
        let after_two = std::fs::read_to_string(&path).unwrap();
        assert!(after_two.contains("temp_2"), "{after_two}");
    }

    /// A read-only session takes no edit and writes no file, whatever it is
    /// asked to do.
    ///
    /// Every way the editor changes something is asked: an edit, a save, an
    /// undo and a redo, the repair of a duplicated name, an arrangement and
    /// the forgetting of a history. The file is byte for byte what it was, no
    /// history is written beside it, and the document in memory is the one
    /// that was read.
    #[test]
    fn a_read_only_session_changes_nothing() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let by_hand = "\npayloads:\n  - type i2c:\n      - payload: temp_1\n";
        std::fs::write(&path, by_hand).expect("a file");
        let journal = crate::history::journal_for(&path);
        let journal_before = journal.as_ref().map(|j| std::fs::read(j).ok());

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.set_read_only(true);
        let read = editor.document().clone();

        editor.edit(Edit::AddNode {
            name: "added".into(),
            node_type: "compute".into(),
            home: None,
        });
        editor.edit(Edit::SetNodeComment {
            node: "temp_1".into(),
            was: None,
            now: Some("a comment".into()),
        });
        editor.save();
        assert!(!editor.undo());
        assert!(!editor.redo());
        editor.settle_name(0, "renamed");
        editor.arrange(crate::pane::default_dividers());
        editor.forget_history();

        assert_eq!(editor.document(), &read, "the document changed");
        assert_eq!(editor.undo_depth(), 0);
        assert!(!editor.unsaved(), "nothing is waiting to be written");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            by_hand,
            "a read-only session wrote the configuration"
        );
        assert_eq!(
            journal.as_ref().map(|j| std::fs::read(j).ok()),
            journal_before,
            "a read-only session touched the history"
        );
    }

    /// Opening a configuration settles its nodes and writes nothing.
    ///
    /// Both halves matter. A payload the file draws in the vehicle working
    /// pane comes up in its own pane, because that is the rule the editor
    /// enforces everywhere else. And the file is byte for byte what it was:
    /// opening a configuration to look at it is not editing it, and an editor
    /// that rewrote a file under version control by being pointed at it would
    /// be an editor nobody could point at anything. Nor does an edit of
    /// something else write them, a save writing only what was edited; a
    /// corrected position reaches the disk when its node is moved.
    #[test]
    fn opening_settles_the_panes_without_writing_the_file() {
        let dividers = crate::pane::default_dividers();
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");

        // Written by hand, with the payload where a file from before the
        // canvas was divided would have put it.
        let by_hand = "\
gateways:
  - type gateway_radio:
      - gateway: gateway
        address: \"10.0.0.1\"
        port: \"5000\"
        gateway-out: temp_1

payloads:
  - type i2c:
      - payload: temp_1
        path: \"/dev/i2c-1\"
        address: \"0x48\"
        trigger: \"01\"
        payload-out: gateway

tcsgui:
  nodes:
    - { node: gateway, x: 240, y: 40 }
    - { node: temp_1, x: 400, y: 240 }
";
        std::fs::write(&path, by_hand).expect("a file");

        let mut editor = Editor::open(&path, dividers);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());

        // The payload is in the payload working pane.
        let sensor = editor.document().node("temp_1").expect("the sensor");
        assert!(
            sensor.at.0 + crate::style::WIDTH as i32 <= dividers.payloads,
            "the payload opened at {:?}, outside its pane",
            sensor.at
        );
        // At the same height it was drawn at, to the grid: the smallest move
        // that obeys the rule, so a diagram is not rearranged by being opened.
        // Drawn well below the ground link: a node that came home on top of
        // another steps down the grid, and that is a rule of its own rather
        // than this one.
        assert_eq!(sensor.at.1, crate::pane::on_node_grid(240));
        // And the ground link, which the file drew in the vehicle's pane: it
        // belongs in the gateway pane now, so it comes home at the same height.
        let gateway =
            editor.document().node("gateway").expect("the ground link");
        assert!(
            gateway.at.0 >= dividers.vehicle,
            "the ground link opened at {:?}, outside its pane",
            gateway.at
        );
        assert_eq!(gateway.at.1, crate::pane::on_node_grid(40));

        // The file is untouched.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            by_hand,
            "opening the configuration wrote to it"
        );
        // Nothing to undo, either: this is not an edit, and undoing it would
        // put a node back where the editor refuses to put one.
        assert_eq!(editor.undo_depth(), 0);

        // An edit of something else writes that, and not the positions.
        editor.edit(Edit::SetNodeComment {
            node: "temp_1".into(),
            was: None,
            now: Some("the fore sensor".into()),
        });
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            written,
            by_hand.replace(
                "      - payload: temp_1\n",
                "      # the fore sensor\n      - payload: temp_1\n"
            )
        );

        // Moving the node writes where it now is.
        let was = editor.document().node("gateway").expect("it").at;
        let at = (was.0, was.1 + 100);
        editor.edit(Edit::MoveNode {
            node: "gateway".into(),
            was,
            now: at,
        });
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(
            written.contains(&format!(
                "{{ node: gateway, x: {}, y: {} }}",
                at.0, at.1
            )),
            "the settled position was not written: {written}"
        );
        editor.forget_history();
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
        let path = where_.path().join("vehicle.ytcs");
        let mut editor = Editor::new(&path);

        editor.edit(add("temp_1", "i2c"));
        assert!(!editor.unsaved(), "an edit that was written owes nothing");

        // Taken away behind the editor's back, as a half-written set of files
        // would be.
        std::fs::remove_file(&path).expect("the file goes");
        editor.save();
        assert!(path.exists(), "Save did not write the file");
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());
        assert!(std::fs::read_to_string(&path).unwrap().contains("temp_1"));
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
        let path = locked.join("vehicle.ytcs");
        let mut editor = Editor::new(&path);

        let shut = |mode: u32| {
            let mut permissions =
                std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(mode);
            std::fs::set_permissions(&locked, permissions).unwrap();
        };

        shut(0o500);
        editor.edit(add("temp_1", "i2c"));
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
        assert!(std::fs::read_to_string(&path).unwrap().contains("temp_1"));
    }

    /// The simulator's file is written with the configuration's, and read
    /// back.
    ///
    /// One session, two files, one Save: the two are edited together, checked
    /// together and must agree, so a write that reached one of them and not
    /// the other would be the pair disagreeing by the editor's own hand.
    #[test]
    fn the_simulator_file_is_written_beside_the_configuration() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let sim = where_.path().join("vehiclesim.yaml");

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());

        editor.edit(add("payload", "network"));
        editor.edit(Edit::SetSimPayload {
            payload: "payload".into(),
            setting: "packet_interval".into(),
            was: None,
            now: Some("1000".into()),
        });

        // Both files, from the one edit.
        let written =
            std::fs::read_to_string(&sim).expect("the simulator file");
        assert!(written.contains("simulated_payloads"), "{written}");
        assert!(written.contains("payload"), "{written}");
        assert!(written.contains("1000"), "{written}");
        assert!(std::fs::read_to_string(&path).unwrap().contains("payload"));

        // And read back: a second session on the same pair finds what the
        // first left.
        let mut reopened = Editor::open(&path, crate::pane::default_dividers());
        reopened.open_simulator(&sim);
        assert_eq!(
            reopened
                .document()
                .simulator
                .as_ref()
                .expect("the simulator configuration")
                .payloads["payload"]["packet_interval"],
            "1000"
        );

        // The simulator's own reader agrees, which is the point of writing it
        // at all.
        tcslibgs::sim_config::SimConfigFile::load(&sim)
            .expect("the simulator should read what the editor wrote");
    }

    /// A physics component placed, given a figure and removed with its
    /// connections reaches the simulator file each time, and each is taken
    /// back by one undo.
    #[test]
    fn a_physics_component_is_placed_set_and_removed_in_the_world() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let sim = where_.path().join("vehicle-sim.yaml");
        std::fs::write(
            &sim,
            "world:\n  ambient_temp: \"283.15 K\"\n  physics:\n    \
             components:\n      - type thermal_zone:\n          \
             - component: zone\n            capacity: \"20 J/K\"\n            \
             time_constant: \"50 s\"\n",
        )
        .expect("written");
        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        let world = |editor: &Editor| {
            editor
                .document()
                .simulator
                .as_ref()
                .expect("a simulator file")
                .world
                .clone()
        };

        let name = world(&editor).fresh_name("heater");
        assert_eq!(name, "heater_1");
        editor.edit(Edit::AddPhysics {
            name: name.clone(),
            kind: "heater".into(),
        });
        for (setting, value) in [("power", "4 W"), ("voltage", "28 V")] {
            editor.edit(Edit::SetPhysics {
                component: name.clone(),
                setting: setting.into(),
                was: None,
                now: Some(value.into()),
            });
        }
        let written = std::fs::read_to_string(&sim).expect("the file");
        assert!(written.contains("- type heater:"), "{written}");
        assert!(written.contains("power: \"4 W\""), "{written}");
        assert!(world(&editor).problems().is_empty());

        // Joined to the zone, by hand in the file's terms, and removed: the
        // connection goes with it, and one undo brings both back.
        let mut joined = world(&editor);
        joined
            .connections
            .insert(("heater_1.heat".into(), "zone.heat".into()));
        editor.document.simulator.as_mut().expect("one").world = joined;
        let before = world(&editor);
        editor.edit(Edit::RemovePhysics {
            name: name.clone(),
            kind: "heater".into(),
            settings: before.components[&name]
                .settings
                .iter()
                .map(|(a, b)| (a.clone(), b.clone()))
                .collect(),
            connections: before.connections_of(&name),
        });
        assert!(!world(&editor).components.contains_key(&name));
        assert!(world(&editor).connections.is_empty());
        editor.undo();
        assert_eq!(world(&editor), before);

        // And the placing itself, taken back.
        editor.undo();
        editor.undo();
        editor.undo();
        assert!(!world(&editor).components.contains_key(&name));
        tcslibgs::sim_config::SimConfigFile::load(&sim)
            .expect("the simulator reads what the editor wrote");
    }

    /// A simulator file that cannot be read leaves the session usable.
    ///
    /// Said in red, and opened as nothing rather than refused: an editor that
    /// would not start on a damaged file is an editor that cannot be used to
    /// fix one.
    #[test]
    fn a_damaged_simulator_file_is_reported_and_opened_as_nothing() {
        let where_ = tempfile::tempdir().expect("a directory");
        let sim = where_.path().join("vehiclesim.yaml");
        std::fs::write(&sim, "simulated_payloads: [[[\n").expect("a file");

        let mut editor = Editor::open(
            &where_.path().join("vehicle.ytcs"),
            crate::pane::default_dividers(),
        );
        editor.open_simulator(&sim);

        let said = editor.trouble().expect("this should be reported");
        assert!(said.contains("vehiclesim.yaml"), "{said}");
        // And there is somewhere to type, so the windows are not unreachable
        // for the rest of the session.
        assert!(editor.document().simulator.is_some());
    }

    /// A simulator file with two entries of one name is refused, and so is
    /// one naming a payload that is not there.
    ///
    /// Both are files the editor could half-read and write back damaged: a
    /// duplicate collapses to whichever entry was read last, and a stray name
    /// is a set of settings that drives nothing. Neither is adopted, so
    /// neither is overwritten, and the session carries on with the
    /// configuration alone.
    #[test]
    fn a_simulator_file_that_does_not_join_up_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        std::fs::write(
            &path,
            "payloads:\n  - type network:\n      - payload: payload\n",
        )
        .expect("a configuration");

        // Two entries of one name.
        let twice = where_.path().join("twicesim.yaml");
        std::fs::write(
            &twice,
            "simulated_payloads:\n  - type network:\n      \
             - payload: payload\n        simulate: true\n        \
             packet_interval: 500 ms\n      - payload: payload\n        \
             simulate: true\n        packet_interval: 900 ms\n",
        )
        .expect("a simulator file");
        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&twice);
        let said = editor.trouble().expect("this should be reported");
        assert!(said.contains("payload"), "{said}");
        assert!(
            editor.document().simulator.is_none(),
            "a file that cannot be read back is a file nothing should be \
             written over"
        );
        // And on the disk, untouched: nothing was adopted, so nothing of it
        // can be lost by the next edit.
        assert!(std::fs::read_to_string(&twice).unwrap().contains("900"));

        // A name no payload has.
        let stray = where_.path().join("straysim.yaml");
        std::fs::write(
            &stray,
            "simulated_payloads:\n  - type network:\n      \
             - payload: payload\n        simulate: true\n      \
             - payload: paylod\n        simulate: true\n",
        )
        .expect("a simulator file");
        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&stray);
        let said = editor.trouble().expect("this should be reported");
        assert!(said.contains("paylod"), "{said}");
        assert!(editor.document().simulator.is_none());

        // And the one that does join up is opened.
        let good = where_.path().join("goodsim.yaml");
        std::fs::write(
            &good,
            "simulated_payloads:\n  - type network:\n      \
             - payload: payload\n        simulate: true\n        \
             packet_interval: 500 ms\n",
        )
        .expect("a simulator file");
        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&good);
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());
        assert!(editor.document().simulator.is_some());

        // Renaming the payload takes its settings with it, so the pair still
        // joins up: a rename that left them behind would make the next
        // session refuse the file the editor itself had written.
        editor.edit(Edit::RenameNode {
            was: "payload".to_string(),
            now: "payload-1".to_string(),
        });
        let written =
            std::fs::read_to_string(&good).expect("the simulator file");
        assert!(written.contains("payload-1"), "{written}");
        let mut again = Editor::open(&path, crate::pane::default_dividers());
        again.open_simulator(&good);
        assert!(again.trouble().is_none(), "{:?}", again.trouble());
    }

    /// Copies the thermostat example and the common directory it includes
    /// into `into`, as they are shipped: the example's simulator file is a
    /// symbolic link into the common directory, and stays one.
    ///
    /// # Arguments
    ///
    /// * `into` - The directory the two example directories are made in.
    ///
    /// # Returns
    ///
    /// The copied configuration, and its simulator file.
    fn thermostat_example(into: &Path) -> (PathBuf, PathBuf) {
        let examples =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        for dir in ["thermostat", "thermostat-common"] {
            std::fs::create_dir(into.join(dir)).expect("a directory");
            for entry in std::fs::read_dir(examples.join(dir)).expect("listed")
            {
                let entry = entry.expect("an entry");
                let to = into.join(dir).join(entry.file_name());
                let kind = entry.file_type().expect("a file type");
                if kind.is_symlink() {
                    let target =
                        std::fs::read_link(entry.path()).expect("its target");
                    std::os::unix::fs::symlink(target, &to).expect("linked");
                } else {
                    std::fs::copy(entry.path(), &to).expect("copied");
                }
            }
        }
        let top = into.join("thermostat");
        (top.join("thermostat.ytcs"), top.join("thermostat-sim.yaml"))
    }

    /// The shipped thermostat opens with its simulator file, though every
    /// payload the file names is in the included common file and drawn in
    /// its subdiagram.
    ///
    /// The file names each payload by its own name, as tcssim does, and the
    /// editor holds a node in a subdiagram under the subdiagram's name too:
    /// joining the two by the editor's name refused the shipped example, and
    /// the Physics pane with it.
    #[test]
    fn the_thermostat_example_opens_with_its_simulator_file() {
        let where_ = tempfile::tempdir().expect("a directory");
        let (path, sim) = thermostat_example(where_.path());

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        editor.open_simulator(&sim);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());

        let simulator =
            editor.document().simulator.as_ref().expect("it is adopted");
        assert!(
            !simulator.world.components.is_empty(),
            "the world's components, for the Physics pane"
        );
        assert!(simulator.payloads.contains_key("gpio-in-tmass"));
        let tmass = editor
            .document()
            .nodes
            .iter()
            .find(|node| node.name == "gpio-in-tmass")
            .expect("the payload");
        assert_eq!(
            tmass.home.as_deref(),
            Some("../thermostat-common/thermostat"),
            "the payload this is about is in the included file"
        );
    }

    /// A simulated setting of a payload in an included file is written
    /// under the payload's own name, and a rename of it carries its
    /// settings, there too.
    ///
    /// The window a setting is typed into knows the node by the name the
    /// editor holds it under, `../thermostat-common/thermostat/gpio-in-tmass`
    /// here; the file knows it as `gpio-in-tmass`, and an entry written
    /// under the other name joins nothing the next session can open.
    #[test]
    fn an_included_payloads_simulated_settings_keep_its_own_name() {
        let where_ = tempfile::tempdir().expect("a directory");
        let (path, sim) = thermostat_example(where_.path());
        let held = "../thermostat-common/thermostat/gpio-in-tmass";

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        assert!(editor.document().node(held).is_some(), "held so");

        editor.edit(Edit::SetSimPayload {
            payload: crate::sim::Simulator::key_of(held),
            setting: "drop_percent".to_string(),
            was: None,
            now: Some("7".to_string()),
        });
        let written =
            std::fs::read_to_string(&sim).expect("the simulator file");
        assert!(
            !written.contains("thermostat/gpio-in-tmass"),
            "a qualified name is a payload nobody has: {written}"
        );
        let entry = written
            .split("- payload: ")
            .find(|entry| entry.starts_with("gpio-in-tmass\n"))
            .expect("the payload's entry");
        assert!(entry.contains("drop_percent: 7\n"), "{entry}");

        let mut again = Editor::open(&path, crate::pane::default_dividers());
        again.open_simulator(&sim);
        assert_eq!(again.trouble(), None, "{:?}", again.trouble());

        // Renamed where it is, as the problems panel renames a node: the
        // settings follow it to its new name.
        let at = editor
            .document()
            .nodes
            .iter()
            .position(|node| node.name == "gpio-in-tmass")
            .expect("the payload");
        editor.settle_name(at, "gpio-in-mass");
        let simulator =
            editor.document().simulator.as_ref().expect("still adopted");
        assert!(!simulator.payloads.contains_key("gpio-in-tmass"));
        assert_eq!(
            simulator.payloads["gpio-in-mass"]
                .get("drop_percent")
                .map(String::as_str),
            Some("7")
        );
        let mut again = Editor::open(&path, crate::pane::default_dividers());
        again.open_simulator(&sim);
        assert_eq!(again.trouble(), None, "{:?}", again.trouble());
        assert!(again
            .document()
            .simulator
            .as_ref()
            .is_some_and(|simulator| simulator
                .payloads
                .contains_key("gpio-in-mass")));
    }

    /// A file that names a node twice is not opened: a node's name is what
    /// every other part of a configuration knows it by, and the reader
    /// refuses two nodes of one name, saying where the second is.
    #[test]
    fn a_file_that_names_a_node_twice_is_refused() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        std::fs::write(
            &path,
            "payloads:\n  - type i2c:\n      - payload: i2c\n      \
             - payload: i2c\n",
        )
        .expect("a configuration naming a node twice");

        let editor = Editor::open(&path, crate::pane::default_dividers());
        let trouble = editor.trouble().expect("it is refused");
        assert!(trouble.contains("\"i2c\" twice"), "{trouble}");
        assert!(trouble.contains("at line 4"), "{trouble}");
    }

    /// A setting typed into a simulated payload reaches the simulator's file.
    ///
    /// The whole path, because every piece of it was in place and nothing came
    /// out the other end: the edit, the document, and the file the editor
    /// writes on every edit. A session that showed the fields and wrote a file
    /// with nothing in it is what this exists to catch.
    #[test]
    fn a_simulated_payloads_setting_reaches_the_file() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let sim = where_.path().join("vehiclesim.yaml");
        std::fs::write(
            &path,
            "payloads:\n  - type network:\n      - payload: payload\n",
        )
        .expect("a configuration");

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        assert!(editor.document().simulator.is_some(), "the session has one");

        editor.edit(Edit::SetSimPayload {
            payload: "payload".to_string(),
            setting: "packet_interval".to_string(),
            was: None,
            now: Some("1 s".to_string()),
        });

        let written =
            std::fs::read_to_string(&sim).expect("the simulator file");
        assert!(
            written.contains("        packet_interval: 1 s\n"),
            "the setting did not reach the file: {written}"
        );
    }

    /// A simulator configuration the session begins names each payload of
    /// the configuration under its type, and says nothing else.
    ///
    /// It was a form once, every setting of every payload written with
    /// nothing after it, so that the file said what there was to answer.
    /// That made every file the editor touched a form too: a file somebody
    /// wrote with three settings came back with fifteen empty keys apiece.
    /// A key with nothing after it says nothing to the simulator, and the
    /// windows are where the questions are asked. So a new file says which
    /// payloads there are, and from then on holds what somebody says.
    #[test]
    fn a_new_simulator_configuration_names_each_payload() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let sim = where_.path().join("vehiclesim.yaml");
        // Two payloads and something that is not one: a computation has
        // nothing standing in for it, so it has no entry.
        std::fs::write(
            &path,
            "payloads:\n  - type i2c:\n      - payload: sensor\n  \
             - type network:\n      - payload: payload\nvehicle:\n  \
             - type compute:\n      - compute: mean\n",
        )
        .expect("a configuration");

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        // One edit, so that the files are written: the editor writes on every
        // edit, and this one is about the configuration rather than the
        // simulator.
        editor.edit(Edit::SetNodeComment {
            node: "sensor".to_string(),
            was: None,
            now: Some("the forward bulkhead".to_string()),
        });

        let written =
            std::fs::read_to_string(&sim).expect("the simulator file");
        let body = written.split_once('\n').expect("a schema line").1;
        assert_eq!(
            body,
            "\nsimulated_payloads:\n  - type i2c:\n      - payload: sensor\n\
             \n  - type network:\n      - payload: payload\n",
            "{written}"
        );

        // And it reads back as those entries, as the session holds them.
        let mut again = Editor::open(&path, crate::pane::default_dividers());
        again.open_simulator(&sim);
        assert_eq!(again.trouble(), None, "{:?}", again.trouble());
        let simulator = again.document().simulator.as_ref().expect("opened");
        assert_eq!(simulator.kinds.len(), 2, "{simulator:?}");
    }

    /// A simulator configuration has to be named `.yaml`, and a name that is
    /// not is refused with nothing opened for it.
    ///
    /// The editor writes one as YAML and only as YAML, so adopting another
    /// name would mean writing YAML into a file called something else at the
    /// next Save -- a file whose name says one thing and whose contents say
    /// another, which nothing would read back. The configuration itself is
    /// still open: what is refused is the simulator's half of the session.
    #[test]
    fn a_simulator_file_must_be_named_yaml() {
        let where_ = tempfile::tempdir().expect("a directory");

        for name in ["vehiclesim.xml", "vehiclesim.yml", "vehiclesim"] {
            let sim = where_.path().join(name);
            // Written, so that what refuses it is the name and not the
            // reading.
            std::fs::write(
                &sim,
                "simulated_payloads: []
",
            )
            .expect("a file");

            let mut editor = Editor::open(
                &where_.path().join("vehicle.ytcs"),
                crate::pane::default_dividers(),
            );
            editor.open_simulator(&sim);

            let said = editor.trouble().expect("this should be reported");
            assert!(
                said.contains(name) && said.contains(".yaml"),
                "{name}: {said}"
            );
            assert!(
                editor.document().simulator.is_none(),
                "{name}: nothing should be opened for a name that cannot \
                 be written"
            );
        }

        // And the name it must have, which opens -- including one that does
        // not exist yet, since that is how a new simulator configuration is
        // begun.
        for name in ["vehiclesim.yaml", "VEHICLESIM.YAML", "not-yet.yaml"] {
            let sim = where_.path().join(name);
            let mut editor = Editor::open(
                &where_.path().join("vehicle.ytcs"),
                crate::pane::default_dividers(),
            );
            editor.open_simulator(&sim);

            assert!(
                editor.trouble().is_none(),
                "{name}: {:?}",
                editor.trouble()
            );
            assert!(editor.document().simulator.is_some(), "{name}");
        }
    }

    /// A group is written out, and read back.
    ///
    /// A group is one of the four things a configuration is made of, and the
    /// one the editor could only read: a group made in the window had to reach
    /// the file or the nodes in it would name something nothing declared.
    #[test]
    fn a_group_made_in_the_editor_is_written_and_read_back() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let mut editor = Editor::new(&path);

        editor.edit(Edit::AddGroup {
            name: "zone_temps".into(),
            interval: "3 s".into(),
        });
        editor.edit(add("temp_1", "i2c"));
        for (attribute, value) in [
            ("group", "zone_temps"),
            ("path", "/dev/i2c-1"),
            ("address", "0x48"),
        ] {
            editor.edit(Edit::SetAttribute {
                node: "temp_1".into(),
                attribute: attribute.into(),
                was: None,
                now: Some(value.into()),
            });
        }
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("zone_temps"), "{written}");
        assert!(written.contains("3 s"), "{written}");

        let reopened = Editor::open(&path, crate::pane::default_dividers());
        assert_eq!(
            reopened
                .document()
                .groups
                .get("zone_temps")
                .map(String::as_str),
            Some("3 s"),
            "the group did not survive a round trip"
        );
        assert_eq!(reopened.document().payloads_of("zone_temps").len(), 1);
    }

    /// A session recovered after a crash has the work *and* the undo.
    ///
    /// Which is the whole reason the journal is written as well as the
    /// document. Simulated by starting a second editor on the same path, which
    /// is what happens when the first one dies.
    #[test]
    fn a_recovered_session_can_still_undo_past_the_crash() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.ytcs");

        {
            let mut editor = Editor::new(&path);
            editor.edit(add("a", "i2c"));
            editor.edit(add("b", "spi"));
            editor.edit(add("c", "compute"));
            assert_eq!(editor.undo_depth(), 3);
            // And the process dies here, with no chance to tidy up.
        }

        let mut recovered =
            Editor::open(&path, crate::pane::default_dividers());
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
        let path = where_.path().join("graph.ytcs");
        let mut editor = Editor::new(&path);

        editor.edit(add("a", "i2c"));
        editor.edit(add("b", "spi"));
        editor.undo();

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("payload: a"), "{saved}");
        assert!(
            !saved.contains("payload: b"),
            "the undone node is still in the file: {saved}"
        );

        editor.redo();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(
            saved.contains("payload: b"),
            "the redo was not saved: {saved}"
        );
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
        let path = locked.join("graph.ytcs");

        let mut editor = Editor::new(&path);
        editor.edit(add("a", "i2c"));
        assert!(editor.trouble().is_none());

        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions =
                std::fs::metadata(&locked).unwrap().permissions();
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
            let mut permissions =
                std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }

        // And once it can be written, the trouble clears.
        editor.edit(add("c", "compute"));
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());
        let saved = std::fs::read_to_string(&path).unwrap();
        for name in ["payload: a", "payload: b", "compute: c"] {
            assert!(saved.contains(name), "{saved}");
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
        let path = locked.join("graph.ytcs");

        let mut editor = Editor::new(&path);
        editor.edit(add("a", "i2c"));

        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions =
                std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o500);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }
        editor.edit(add("b", "spi"));
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions =
                std::fs::metadata(&locked).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&locked, permissions).unwrap();
        }

        // What a recovery would find: one node and one edit of history, which
        // agree with each other. The second edit is lost, which is what a
        // device that could not be written to costs.
        let recovered = Editor::open(&path, crate::pane::default_dividers());
        assert_eq!(recovered.document().nodes.len(), 1);
        assert_eq!(recovered.undo_depth(), 1);
    }

    /// A subdiagram is written to a file of its own, beside the top diagram.
    #[test]
    fn a_subdiagram_is_written_to_its_own_file() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.ytcs");
        let mut editor = Editor::new(&path);

        editor.edit(Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("zone".into()),
        });
        editor.edit(Edit::AddNode {
            name: "gateway".into(),
            node_type: "gateway_radio".into(),
            home: None,
        });
        // A link that crosses the boundary, which was asked for.
        editor.edit(Edit::AddLink {
            link: crate::journal::Link::new("zone/temp_1", "gateway", None),
        });
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        let top = std::fs::read_to_string(&path).expect("the top diagram");
        let inner = std::fs::read_to_string(where_.path().join("zone.ytcs"))
            .expect("the subdiagram");

        // The top file names what it includes and holds its own nodes.
        assert!(top.contains("include:\n  - zone"), "{top}");
        assert!(top.contains("gateway: gateway"), "{top}");
        assert!(
            !top.contains("payload: temp_1"),
            "the subdiagram's node is in the top file too: {top}"
        );

        // The subdiagram holds its node, and the crossing link is written
        // where its source lives -- qualified at the far end only.
        assert!(inner.contains("payload: temp_1"), "{inner}");
        // A link is an attribute of the node it leaves, so a link that
        // crosses the boundary is written in the file its *source* is in --
        // once, and with the far end plain, gateway having no subdiagram to
        // qualify it with.
        assert!(
            inner.contains("payload-out:\n          - target: gateway\n"),
            "the crossing link should be written on the node it leaves: {inner}"
        );
        assert!(
            !top.contains("payload-out:"),
            "the crossing link is written twice: {top}"
        );
    }

    /// A subdiagram's own files are written whatever format the parent is.
    #[test]
    fn subdiagrams_follow_the_parent_into_xml() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("vehicle.xtcs");
        let mut editor = Editor::new(&path);

        editor.edit(Edit::AddNode {
            name: "zone".into(),
            node_type: "subdiagram".into(),
            home: None,
        });
        editor.edit(Edit::AddNode {
            name: "temp_1".into(),
            node_type: "i2c".into(),
            home: Some("zone".into()),
        });

        let top = std::fs::read_to_string(&path).expect("the top diagram");
        let inner = std::fs::read_to_string(where_.path().join("zone.xtcs"))
            .expect("the subdiagram, in XML as its parent is");
        assert!(top.contains("<include>zone</include>"), "{top}");
        assert!(inner.starts_with("<?xml"), "{inner}");
        assert!(inner.contains("<payload name=\"temp_1\""), "{inner}");
    }

    /// An XML name gets an XML file, and the history still works.
    #[test]
    fn a_configuration_named_xml_is_written_as_xml() {
        let where_ = tempfile::tempdir().expect("a directory");
        let path = where_.path().join("graph.xtcs");
        let mut editor = Editor::new(&path);

        editor.edit(add("temp_1", "i2c"));
        let written = std::fs::read_to_string(&path).expect("the file exists");
        assert!(written.starts_with("<?xml"), "{written}");
        assert!(written.contains("<type name=\"i2c\">"), "{written}");
        assert!(editor.trouble().is_none(), "{:?}", editor.trouble());

        // And the history is beside it as it is for YAML, so undo reaches past
        // a crash whichever format the configuration is written in.
        editor.edit(add("temp_2", "i2c"));
        let recovered = Editor::open(&path, crate::pane::default_dividers());
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
        assert!(said.contains(".ytcs"), "{said}");
        // The edit is still in the document: the person at the keyboard has
        // lost nothing but the saving, and can say where to put it.
        assert_eq!(editor.document().nodes.len(), 1);
    }

    /// The journal is named after the document and kept out of its directory.
    ///
    /// Where and how long for is `crate::history`, which has the tests for
    /// both; this is the one thing this module still says about it. Asked
    /// without setting `XDG_STATE_HOME`, which would set it for every other
    /// test in the process: the editor writes a history on every edit.
    #[test]
    fn the_history_is_named_after_the_document() {
        let editor = Editor::new(Path::new("/var/lib/tcs/graph.ytcs"));
        let named = match editor.journal_path() {
            Some(named) => named,
            // Neither XDG_STATE_HOME nor HOME on this machine, which is a
            // session with nowhere to keep a history. The editor still edits.
            None => return,
        };

        // Out of the configuration's own directory, and named after it: see
        // crate::history.
        assert_ne!(named.parent(), Some(Path::new("/var/lib/tcs")));
        assert_eq!(
            named,
            crate::history::journal_in(
                &crate::history::state_dir().expect("somewhere"),
                Path::new("/var/lib/tcs/graph.ytcs")
            )
        );
        assert!(
            named
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("graph-"),
            "{named:?}"
        );
    }

    /// Copies every shipped configuration and simulator file into a
    /// directory, laid out as the repository lays them out, with the
    /// schemas they name, so that a test can save them without touching
    /// the repository's own.
    ///
    /// A link is copied as a link, as the thermostat's simulator file is.
    ///
    /// # Arguments
    ///
    /// * `into` - The directory.
    ///
    /// # Returns
    ///
    /// Each configuration copied, with its simulator file where it has one:
    /// the file beside it named after it with `-sim.yaml` on the end.
    fn shipped_tree(into: &Path) -> Vec<(PathBuf, Option<PathBuf>)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut directories = vec![
            PathBuf::from("tests/manual"),
            PathBuf::from("tcsgui/share/schema"),
        ];
        for entry in std::fs::read_dir(root.join("examples")).expect("listed") {
            let entry = entry.expect("an entry");
            if entry.file_type().expect("a type").is_dir() {
                directories.push(Path::new("examples").join(entry.file_name()));
            }
        }
        let mut configurations = Vec::new();
        for directory in directories {
            std::fs::create_dir_all(into.join(&directory)).expect("made");
            for entry in std::fs::read_dir(root.join(&directory)).expect("ls") {
                let entry = entry.expect("an entry");
                let name = PathBuf::from(entry.file_name());
                let extension = name.extension().and_then(|e| e.to_str());
                let wanted = directory.ends_with("schema")
                    || matches!(extension, Some("ytcs" | "xtcs" | "yaml"));
                if !wanted {
                    continue;
                }
                let to = into.join(&directory).join(&name);
                if entry.file_type().expect("a type").is_symlink() {
                    let target = std::fs::read_link(entry.path()).expect("it");
                    std::os::unix::fs::symlink(target, &to).expect("linked");
                } else if entry.file_type().expect("a type").is_file() {
                    std::fs::copy(entry.path(), &to).expect("copied");
                }
                if matches!(extension, Some("ytcs" | "xtcs")) {
                    let stem = name.file_stem().expect("a stem");
                    let sim = into
                        .join(&directory)
                        .join(format!("{}-sim.yaml", stem.to_string_lossy()));
                    configurations.push((to, sim.exists().then_some(sim)));
                }
            }
        }
        configurations.sort();
        configurations
    }

    /// Every file under a directory, by its path, with what it holds and,
    /// for a link, where it leads.
    ///
    /// # Arguments
    ///
    /// * `root` - The directory.
    ///
    /// # Returns
    ///
    /// The files, by path.
    fn every_file(root: &Path) -> BTreeMap<PathBuf, (Option<PathBuf>, String)> {
        let mut files = BTreeMap::new();
        let mut waiting = vec![root.to_path_buf()];
        while let Some(directory) = waiting.pop() {
            for entry in std::fs::read_dir(&directory).expect("listed") {
                let entry = entry.expect("an entry");
                let path = entry.path();
                let kind = entry.file_type().expect("a type");
                if kind.is_dir() {
                    waiting.push(path);
                    continue;
                }
                let link = kind
                    .is_symlink()
                    .then(|| std::fs::read_link(&path).expect("its target"));
                let text = std::fs::read_to_string(&path).expect("readable");
                files.insert(path, (link, text));
            }
        }
        files
    }

    /// Opening a shipped configuration and saving it with no edit leaves
    /// every file as it was, byte for byte: both spellings, the included
    /// files and the simulator file.
    ///
    /// A save rewrote them all once, in the editor's own hand: comments
    /// broken again, quotes put round names, fifteen empty keys added to
    /// every simulated payload and `simulate: true` made a string. A save
    /// writes what was edited, and here nothing was.
    #[test]
    fn saving_a_shipped_configuration_unedited_changes_no_file() {
        let where_ = tempfile::tempdir().expect("a directory");
        let configurations = shipped_tree(where_.path());
        let with_sim = configurations.iter().filter(|(_, s)| s.is_some());
        assert!(with_sim.count() >= 14, "{configurations:?}");
        assert!(configurations.len() >= 34, "{configurations:?}");
        let before = every_file(where_.path());

        let mut saved = 0;
        for (path, sim) in &configurations {
            let mut editor =
                Editor::open(path, crate::pane::default_dividers());
            // A file that does not open on its own -- one only ever
            // included, which names nodes of the file including it -- is not
            // a configuration to save.
            if editor.trouble().is_some() {
                let parent = path.parent().expect("a directory");
                assert!(
                    parent.to_string_lossy().ends_with("-common"),
                    "{}: {:?}",
                    path.display(),
                    editor.trouble()
                );
                continue;
            }
            saved += 1;
            if let Some(sim) = sim {
                editor.open_simulator(sim);
            }
            editor.save();
            assert_eq!(editor.trouble(), None, "{}", path.display());
            editor.forget_history();
            let after = every_file(where_.path());
            for (file, was) in &before {
                let now = after.get(file).expect("still there");
                assert!(
                    now == was,
                    "saving {} changed {}:\n{}",
                    path.display(),
                    file.display(),
                    similar::TextDiff::from_lines(&was.1, &now.1)
                        .unified_diff()
                        .header("was", "written")
                );
            }
            assert_eq!(after.len(), before.len(), "{}", path.display());
        }
        assert!(saved >= 30, "only {saved} were saved");
    }

    /// Each file of a set after an edit, held against the set before it: the
    /// files named changed as given, and every other file not at all.
    ///
    /// # Arguments
    ///
    /// * `before`  - Every file before the edit.
    /// * `root`    - The directory they are in.
    /// * `changed` - Each file that changes, by its path under `root`, with
    ///               the text it held that changes and what it becomes. A
    ///               link to one changes with it.
    fn only_changed(
        before: &BTreeMap<PathBuf, (Option<PathBuf>, String)>,
        root: &Path,
        changed: &[(&str, &str, &str)],
    ) {
        let after = every_file(root);
        assert_eq!(after.len(), before.len());
        for (file, (link, was)) in before {
            let now = &after[file];
            assert_eq!(&now.0, link, "{}", file.display());
            let mut expected = was.clone();
            for (name, old, new) in changed {
                // A link changes as the file it leads to does.
                let target = std::fs::canonicalize(file).expect("there");
                if target.ends_with(name) {
                    assert_eq!(
                        was.matches(old).count(),
                        1,
                        "{}: {old}",
                        file.display()
                    );
                    expected = expected.replace(old, new);
                }
            }
            assert!(
                now.1 == expected,
                "{} is not as expected:\n{}",
                file.display(),
                similar::TextDiff::from_lines(&expected, &now.1)
                    .unified_diff()
                    .header("expected", "written")
            );
        }
    }

    /// Moving a node changes its position line, in both spellings of the
    /// file it is drawn in, and no other line of any file.
    #[test]
    fn moving_a_node_changes_only_its_position_line() {
        let where_ = tempfile::tempdir().expect("a directory");
        shipped_tree(where_.path());
        let root = where_.path();
        let path = root.join("examples/thermostat/thermostat.ytcs");
        let sim = root.join("examples/thermostat/thermostat-sim.yaml");
        let before = every_file(root);

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        let at = |editor: &Editor, name: &str| {
            editor.document().node(name).expect("the node").at
        };
        let was = at(&editor, "gateway");
        assert_eq!(was, (790, 34));
        editor.edit(Edit::MoveNode {
            node: "gateway".to_string(),
            was,
            now: (800, 40),
        });
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        only_changed(
            &before,
            root,
            &[
                (
                    "examples/thermostat/thermostat.ytcs",
                    "    - { node: gateway, x: 790, y: 34 }\n",
                    "    - { node: gateway, x: 800, y: 40 }\n",
                ),
                (
                    "examples/thermostat/thermostat.xtcs",
                    "<node name=\"gateway\" x=\"790\" y=\"34\"/>",
                    "<node name=\"gateway\" x=\"800\" y=\"40\"/>",
                ),
            ],
        );

        // And one drawn in the included file: that file's position line.
        let before = every_file(root);
        let held = "../thermostat-common/thermostat/gpio-in-tmass";
        // Wherever settling put it as the file was opened, which depends
        // on the panes: the file has it at 91, 544.
        let was = at(&editor, held);
        editor.edit(Edit::MoveNode {
            node: held.to_string(),
            was,
            now: (95, 550),
        });
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        editor.forget_history();
        only_changed(
            &before,
            root,
            &[
                (
                    "examples/thermostat-common/thermostat.ytcs",
                    "    - { node: gpio-in-tmass, x: 91, y: 544 }\n",
                    "    - { node: gpio-in-tmass, x: 95, y: 550 }\n",
                ),
                (
                    "examples/thermostat-common/thermostat.xtcs",
                    "<node name=\"gpio-in-tmass\" x=\"91\" y=\"544\"/>",
                    "<node name=\"gpio-in-tmass\" x=\"95\" y=\"550\"/>",
                ),
            ],
        );
    }

    /// Setting one simulated setting changes that setting's line, or adds
    /// it, or takes it out, and no other line: the comments, the blank
    /// lines, the quoting and the folded description all stay.
    #[test]
    fn setting_one_simulated_setting_changes_only_its_line() {
        let where_ = tempfile::tempdir().expect("a directory");
        shipped_tree(where_.path());
        let root = where_.path();
        let path = root.join("examples/thermostat/thermostat.ytcs");
        let sim = root.join("examples/thermostat/thermostat-sim.yaml");
        let common = "examples/thermostat-common/thermostat-sim.yaml";

        let mut editor = Editor::open(&path, crate::pane::default_dividers());
        editor.open_simulator(&sim);
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        let set = |editor: &mut Editor,
                   setting: &str,
                   was: Option<&str>,
                   now: Option<&str>| {
            editor.edit(Edit::SetSimPayload {
                payload: "temp_1".to_string(),
                setting: setting.to_string(),
                was: was.map(str::to_string),
                now: now.map(str::to_string),
            });
            assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        };

        // A value changed.
        let before = every_file(root);
        set(&mut editor, "lag", Some("4 s"), Some("5 s"));
        only_changed(
            &before,
            root,
            &[(
                common,
                "temp_1\n        accuracy: \"0.5 K\"\n        \
                 group: zone_temps\n        lag: 4 s\n",
                "temp_1\n        accuracy: \"0.5 K\"\n        \
                 group: zone_temps\n        lag: 5 s\n",
            )],
        );

        // A setting added, in its place among the others.
        let before = every_file(root);
        set(&mut editor, "jitter", None, Some("10 ms"));
        only_changed(
            &before,
            root,
            &[(
                common,
                "        group: zone_temps\n        lag: 5 s\n",
                "        group: zone_temps\n        jitter: 10 ms\n        \
                 lag: 5 s\n",
            )],
        );

        // A value its author quoted, which keeps its quotes.
        let before = every_file(root);
        set(&mut editor, "accuracy", Some("0.5 K"), Some("0.4 K"));
        only_changed(
            &before,
            root,
            &[(
                common,
                "- payload: temp_1\n        accuracy: \"0.5 K\"\n",
                "- payload: temp_1\n        accuracy: \"0.4 K\"\n",
            )],
        );

        // And one emptied, which takes its line with it.
        let before = every_file(root);
        set(&mut editor, "jitter", Some("10 ms"), None);
        only_changed(&before, root, &[(common, "        jitter: 10 ms\n", "")]);

        // A physics figure, likewise.
        let before = every_file(root);
        editor.edit(Edit::SetPhysics {
            component: "heater_2".to_string(),
            setting: "power".to_string(),
            was: Some("4 W".to_string()),
            now: Some("5 W".to_string()),
        });
        assert_eq!(editor.trouble(), None, "{:?}", editor.trouble());
        editor.forget_history();
        only_changed(
            &before,
            root,
            &[(
                common,
                "- component: heater_2\n            power: \"4 W\"\n",
                "- component: heater_2\n            power: \"5 W\"\n",
            )],
        );
    }
}
