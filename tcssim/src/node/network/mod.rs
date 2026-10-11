//! A simulated device reached over a socket.
//!
//! The three transports a network node may run over, each a module of its
//! own. There is no shared code between them worth factoring: what a
//! datagram socket does and what a stream socket does differ in nearly every
//! line, and the thing they have in common -- the pacing -- is in
//! `crate::payload` with every other type's.
//!
//! They nest here rather than sitting beside network in node/, because the
//! configuration already treats them as one type with a protocol attribute: a
//! network node states which transport it runs over, where every other type is
//! addressed by what it is.

// The three transports this type covers. They nest here rather than sitting
// beside network in node/, because the configuration already treats them as
// one type with a protocol attribute -- a network node states which transport
// it runs over, where every other type is addressed by what it is.
pub mod tcp;
pub mod udp;
pub mod unix;
