//! An app that misuses the standard streams, for tcsapp's own tests: see
//! `tests/std_streams.rs`. Not one of the five the app node types are tested
//! with, and not installed.
//!
//! An `app_payload_veh` that, once it holds its interfaces, prints to stdout
//! and stderr and reads stdin, which is what a careless app does. It then
//! says, as MOC data, how many bytes stdin gave, and echoes payload data as
//! MOC data -- until a packet says `panic`, when it panics.

use std::io::Read;
use std::process::ExitCode;

use tcsapp::app::{complain, AppPayloadVeh};

/// Prints, reads stdin, reports what it read, then echoes until told to
/// panic.
///
/// # Returns
///
/// Success when the interface ends, failure when the interfaces are not this
/// kind's or a packet cannot be read or written.
fn main() -> ExitCode {
    let mut app = match AppPayloadVeh::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("stray_app: {why}"));
            return ExitCode::FAILURE;
        }
    };
    println!("stray text on stdout");
    eprintln!("stray text on stderr");
    let mut stdin = Vec::new();
    let read = std::io::stdin().read_to_end(&mut stdin);
    let report = format!("stdin gave {:?}", read.map(|_| stdin.len()).ok());
    if app.payload.write_moc(report.as_bytes()).is_err() {
        return ExitCode::FAILURE;
    }
    loop {
        match app.payload.read_payload() {
            Ok(Some(data)) if data == b"panic" => panic!("asked to panic"),
            Ok(Some(data)) => {
                if app.payload.write_moc(&data).is_err() {
                    return ExitCode::FAILURE;
                }
            }
            Ok(None) => return ExitCode::SUCCESS,
            Err(_) => return ExitCode::FAILURE,
        }
    }
}
