//! The node that takes a payload's data and drops it: a `null-payload`.
//!
//! For a configuration that has to send a payload's data somewhere and has
//! nowhere for it to go yet -- a reading nobody is listening to. It has one
//! terminal, a `payload-in`, and no data source, so nothing can be linked out
//! of it. Its mirror is the `null-gateway`, in `super::null_gateway`.
//!
//! It holds nothing and is told nothing, as a `null-gateway` does not; the
//! node, as the fabric holds it, is `crate::runtime::port::NullPort`.
