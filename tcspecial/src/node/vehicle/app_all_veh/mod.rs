//! The node that is a program on both of the file interfaces.
//!
//! An app node is a program of the vehicle's own, run by tcspecial, that takes
//! part in the graph. What a file may say about one, and what is wrong with
//! what it says, is `tcslibgs::node::app`; the file interfaces it is
//! run with, the packets they carry and the starting of it are `tcsapp`. What
//! is here is this kind of node: which of those interfaces it has, and which
//! way each of them carries data.
//!
//! Both interfaces, so all four directions: payload data written to it at
//! descriptor 0 and read back from it at descriptor 1, and MOC data written at
//! descriptor 1 and read back at descriptor 0. Which makes this the one kind
//! that can carry a stream through, and so the one kind that can stand where a
//! `null` node stands -- with something done to what passes.
//!
//! # Where it sits
//!
//! `runtime::port::AppPort` holds one of these in the fabric, and every pass
//! asks it -- both of its interfaces, turn and turn about, so that a busy
//! stream does not starve the other. Each datum it produces says which stream
//! it is on, and the fabric follows only the links that carry that stream:
//! without it, a reading would go out of this node's `moc-out` as well and the
//! ground's commands would carry telemetry.

use std::process::ExitStatus;

use tcsapp::parent::{self, Arrived, ReadInto, Running, Supports};
use tcslibgs::node::app::{AppParams, Kind};

/// The program this node is, running, and this end of its interfaces.
#[derive(Debug)]
pub struct AppAllVeh {
    running: Running,
}

impl AppAllVeh {
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
    pub fn of(params: &AppParams) -> Result<AppAllVeh, String> {
        if params.kind != Kind::AllVeh {
            return Err(format!(
                "a {} node cannot be opened as {}: the two are run with \
                 different interfaces",
                params.kind.node_type(),
                Kind::AllVeh.node_type()
            ));
        }
        Ok(AppAllVeh {
            running: parent::start(
                &params.executable_path()?,
                &params.arguments(),
                Supports::ALL,
            )?,
        })
    }

    /// Hand it a packet of payload data, on its way groundward.
    pub fn give_payload_data(&mut self, data: &[u8]) -> Result<(), String> {
        self.running.write_payload(data)
    }

    /// Whatever payload data it has written, without waiting.
    ///
    /// What the fabric calls, every pass.
    pub fn take_payload_data_now(&mut self) -> Result<Arrived, String> {
        self.running.read_payload_now()
    }

    /// The same, into the fabric's buffer, allocating nothing.
    pub fn take_payload_data_now_into(&mut self, out: &mut [u8]) -> Result<ReadInto, String> {
        self.running.read_payload_now_into(out)
    }

    /// And whatever MOC data, likewise.
    pub fn take_moc_data_now(&mut self) -> Result<Arrived, String> {
        self.running.read_moc_now()
    }

    /// The same, into the fabric's buffer, allocating nothing.
    pub fn take_moc_data_now_into(&mut self, out: &mut [u8]) -> Result<ReadInto, String> {
        self.running.read_moc_now_into(out)
    }

    /// And take one back from it, which is what it made of what it was given.
    pub fn take_payload_data(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.running.read_payload()
    }

    /// Hand it a packet of MOC data, on its way vehicleward.
    pub fn give_moc_data(&mut self, data: &[u8]) -> Result<(), String> {
        self.running.write_moc(data)
    }

    /// And take one back.
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
    /// `cat`, which with both interfaces open is a pass-through of the groundward
    /// stream and nothing else: it reads payload data at descriptor 0 and
    /// writes it as payload data at descriptor 1, which is exactly what this
    /// kind of app is for. The vehicleward direction it leaves alone, and the
    /// test asks nothing of it.
    fn stands_in(where_: &std::path::Path) -> PathBuf {
        let path = where_.join("stands_in");
        let mut file = std::fs::File::create(&path).expect("a program");
        file.write_all(b"#!/bin/sh\nexec cat\n").expect("a program");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("a program that can be run");
        path
    }

    /// What such a node is told, for a program at `executable`.
    fn says(executable: &std::path::Path) -> AppParams {
        AppParams {
            name: "through".to_string(),
            kind: Kind::AllVeh,
            executable: executable.display().to_string(),
            payload_arg: Some("--readings".to_string()),
            moc_arg: Some("--commands".to_string()),
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
        let mut node = crate::when_not_busy(|| AppAllVeh::of(&says(&program))).expect("it starts");

        node.give_payload_data(b"293.15")
            .expect("it takes a packet");
        assert_eq!(
            node.take_payload_data().expect("it is carried"),
            Some(b"293.15".to_vec())
        );

        node.give_payload_data(b"")
            .expect("it takes an empty packet");
        assert_eq!(
            node.take_payload_data().expect("it is carried"),
            Some(Vec::new())
        );

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
        let said = AppAllVeh::of(&says(&where_.path().join("nothing-here")))
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
        params.kind = Kind::PayloadVeh;

        let said = AppAllVeh::of(&params).expect_err("a node of another kind");
        assert!(said.contains(Kind::PayloadVeh.node_type()), "{said}");
        assert!(said.contains(Kind::AllVeh.node_type()), "{said}");
    }
}
