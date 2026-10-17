//! The node that is a program reading payload data and writing MOC data.
//!
//! An app node is a program of the vehicle's own, run by tcspecial, that takes
//! part in the graph. What a file may say about one, and what is wrong with
//! what it says, is `tcslibgs::node::app`; the file interfaces it is
//! run with, the packets they carry and the starting of it are `tcsapp`. What
//! is here is this kind of node: which of those interfaces it has, and which
//! way each of them carries data.
//!
//! One interface, at descriptor 0 of the program: it reads payload data there
//! and writes MOC data there. So this end writes payload data and reads MOC
//! data, which is what the two methods below are. Descriptor 1 is not opened
//! for it at all -- there is nothing here to reach it with, and `tcsapp` makes
//! the program refuse to start if it finds one.
//!
//! Which makes a node of this kind the shape the ground link has: it takes
//! what travels groundward and answers with what travels vehicleward.
//!
//! # Where it sits
//!
//! `runtime::port::AppPort` is what holds one of these in the fabric, and
//! `runtime::system::is_polled` is why it is read: an app is in no group and
//! answers when it answers, so every pass asks it. What it answers arrives on
//! the vehicleward stream, which is the other one from what it was given --
//! the fabric carries the stream with the datum for exactly this reason.

use std::process::ExitStatus;

use tcsapp::parent::{self, Arrived, Running, Supports};
use tcslibgs::node::app::{AppParams, Kind};

/// The program this node is, running, and this end of its interfaces.
#[derive(Debug)]
pub struct AppPayloadVeh {
    running: Running,
}

impl AppPayloadVeh {
    /// Start the program this node names.
    ///
    /// Started when the configuration is loaded rather than at the first
    /// datum, which is the rule a computation's `process` is held to as well:
    /// a program that cannot be executed is then a fault found on the ground
    /// rather than three days into a mission.
    ///
    /// `params.restart` is not acted on yet. Nothing here restarts anything,
    /// because nothing here is driving it: the policy is read and checked so
    /// that a configuration can state it, and what acts on it is the fabric
    /// work this node is waiting for.
    pub fn of(params: &AppParams) -> Result<AppPayloadVeh, String> {
        if params.kind != Kind::PayloadVeh {
            return Err(format!(
                "a {} node cannot be opened as {}: the two are run with \
                 different interfaces",
                params.kind.node_type(),
                Kind::PayloadVeh.node_type()
            ));
        }
        Ok(AppPayloadVeh {
            running: parent::start(&params.executable_path()?, &params.arguments(), Supports::PAYLOAD)?,
        })
    }

    /// Hand it a packet of payload data, which it reads as payload data.
    pub fn give_payload_data(&mut self, data: &[u8]) -> Result<(), String> {
        self.running.write_payload(data)
    }

    /// Whatever MOC data it has written, without waiting.
    ///
    /// What the fabric calls, every pass: [`Arrived::Nothing`] is a program
    /// with nothing to say, which is the ordinary answer.
    pub fn take_moc_data_now(&mut self) -> Result<Arrived, String> {
        self.running.read_moc_now()
    }

    /// Take a packet of MOC data from it, or `None` once it has ended.
    pub fn take_moc_data(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.running.read_moc()
    }

    /// What it said went wrong, or `None` once it has said all it will.
    pub fn complaint(&mut self) -> Result<Option<String>, String> {
        self.running.read_complaint()
    }

    /// Say that nothing more will be written to it, which is how it is told to
    /// finish.
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

    /// A program standing in for an app of this kind.
    ///
    /// `cat`, with its output sent to the one interface this kind is given: it
    /// reads payload data at descriptor 0 and writes what it read back to the
    /// same descriptor, which is MOC data there. Five lines of shell rather
    /// than a crate of its own, because what is tested here is this node and
    /// not an app -- the apps are `app_payload_veh`, `app_moc_veh` and
    /// `app_all_veh`, which are crates and have tests of their own.
    fn stands_in(where_: &std::path::Path) -> PathBuf {
        let path = where_.join("stands_in");
        let mut file = std::fs::File::create(&path).expect("a program");
        file.write_all(b"#!/bin/sh\nexec cat >&0\n")
            .expect("a program");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("a program that can be run");
        path
    }

    /// What such a node is told, for a program at `program`.
    fn says(program: &std::path::Path) -> AppParams {
        AppParams {
            name: "fore".to_string(),
            kind: Kind::PayloadVeh,
            executable: program.display().to_string(),
            payload_arg: Some("--readings".to_string()),
            moc_arg: None,
            restart: Restart::Never,
        }
    }

    /// Packets reach the program and come back.
    ///
    /// Which is the whole of what this node does so far: it starts the
    /// program, and what the configuration sends to the node reaches it over
    /// the interface its kind has.
    #[test]
    fn packets_reach_the_program_and_come_back() {
        let where_ = tempfile::tempdir().expect("a directory");
        let program = stands_in(where_.path());
        let mut node = crate::when_not_busy(|| AppPayloadVeh::of(&says(&program))).expect("it starts");

        node.give_payload_data(b"293.15").expect("it takes a packet");
        assert_eq!(
            node.take_moc_data().expect("an answer"),
            Some(b"293.15".to_vec())
        );

        // A packet of no bytes is a packet, and comes back as one.
        node.give_payload_data(b"").expect("it takes an empty packet");
        assert_eq!(node.take_moc_data().expect("an answer"), Some(Vec::new()));

        // Told that nothing more is coming, it finishes of its own accord.
        node.no_more().expect("the interfaces end");
        assert!(
            node.waited_for().expect("it stops").success(),
            "a program whose interfaces ended has finished, not failed"
        );
    }

    /// A program that is not there is a fault found at once.
    ///
    /// Started when the configuration is loaded, so a path nobody checked is
    /// refused on the ground rather than at the first datum.
    #[test]
    fn a_program_that_cannot_be_started_is_said_so() {
        let where_ = tempfile::tempdir().expect("a directory");
        let said = AppPayloadVeh::of(&says(&where_.path().join("nothing-here")))
            .expect_err("a program that is not there");
        assert!(said.contains("nothing-here"), "{said}");
        assert!(said.contains("could not be started"), "{said}");
    }

    /// And a node of another kind is not opened as one of these.
    ///
    /// The kinds are run with different interfaces, so opening one as another
    /// would hand a program a descriptor it does not read and withhold the one
    /// it does.
    #[test]
    fn a_node_of_another_kind_is_not_opened_as_this_one() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut params = says(&stands_in(where_.path()));
        params.kind = Kind::MocVeh;

        let said = AppPayloadVeh::of(&params).expect_err("a node of another kind");
        assert!(said.contains(Kind::MocVeh.node_type()), "{said}");
        assert!(said.contains(Kind::PayloadVeh.node_type()), "{said}");
    }
}
