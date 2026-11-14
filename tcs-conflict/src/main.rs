//! tcs-conflict: whether two tcspecial configurations can be run at once.
//!
//! Two tcspecials on one machine, or on one network, must not want the same
//! socket, device, place on a bus, state file or log directory: the second
//! would fail as it started, or each would take part of what the other is
//! given. This checks each configuration as tcs-verify does, and then
//! whether the two want anything in common, with the files and lines that
//! say so. See `tcsverify::conflict`, which is the check, and
//! `tcsprotocol::graph::conflict`, which is the rules.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tcsverify::conflict::{conflicts, Given};

/// What the program is run as, and what it says when run otherwise.
const USAGE: &str = "\
Usage: tcs-conflict [OPTION]... FIRST SECOND
       tcs-conflict [OPTION]... FIRST FIRST-SIM SECOND SECOND-SIM
       tcs-conflict -h | --help

Checks whether two tcspecial configurations (.ytcs or .xtcs) can be run at
once: each as tcs-verify checks it, with its payload simulator file where
one is given, and then everything both want -- a socket, a device, an
address on a bus, a GPIO line, a state file, a log directory, the
directory configurations are kept in, and where tcs-sim answers. Each
conflict is printed against the second configuration, at the line that
says it, naming the first's file and line.

Options:
";

/// What follows the options in `--help`.
const USAGE_END: &str = "
  -h, --help         print this message and exit

Exit status: 0 nothing is wrong with either and they want nothing in
common, or only warnings were found; 1 an error was printed, or a warning
under --fatal-warnings; 2 the command line was not one of these.";

/// Prints how the program is run, on stdout where it was asked for and on
/// stderr where the command line was wrong.
///
/// # Arguments
///
/// * `asked` - Whether `--help` asked for it.
fn usage(asked: bool) {
    let text = format!("{USAGE}{}{USAGE_END}", tcsverify::flags::USAGE);
    if asked {
        println!("{text}");
    } else {
        eprintln!("{text}");
    }
}

/// Whether a file is named as a configuration is: `.ytcs` or `.xtcs`.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// Whether it is.
fn is_configuration(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ytcs") | Some("xtcs")
    )
}

/// Reads the command line, checks the two configurations and prints what
/// it found.
///
/// # Returns
///
/// The exit status `USAGE` describes.
fn main() -> ExitCode {
    let mut limits = tcsverify::Limits::EVERYTHING;
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args_os().skip(1) {
        let said = arg.to_string_lossy();
        if said == "-h" || said == "--help" {
            usage(true);
            return ExitCode::SUCCESS;
        }
        match tcsverify::flags::take(&said, &mut limits) {
            Some(Ok(())) => continue,
            Some(Err(why)) => {
                eprintln!("tcs-conflict: {why}\n");
                usage(false);
                return ExitCode::from(2);
            }
            None if said.starts_with('-') => {
                eprintln!("tcs-conflict: {said} is not an option\n");
                usage(false);
                return ExitCode::from(2);
            }
            None => files.push(PathBuf::from(&arg)),
        }
    }
    if let Err(why) = tcsverify::flags::agree(&limits) {
        eprintln!("tcs-conflict: {why}\n");
        usage(false);
        return ExitCode::from(2);
    }

    let (one, two) = match files.as_slice() {
        [first, second] => (
            Given {
                configuration: first,
                simulator: None,
            },
            Given {
                configuration: second,
                simulator: None,
            },
        ),
        [first, first_sim, second, second_sim] => (
            Given {
                configuration: first,
                simulator: Some(first_sim),
            },
            Given {
                configuration: second,
                simulator: Some(second_sim),
            },
        ),
        _ => {
            eprintln!(
                "tcs-conflict: give two configurations, or two \
                 configurations each followed by its simulator file\n"
            );
            usage(false);
            return ExitCode::from(2);
        }
    };
    // A simulator file where a configuration belongs is a command line in
    // the wrong order, which would otherwise be read as two configurations
    // that do not load.
    for given in [one, two] {
        if !is_configuration(given.configuration) {
            eprintln!(
                "tcs-conflict: {} is not a configuration, which is named \
                 .ytcs or .xtcs: give each configuration before its \
                 simulator file\n",
                given.configuration.display()
            );
            usage(false);
            return ExitCode::from(2);
        }
    }

    let report = conflicts(one, two, limits);
    if let Err(e) = report.write(&mut std::io::stdout().lock()) {
        eprintln!("tcs-conflict: cannot write what was found: {e}");
        return ExitCode::from(2);
    }
    ExitCode::from(u8::try_from(report.status()).unwrap_or(2))
}
