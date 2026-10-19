//! Check a tcspecial configuration file, and the simulator file beside it.
//!
//! ```text
//! tcsverify tests/manual/tcspecial2.yaml
//! tcsverify tests/manual/tcspecial2.yaml tests/manual/tcspecial2sim.yaml
//! tcsverify tests/manual/tcspecial2.xml tests/manual/tcspecial2.yaml tests/manual/tcspecial2sim.yaml
//! tcsverify --digest tests/manual/tcspecial2.yaml
//! ```
//!
//! Prints one line per problem -- file, line, and what is wrong -- and exits
//! with a status that says whether it found any, so `make` and a shell `&&`
//! can be told by it. See the library for what is checked.
//!
//! Three files are two spellings of one configuration, one XML and one YAML,
//! and their simulator file: each spelling is checked with the simulator
//! file, and then the two are compared, every section, attribute and value.

use std::io::Write;
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
file:line: what is wrong, in the order it finds them. Given a limit, it
stops as soon as it has printed that many errors.

With one file, checks that tcspecial configuration. With two, checks the
configuration and the payload simulator file, and that the two agree.

With three, the first two are one configuration written twice -- one an
XML file and one a YAML file, in either order -- and the third is their
payload simulator file. Each configuration is checked with the simulator
file, the first first, and then the two are compared: every section,
attribute and value, which must be the same for the two to be read as one
configuration.

  --digest           print each configuration's digest as well, one line
                     per file, before the problems. With two files that is
                     the first of them and with three it is the first two:
                     a payload simulator file has none. The digest is of
                     what a file says and not of its bytes, so the two
                     spellings of one configuration have the same digest
                     and `md5sum` of the file does not. It is the digest
                     tcspecial prints as it starts and puts in every
                     beacon, which the MOC compares with its own to know
                     the vehicle is flying the configuration on the desk
  --error-limit=<n>  stop after <n> errors: a whole number, 0 included,
                     or \"unlimited\", which is the default and looks
                     at everything
  --warnings=<what>  on: print warnings, not counting them (the default);
                     off: print none; errors: print them and count them
                     against the limit as errors are
  -h, --help         print this message and exit

Exit status: 0 no error was printed -- a run that stopped before it found
one included, as --error-limit=0 always does, and warnings never fail a
run -- 1 an error was printed, or a digest was asked for and the file
could not be read to take one, 2 the command line was not one of these.";

/// How many errors are printed before the run stops looking: every one
/// unless asked.
///
/// Nought is a limit like any other: whether the files are good is the exit
/// status, which does not depend on how much of the reason was printed, so a
/// script that wants only the answer may ask for none of the reasons and is
/// answered at the first error.
const ERROR_LIMIT_FLAG: &str = "--error-limit=";

/// What is done with warnings: printed and not counted unless asked.
const WARNINGS_FLAG: &str = "--warnings=";

/// Whether to print each configuration's digest as well as its problems.
///
/// Takes no value, where the other two do: there is one digest of a
/// configuration and nothing to choose about it.
const DIGEST_FLAG: &str = "--digest";

fn main() -> ExitCode {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut limits = tcsverify::Limits::EVERYTHING;
    let mut digest = false;
    for arg in std::env::args_os().skip(1) {
        let said = arg.to_string_lossy();
        // Asked for, so on stdout and a success: the message is the answer.
        if said == "-h" || said == "--help" {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        if said == DIGEST_FLAG {
            digest = true;
            continue;
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
    // What was asked for comes before what was found: a digest is the answer
    // to a question, and the problems are what the files have to say for
    // themselves.
    let mut digest_failed = false;
    if digest {
        for path in configurations(&files) {
            match tcslibgs::config_digest::digest_of_file(path) {
                Ok(said) => {
                    if let Err(e) = writeln!(out, "{}: md5 {said}", path.display()) {
                        eprintln!("tcsverify: cannot write the digest: {e}");
                        return ExitCode::from(2);
                    }
                }
                // On stderr and remembered rather than returned on: the
                // problems found in the file are the reason there is no
                // digest, and a reader wants them printed. A digest asked for
                // and not given is a failed run even where the checking found
                // nothing to count -- `--error-limit=0` prints no errors, and
                // without this the run would end silently and successfully
                // having answered nothing.
                Err(why) => {
                    eprintln!("tcsverify: {}: no digest: {why}", path.display());
                    digest_failed = true;
                }
            }
        }
    }

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

    if digest_failed {
        return ExitCode::from(1);
    }

    ExitCode::from(u8::try_from(report.status()).unwrap_or(1))
}

/// The configurations among the pathnames: every one but the simulator file.
///
/// One pathname is a configuration. Two are a configuration and its simulator
/// file, and three are one configuration written twice and their simulator
/// file -- so the simulator file is the last of two or three, and the rest are
/// configurations.
///
/// A simulator file has no digest of this kind. It is not a graph, so there is
/// nothing to digest it over, and it does not fly: the digest exists so that
/// the vehicle and the ground can prove they read the same configuration, and
/// nothing aboard reads a simulator file.
fn configurations(files: &[PathBuf]) -> &[PathBuf] {
    match files.len() {
        0 => &[],
        1 => &files[..1],
        other => &files[..other - 1],
    }
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
