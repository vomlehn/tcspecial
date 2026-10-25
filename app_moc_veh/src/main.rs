//! An app that reads MOC data and writes payload data.
//!
//! The node type `app_moc_veh`, and the mirror of `app_payload_veh`: every
//! packet of MOC data that arrives comes back as a packet of payload data
//! holding the same bytes. It has the one interface, on which MOC data is read
//! and payload data written, so an echo is the only shape available to it --
//! and the shape a test wants.
//!
//! Descriptor 0 is not open, and `tcsapp` refuses to start if it is.
//!
//! See `tcsapp` for the interfaces and the packets.

use std::process::ExitCode;

use tcsapp::app::{complain, AppMocVeh};

fn main() -> ExitCode {
    let app = match AppMocVeh::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("app_moc_veh: {why}"));
            return ExitCode::FAILURE;
        }
    };

    // Which node this process is, for every complaint it makes: see
    // `tcsapp::app`.
    let AppMocVeh {
        name,
        moc_arg: _,
        mut moc,
        mut errors,
    } = app;

    loop {
        match moc.read_moc() {
            Ok(Some(data)) => {
                if let Err(e) = moc.write_payload(&data) {
                    let _ = errors.say(&format!(
                        "app_moc_veh {name}: {} bytes of payload data could \
                         not be written: {e}",
                        data.len()
                    ));
                    return ExitCode::FAILURE;
                }
            }
            Ok(None) => return ExitCode::SUCCESS,
            Err(e) => {
                let _ = errors.say(&format!(
                    "app_moc_veh {name}: MOC data could not be read: {e}"
                ));
                return ExitCode::FAILURE;
            }
        }
    }
}
