//! The node that takes what the ground sends and drops it: a `null-gateway`.
//!
//! For a configuration that has to send what the ground sends somewhere and
//! has nowhere for it to go yet -- a command for a payload not yet fitted. It
//! has one terminal, a `gateway-in`, and no data source, so nothing can be
//! linked out of it. Its mirror is the `null-payload`, in
//! `super::null_payload`.
//!
//! It holds nothing and is told nothing: no attributes, which the
//! configuration check refuses, and nothing kept of what arrives. So there is
//! nothing here but this; the node, as the fabric holds it, is
//! `crate::runtime::port::NullPort`, which both null types are.
