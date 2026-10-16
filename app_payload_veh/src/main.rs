//! An app that reads payload data and writes MOC data.
//!
//! The node type `app_payload_veh`, and one of the three programs the app node
//! types are tested with. What it does is answer: every packet of payload data
//! that arrives comes back as a packet of MOC data holding the same bytes. An
//! echo is the only shape available to this kind of app -- it has the one
//! interface, on which payload data is read and MOC data written -- and it is
//! the shape a test wants, because what arrives being exactly what was sent is
//! a thing a test can assert without knowing anything about the app.
//!
//! Descriptor 1 is not open. Nothing here reaches for it, and `tcsapp` refuses
//! to start if it is open anyway: a program run as a kind of app it is not
//! would write the vehicle's data wherever that descriptor happens to go.
//!
//! See `tcsapp` for the interfaces and the packets, and `tcsapp::parent` for
//! the other end of them.

use std::process::ExitCode;

use tcsapp::app::{complain, AppPayloadVeh};

fn main() -> ExitCode {
    // The interfaces first, and nothing else if they are not what this kind of
    // app is run with: see `tcsapp::app::complain` for why the complaint goes
    // where it does.
    let mut app = match AppPayloadVeh::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("app_payload_veh: {why}"));
            return ExitCode::FAILURE;
        }
    };

    loop {
        match app.payload.read_payload() {
            // Answered with the same bytes, as MOC data.
            Ok(Some(data)) => {
                if let Err(e) = app.payload.write_moc(&data) {
                    let _ = app.errors.say(&format!(
                        "app_payload_veh: {} bytes of MOC data could not be written: {e}",
                        data.len()
                    ));
                    return ExitCode::FAILURE;
                }
            }
            // The interface has ended, which is how an app is told that
            // nothing more is coming: finished, and not a failure.
            Ok(None) => return ExitCode::SUCCESS,
            Err(e) => {
                let _ = app
                    .errors
                    .say(&format!("app_payload_veh: payload data could not be read: {e}"));
                return ExitCode::FAILURE;
            }
        }
    }
}
