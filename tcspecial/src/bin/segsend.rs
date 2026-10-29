//! segsend: what tcspecial hands each filled telemetry log segment to.
//!
//! `segsend <file>`. tcspecial runs it, from beside its own executable, once
//! for every segment a log has filled, with the segment's name -- renamed out
//! of the log by then, ending in `.segsend` -- and waits for it to finish
//! before running it on the next. See `tcspecial::segsend`.
//!
//! A mission puts its own program here, one that sends the file to the
//! ground. This one is for testing, and deletes the file: what it shows is
//! that every filled segment reaches it, and a log directory that empties as
//! the vehicle runs is the evidence.

use std::process::ExitCode;

/// Deletes the one file named on the command line, the filled segment.
///
/// # Returns
///
/// Success once the file is deleted; 2 for a command line that does not
/// name exactly one file, and failure for a file that cannot be deleted,
/// each said on stderr.
fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(file), None) = (args.next(), args.next()) else {
        eprintln!("segsend: give it one file, the filled segment to send");
        return ExitCode::from(2);
    };
    match std::fs::remove_file(&file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "segsend: {} could not be deleted: {e}",
                std::path::Path::new(&file).display()
            );
            ExitCode::FAILURE
        }
    }
}
