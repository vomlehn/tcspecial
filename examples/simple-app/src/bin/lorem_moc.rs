//! The example's ground end: it prints the line the vehicle sends.
//!
//! What stands in for a ground station while this example runs. The
//! configuration's `moc` node says where the ground is -- its `ground_base` --
//! and the vehicle sends what each node linked into it produces to that
//! address, at that port plus the node's device id, from the start. So this
//! program binds the port `lorem`'s words arrive at and reads what arrives. It
//! does not have to say anything first: where the ground is, is written down.
//!
//! One packet is one word, which is how `lorem_pl.py` sent it. A packet
//! holding nothing but `simple_app::TERMINATOR` is the end of the line: it is
//! written out as well, and then this stops -- the line is whole, and a ground
//! station waiting for more would be waiting for a payload that has said
//! everything.
//!
//! The address is the one argument, for a configuration that was edited;
//! otherwise it is the one `simple-app.yaml` names. Once it is listening it
//! says so on standard error, so that whoever started it knows the vehicle can
//! be started.

use std::io::Write;
use std::net::UdpSocket;
use std::process::ExitCode;

use simple_app::TERMINATOR;

/// Where `lorem`'s words arrive, where nobody says otherwise: the `ground_base`
/// of the `moc` node in `simple-app.yaml`, 127.0.0.1:5100, plus `lorem`'s
/// device id, 1 -- it is the second node read, after the `moc` node.
const GROUND: &str = "127.0.0.1:5101";

fn main() -> ExitCode {
    let ground = std::env::args()
        .nth(1)
        .unwrap_or_else(|| GROUND.to_string());

    let socket = match UdpSocket::bind(&ground) {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("lorem_moc: cannot listen on {ground}: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("lorem_moc: listening on {ground}");

    let mut said_anything = false;
    let mut datum = [0u8; 4096];
    loop {
        let got = match socket.recv(&mut datum) {
            Ok(got) => got,
            Err(e) => {
                eprintln!("lorem_moc: the read failed: {e}");
                return ExitCode::FAILURE;
            }
        };

        let word = String::from_utf8_lossy(&datum[..got]).to_string();

        // The end of the line: written out, and then there is no more line to
        // wait for.
        if word == TERMINATOR {
            print!("{TERMINATOR}");
            let _ = std::io::stdout().flush();
            return ExitCode::SUCCESS;
        }

        // One space between the words and none before the first, which is the
        // line put back together.
        if said_anything {
            print!(" ");
        }
        print!("{word}");
        let _ = std::io::stdout().flush();
        said_anything = true;
    }
}
