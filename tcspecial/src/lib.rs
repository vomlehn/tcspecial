//! TCSpecial - Spacecraft Command Interpreter and Data Handler Manager
//!
//! TCSpecial is flight software: it runs on the spacecraft and manages
//! communication between ground operations and payloads.

pub mod beacon_send;
pub mod config;
pub mod endpoint;
pub mod node;
pub mod runtime;
pub mod segsend;
pub mod telemetry_log;
// The visual window, which never flies: see the `visual` feature.
#[cfg(feature = "visual")]
pub mod visual;

pub use beacon_send::*;
pub use config::*;
pub use endpoint::*;
#[cfg(feature = "device-file")]
pub use node::payload::device_file::*;
#[cfg(feature = "i2c")]
pub use node::payload::i2c::*;
#[cfg(feature = "raw")]
pub use node::payload::network::raw::*;
pub use node::payload::network::tcp::*;
pub use node::payload::network::udp::*;
pub use node::payload::network::unix::*;
pub use node::payload::network::*;
#[cfg(feature = "serial")]
pub use node::payload::serial::*;
#[cfg(feature = "spi")]
pub use node::payload::spi::*;
pub use telemetry_log::*;

/// Start something, waiting out a program that is still being written.
///
/// `Text file busy` -- `os error 26` -- is what `exec` answers while anything
/// holds the program open for writing, and in a test binary that includes this
/// process itself: a test in another thread writes a stand-in program, and a
/// third thread forks between its own fork and its exec, so the fork inherits
/// the open descriptor and the program cannot be run until it is closed. The
/// descriptor goes as soon as that exec happens, so the condition lasts
/// microseconds and has nothing to do with what is being tested.
///
/// Only for the tests, and only around starting a program from a file a test
/// has just written. Nothing in flight does that: a mission's app is installed
/// before the vehicle is powered, and `Text file busy` there would be a real
/// fault worth reporting rather than waiting out.
#[cfg(test)]
pub(crate) fn when_not_busy<T>(mut start: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    for _ in 0..200 {
        match start() {
            Err(why) if why.contains("os error 26") => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            said => return said,
        }
    }
    start()
}
