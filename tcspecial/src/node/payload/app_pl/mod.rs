//! The node that is a payload nothing asks: a program that writes readings.
//!
//! An app node is a node whose work is a program, run by tcspecial, that takes
//! part in the graph. What a file may say about one, and what is wrong with
//! what it says, is `tcslibgs::node::app`; the file interfaces it is run with,
//! the packets they carry and the starting of it are `tcsapp`. What is here is
//! this kind of node: which of those interfaces it has, and which way each of
//! them carries data.
//!
//! One interface, at descriptor 1 of the program, and the program only writes
//! on it: payload data, which is a reading on its way to the ground. So this
//! end only reads. Descriptor 0 is not opened for it at all.
//!
//! Which makes a node of this kind a payload that is never asked. It is the
//! one type in the configuration with no data sink of any sort -- nothing may
//! be linked into it -- because there is nothing to ask it with: it samples on
//! a clock of its own, or reports when something happens, and what decides is
//! in the program.
//!
//! A payload, then, and not a program of the vehicle's own: it stands where an
//! I2C sensor stands and is read the same way. What it is not is a device,
//! which is the whole of what this type adds -- a payload whose readings come
//! from something the mission wrote rather than something on a bus.
//!
//! # Where it sits
//!
//! `runtime::port::AppPort` holds one of these in the fabric, and every pass
//! asks it: a payload in no group is polled, and an app answers when it
//! answers. What it answers is on the groundward stream, which is the only
//! stream it has.

use std::process::ExitStatus;

use tcsapp::parent::{self, Arrived, Running, Supports};
use tcslibgs::node::app::{AppParams, Kind};

/// The program this node is, running, and this end of its interface.
#[derive(Debug)]
pub struct AppPl {
    running: Running,
}

impl AppPl {
    /// Start the program this node names.
    ///
    /// Started when the configuration is loaded rather than at the first datum,
    /// which is the rule a computation's `process` is held to as well: a program
    /// that cannot be executed is then a fault found on the ground rather than
    /// three days into a mission.
    ///
    /// `params.restart` is not acted on yet. Nothing here restarts anything,
    /// because nothing here is driving it: the policy is read and checked so
    /// that a configuration can state it, and what acts on it is the fabric work
    /// this node is waiting for.
    pub fn of(params: &AppParams) -> Result<AppPl, String> {
        if params.kind != Kind::Pl {
            return Err(format!(
                "a {} node cannot be opened as {}: the two are run with \
                 different interfaces",
                params.kind.node_type(),
                Kind::Pl.node_type()
            ));
        }
        // The MOC interface, which is the interface payload data is written
        // on: what a payload sends is what it writes, and this kind writes and
        // nothing else. See `tcsapp::parent::Supports::MOC`.
        Ok(AppPl {
            running: parent::start(&params.program_path()?, &params.arguments(), Supports::MOC)?,
        })
    }

    /// Whatever payload data it has written, without waiting.
    ///
    /// What the fabric calls, every pass. There is no `give` beside it: a
    /// payload nothing asks has no sink for a datum to arrive at, and the
    /// checking refuses a link that would make one.
    pub fn take_payload_data_now(&mut self) -> Result<Arrived, String> {
        self.running.read_payload_now()
    }

    /// Take a packet of payload data from it, or `None` once it has ended.
    pub fn take_payload_data(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.running.read_payload()
    }

    /// What it said went wrong, or `None` once it has said all it will.
    pub fn complaint(&mut self) -> Result<Option<String>, String> {
        self.running.read_complaint()
    }

    /// Say that nothing more will be written to it, which is how it is told to
    /// finish.
    ///
    /// Nothing is ever written to one of these, so what this says is the end of
    /// an interface that carried nothing this way: a program watching for it
    /// stops, and one that is not watching is stopped with [`AppPl::stop`].
    pub fn no_more(&mut self) -> Result<(), String> {
        self.running.no_more()
    }

    /// Wait for it to stop, and say how it stopped.
    pub fn waited_for(&mut self) -> Result<ExitStatus, String> {
        self.running.waited_for()
    }

    /// Stop it, whatever it is doing.
    pub fn stop(&mut self) -> Result<ExitStatus, String> {
        self.running.stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use tcslibgs::node::vehicle::Restart;

    /// A program standing in for a payload of this kind.
    ///
    /// Five lines of shell rather than a crate of its own, because what is
    /// tested here is this node: `printf` writes one packet -- four bytes of
    /// length and then that many bytes -- to descriptor 1, which is the
    /// interface this kind is given, and reads nothing. The real thing is the
    /// `app_pl` crate, which has tests of its own.
    fn stands_in(where_: &std::path::Path) -> PathBuf {
        let path = where_.join("stands_in");
        let mut file = std::fs::File::create(&path).expect("a program");
        file.write_all(b"#!/bin/sh\nprintf '\\006\\000\\000\\000293.15' >&1\n")
            .expect("a program");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("a program that can be run");
        path
    }

    /// What such a node is told, for a program at `program`.
    fn says(program: &std::path::Path) -> AppParams {
        AppParams {
            name: "bay".to_string(),
            kind: Kind::Pl,
            program: program.display().to_string(),
            payload_arg: Some("--bay-2".to_string()),
            moc_arg: None,
            restart: Restart::Never,
        }
    }

    /// What the program writes is read as a reading.
    ///
    /// Which is the whole of what this node does: it starts the program and
    /// takes what the program says, nothing having been asked of it.
    #[test]
    fn what_the_program_writes_is_taken_as_a_reading() {
        let where_ = tempfile::tempdir().expect("a directory");
        let program = stands_in(where_.path());
        let mut node = crate::when_not_busy(|| AppPl::of(&says(&program))).expect("it starts");

        assert_eq!(
            node.take_payload_data().expect("a reading"),
            Some(b"293.15".to_vec())
        );

        // And then the program has finished, which the interface ending says.
        assert_eq!(node.take_payload_data().expect("the end"), None);
        assert!(
            node.waited_for().expect("it stops").success(),
            "a program that has said what it had to say has finished, not failed"
        );
    }

    /// A program that is not there is a fault found at once.
    ///
    /// Started when the configuration is loaded, so a path nobody checked is
    /// refused on the ground rather than at the first datum.
    #[test]
    fn a_program_that_cannot_be_started_is_said_so() {
        let where_ = tempfile::tempdir().expect("a directory");
        let said = AppPl::of(&says(&where_.path().join("nothing-here")))
            .expect_err("a program that is not there");
        assert!(said.contains("nothing-here"), "{said}");
        assert!(said.contains("could not be started"), "{said}");
    }

    /// And a node of another kind is not opened as one of these.
    ///
    /// The kinds are run with different interfaces, so opening one as another
    /// would hand a program a descriptor it does not read and withhold the one
    /// it does. The payload-side pair share an interface and are still not each
    /// other: one is asked and one is not, and a program told to wait for a
    /// question nobody will send would wait for ever.
    #[test]
    fn a_node_of_another_kind_is_not_opened_as_this_one() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut params = says(&stands_in(where_.path()));
        params.kind = Kind::AllPl;

        let said = AppPl::of(&params).expect_err("a node of another kind");
        assert!(said.contains(Kind::AllPl.node_type()), "{said}");
        assert!(said.contains(Kind::Pl.node_type()), "{said}");
    }
}
