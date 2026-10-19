//! Check a tcspecial configuration file, and the simulator file beside it.
//!
//! ```text
//! tcsverify tests/manual/tcspecial2.yaml
//! tcsverify tests/manual/tcspecial2.yaml tests/manual/tcspecial2sim.yaml
//! tcsverify tests/manual/tcspecial2.xml tests/manual/tcspecial2.yaml tests/manual/tcspecial2sim.yaml
//! ```
//!
//! Prints one line per problem -- file, line, and what is wrong -- and exits
//! with a status that says whether it found any, so `make` and a shell `&&`
//! can be told by it. See the library for what is checked.
//!
//! Three files are two spellings of one configuration, one XML and one YAML,
//! and their simulator file: each spelling is checked with the simulator
//! file, and then the two are compared, every section, attribute and value.

use std::path::PathBuf;
use std::process::ExitCode;

/// What is said when the command line is not one of the two shapes.
///
/// A usage mistake is not a problem with a file, and the status says so with
/// a number of its own: a script that treats "no files named" as "the files
/// are bad" would be reporting a configuration error that nobody wrote.
const USAGE: &str = "\
usage: tcsverify [options] <tcspecial configuration file> [payload simulator file]
       tcsverify [options] <configuration> <configuration> <payload simulator file>
       tcsverify -h | --help

Checks the files and prints the problems it finds, each as
file:line: what is wrong, in the order it finds them. It stops as soon
as it has printed as many as its limit.

With one file, checks that tcspecial configuration. With two, checks the
configuration and the payload simulator file, and that the two agree.

With three, the first two are one configuration written twice -- one an
XML file and one a YAML file, in either order -- and the third is their
payload simulator file. Each configuration is checked with the simulator
file, the first first, and then the two are compared: every section,
attribute and value, which must be the same for the two to be read as one
configuration.

  --error-limit=<n>  stop after <n> errors, five unless asked: a whole
                     number, 0 included, or \"unlimited\" to look at
                     everything
  --warnings=<what>  on: print warnings, not counting them (the default);
                     off: print none; errors: print them and count them
                     against the limit as errors are
  -h, --help         print this message and exit

Exit status: 0 no error was printed -- a run that stopped before it found
one included, as --error-limit=0 always does, and warnings never fail a
run -- 1 an error was printed, 2 the command line was not one of these.";

/// How many errors are printed before the run stops looking, five unless
/// asked.
///
/// Nought is a limit like any other: whether the files are good is the exit
/// status, which does not depend on how much of the reason was printed, so a
/// script that wants only the answer may ask for none of the reasons and is
/// answered at the first error.
const ERROR_LIMIT_FLAG: &str = "--error-limit=";

/// The limit when none is asked for.
const DEFAULT_ERROR_LIMIT: usize = 5;

/// What is done with warnings: printed and not counted unless asked.
const WARNINGS_FLAG: &str = "--warnings=";

fn main() -> ExitCode {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut limits = tcsverify::Limits {
        error_limit: Some(DEFAULT_ERROR_LIMIT),
        warnings: tcsverify::Warnings::On,
    };
    for arg in std::env::args_os().skip(1) {
        let said = arg.to_string_lossy();
        // Asked for, so on stdout and a success: the message is the answer.
        if said == "-h" || said == "--help" {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        let asked = if let Some(value) = said.strip_prefix(ERROR_LIMIT_FLAG) {
            error_limit(value).map(|limit| limits.error_limit = limit)
        } else if let Some(value) = said.strip_prefix(WARNINGS_FLAG) {
            warnings(value).map(|warnings| limits.warnings = warnings)
        } else {
            files.push(PathBuf::from(&arg));
            continue;
        };
        if let Err(why) = asked {
            eprintln!("tcsverify: {why}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    let report = match files.as_slice() {
        [payload] => tcsverify::verify(payload, None, limits),
        [payload, sim] => tcsverify::verify(payload, Some(sim), limits),
        [first, second, sim] => {
            if let Err(why) = one_of_each(first, second) {
                eprintln!("tcsverify: {why}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            tcsverify::verify_pair(first, second, sim, limits)
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let sim = match files.as_slice() {
        [_, sim] | [_, _, sim] => Some(sim),
        _ => None,
    };
    let payload = &files[0];

    // To stdout, because they are what was asked for. A run with nothing to
    // say says nothing: this is a program that is run from a Makefile and
    // whose silence is the good answer.
    let mut out = std::io::stdout().lock();
    // Every one recorded, in the order found: the limit was applied as the
    // run looked.
    if let Err(e) = report.write(&mut out) {
        eprintln!("tcsverify: cannot write what it found: {e}");
        return ExitCode::from(2);
    }

    // Said once, at the end, and on stderr: it is not a problem with a file,
    // it is this program saying which of its two jobs it did.
    if sim.is_some() && !report.joined && report.found_errors() {
        let against = match files.as_slice() {
            [first, second, _] => format!(
                "both of {} and {}, one of which has problems of its own",
                first.display(),
                second.display()
            ),
            _ => format!("{}, which has problems of its own", payload.display()),
        };
        eprintln!("tcsverify: the simulator file was read but not checked against {against}");
    }

    ExitCode::from(u8::try_from(report.status()).unwrap_or(1))
}

/// What `--error-limit=` was given, as how many to list: `None` for every one.
fn error_limit(value: &str) -> Result<Option<usize>, String> {
    if value == "unlimited" {
        return Ok(None);
    }
    value.parse::<usize>().map(Some).map_err(|_| {
        format!(
            "{ERROR_LIMIT_FLAG}{value}: the limit is a whole number, 0 included, \
             or \"unlimited\""
        )
    })
}

/// What `--warnings=` was given.
fn warnings(value: &str) -> Result<tcsverify::Warnings, String> {
    use tcsverify::Warnings;

    match value {
        "on" => Ok(Warnings::On),
        "off" => Ok(Warnings::Off),
        "errors" => Ok(Warnings::Errors),
        _ => Err(format!(
            "{WARNINGS_FLAG}{value}: warnings are \"on\", \"off\" or \"errors\""
        )),
    }
}

/// Whether two configurations are one XML file and one YAML file, in either
/// order, which is what comparing two spellings of one configuration is.
fn one_of_each(first: &std::path::Path, second: &std::path::Path) -> Result<(), String> {
    use tcslibgs::ConfigFormat;

    let format = |path: &std::path::Path| {
        ConfigFormat::of_file(path).map_err(|e| format!("{}: {e}", path.display()))
    };
    match (format(first)?, format(second)?) {
        (ConfigFormat::Xml, ConfigFormat::Yaml) | (ConfigFormat::Yaml, ConfigFormat::Xml) => Ok(()),
        _ => Err(format!(
            "{} and {} are compared as two spellings of one configuration, so \
             one must be an XML file and the other a YAML file",
            first.display(),
            second.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole number, nought included, or "unlimited"; anything else is a
    /// command line that is not one of the shapes, and is told so.
    #[test]
    fn an_error_limit_is_a_number_or_unlimited() {
        assert_eq!(error_limit("0"), Ok(Some(0)));
        assert_eq!(error_limit("5"), Ok(Some(5)));
        assert_eq!(error_limit("250"), Ok(Some(250)));
        assert_eq!(error_limit("unlimited"), Ok(None));
        for bad in ["", "-1", "five", "Unlimited", "5.0", "all"] {
            assert!(error_limit(bad).is_err(), "{bad:?} was taken as a limit");
        }
    }

    /// On, off or errors, spelled so; anything else is told so.
    #[test]
    fn warnings_are_on_off_or_errors() {
        use tcsverify::Warnings;

        assert_eq!(warnings("on"), Ok(Warnings::On));
        assert_eq!(warnings("off"), Ok(Warnings::Off));
        assert_eq!(warnings("errors"), Ok(Warnings::Errors));
        for bad in ["", "On", "none", "error", "yes"] {
            assert!(warnings(bad).is_err(), "{bad:?} was taken");
        }
    }
}
