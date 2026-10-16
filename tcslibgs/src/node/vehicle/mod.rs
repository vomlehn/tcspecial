//! What the vehicle itself runs: the attributes, and how they are checked.
//!
//! A computation and a log are node types like any other -- each is named in a
//! configuration, each takes what arrives and does something with it -- and
//! what tells them from a payload is that nothing is bolted to the vehicle for
//! them. They are parts of the telemetry and control system itself, and they
//! are what the configuration's `vehicle` section holds.
//!
//! The arithmetic and the writing are `tcspecial::node::vehicle`; what is here
//! is what a file may say and what is wrong with what it says.
//!
//! Three of this family's types are not here but in `super::app`: the app
//! nodes, which are programs of the vehicle's own taking part in the graph
//! over the file interfaces `tcsapp` describes. Two of that module's five
//! kinds are payloads, so the rule about any of them sits beside both
//! families rather than inside one.

// Computation aboard: what a compute node may say, the arithmetic being
// tcspecial's.
pub mod compute;
// Somewhere data stops rather than being passed on.
pub mod log;

use serde::{Deserialize, Serialize};

/// What to do when a program a node runs stops.
///
/// Two of this family's node types run one -- a computation whose function is
/// `process`, and an app node -- and both are held to the same two spellings:
/// `never`, or a number of times. Declared here rather than in either of them
/// because a second statement of it would be a second default, and the default
/// is the whole of the argument: a part of the control system that restarts
/// under load is worse than one that stays stopped, the second being obviously
/// broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Restart {
    /// Leave it stopped and refuse the link. The default.
    Never,
    /// Start it again, up to this many times.
    UpTo(u32),
}
