//! The example's ground end: it prints the line the vehicle sends.
//!
//! What stands in for ground software while this example runs. tcspecial
//! sends `lorem`'s words as UDP datagrams to the configuration's `moc` node,
//! and this program gets them from that node with one function call --
//! `tcslib::moc::Moc::receive` -- rather than opening a socket of its own: the
//! library works out from the configuration which port each source's data
//! arrives at, exactly as the vehicle does, and says which node each datum
//! came from.
//!
//! One datum is one word, which is how `lorem_pl.py` sent it. A datum holding
//! nothing but `simple_app::TERMINATOR` is the end of the line: it is written
//! out as well, and then this stops -- the line is whole, and a ground station
//! waiting for more would be waiting for a payload that has said everything.
//!
//! Arguments, both optional: the configuration, and the `moc` node to get the
//! data from. Run from the repository's root they need not be given. Once the
//! node is open it says so on standard error, so that whoever started this
//! knows the vehicle can be started.

use std::io::Write;
use std::process::ExitCode;

use simple_app::TERMINATOR;
use tcslib::moc::Moc;

/// The configuration, where nobody says otherwise.
const CONFIGURATION: &str = "examples/simple-app/simple-app.yaml";

/// The `moc` node in it that `lorem`'s words go to.
const MOC: &str = "moc";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let configuration = args.next().unwrap_or_else(|| CONFIGURATION.to_string());
    let moc_name = args.next().unwrap_or_else(|| MOC.to_string());

    let mut moc = match Moc::open_file(&configuration, &moc_name) {
        Ok(moc) => moc,
        Err(e) => {
            eprintln!("lorem_moc: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "lorem_moc: listening on {} for {}",
        moc.name(),
        moc.sources().join(", ")
    );

    let mut said_anything = false;
    loop {
        let received = match moc.receive(None) {
            Ok(Some(received)) => received,
            Ok(None) => continue,
            Err(e) => {
                eprintln!("lorem_moc: {e}");
                return ExitCode::FAILURE;
            }
        };

        let word = String::from_utf8_lossy(&received.data).to_string();

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
