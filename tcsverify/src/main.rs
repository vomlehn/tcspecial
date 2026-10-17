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
usage: tcsverify <tcspecial configuration file> [payload simulator file]
       tcsverify <configuration> <configuration> <payload simulator file>
       tcsverify -h | --help

Checks the files and prints every problem it finds, each as
file:line: what is wrong.

With one file, checks that tcspecial configuration. With two, checks the
configuration and the payload simulator file, and that the two agree.

With three, the first two are one configuration written twice -- one an
XML file and one a YAML file, in either order -- and the third is their
payload simulator file. Each configuration is checked with the simulator
file, the first first, and then the two are compared: every section,
attribute and value, which must be the same for the two to be read as one
configuration.

  -h, --help   print this message and exit

Exit status: 0 nothing wrong, 1 something wrong, 2 the command line was
not one of these.";

fn main() -> ExitCode {
    let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    // Asked for, so on stdout and a success: the message is the answer.
    if files.iter().any(|file| file.as_os_str() == "-h" || file.as_os_str() == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let report = match files.as_slice() {
        [payload] => tcsverify::verify(payload, None),
        [payload, sim] => tcsverify::verify(payload, Some(sim)),
        [first, second, sim] => {
            if let Err(why) = one_of_each(first, second) {
                eprintln!("tcsverify: {why}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            tcsverify::verify_pair(first, second, sim)
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
