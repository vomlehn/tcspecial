//! Turns a tcspecial configuration into the Rust that builds it.
//!
//! Two paths: the configuration to read and the `.rs` file to write. It opens
//! no socket and no device node, reads one file and writes one, so it can be
//! run anywhere -- by a person, by a Makefile, or by tcsgui's Edit menu, which
//! passes the configuration it is editing and the path from the
//! `rust-code-path` attribute.
//!
//! What it writes and why is [`tcsrustgen`]'s own documentation; this is the
//! command line and nothing else.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: tcsrustgen <configuration file> <rust file>";

fn main() -> ExitCode {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let (configuration, output) = match files.as_slice() {
        [configuration, output] => (configuration.as_path(), output.as_path()),
        // Said on the error output and answered with a status of its own: a
        // command line that is not one of the shapes above is not a statement
        // about any file, and a recipe should be able to tell the two apart.
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match tcsrustgen::generate(configuration, output) {
        Ok(()) => {
            // Said, because a program that writes a file somebody named and
            // nothing else leaves them wondering whether it ran. One line, on
            // the standard output, so a recipe can keep it or drop it.
            println!("{}", said(configuration, output));
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("tcsrustgen: {why}");
            ExitCode::FAILURE
        }
    }
}

/// What it did, in one line.
fn said(configuration: &Path, output: &Path) -> String {
    format!("{} -> {}", configuration.display(), output.display())
}
