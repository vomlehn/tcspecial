//! Check a tcspecial configuration file, and the simulator file beside it.
//!
//! ```text
//! tcsverify tests/manual/tcspecial2.ytcs
//! tcsverify tests/manual/tcspecial2.ytcs tests/manual/tcspecial2sim.yaml
//! tcsverify tests/manual/tcspecial2.xtcs tests/manual/tcspecial2.ytcs \
//!     tests/manual/tcspecial2sim.yaml
//! tcsverify --digest tests/manual/tcspecial2.ytcs
//! tcsverify --interface-digest
//! tcsverify --ambient examples/thermostat/thermostat-sim.yaml
//! tcsverify --ambient examples/thermostat/thermostat.ytcs
//! tcsverify --set-interface examples/*/*.ytcs examples/*/*.xtcs
//! tcsverify --xtce examples/thermostat/thermostat.ytcs
//! tcsverify --xtce-check examples/*/*.ytcs
//! tcsverify --keep=/var/lib/tcspecial/configurations vehicle.ytcs
//! tcsverify --fprime-model=fprime/Tcs/TcsBridge/model fprime.ytcs
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
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// What is said when the command line is not one of the two shapes.
///
/// A usage mistake is not a problem with a file, and the status says so with
/// a number of its own: a script that treats "no files named" as "the files
/// are bad" would be reporting a configuration error that nobody wrote.
const USAGE: &str = "\
usage: tcsverify [options] <tcspecial configuration file> \
[payload simulator file]
       tcsverify [options] <configuration> <configuration> \
<payload simulator file>
       tcsverify --interface-digest
       tcsverify --ambient <payload simulator file | configuration>
       tcsverify --set-interface <configuration>...
       tcsverify --xtce <configuration>...
       tcsverify --xtce-check <configuration>...
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
  --types=<types>    check the configuration for a tcspecial built with
                     only these payload types, separated by commas --
                     what `tcspecial --types` prints -- reporting each
                     node of another type as that tcspecial would refuse
                     it. Without it, every type is taken to be built
  --keep=<directory> with one configuration, and its payload simulator
                     file if it has one: once it is checked and nothing is
                     wrong with it, keep it in <directory>, the directory a
                     tcspecial section's configurations attribute names,
                     under its checksum -- <checksum>/<checksum>.ytcs or
                     .xtcs, one top/ further down for each directory its
                     includes climb with .., and each file it includes
                     where its include name puts it, mode 0444 -- as the
                     vehicle keeps a configuration it may restart onto.
                     One whose
                     checksum is kept already is not written again.
                     Prints where it is kept. For first
                     provisioning and for testing: making the directory
                     read-only afterwards is the provisioner's
  --fprime-model=<directory>
                     with one configuration, and its payload simulator
                     file if it has one: once it is checked and nothing is
                     wrong with it, write the F´ model of the bridge at
                     the far end of its one gateway_fprime node into
                     <directory> -- TcsBridgeModel.fppi, the channels and
                     constants the TcsBridge component includes, and
                     TcsBridgeModel.cpp, which says which channel each
                     source's data is written to -- and say which files
                     were written. A file that already says what would be
                     written is left alone. See \"F´: gateway_fprime\" in
                     docs/tcsverify-design.rst
  --interface-digest print the digest of the interface this build speaks,
                     the 32 lower-case hex digits a configuration's
                     tcspecial section gives as its interface, and exit.
                     It is the digest of the tcslibgs/xtce/link.xtce and
                     types.xtce the build was made from, which every beacon carries, and
                     a configuration naming another is refused by this
                     build's tcsverify, tcspecial and ground programs
  --ambient          with one payload simulator file and nothing else:
                     print its world's ambient_temp in kelvin, a number
                     alone, as 288.15, and exit. Given a configuration
                     instead, a .ytcs or .xtcs, the simulator file is the
                     one beside it that tcssim would read. For a ground
                     station that shows the simulated world's temperature
                     beside the vehicle's readings, the vehicle sending
                     nothing of it: read here, by the rule tcssim reads it
                     by. A file whose world states none, or one that does
                     not read, or a configuration with no simulator file
                     beside it, is said on stderr, with status 1
  --set-interface    with one or more configurations and nothing else:
                     write this build's interface digest into each, as
                     its tcspecial section's interface, in place, every
                     other byte of the file left as it was, and say for
                     each what was changed; a file that names it already
                     is not written. For when tcslibgs/xtce/link.xtce or
                     types.xtce has changed and every configuration is
                     to follow it
  --xtce             with one or more configurations and nothing else:
                     write each one's own XTCE beside it, the file of its
                     name with the extension .xtce -- what the vehicle
                     sends the ground, each source a parameter of its
                     type -- and say for each whether it was written; a
                     file that says it already is not written
  --xtce-check       as --xtce, writing nothing: name each configuration
                     whose own XTCE is not what --xtce would write, with
                     status 1 where there is one
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

/// Which payload types the tcspecial being checked for contains: every one
/// unless asked.
const TYPES_FLAG: &str = "--types=";

/// Whether to print each configuration's digest as well as its problems.
///
/// Takes no value, where the other two do: there is one digest of a
/// configuration and nothing to choose about it.
const DIGEST_FLAG: &str = "--digest";

/// Asks for a simulator file's world's ambient temperature, in kelvin, with
/// the file and nothing else: a shape of its own, as the interface digest's
/// is, being a question about one file and no check of it.
const AMBIENT_FLAG: &str = "--ambient";

/// Writes this build's interface digest into each configuration named: see
/// `tcsverify::set_interface`. A shape of its own.
const SET_INTERFACE_FLAG: &str = "--set-interface";

/// Writes each configuration's own XTCE beside it, or says which is not
/// what would be written: see `tcsverify::own_xtce`. Shapes of their own.
const XTCE_FLAG: &str = "--xtce";
const XTCE_CHECK_FLAG: &str = "--xtce-check";

/// Keeps the configuration, once checked, in the directory given: see
/// `tcslibgs::store`.
const KEEP_FLAG: &str = "--keep=";

/// Writes the F´ model of the configuration's bridge, once checked, into
/// the directory given: see `tcsverify::fprime_model`.
const FPRIME_MODEL_FLAG: &str = "--fprime-model=";

/// Asks for the interface digest of this build, alone on the command line.
///
/// A shape of its own, with nothing beside it: it is a fact about this
/// build, not about any file, and a file named with it would be a file the
/// run did not check.
const INTERFACE_DIGEST_FLAG: &str = "--interface-digest";

/// Reads the command line, checks the files it names and prints what it
/// found, and each configuration's digest where `--digest` asks for it.
///
/// # Returns
///
/// The exit status `USAGE` describes: 0 when no error was printed, 1 when
/// one was or a digest could not be taken, 2 when the command line is not
/// one of its shapes or the output cannot be written.
fn main() -> ExitCode {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut limits = tcsverify::Limits::EVERYTHING;
    let mut digest = false;
    let mut keep: Option<PathBuf> = None;
    let mut fprime_model: Option<PathBuf> = None;
    let mut built: Vec<&str> = tcslibgs::graph::built::OPTIONAL_TYPES.to_vec();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == INTERFACE_DIGEST_FLAG) {
        if args.len() != 1 {
            eprintln!(
                "tcsverify: {INTERFACE_DIGEST_FLAG} is given alone: it says \
                 which interface this build speaks, and checks no file\n\n\
                 {USAGE}"
            );
            return ExitCode::from(2);
        }
        println!("{}", tcslibgs::InterfaceDigest::of_this_build());
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|arg| arg == SET_INTERFACE_FLAG) {
        return set_interfaces(&args[1..]);
    }
    if args.first().is_some_and(|arg| arg == XTCE_FLAG) {
        return own_xtces(&args[1..], true);
    }
    if args.first().is_some_and(|arg| arg == XTCE_CHECK_FLAG) {
        return own_xtces(&args[1..], false);
    }
    if args.iter().any(|arg| arg == AMBIENT_FLAG) {
        return match args.as_slice() {
            [flag, file] if flag == AMBIENT_FLAG => ambient(Path::new(file)),
            _ => {
                eprintln!(
                    "tcsverify: {AMBIENT_FLAG} is given with one payload \
                     simulator file and nothing else\n\n{USAGE}"
                );
                ExitCode::from(2)
            }
        };
    }
    for arg in args {
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
        if let Some(value) = said.strip_prefix(KEEP_FLAG) {
            if value.is_empty() {
                eprintln!(
                    "tcsverify: {KEEP_FLAG} names the directory to keep the \
                     configuration in, and names none\n\n{USAGE}"
                );
                return ExitCode::from(2);
            }
            keep = Some(PathBuf::from(value));
            continue;
        }
        if let Some(value) = said.strip_prefix(FPRIME_MODEL_FLAG) {
            if value.is_empty() {
                eprintln!(
                    "tcsverify: {FPRIME_MODEL_FLAG} names the directory to \
                     write the F´ model in, and names none\n\n{USAGE}"
                );
                return ExitCode::from(2);
            }
            fprime_model = Some(PathBuf::from(value));
            continue;
        }
        let asked = if let Some(value) = said.strip_prefix(ERROR_LIMIT_FLAG) {
            error_limit(value).map(|limit| limits.error_limit = limit)
        } else if let Some(value) = said.strip_prefix(WARNINGS_FLAG) {
            warnings(value).map(|warnings| limits.warnings = warnings)
        } else if let Some(value) = said.strip_prefix(TYPES_FLAG) {
            tcslibgs::graph::built::types_from(value)
                .map(|types| built = types)
                .map_err(|why| format!("{TYPES_FLAG}{value}: {why}"))
        } else {
            files.push(PathBuf::from(&arg));
            continue;
        };
        if let Err(why) = asked {
            eprintln!("tcsverify: {why}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    // One configuration is kept, not a pair of spellings: the two would be
    // one checksum and one file kept.
    if keep.is_some() && !matches!(files.len(), 1 | 2) {
        eprintln!(
            "tcsverify: {KEEP_FLAG} keeps one configuration, given with its \
             payload simulator file or alone\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }
    // A model is of one configuration's bridge, so it is made from one
    // configuration and not from a pair of spellings of it.
    if fprime_model.is_some() && !matches!(files.len(), 1 | 2) {
        eprintln!(
            "tcsverify: {FPRIME_MODEL_FLAG} writes the model of one \
             configuration, given with its payload simulator file or \
             alone\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }

    let report = match files.as_slice() {
        [payload] => tcsverify::verify_built(payload, None, limits, &built),
        [payload, sim] => {
            tcsverify::verify_built(payload, Some(sim), limits, &built)
        }
        [first, second, sim] => {
            if let Err(why) = one_of_each(first, second) {
                eprintln!("tcsverify: {why}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            tcsverify::verify_pair(first, second, sim, limits, &built)
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
                    if let Err(e) =
                        writeln!(out, "{}: md5 {said}", path.display())
                    {
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
                    eprintln!(
                        "tcsverify: {}: no digest: {why}",
                        path.display()
                    );
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
            _ => {
                format!("{}, which has problems of its own", payload.display())
            }
        };
        eprintln!(
            "tcsverify: the simulator file was read but not checked \
             against {against}"
        );
    }

    if digest_failed {
        return ExitCode::from(1);
    }

    // Kept only once nothing is wrong with it: a configuration the vehicle
    // may restart onto is one the ground has checked, and the vehicle
    // checks it again as it loads it.
    if let Some(dir) = keep {
        if report.found_errors() {
            eprintln!(
                "tcsverify: {} has errors, so it is not kept in {}",
                payload.display(),
                dir.display()
            );
            return ExitCode::from(1);
        }
        match tcslibgs::store::Store::at(&dir).keep(payload) {
            Ok((checksum, kept)) => {
                let how = match kept {
                    tcslibgs::store::Kept::Written(_) => "kept as",
                    tcslibgs::store::Kept::Already(_) => "already kept as",
                };
                if let Err(e) = writeln!(
                    out,
                    "{}: md5 {checksum}, {how} {}",
                    payload.display(),
                    kept.path().display()
                ) {
                    eprintln!("tcsverify: cannot write where it is kept: {e}");
                    return ExitCode::from(2);
                }
            }
            Err(why) => {
                eprintln!("tcsverify: not kept: {why}");
                return ExitCode::from(1);
            }
        }
    }

    // Written only once nothing is wrong with it, as a configuration is
    // kept: a bridge is built from a configuration the vehicle would load.
    if let Some(dir) = fprime_model {
        if report.found_errors() {
            eprintln!(
                "tcsverify: {} has errors, so no F´ model is written from it",
                payload.display()
            );
            return ExitCode::from(1);
        }
        match tcsverify::fprime_model::write(payload, &dir) {
            Ok((gateway, written)) => {
                let said = if written.is_empty() {
                    writeln!(
                        out,
                        "{}: the F´ model of {gateway} in {} says this \
                         already",
                        payload.display(),
                        dir.display()
                    )
                } else {
                    written.iter().try_for_each(|path| {
                        writeln!(
                            out,
                            "{}: the F´ model of {gateway} written to {}",
                            payload.display(),
                            path.display()
                        )
                    })
                };
                if let Err(e) = said {
                    eprintln!("tcsverify: cannot write what was written: {e}");
                    return ExitCode::from(2);
                }
            }
            Err(why) => {
                eprintln!("tcsverify: no F´ model: {why}");
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::from(u8::try_from(report.status()).unwrap_or(1))
}
/// Prints a simulator file's world's ambient temperature in kelvin.
///
/// # Arguments
///
/// * `file` - The payload simulator file, or a configuration beside one.
///
/// # Returns
///
/// 0 with the temperature printed; 1 where there is no simulator file, it
/// does not read, has no world, or its world states no ambient_temp that
/// reads, said on stderr; 2 where stdout cannot be written.
fn ambient(file: &Path) -> ExitCode {
    let said = file.to_string_lossy();
    let configuration = said.ends_with(".ytcs") || said.ends_with(".xtcs");
    let beside = configuration
        .then(|| tcslibgs::config::sim_path_beside(&said))
        .flatten();
    let sim = match (configuration, &beside) {
        (false, _) => file,
        (true, Some(beside)) => Path::new(beside),
        (true, None) => {
            eprintln!(
                "tcsverify: {said}: no payload simulator file beside it, \
                 so no simulated world: name the simulator file instead"
            );
            return ExitCode::from(1);
        }
    };
    let file = match tcslibgs::sim_config::SimConfigFile::load(sim) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("tcsverify: {}: {e}", sim.display());
            return ExitCode::from(1);
        }
    };
    let Some(world) = file.world.as_ref() else {
        eprintln!(
            "tcsverify: {}: there is no world: section, so no ambient \
             temperature; state one under world:, as ambient_temp: \
             \"288.15 K\"",
            sim.display()
        );
        return ExitCode::from(1);
    };
    let Some(kelvin) = world.ambient() else {
        // What is wrong is what tcssim would say, from the one reading of
        // ambient_temp; or, where it reads, that it is not stated.
        let why = world
            .problems()
            .into_iter()
            .find(|problem| problem.ambient)
            .map_or_else(
                || {
                    "world ambient_temp: it is not stated: state it under \
                     world:, as ambient_temp: \"288.15 K\""
                        .to_string()
                },
                |problem| problem.to_string(),
            );
        eprintln!("tcsverify: {}: {why}", sim.display());
        return ExitCode::from(1);
    };
    if let Err(e) = writeln!(std::io::stdout(), "{kelvin}") {
        eprintln!("tcsverify: cannot write the temperature: {e}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

/// Sets the interface of every configuration named to this build's, and
/// says what was done to each.
///
/// # Arguments
///
/// * `files` - The configurations.
///
/// # Returns
///
/// 0 when every one names this build's interface now, 1 when one could
/// not be set, said on stderr, and 2 for no files or another option.
fn set_interfaces(files: &[std::ffi::OsString]) -> ExitCode {
    use tcsverify::set_interface::{set_in_file, Set};

    if files.is_empty()
        || files.iter().any(|f| f.to_string_lossy().starts_with('-'))
    {
        eprintln!(
            "tcsverify: {SET_INTERFACE_FLAG} is given configurations and \
             nothing else\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }
    let ours = tcslibgs::InterfaceDigest::of_this_build();
    let mut failed = false;
    for file in files {
        let path = std::path::Path::new(file);
        match set_in_file(path) {
            Ok(Set::Already) => {
                println!("{}: interface is {ours} already", path.display())
            }
            Ok(Set::Changed(was)) => println!(
                "{}: interface set to {ours}, from {was}",
                path.display()
            ),
            Ok(Set::Added) => println!(
                "{}: interface set to {ours}, where it named none",
                path.display()
            ),
            Err(why) => {
                eprintln!("tcsverify: {why}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Writes each configuration's own XTCE beside it, or names each whose own
/// XTCE is not what would be written.
///
/// # Arguments
///
/// * `files` - The configurations.
/// * `write` - Whether to write them, for `--xtce`, rather than only say
///             which are stale, for `--xtce-check`.
///
/// # Returns
///
/// 0 when every one is current now, 1 when one could not be written or,
/// only checking, is stale, said on stderr, and 2 for no files or another
/// option.
fn own_xtces(files: &[std::ffi::OsString], write: bool) -> ExitCode {
    use tcsverify::own_xtce::{of, path_of, Found};

    let flag = if write { XTCE_FLAG } else { XTCE_CHECK_FLAG };
    if files.is_empty()
        || files.iter().any(|f| f.to_string_lossy().starts_with('-'))
    {
        eprintln!(
            "tcsverify: {flag} is given configurations and nothing \
             else\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }
    let mut failed = false;
    for file in files {
        let path = std::path::Path::new(file);
        let xtce = path_of(path);
        match of(path, write) {
            Ok(Found::Current) => {
                println!("{}: says {} already", xtce.display(), path.display())
            }
            Ok(Found::Written) => {
                println!("{}: written from {}", xtce.display(), path.display())
            }
            Ok(Found::Stale) => {
                eprintln!(
                    "tcsverify: {} is not what {} says: run tcsverify \
                     {XTCE_FLAG} {}",
                    xtce.display(),
                    path.display(),
                    path.display()
                );
                failed = true;
            }
            Err(why) => {
                eprintln!("tcsverify: {why}");
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Picks the configurations among the pathnames: every one but the simulator
/// file.
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
///
/// # Arguments
///
/// * `files` - The pathnames on the command line.
///
/// # Returns
///
/// The first of one, or all but the last of two or more.
fn configurations(files: &[PathBuf]) -> &[PathBuf] {
    match files.len() {
        0 => &[],
        1 => &files[..1],
        other => &files[..other - 1],
    }
}

/// Reads what `--error-limit=` was given, as how many to list.
///
/// # Arguments
///
/// * `value` - What follows the `=`.
///
/// # Returns
///
/// The limit, or `None` for "unlimited", which is every one.
///
/// # Errors
///
/// A sentence saying what the limit may be, if it is anything else.
fn error_limit(value: &str) -> Result<Option<usize>, String> {
    if value == "unlimited" {
        return Ok(None);
    }
    value.parse::<usize>().map(Some).map_err(|_| {
        format!(
            "{ERROR_LIMIT_FLAG}{value}: the limit is a whole number, \
             0 included, or \"unlimited\""
        )
    })
}

/// Reads what `--warnings=` was given.
///
/// # Arguments
///
/// * `value` - What follows the `=`: "on", "off" or "errors".
///
/// # Returns
///
/// What is done with warnings.
///
/// # Errors
///
/// A sentence saying what it may be, if it is anything else.
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

/// Checks that two configurations are one XML file and one YAML file, in
/// either order, which is what comparing two spellings of one configuration
/// is.
///
/// # Arguments
///
/// * `first`  - One configuration.
/// * `second` - The other.
///
/// # Errors
///
/// A sentence naming them if they are not one of each, or naming a file
/// whose name says neither format.
fn one_of_each(
    first: &std::path::Path,
    second: &std::path::Path,
) -> Result<(), String> {
    use tcslibgs::ConfigFormat;

    let format = |path: &std::path::Path| {
        ConfigFormat::of_file(path)
            .map_err(|e| format!("{}: {e}", path.display()))
    };
    match (format(first)?, format(second)?) {
        (ConfigFormat::Xml, ConfigFormat::Yaml)
        | (ConfigFormat::Yaml, ConfigFormat::Xml) => Ok(()),
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
