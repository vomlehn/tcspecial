//! A payload that is a program and is asked: it answers what arrives.
//!
//! The node type `app_all_pl`. It has the one interface, at descriptor 1, on
//! which what asks it for a reading is read and the reading is written: so
//! every packet that arrives comes back as a packet of payload data holding
//! the same bytes.
//!
//! An echo, which is the shape a test wants and is also the thing being
//! tested: a payload is asked and answers, and answering with what was asked
//! is what makes the pairing visible -- a packet that came back out of the
//! interface it was *not* going to would be the streams crossed, and here it
//! would be the same bytes and so unmistakable.
//!
//! The mirror of `app_moc_veh`, whose program holds the same interface and
//! does the same two things with it. What tells the two node types apart is
//! which side of the graph each stands on, which is the configuration's
//! business and not the program's.
//!
//! Descriptor 0 is not open, and `tcsapp` refuses to start if it is.
//!
//! See `tcsapp` for the interfaces and the packets.

use std::process::ExitCode;

use tcsapp::app::{complain, AppAllPl};

fn main() -> ExitCode {
    let app = match AppAllPl::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("app_all_pl: {why}"));
            return ExitCode::FAILURE;
        }
    };

    // Which node this process is, for every complaint it makes: see
    // `tcsapp::app`.
    let AppAllPl {
        name,
        payload_arg: _,
        moc_arg: _,
        mut moc,
        mut errors,
    } = app;

    loop {
        match moc.read_moc() {
            Ok(Some(data)) => {
                if let Err(e) = moc.write_payload(&data) {
                    let _ = errors.say(&format!(
                        "app_all_pl {name}: a reading of {} bytes could not be \
                         written: {e}",
                        data.len()
                    ));
                    return ExitCode::FAILURE;
                }
            }
            // Nothing more will be asked of it: finished, and not a failure.
            Ok(None) => return ExitCode::SUCCESS,
            Err(e) => {
                let _ = errors.say(&format!(
                    "app_all_pl {name}: what asks for a reading could not be \
                     read: {e}"
                ));
                return ExitCode::FAILURE;
            }
        }
    }
}
