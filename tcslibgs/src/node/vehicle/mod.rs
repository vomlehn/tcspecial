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
// What a ping from the ground says, and the answer the vehicle sends back.
pub mod ping;

use serde::{Deserialize, Serialize};

/// Whether a program a node runs is started again when it stops.
///
/// Two of this family's node types run one -- a computation whose function is
/// `process`, and an app node -- and both are held to the same two spellings:
/// `true` or `false`. Declared here rather than in either of them because a
/// second statement of it would be a second default, and the default is the
/// whole of the argument: a part of the control system that restarts under
/// load is worse than one that stays stopped, the second being obviously
/// broken. So a file that says nothing says `Never`, and saying otherwise
/// takes the word `true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Restart {
    /// Leave it stopped and refuse the link. What `false` says, and what a
    /// file that says nothing about it means.
    Never,
    /// Start it again whenever it stops, which is what `true` says.
    WhenItStops,
}

impl Restart {
    /// What a `restart` says, or what is wrong with what it says.
    ///
    /// `said` is the attribute as the file wrote it, and `None` is a file
    /// that did not write it at all. The one place the two spellings are
    /// read: an app node and a `process` computation take the same attribute,
    /// and two readings of it would be two answers on the day one of them
    /// learned a third spelling.
    ///
    /// Nothing else is accepted, a number of times included. A bound on the
    /// restarts is a thing somebody would have to choose, and a configuration
    /// that chose one would be a configuration saying how long to go on
    /// failing -- where what is wanted of this attribute is an answer to
    /// "should it come back at all".
    ///
    /// An empty value is a file that says nothing, which is what the editor
    /// writes when somebody empties the field: a cleared box should leave the
    /// node as it was before anybody typed in it, not make it faulty.
    pub fn of(said: Option<&str>) -> Result<Restart, String> {
        match said.map(str::trim) {
            None | Some("") | Some("false") => Ok(Restart::Never),
            Some("true") => Ok(Restart::WhenItStops),
            Some(other) => Err(format!(
                "restart: \"{other}\" is neither \"true\" nor \"false\""
            )),
        }
    }
}
