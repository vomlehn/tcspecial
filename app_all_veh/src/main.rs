//! An app that supports both data interfaces.
//!
//! The node type `app_all_veh`. Having both, it is the one kind that can carry
//! a stream *through*: payload data read on the first interface is written as
//! payload data on the second, and gateway data read on the second is written
//! as gateway data on the first. Which makes it a pass-through for both
//! streams, written as a program -- and that is what makes it the one to test
//! with, because a packet put in at either end comes out at the other and
//! nothing else does. An app that crossed the two streams would pass an echo
//! test and fail this one.
//!
//! One thread per stream, because both interfaces are read and a program
//! waiting on the one is not waiting on the other. Each thread reads one
//! interface and writes the other, so neither touches what the other does:
//! each direction holds handles of its own and the interfaces are shared
//! underneath them. See [`tcsapp::app::PayloadInterface::try_clone`].
//!
//! See `tcsapp` for the interfaces and the packets.

use std::io;
use std::process::ExitCode;
use std::thread;

use tcsapp::app::{complain, AppAllVeh, Errors};

/// Carries payload data from the first interface to the second and gateway data
/// from the second to the first, one thread per stream, until both end.
///
/// # Returns
///
/// Success when both streams ended as they should, failure when the
/// interfaces are not this kind's, cannot be held twice, or either stream
/// stopped on an error.
fn main() -> ExitCode {
    let app = match AppAllVeh::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("app_all_veh: {why}"));
            return ExitCode::FAILURE;
        }
    };
    let AppAllVeh {
        name,
        payload_arg: _,
        gateway_arg: _,
        mut payload,
        mut gateway,
        mut errors,
    } = app;

    // A handle apiece for the direction that writes what the other reads, and
    // for the complaints of each: a clone that cannot be made is a program
    // that cannot do half its work, which is said now rather than on the first
    // packet that needs it.
    let (mut writes_gateway, mut writes_payload, mut other_errors) =
        match (payload.try_clone(), gateway.try_clone(), errors.try_clone()) {
            (Ok(writes_gateway), Ok(writes_payload), Ok(other_errors)) => {
                (writes_gateway, writes_payload, other_errors)
            }
            _ => {
                let _ = errors.say(&format!(
                    "app_all_veh {name}: its interfaces could not be held \
                     twice over, and it carries one stream on each"
                ));
                return ExitCode::FAILURE;
            }
        };

    // Groundward: what the vehicle's payloads produced, on its way out.
    let groundward = thread::spawn(move || {
        carry(
            "payload data",
            || payload.read_payload(),
            |data| writes_payload.write_payload(data),
            &mut errors,
        )
    });

    // And vehicleward: what the ground sent, on its way in.
    let vehicleward = thread::spawn(move || {
        carry(
            "gateway data",
            || gateway.read_gateway(),
            |data| writes_gateway.write_gateway(data),
            &mut other_errors,
        )
    });

    // Both, and not the first to finish: one interface ending is not the
    // other's business, and a program that stopped on the first would drop
    // whatever was still arriving on the second. Collected before they are
    // asked about, so that both are joined: `all` stops at the first that did
    // not carry, and the second would be left running.
    let joined: Vec<bool> = [groundward, vehicleward]
        .into_iter()
        .map(|stream| stream.join().unwrap_or(false))
        .collect();
    let carried = joined.iter().all(|carried| *carried);

    if carried {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Carries one stream through, saying whether it ended the way it should.
///
/// # Arguments
///
/// * `what`   - What the stream is called, for the complaint: a program with
///              two of these has to say which of them stopped.
/// * `read`   - Reads the next packet, or `None` when the interface has ended.
/// * `write`  - Writes a packet on to the other interface.
/// * `errors` - Where a complaint is written.
///
/// # Returns
///
/// `true` when the interface ended, `false` when a packet could not be read
/// or passed on (the complaint has then been made).
fn carry(
    what: &str,
    mut read: impl FnMut() -> io::Result<Option<Vec<u8>>>,
    mut write: impl FnMut(&[u8]) -> io::Result<()>,
    errors: &mut Errors,
) -> bool {
    loop {
        match read() {
            Ok(Some(data)) => {
                if let Err(e) = write(&data) {
                    let _ = errors.say(&format!(
                        "app_all_veh: {} bytes of {what} could not be passed \
                         on: {e}",
                        data.len()
                    ));
                    return false;
                }
            }
            // The interface has ended, which is how an app is told that
            // nothing more is coming.
            Ok(None) => return true,
            Err(e) => {
                let _ = errors.say(&format!(
                    "app_all_veh: {what} could not be read: {e}"
                ));
                return false;
            }
        }
    }
}
