//! Check a tcspecial configuration file, and the simulator file beside it.
//!
//! ```text
//! tcsverify tests/manual/tcspecial2.yaml
//! tcsverify tests/manual/tcspecial2.yaml tests/manual/tcspecial2sim.yaml
//! ```
//!
//! Prints one line per problem -- file, line, and what is wrong -- and exits
//! with a status that says whether it found any, so `make` and a shell `&&`
//! can be told by it. See the library for what is checked.

use std::path::PathBuf;
use std::process::ExitCode;

/// What is said when the command line is not one of the two shapes.
///
/// A usage mistake is not a problem with a file, and the status says so with
/// a number of its own: a script that treats "no files named" as "the files
/// are bad" would be reporting a configuration error that nobody wrote.
const USAGE: &str = "\
usage: tcsverify <tcspecial configuration file> [payload simulator file]

Checks the files and prints every problem it finds, each as
file:line: what is wrong.

Exit status: 0 nothing wrong, 1 something wrong, 2 this message.";

fn main() -> ExitCode {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let (payload, sim) = match files.as_slice() {
        [payload] => (payload, None),
        [payload, sim] => (payload, Some(sim.as_path())),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    let report = tcsverify::verify(payload, sim);

    // To stdout, because they are what was asked for. A run with nothing to
    // say says nothing: this is a program that is run from a Makefile and
    // whose silence is the good answer.
    let mut out = std::io::stdout().lock();
    if let Err(e) = report.write(&mut out) {
        eprintln!("tcsverify: cannot write what it found: {e}");
        return ExitCode::from(2);
    }

    // Said once, at the end, and on stderr: it is not a problem with a file,
    // it is this program saying which of its two jobs it did.
    if sim.is_some() && !report.joined && report.found_errors() {
        eprintln!(
            "tcsverify: the simulator file was read but not checked against \
             {}, which has problems of its own",
            payload.display()
        );
    }

    ExitCode::from(u8::try_from(report.status()).unwrap_or(1))
}
