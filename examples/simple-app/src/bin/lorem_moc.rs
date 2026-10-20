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
//! Arguments, all optional: the configuration, the `moc` node to get the data
//! from, `--yaml` or `--xml` but not both -- which of the example's two files
//! to read, YAML where neither is given -- and `-v` or `--verbose`. Run from
//! the repository's root none need be given. Quiet unless `-v` or `--verbose` is given: the line is what this
//! prints, on standard output, and errors are said on standard error whatever
//! the flag. Verbose, it also says on standard error when the node is open,
//! so that whoever started this knows the vehicle can be started.

use std::io::Write;
use std::process::ExitCode;

use simple_app::TERMINATOR;
use tcslib::moc::Moc;

/// The example's configuration, without its extension: `.yaml` or `.xml`,
/// the two saying the same thing.
const CONFIGURATION: &str = "examples/simple-app/simple-app";

/// The spelling read where neither `--yaml` nor `--xml` is given.
const SPELLING: tcslib::ConfigFormat = tcslib::ConfigFormat::Yaml;

/// The node in it that `lorem`'s words go to, which the configuration calls
/// `moc-1`.
const MOC: &str = "moc-1";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
struct Arguments {
    configuration: String,
    moc_name: String,
    verbose: bool,
}

impl Arguments {
    /// The arguments, or what is wrong with them, said for the person who
    /// typed them: the two names in order, and the flag anywhere.
    fn of(args: impl Iterator<Item = String>) -> Result<Arguments, String> {
        let mut names = Vec::new();
        let mut verbose = false;
        let mut spelling = tcslib::Spelling::default();
        for arg in args {
            if spelling.take(&arg)? {
                continue;
            }
            match arg.as_str() {
                "-v" | "--verbose" => verbose = true,
                flag if flag.starts_with('-') => {
                    return Err(format!(
                        "{flag} is not an option: the options are --yaml, --xml, -v \
                         and --verbose"
                    ))
                }
                _ if names.len() == 2 => {
                    return Err(format!(
                        "{arg}: a configuration and a moc node may be named, and \
                         nothing more"
                    ))
                }
                _ => names.push(arg),
            }
        }
        let mut names = names.into_iter();
        Ok(Arguments {
            configuration: spelling.configuration(names.next(), CONFIGURATION, SPELLING)?,
            moc_name: names.next().unwrap_or_else(|| MOC.to_string()),
            verbose,
        })
    }
}

fn main() -> ExitCode {
    let Arguments {
        configuration,
        moc_name,
        verbose,
    } = match Arguments::of(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(why) => {
            eprintln!("lorem_moc: {why}");
            eprintln!(
                "usage: lorem_moc [--yaml | --xml] [-v | --verbose] [configuration [moc node]]"
            );
            return ExitCode::from(2);
        }
    };

    let mut moc = match Moc::open_file(&configuration, &moc_name) {
        Ok(moc) => moc,
        Err(e) => {
            eprintln!("lorem_moc: {e}");
            return ExitCode::FAILURE;
        }
    };
    if verbose {
        eprintln!(
            "lorem_moc: listening on {} for {}",
            moc.name(),
            moc.sources().join(", ")
        );
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn of(args: &[&str]) -> Result<Arguments, String> {
        Arguments::of(args.iter().map(|arg| arg.to_string()))
    }

    /// Quiet unless asked, with the flag anywhere on the line.
    #[test]
    fn verbose_is_asked_for() {
        let quiet = of(&[]).expect("nothing is fine");
        assert!(!quiet.verbose);
        assert_eq!(quiet.configuration, "examples/simple-app/simple-app.yaml");
        assert_eq!(quiet.moc_name, MOC);

        for args in [
            &["-v"][..],
            &["--verbose"],
            &["a.yaml", "-v", "m"],
            &["a.yaml", "m", "--verbose"],
        ] {
            assert!(of(args).expect("they read").verbose, "{args:?}");
        }
        let both = of(&["-v", "a.yaml", "m"]).expect("they read");
        assert_eq!(
            (both.configuration.as_str(), both.moc_name.as_str()),
            ("a.yaml", "m")
        );

        assert!(of(&["-x"])
            .expect_err("not an option")
            .contains("-v and --verbose"));
        assert!(of(&["a", "b", "c"]).is_err());
    }

    /// YAML where nobody says, XML with --xml, and not both.
    #[test]
    fn the_spelling_is_yaml_unless_xml_is_asked_for() {
        assert_eq!(
            of(&["--xml"]).expect("it reads").configuration,
            "examples/simple-app/simple-app.xml"
        );
        assert!(of(&["--yaml", "--xml"]).is_err());
        assert!(of(&["--xml", "a.yaml"]).is_err());
    }
}
