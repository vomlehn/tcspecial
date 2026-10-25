//! The node that is a payload and is asked: a program that answers readings.
//!
//! An app node is a node whose work is a program, run by tcspecial, that takes
//! part in the graph. What a file may say about one, and what is wrong with
//! what it says, is `tcslibgs::node::app`; the file interfaces it is run with,
//! the packets they carry and the starting of it are `tcsapp`. What is here is
//! this kind of node: which of those interfaces it has, and which way each of
//! them carries data.
//!
//! One interface, at descriptor 1 of the program: it reads MOC data there and
//! writes payload data there. So this end writes MOC data and reads payload
//! data. Descriptor 0 is not opened for it at all.
//!
//! Which makes a node of this kind the shape every payload has: it is asked
//! for a reading -- by a trigger, a command, whatever the mission sends -- and
//! answers with one. The difference between this and an `app_moc_veh`, whose
//! program holds the same interface and does the same two things with it, is
//! which side of the graph the node stands on: this one is in the `payloads`
//! section, is drawn on the payload pane, and is what the vehicle carries this
//! flight.
//!
//! What asks it is the datum that arrives, which is why it states no `trigger`
//! as a device on a bus does: the bytes that ask a sensor are written before a
//! read because the sensor has no other way of being told, and a program is
//! told by being given the packet.
//!
//! # Where it sits
//!
//! `runtime::port::AppPort` holds one of these in the fabric, and every pass
//! asks it: a payload in no group is polled, and an app answers when it
//! answers. What it answers arrives on the groundward stream, which is the
//! other one from what it was given.

use std::process::ExitStatus;

use tcsapp::parent::{self, Arrived, ReadInto, Running, Supports};
use tcslibgs::node::app::{AppParams, Kind};

/// The program this node is, running, and this end of its interface.
#[derive(Debug)]
pub struct AppAllPl {
    running: Running,
}

impl AppAllPl {
    /// Start the program this node names.
    ///
    /// Started when the configuration is loaded rather than at the first datum,
    /// which is the rule a computation's `process` is held to as well: a
    /// program that cannot be executed is then a fault found on the ground
    /// rather than three days into a mission.
    ///
    /// `params.restart` is not acted on yet. Nothing here restarts anything,
    /// because nothing here is driving it: the policy is read and checked so
    /// that a configuration can state it, and what acts on it is the fabric
    /// work this node is waiting for.
    pub fn of(params: &AppParams) -> Result<AppAllPl, String> {
        if params.kind != Kind::AllPl {
            return Err(format!(
                "a {} node cannot be opened as {}: the two are run with \
                 different interfaces",
                params.kind.node_type(),
                Kind::AllPl.node_type()
            ));
        }
        Ok(AppAllPl {
            running: parent::start(
                &params.executable_path()?,
                &params.arguments(),
                Supports::MOC,
            )?,
        })
    }

    /// Hand it a packet of MOC data, which is what asks it for a reading.
    pub fn give_moc_data(&mut self, data: &[u8]) -> Result<(), String> {
        self.running.write_moc(data)
    }

    /// Whatever payload data it has written, without waiting.
    ///
    /// What the fabric calls, every pass.
    pub fn take_payload_data_now(&mut self) -> Result<Arrived, String> {
        self.running.read_payload_now()
    }

    /// The same, into the fabric's buffer, allocating nothing.
    pub fn take_payload_data_now_into(
        &mut self,
        out: &mut [u8],
    ) -> Result<ReadInto, String> {
        self.running.read_payload_now_into(out)
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
    /// `cat`, reading and writing the one interface this kind is given: what
    /// asks arrives at descriptor 1 and the answer goes back to the same
    /// descriptor, which is payload data there. The real thing is the
    /// `app_all_pl` crate, which has tests of its own.
    fn stands_in(where_: &std::path::Path) -> PathBuf {
        let path = where_.join("stands_in");
        let mut file = std::fs::File::create(&path).expect("a program");
        file.write_all(b"#!/bin/sh\nexec cat <&1\n")
            .expect("a program");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("a program that can be run");
        path
    }

    /// What such a node is told, for a program at `executable`.
    fn says(executable: &std::path::Path) -> AppParams {
        AppParams {
            name: "bay".to_string(),
            kind: Kind::AllPl,
            executable: executable.display().to_string(),
            payload_arg: Some("--bay-2".to_string()),
            moc_arg: Some("--bay-2-commands".to_string()),
            restart: Restart::Never,
        }
    }

    /// Asked for a reading, it answers with one.
    ///
    /// Which is the whole of what this node does so far: it starts the program,
    /// and what the configuration sends to the node reaches it over the
    /// interface its kind has.
    #[test]
    fn asked_for_a_reading_it_answers() {
        let where_ = tempfile::tempdir().expect("a directory");
        let program = stands_in(where_.path());
        let mut node = crate::when_not_busy(|| AppAllPl::of(&says(&program)))
            .expect("it starts");

        node.give_moc_data(b"READ").expect("it takes a packet");
        assert_eq!(
            node.take_payload_data().expect("an answer"),
            Some(b"READ".to_vec())
        );

        node.give_moc_data(b"").expect("it takes an empty packet");
        assert_eq!(
            node.take_payload_data().expect("an answer"),
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
    #[test]
    fn a_program_that_cannot_be_started_is_said_so() {
        let where_ = tempfile::tempdir().expect("a directory");
        let said = AppAllPl::of(&says(&where_.path().join("nothing-here")))
            .expect_err("a program that is not there");
        assert!(said.contains("nothing-here"), "{said}");
        assert!(said.contains("could not be started"), "{said}");
    }

    /// And a node of another kind is not opened as one of these.
    #[test]
    fn a_node_of_another_kind_is_not_opened_as_this_one() {
        let where_ = tempfile::tempdir().expect("a directory");
        let mut params = says(&stands_in(where_.path()));
        params.kind = Kind::MocVeh;

        let said = AppAllPl::of(&params).expect_err("a node of another kind");
        assert!(said.contains(Kind::MocVeh.node_type()), "{said}");
        assert!(said.contains(Kind::AllPl.node_type()), "{said}");
    }
}
