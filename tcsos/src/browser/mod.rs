//! The browser's substitutes for what `tcsos` offers, for WebAssembly in a
//! web page, where every program is threads of one: see [`context`].

pub mod child;
pub mod console;
pub mod context;
pub mod env;
pub mod fs;
pub mod interfaces;
pub mod memory;
pub mod net;
pub mod process;
pub mod signals;
pub mod thread;
pub mod time;
pub mod wait;
