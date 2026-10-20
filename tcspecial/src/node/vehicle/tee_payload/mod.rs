//! The node that takes a payload's data and sends it on both ways: a
//! `tee-payload`.
//!
//! The mirror of a `tee-moc`, in `super::tee_moc`: each datum a payload sends
//! arrives at its `payload-in` and is sent on by its `payload-out`, as it was
//! going, and again by its `moc-out` -- which is how a payload's data reaches
//! a `moc-in`, a link being unable to carry one stream into the other.
//!
//! What it holds is what a `tee-moc` holds, a datum waiting to be taken, so it
//! is the same [`Through`]: one queue, and one rule about a datum that will
//! not fit, for the two of them.

pub use super::tee_moc::Through;
