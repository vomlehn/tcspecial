//! What a program is told about the programs it starts, in a web page,
//! where each is a thread of the page rather than a process.

use std::process::ExitStatus;

/// The signal a program was ended by: never, in a web page, where nothing
/// is ended by a signal.
///
/// # Arguments
///
/// * `_status` - How the program ended.
///
/// # Returns
///
/// `None`.
pub fn ended_by(_status: &ExitStatus) -> Option<i32> {
    None
}
