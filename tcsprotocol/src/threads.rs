//! What the ground is told of tcspecial's threads -- what each is for, the
//! stack it was given, and the most of that stack it has used -- and of
//! the memory the whole process has.
//!
//! A thread whose stack overflows aborts the vehicle's software, and how
//! deep a thread goes is not something a desk can say for a vehicle: the
//! paths a vehicle runs -- its devices, its errors -- are not all the paths
//! a test runs. So the vehicle measures each of its threads and the ground
//! asks for the figures with QUERY_THREADS, which is how a configuration's
//! `thread_stack` is set from what a vehicle does rather than guessed.

use crate::types::NodeId;

/// How many threads one answer to QUERY_THREADS tells of.
///
/// A fixed number, so that the answer has one size and packs without
/// allocating. A vehicle with more threads than this is read in turns, each
/// QUERY_THREADS naming the first thread it asks about; the answer says how
/// many there are in all.
pub const THREADS_PER_ANSWER: usize = 32;

/// What a thread is for.
///
/// The numbers are each role's on the link, and keep their meanings for
/// ever; `xtce/link.xtce` gives the same ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadRole {
    /// The process's first thread, which starts the vehicle and then waits
    /// for it to end.
    Main,
    /// The thread that takes SIGINT and SIGTERM.
    Signals,
    /// The thread that takes the ground's commands and answers them.
    Commands,
    /// The thread that sends the beacons.
    Beacons,
    /// The thread that hands a log's full segments to segsend.
    Segsend,
    /// A node's thread: the node is the one its id names.
    Node,
    /// The thread of a payload's link to the ground: the payload is the
    /// node its id names.
    GroundLink,
    /// A role this build does not know, by its number on the link, which a
    /// vehicle of a later build may give.
    Unknown(u8),
}

impl ThreadRole {
    /// Gives the role's number on the link.
    ///
    /// # Returns
    ///
    /// The number.
    pub fn to_u8(self) -> u8 {
        match self {
            ThreadRole::Main => 0,
            ThreadRole::Signals => 1,
            ThreadRole::Commands => 2,
            ThreadRole::Beacons => 3,
            ThreadRole::Segsend => 4,
            ThreadRole::Node => 5,
            ThreadRole::GroundLink => 6,
            ThreadRole::Unknown(number) => number,
        }
    }

    /// The role a number on the link gives.
    ///
    /// # Arguments
    ///
    /// * `number` - The number.
    ///
    /// # Returns
    ///
    /// The role, or [`ThreadRole::Unknown`] for one this build has not got.
    pub fn from_u8(number: u8) -> Self {
        match number {
            0 => ThreadRole::Main,
            1 => ThreadRole::Signals,
            2 => ThreadRole::Commands,
            3 => ThreadRole::Beacons,
            4 => ThreadRole::Segsend,
            5 => ThreadRole::Node,
            6 => ThreadRole::GroundLink,
            other => ThreadRole::Unknown(other),
        }
    }

    /// Whether a thread of this role belongs to a node, which its id names.
    pub fn has_node(self) -> bool {
        matches!(self, ThreadRole::Node | ThreadRole::GroundLink)
    }

    /// What the role is called where it is shown.
    ///
    /// # Returns
    ///
    /// The role's name, in lower case.
    pub fn as_str(self) -> &'static str {
        match self {
            ThreadRole::Main => "main",
            ThreadRole::Signals => "signals",
            ThreadRole::Commands => "commands",
            ThreadRole::Beacons => "beacons",
            ThreadRole::Segsend => "segsend",
            ThreadRole::Node => "node",
            ThreadRole::GroundLink => "ground link",
            ThreadRole::Unknown(_) => "unknown",
        }
    }
}

impl std::fmt::Display for ThreadRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadRole::Unknown(number) => write!(f, "role {number}"),
            known => f.write_str(known.as_str()),
        }
    }
}

/// One thread's stack, as the ground is told of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadStack {
    /// What the thread is for.
    pub role: ThreadRole,
    /// The node it belongs to, where its role has one; [`NO_NODE`]
    /// otherwise.
    pub node: NodeId,
    /// The stack it was given, in bytes.
    pub stack: u32,
    /// The most of its stack it has used since it started, in bytes. A
    /// little more than it used, never less: see `tcspecial`'s
    /// `runtime::thread`.
    pub deepest: u32,
}

/// How much memory the whole of tcspecial -- the process that runs the
/// nodes -- has, as the ground is told of it with its threads.
///
/// What decides whether its memory can be locked: a lock of everything the
/// process has mapped is refused when that is more than the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessMemory {
    /// Everything mapped, in bytes: touched or not, which is what a lock
    /// of everything is held to the limit by.
    pub mapped: u64,
    /// What is in RAM, in bytes.
    pub resident: u64,
    /// What is locked into RAM, in bytes: as much as is mapped once the
    /// lock has succeeded, nought where it has not.
    pub locked: u64,
    /// The limit on what the process may lock, in bytes; [`UNLIMITED`]
    /// where there is none.
    pub lock_limit: u64,
}

/// What [`ProcessMemory::lock_limit`] is where there is no limit.
pub const UNLIMITED: u64 = u64::MAX;

/// The node id a thread that belongs to no node is given.
pub const NO_NODE: NodeId = NodeId(u32::MAX);

impl Default for ThreadStack {
    fn default() -> Self {
        Self {
            role: ThreadRole::Main,
            node: NO_NODE,
            stack: 0,
            deepest: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every number from 0 to 255 is a role, and goes back to the same
    /// number, so a role a later build adds survives a station of this one.
    #[test]
    fn every_role_number_goes_and_comes_back() {
        for number in 0..=u8::MAX {
            assert_eq!(ThreadRole::from_u8(number).to_u8(), number);
        }
        assert_eq!(ThreadRole::from_u8(5), ThreadRole::Node);
        assert!(ThreadRole::GroundLink.has_node());
        assert!(!ThreadRole::Commands.has_node());
    }
}
