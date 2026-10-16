//! One module per node type, in three families.
//!
//! A node type is three pieces of code in three crates: what its attributes
//! are and how they are checked, which is here; the flight-side node, which is
//! `tcspecial::node`; and the stand-in the simulator puts at the far end of
//! it, which is `tcssim::node`. The three trees carry the same names, so a
//! type is `node/payload/i2c` wherever you are standing and
//! `grep -r node/payload/gpio` finds everything about discrete lines.
//!
//! Three parallel trees rather than one directory per type holding all three
//! pieces. A single crate per type would either link the simulator's
//! stand-ins into the flight image or hide them behind cargo features, and
//! feature combinations are configurations that can be wrong -- which trades
//! reliability for convenience, the wrong way round for this project.
//!
//! # The three families
//!
//! Each tree is divided the way a configuration file is divided, and the way
//! the editor's working panes are: `moc` is the ground link, `vehicle` is what
//! the vehicle itself runs, and `payload` is what it carries. See
//! `crate::graph::section_of`, which is the one statement of which family a
//! type belongs to -- the file reader, the editor's panes and these
//! directories all read it rather than deciding for themselves.
//!
//! There is no `moc` directory here, and that is not an omission. A ground
//! link is a network endpoint: what its attributes are is the network type's
//! own rule -- see `crate::graph::reach` -- and the code that opens one is
//! `tcspecial::node::payload::network`. A directory holding nothing but a
//! comment saying so would be a module to maintain for the sake of symmetry.
//!
//! Where a type needs more than one file it gets a directory of its own:
//! `payload/network` holds the three transports it covers, and
//! `payload/serial` holds the two-wire variant that is one of its own terms.

/// What any app node says, whichever side of the graph it stands on.
///
/// The one module here that is not a family. An app node is a node whose work
/// is a program, and a configuration says the same things about one wherever
/// it sits -- the program to run, and what to do when it stops -- so the rule
/// is written once and the kind says which family the node belongs to. Three
/// of the five kinds are the vehicle's own and two are payloads: see
/// `app::Kind::is_payload`, which `crate::graph::section_of` reads.
pub mod app;
/// What the vehicle itself runs: a computation, a log.
pub mod vehicle;
/// What the vehicle carries: a device on a bus, a line it drives, a link to
/// something aboard.
pub mod payload;
