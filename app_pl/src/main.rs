//! A payload that is a program and is never asked: it reports of its own
//! accord.
//!
//! The node type `app_pl`. It has the one interface, at descriptor 1, and only
//! writes on it: payload data, which is a reading on its way to the ground.
//! Nothing is linked into a node of this kind, so there is nothing to read and
//! `tcsapp` hands it a type with no read on it.
//!
//! What it reports is its payload argument and a count, one packet at a time,
//! a tenth of a second apart. A real payload of this kind writes what it has
//! sampled, and when it writes is its own business -- which is the whole of
//! what the kind is for -- so this one's business is to say what it was told:
//! the argument proves the configuration's word reached the program, and the
//! count lets a test tell one packet from the next. The interval is what keeps
//! a program with nothing to read from spinning.
//!
//! It stops when nobody is listening. The interface going away is not a
//! failure: a payload whose vehicle has stopped reading it has nothing left to
//! do, which is what tcspecial's `no_more` and `stop` both come to in the end.
//!
//! Descriptor 0 is not open, and `tcsapp` refuses to start if it is.
//!
//! See `tcsapp` for the interfaces and the packets.

use std::io;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::Duration;

use tcsapp::app::{complain, AppPl};

/// How long between one reading and the next.
///
/// Its own, there being no interface to be told on. Short enough that a test
/// waiting for the second packet is not waiting long, long enough that a
/// program with nothing else to do is not spinning.
const REPORTS_EVERY: Duration = Duration::from_millis(100);

fn main() -> ExitCode {
    let app = match AppPl::inherited() {
        Ok(app) => app,
        Err(why) => {
            complain(&format!("app_pl: {why}"));
            return ExitCode::FAILURE;
        }
    };
    // Which node this process is, and what it was told about the one stream it
    // handles: see `tcsapp::app`.
    let AppPl {
        name,
        payload_arg,
        mut payload,
        mut errors,
    } = app;

    let mut count: u64 = 0;
    loop {
        count += 1;
        let reading = format!("{payload_arg} {count}");
        match payload.write_payload(reading.as_bytes()) {
            Ok(()) => {}
            // Nobody is reading: the vehicle has finished with this payload,
            // which is an end and not a fault.
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => return ExitCode::SUCCESS,
            Err(e) => {
                let _ = errors.say(&format!(
                    "app_pl {name}: reading {count} could not be written: {e}"
                ));
                return ExitCode::FAILURE;
            }
        }
        sleep(REPORTS_EVERY);
    }
}
