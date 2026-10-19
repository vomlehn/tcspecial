//! Running a configuration: nodes joined by links.
//!
//! What replaces the data handler. A handler was a fixed pair -- one device
//! endpoint, one ground endpoint, a conduit each way -- and that is the whole
//! of the topology it could express. A graph is not a set of pairs, so the
//! pair is replaced by two pieces that between them do what it did and more:
//!
//! * [`route`] turns the links into a table: where each node's output goes,
//!   and which link fills each named input of a node.
//! * [`schedule`] says when a group of devices is next read, so that readings
//!   which will be compared with one another are taken together.
//! * [`fabric`] holds the nodes and moves the bytes, using the other two.
//! * [`port`] is the one piece that does open something: it presents an
//!   opened endpoint as a node the fabric can move bytes to and from.
//! * [`system`] is the assembly and the loop: a port per node, held in a
//!   fabric, with the schedule deciding when each group is read.
//! * [`node_state`] is what each node's state is -- enabled or disabled -- and
//!   the descriptor through which a disable reaches a node that is waiting.
//! * [`command`] is the ground's two command links, answered on a thread of
//!   their own so that a command never waits for the pass.
//!
//! The first two open nothing. They are the arithmetic of the graph, and they
//! are separate from the moving of bytes on purpose: both can be tested
//! against a configuration with no vehicle present, where a fault in either
//! would otherwise look like a fault in a device.

pub mod command;
pub mod fabric;
pub mod node_state;
pub mod port;
pub mod route;
pub mod schedule;
pub mod start;
pub mod system;
