//! Check a tcspecial configuration file, and the simulator file beside it.
//!
//! ```text
//! tcs-verify tests/manual/tcspecial2.ytcs
//! tcs-verify tests/manual/tcspecial2.ytcs tests/manual/tcspecial2sim.yaml
//! tcs-verify tests/manual/tcspecial2.xtcs tests/manual/tcspecial2.ytcs \
//!     tests/manual/tcspecial2sim.yaml
//! tcs-verify --digest tests/manual/tcspecial2.ytcs
//! tcs-verify --interface-digest
//! tcs-verify --ambient examples/thermostat/thermostat-sim.yaml
//! tcs-verify --ambient examples/thermostat/thermostat.ytcs
//! tcs-verify --set-interface examples/*/*.ytcs examples/*/*.xtcs
//! tcs-verify --xtce examples/thermostat/thermostat.ytcs
//! tcs-verify --xtce-check examples/*/*.ytcs
//! tcs-verify --keep=/var/lib/tcspecial/configurations vehicle.ytcs
//! tcs-verify --fprime-model=fprime/Tcs/TcsBridge/model fprime.ytcs
//! tcs-verify --summary examples/thermostat/thermostat.ytcs
//! tcs-verify --links examples/thermostat-yamcs/thermostat-yamcs.ytcs
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
use tcsos::process::ExitCode;

/// What is said when the command line is not one of the two shapes.
///
/// A usage mistake is not a problem with a file, and the status says so with
/// a number of its own: a script that treats "no files named" as "the files
/// are bad" would be reporting a configuration error that nobody wrote.
const USAGE: &str = "\
usage: tcs-verify [options] <tcspecial configuration file> \
[payload simulator file]
       tcs-verify [options] <configuration> <configuration> \
<payload simulator file>
       tcs-verify --interface-digest
       tcs-verify --ambient <payload simulator file | configuration>
       tcs-verify --set-interface <configuration>...
       tcs-verify --xtce <configuration>...
       tcs-verify --xtce-check <configuration>...
       tcs-verify --links <configuration>
       tcs-verify -h | --help

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
  --fatal-warnings   fail the run on a warning as on an error, the
                     warnings still printed as warnings: for a build that
                     wants nothing left unfinished, a string or bytes
                     that states no most among it. Not with
                     --warnings=off, which would hide what failed it
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
                     docs/tcs-verify-design.rst
  --summary          with one configuration, and its payload simulator
                     file if it has one: once it is checked, print what
                     it uses of what must not clash or overflow -- every
                     address and port it states or implies and who binds
                     it, every node's device id, every message ID and
                     APID, and how many data and bytes a second each
                     link to the ground carries, with the beacon and the
                     total -- in aligned columns, after the problems.
                     Two sockets that are one, a counted port past 65535
                     and a message ID or APID used twice or out of its
                     range are problems, said with the others
  --interface-digest print the digest of the interface this build speaks,
                     the 32 lower-case hex digits a configuration's
                     tcspecial section gives as its interface, and exit.
                     It is the digest of the tcsprotocol/xtce/link.xtce and
                     types.xtce the build was made from, which every beacon carries, and
                     a configuration naming another is refused by this
                     build's tcs-verify, tcspecial and ground programs
  --ambient          with one payload simulator file and nothing else:
                     print its world's ambient_temp in kelvin, a number
                     alone, as 288.15, and exit. Given a configuration
                     instead, a .ytcs or .xtcs, the simulator file is the
                     one beside it that tcs-sim would read. For a ground
                     station that shows the simulated world's temperature
                     beside the vehicle's readings, the vehicle sending
                     nothing of it: read here, by the rule tcs-sim reads it
                     by. A file whose world states none, or one that does
                     not read, or a configuration with no simulator file
                     beside it, is said on stderr, with status 1
  --set-interface    with one or more configurations and nothing else:
                     write this build's interface digest into each, as
                     its tcspecial section's interface, in place, every
                     other byte of the file left as it was, and say for
                     each what was changed; a file that names it already
                     is not written. For when tcsprotocol/xtce/link.xtce or
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
  --links            with one configuration and nothing else: print, as
                     YAML, what the vehicle and the ground work out of
                     it -- each node's device id; every link to the
                     ground, the sent and those relayed from a
                     consolidator, with where it arrives, its APID and
                     how it is counted; every APID and message ID; each
                     consolidation's layout -- after its link_digest, the
                     digest every beacon carries of the same. For a
                     ground station to read rather than work out again;
                     a configuration whose links do not work out is said
                     on stderr, with status 1
  -h, --help         print this message and exit

Exit status: 0 no error was printed -- a run that stopped before it found
one included, as --error-limit=0 always does, and warnings fail a run only
under --fatal-warnings -- 1 an error was printed, or a warning under
--fatal-warnings, or a digest or a summary was asked for and
the file could not be read to give it, 2 the command line was not one of
these.";

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
/// `tcs_verify::set_interface`. A shape of its own.
const SET_INTERFACE_FLAG: &str = "--set-interface";

/// Writes each configuration's own XTCE beside it, or says which is not
/// what would be written: see `tcs_verify::own_xtce`. Shapes of their own.
const XTCE_FLAG: &str = "--xtce";
const XTCE_CHECK_FLAG: &str = "--xtce-check";

/// Keeps the configuration, once checked, in the directory given: see
/// `tcsprotocol::store`.
const KEEP_FLAG: &str = "--keep=";

/// Writes the F´ model of the configuration's bridge, once checked, into
/// the directory given: see `tcs_verify::fprime_model`.
const FPRIME_MODEL_FLAG: &str = "--fprime-model=";

/// Prints what the configuration uses of what must not clash or overflow,
/// once it is checked: see `tcs_verify::summary`.
const SUMMARY_FLAG: &str = "--summary";

/// Prints the links a configuration's vehicle and ground work out of it, and
/// their link digest, as YAML: see `tcsprotocol::graph::link_table`. A
/// shape of its own, with one configuration.
const LINKS_FLAG: &str = "--links";

/// Asks for the interface digest of this build, alone on the command line.
///
/// A shape of its own, with nothing beside it: it is a fact about this
/// build, not about any file, and a file named with it would be a file the
/// run did not check.
const INTERFACE_DIGEST_FLAG: &str = "--interface-digest";

/// Reads the command line, checks the files it names and prints what it
/// found, each configuration's digest where `--digest` asks for it, and its
/// summary where `--summary` does.
///
/// # Returns
///
/// The exit status `USAGE` describes: 0 when no error was printed, 1 when
/// one was or a digest or a summary could not be given, 2 when the command
/// line is not one of its shapes or the output cannot be written.
fn main() -> ExitCode {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut limits = tcsverify::Limits::EVERYTHING;
    let mut digest = false;
    let mut summary = false;
    let mut keep: Option<PathBuf> = None;
    let mut fprime_model: Option<PathBuf> = None;
    let mut built: Vec<&str> =
        tcsprotocol::graph::built::OPTIONAL_TYPES.to_vec();
    let args: Vec<_> = tcsos::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == INTERFACE_DIGEST_FLAG) {
        if args.len() != 1 {
            eprintln!(
                "tcs-verify: {INTERFACE_DIGEST_FLAG} is given alone: it says \
                 which interface this build speaks, and checks no file\n\n\
                 {USAGE}"
            );
            return ExitCode::from(2);
        }
        println!("{}", tcsprotocol::InterfaceDigest::of_this_build());
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|arg| arg == SET_INTERFACE_FLAG) {
        return set_interfaces(&args[1..]);
    }
    if args.first().is_some_and(|arg| arg == LINKS_FLAG) {
        return links(&args[1..]);
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
                    "tcs-verify: {AMBIENT_FLAG} is given with one payload \
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
        if said == SUMMARY_FLAG {
            summary = true;
            continue;
        }
        if let Some(value) = said.strip_prefix(KEEP_FLAG) {
            if value.is_empty() {
                eprintln!(
                    "tcs-verify: {KEEP_FLAG} names the directory to keep the \
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
                    "tcs-verify: {FPRIME_MODEL_FLAG} names the directory to \
                     write the F´ model in, and names none\n\n{USAGE}"
                );
                return ExitCode::from(2);
            }
            fprime_model = Some(PathBuf::from(value));
            continue;
        }
        let asked =
            if let Some(taken) = tcsverify::flags::take(&said, &mut limits) {
                taken
            } else if let Some(value) = said.strip_prefix(TYPES_FLAG) {
                tcsprotocol::graph::built::types_from(value)
                    .map(|types| built = types)
                    .map_err(|why| format!("{TYPES_FLAG}{value}: {why}"))
            } else {
                files.push(PathBuf::from(&arg));
                continue;
            };
        if let Err(why) = asked {
            eprintln!("tcs-verify: {why}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    // A warning that fails the run and is not printed would be a failure
    // nothing explains.
    if let Err(why) = tcsverify::flags::agree(&limits) {
        eprintln!("tcs-verify: {why}\n\n{USAGE}");
        return ExitCode::from(2);
    }

    // One configuration is kept, not a pair of spellings: the two would be
    // one checksum and one file kept.
    if keep.is_some() && !matches!(files.len(), 1 | 2) {
        eprintln!(
            "tcs-verify: {KEEP_FLAG} keeps one configuration, given with its \
             payload simulator file or alone\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }
    // A model is of one configuration's bridge, so it is made from one
    // configuration and not from a pair of spellings of it.
    if fprime_model.is_some() && !matches!(files.len(), 1 | 2) {
        eprintln!(
            "tcs-verify: {FPRIME_MODEL_FLAG} writes the model of one \
             configuration, given with its payload simulator file or \
             alone\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }

    // A summary is of one configuration, as a model is: a pair of
    // spellings says one thing, and is compared rather than summed.
    if summary && !matches!(files.len(), 1 | 2) {
        eprintln!(
            "tcs-verify: {SUMMARY_FLAG} summarises one configuration, given \
             with its payload simulator file or alone\n\n{USAGE}"
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
                eprintln!("tcs-verify: {why}\n\n{USAGE}");
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
    // Set where a digest or a summary was asked for and could not be given.
    let mut unanswered = false;
    if digest {
        for path in configurations(&files) {
            match tcsprotocol::config_digest::digest_of_file(path) {
                Ok(said) => {
                    if let Err(e) =
                        writeln!(out, "{}: md5 {said}", path.display())
                    {
                        eprintln!("tcs-verify: cannot write the digest: {e}");
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
                        "tcs-verify: {}: no digest: {why}",
                        path.display()
                    );
                    unanswered = true;
                }
            }
        }
    }

    // Every one recorded, in the order found: the limit was applied as the
    // run looked.
    if let Err(e) = report.write(&mut out) {
        eprintln!("tcs-verify: cannot write what it found: {e}");
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
            "tcs-verify: the simulator file was read but not checked \
             against {against}"
        );
    }

    // After the problems, which say what is wrong with what it lists:
    // a clash is said there, and the summary shows where it is.
    if summary {
        match tcs_verify::summary::of(payload) {
            Ok(text) => {
                if let Err(e) = write!(out, "{text}") {
                    eprintln!("tcs-verify: cannot write the summary: {e}");
                    return ExitCode::from(2);
                }
            }
            // The reason it does not read is among the problems printed;
            // a summary asked for and not given fails, as a digest does.
            Err(why) => {
                eprintln!("tcs-verify: no summary: {why}");
                unanswered = true;
            }
        }
    }

    if unanswered {
        return ExitCode::from(1);
    }

    // Kept only once nothing is wrong with it: a configuration the vehicle
    // may restart onto is one the ground has checked, and the vehicle
    // checks it again as it loads it.
    if let Some(dir) = keep {
        if report.fails() {
            eprintln!(
                "tcs-verify: {} has errors, so it is not kept in {}",
                payload.display(),
                dir.display()
            );
            return ExitCode::from(1);
        }
        match tcsprotocol::store::Store::at(&dir).keep(payload) {
            Ok((checksum, kept)) => {
                let how = match kept {
                    tcsprotocol::store::Kept::Written(_) => "kept as",
                    tcsprotocol::store::Kept::Already(_) => "already kept as",
                };
                if let Err(e) = writeln!(
                    out,
                    "{}: md5 {checksum}, {how} {}",
                    payload.display(),
                    kept.path().display()
                ) {
                    eprintln!("tcs-verify: cannot write where it is kept: {e}");
                    return ExitCode::from(2);
                }
            }
            Err(why) => {
                eprintln!("tcs-verify: not kept: {why}");
                return ExitCode::from(1);
            }
        }
    }

    // Written only once nothing is wrong with it, as a configuration is
    // kept: a bridge is built from a configuration the vehicle would load.
    if let Some(dir) = fprime_model {
        if report.fails() {
            eprintln!(
                "tcs-verify: {} has errors, so no F´ model is written from it",
                payload.display()
            );
            return ExitCode::from(1);
        }
        match tcs_verify::fprime_model::write(payload, &dir) {
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
                    eprintln!("tcs-verify: cannot write what was written: {e}");
                    return ExitCode::from(2);
                }
            }
            Err(why) => {
                eprintln!("tcs-verify: no F´ model: {why}");
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
        .then(|| tcsprotocol::config::sim_path_beside(&said))
        .flatten();
    let sim = match (configuration, &beside) {
        (false, _) => file,
        (true, Some(beside)) => Path::new(beside),
        (true, None) => {
            eprintln!(
                "tcs-verify: {said}: no payload simulator file beside it, \
                 so no simulated world: name the simulator file instead"
            );
            return ExitCode::from(1);
        }
    };
    let file = match tcsprotocol::sim_config::SimConfigFile::load(sim) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("tcs-verify: {}: {e}", sim.display());
            return ExitCode::from(1);
        }
    };
    let Some(world) = file.world.as_ref() else {
        eprintln!(
            "tcs-verify: {}: there is no world: section, so no ambient \
             temperature; state one under world:, as ambient_temp: \
             \"288.15 K\"",
            sim.display()
        );
        return ExitCode::from(1);
    };
    let Some(kelvin) = world.ambient() else {
        // What is wrong is what tcs-sim would say, from the one reading of
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
        eprintln!("tcs-verify: {}: {why}", sim.display());
        return ExitCode::from(1);
    };
    if let Err(e) = writeln!(std::io::stdout(), "{kelvin}") {
        eprintln!("tcs-verify: cannot write the temperature: {e}");
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
    use tcs_verify::set_interface::{set_in_file, Set};

    if files.is_empty()
        || files.iter().any(|f| f.to_string_lossy().starts_with('-'))
    {
        eprintln!(
            "tcs-verify: {SET_INTERFACE_FLAG} is given configurations and \
             nothing else\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }
    let ours = tcsprotocol::InterfaceDigest::of_this_build();
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
                eprintln!("tcs-verify: {why}");
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

/// Prints a configuration's link table and its digest, as YAML.
///
/// # Arguments
///
/// * `files` - What followed the flag: one configuration.
///
/// # Returns
///
/// 0 with the table printed, 1 where the configuration does not read or its
/// links do not work out, said on stderr, and 2 for anything but one file.
fn links(files: &[std::ffi::OsString]) -> ExitCode {
    let [file] = files else {
        eprintln!(
            "tcs-verify: {LINKS_FLAG} is given one configuration and nothing \
             else\n\n{USAGE}"
        );
        return ExitCode::from(2);
    };
    if file.to_string_lossy().starts_with('-') {
        eprintln!(
            "tcs-verify: {LINKS_FLAG} is given one configuration and nothing \
             else\n\n{USAGE}"
        );
        return ExitCode::from(2);
    }
    let path = Path::new(file);
    let table = tcsprotocol::graph::Graph::load(path)
        .map_err(|why| why.to_string())
        .and_then(|graph| {
            tcsprotocol::graph::link_table::LinkTable::of(&graph)
                .map_err(|refused| refused.why)
        });
    match table {
        Ok(table) => {
            print!("{}", table.yaml(&path.display().to_string()));
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("tcs-verify: {}: {why}", path.display());
            ExitCode::from(1)
        }
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
    use tcs_verify::own_xtce::{of, path_of, Found};

    let flag = if write { XTCE_FLAG } else { XTCE_CHECK_FLAG };
    if files.is_empty()
        || files.iter().any(|f| f.to_string_lossy().starts_with('-'))
    {
        eprintln!(
            "tcs-verify: {flag} is given configurations and nothing \
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
                    "tcs-verify: {} is not what {} says: run tcs-verify \
                     {XTCE_FLAG} {}",
                    xtce.display(),
                    path.display(),
                    path.display()
                );
                failed = true;
            }
            Err(why) => {
                eprintln!("tcs-verify: {why}");
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
    use tcsprotocol::ConfigFormat;

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
