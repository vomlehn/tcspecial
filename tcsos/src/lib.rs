//! Everything the TCS programs ask of the operating system, in one place.
//!
//! The rest of the TCS crates are written against what this crate offers,
//! not against an operating system. Each module here says what it offers
//! once, in words that hold everywhere, and has an implementation for each
//! system the programs are built for:
//!
//! * `linux/`, for Linux, which is what flies. It is the only code in the
//!   TCS crates that names `libc`, `nix` or `std::os::unix`. Where the
//!   standard library already does a thing on every system, the rest of
//!   the crates call it directly, and nothing for it is here.
//! * `browser/`, for WebAssembly in a web page, which is how the examples
//!   are shown to anyone with a browser and nothing installed. It has the
//!   substitutes: a thread is a Web Worker, a socket is a channel inside
//!   the page, a file is held in memory.
//!
//! A module offers the same functions, with the same meaning, from both.
//! Where a system cannot do what a function asks -- a browser cannot lock
//! memory, nor send a signal -- its implementation says so in its own
//! documentation and does the nearest thing that is honest about it.
//!
//! What Linux alone has, and a browser has no substitute for -- the device
//! nodes of real hardware -- is under [`linux`] alone, behind
//! `cfg(target_os = "linux")`, for the crates that drive hardware to
//! reach.

// What a program thread prints, shown in a page's console: see
// browser::console. The page is built with nightly Rust in any case.
#![cfg_attr(target_arch = "wasm32", feature(internal_output_capture))]

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
use linux as system;

#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(target_arch = "wasm32")]
use browser as system;

pub use system::{
    child, env, fs, interfaces, memory, net, process, signals, sync, thread,
    time, wait,
};
