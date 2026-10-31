//! An app that reads gateway data and writes payload data.
//!
//! The node type `app_gateway_veh`, and the mirror of `app_payload_veh`: every
//! packet of gateway data that arrives comes back as a packet of payload data
//! holding the same bytes. It has the one interface, on which gateway data is
//! read and payload data written, so an echo is the only shape available to it
//! -- and the shape a test wants.
//!
//! Descriptor 0 is not open, and `tcsapp` refuses to start if it is.
//!
//! See `tcsapp` for the interfaces and the packets.

use std::process::ExitCode;

use tcsapp::app::{complain, AppGatewayVeh};

/// Writes every packet of gateway data that arrives back as payload data
/// holding the same bytes, until the interface ends.
///
/// # Returns
///
/// Success when the interface ends, failure when the interfaces are not this
/// kind's or a packet cannot be read or written.
fn main() -> ExitCode {
    let app = match AppGatewayVeh::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("app_gateway_veh: {why}"));
            return ExitCode::FAILURE;
        }
    };

    // Which node this process is, for every complaint it makes: see
    // `tcsapp::app`.
    let AppGatewayVeh {
        name,
        gateway_arg: _,
        mut gateway,
        mut errors,
    } = app;

    loop {
        match gateway.read_gateway() {
            Ok(Some(data)) => {
                if let Err(e) = gateway.write_payload(&data) {
                    let _ = errors.say(&format!(
                        "app_gateway_veh {name}: {} bytes of payload data \
                         could not be written: {e}",
                        data.len()
                    ));
                    return ExitCode::FAILURE;
                }
            }
            Ok(None) => return ExitCode::SUCCESS,
            Err(e) => {
                let _ = errors.say(&format!(
                    "app_gateway_veh {name}: gateway data could not be \
                     read: {e}"
                ));
                return ExitCode::FAILURE;
            }
        }
    }
}
