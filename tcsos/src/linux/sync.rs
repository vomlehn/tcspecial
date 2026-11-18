//! Locks and the rest of what threads share: the standard library's, which
//! on Linux is the interface. In a browser, the page's own thread may not
//! wait for a lock, and takes one by trying until it is free.

pub use std::sync::*;
