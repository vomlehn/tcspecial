//! A tcspecial configuration, as the Rust that builds it.
//!
//! What tcspecial does today is read a configuration file when it starts. The
//! other way of building it is for the configuration to *become* Rust: a
//! source file, compiled into the executable along with the node types it uses
//! and the infrastructure around them. A vehicle built that way carries no
//! configuration file at all and nothing aboard has to parse one.
//!
//! This is that generator, and it is a program of its own rather than a corner
//! of the editor. It reads a configuration -- YAML or XML, as the extension
//! says -- and writes a `.rs` file. tcsgui runs it from the Edit menu, passing
//! the configuration it is editing and the path from the `rust-code-path`
//! attribute; a Makefile or a build script can run it the same way, which is
//! the point of its being a program: generating the flight software's
//! configuration should not require a window.
//!
//! # What it generates
//!
//! A section of the configuration at a time, each as a struct of its own and a
//! function that builds one. So far the `tcspecial` section: `TcSpecial`, a
//! field for every parameter the section holds and a struct for every
//! subsection of it, built by `tcspecial()`. The nodes, the links, the payload
//! groups and the command bindings are each their own section and are to come.
//!
//! The fields are typed as what the attributes mean rather than as the text a
//! file holds them in: a socket is a [`HostPort`], which is an address and a
//! port, the beacon's interval is a [`Duration`](std::time::Duration), and the
//! log's directory is a [`PathBuf`](std::path::PathBuf). That is most of the
//! point of generating this at all -- the splitting of `0.0.0.0:4000` and the
//! reading of `5000` happen once, here, while there is still somebody reading
//! the error, and what is compiled into the vehicle is the answer.
//!
//! What varies between configurations is generated; what does not vary -- what
//! an i2c node does, what a mean computes, what the command interpreter
//! answers -- is the code that is already there, and the generated file names
//! those types rather than repeating them.
//!
//! # What it refuses
//!
//! A `tcspecial` section that is not finished. A typed field cannot be written
//! from an attribute nobody stated, so the section is read by
//! [`parameters::ci_config_of`] and `to_ci_config`, which are the same two
//! calls the ground tool and the flight software read it with, and whatever
//! they say about it is what this says -- naming the attribute, because a
//! section of nine attributes is read by somebody who needs to know which one
//! to change. Nothing is written when it refuses: the generated file from
//! before is still the generated file from before.
//!
//! `rust-code-path` is not generated. It says where this file goes, which is
//! not something the vehicle is told -- and a field carried into the flight
//! software and never acted on is a field somebody will one day believe in.
//!
//! Beyond the section it does not judge the configuration. tcsgui asks whether
//! the whole of it is worth generating from before it runs this, because an
//! editor has somebody to tell; a Makefile has `tcsverify`.

use std::path::Path;

use tcslibgs::graph::{parameters, Graph};
use tcslibgs::{CIConfig, HostPort};

/// Read the configuration at `configuration` and write its Rust to `output`.
///
/// The whole of what the program does, so that a caller with two paths needs
/// nothing else. The directory of `output` is made if it is not there:
/// somebody who asked for `generated/vehicle.rs` meant the directory as much
/// as the file.
///
/// `Err` carries a sentence to print: a configuration that cannot be read, a
/// section that is not finished, a directory that cannot be made, a file that
/// cannot be written. The code is generated before the directory is made, so a
/// configuration this refuses leaves nothing behind it at all.
pub fn generate(configuration: &Path, output: &Path) -> Result<(), String> {
    let graph = Graph::load(configuration)
        .map_err(|e| format!("{}: {e}", configuration.display()))?;
    let rust = rust_of(&graph, configuration)
        .map_err(|why| format!("{}: {why}", configuration.display()))?;

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("{} cannot be made: {e}", parent.display()))?;
        }
    }
    std::fs::write(output, rust).map_err(|e| format!("{} cannot be written: {e}", output.display()))
}

/// The configuration as Rust, or what is wrong with it.
///
/// `from` is the configuration this was generated from, which the file names
/// at the top: a reader who finds it in a tree has to be able to get back to
/// the thing it was made of.
pub fn rust_of(graph: &Graph, from: &Path) -> Result<String, String> {
    let named = from
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| from.display().to_string());

    let mut out = header(&named);
    out.push_str(DEFINITIONS);
    out.push_str(&tcspecial_of(&settings_of(graph)?));
    Ok(out)
}

/// The `tcspecial` section, read as the command interpreter reads it.
///
/// Two calls and no rules of its own: [`parameters::ci_config_of`] says which
/// attributes the section must have and `to_ci_config` splits each socket and
/// refuses a pair of them that are one socket. Both are what tcsverify and
/// tcspecial itself read the section with, so a section this refuses is a
/// section they would have refused, in the same words.
///
/// The sentence for a section that is not there is `Graph::check`'s own, for
/// the same reason: somebody who has run the verifier should meet the
/// complaint they already know.
fn settings_of(graph: &Graph) -> Result<CIConfig, String> {
    let said = graph.tcspecial.as_ref().ok_or(
        "this configuration has no tcspecial section, so nothing says where \
         commands are taken, where beacons go or where the telemetry log is \
         written",
    )?;
    parameters::ci_config_of(&parameters::flatten(said))?.to_ci_config()
}

/// The top of the generated file: what it is, and what it was made from.
fn header(named: &str) -> String {
    format!(
        r#"//! What this vehicle carries, as Rust.
//!
//! Generated by tcsgen-rust from `{named}`. Every generation overwrites this
//! file whole, so nothing written here by hand survives: the configuration is
//! the thing to edit, and this is what comes of it.
//!
//! What tcspecial itself is configured with, so far: a struct for the
//! `tcspecial` section and one for each of its subsections, and a function
//! that builds them. The nodes, the links, the payload groups and the command
//! bindings are each their own section and are to come.
//!
//! Every value here was read while this file was written, and is held as what
//! it means rather than as the text the configuration spelled it in: a socket
//! is an address and a port, the beacon's interval is a duration. A section
//! that left one of them unstated, or wrote a port that is not one, was
//! refused then -- with the attribute named -- so nothing here is a value
//! anything aboard still has to make sense of.

use std::path::PathBuf;
use std::time::Duration;

use tcslibgs::HostPort;

"#
    )
}

/// The structs, which do not vary between configurations.
///
/// Written out rather than named from `tcslibgs` because they are what this
/// configuration is, in the shape it wrote: a generated file that a person
/// opens should say what the vehicle is configured with without their having
/// to read another crate to find out. The types inside them are named rather
/// than repeated -- `HostPort` is the one statement of what a socket is, and
/// both ends of every link in the section are one.
const DEFINITIONS: &str = r#"/// What tcspecial itself is configured with.
///
/// The `tcspecial` section of the configuration this file came from: where
/// commands are taken, where beacons go, and where the telemetry log is
/// written. None of what the vehicle carries reaches the ground until these
/// are answered, which is why a configuration that leaves one of them unsaid
/// does not reach this file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcSpecial {
    /// The spacecraft command link: both ends of it.
    pub tcs_cmd: TcsCmd,
    /// Where commands about a payload are taken, which is not that link.
    pub payload_command: PayloadCommand,
    /// Where each payload's telemetry comes from and goes to.
    pub payload_telemetry: PayloadTelemetry,
    /// The beacon link, and how often it carries one.
    pub beacon: Beacon,
    /// Where the telemetry log is written, and in what sized pieces.
    pub log: Log,
}

/// The spacecraft command link, as both ends of it.
///
/// One socket apiece for both directions, which is what a UDP exchange is: a
/// response goes back out of the socket the command arrived on, to the address
/// it arrived from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcsCmd {
    /// tcspecial's own socket: commands from the MOC arrive here, and its
    /// responses leave from here.
    pub recv: HostPort,
    /// The MOC's own socket: commands leave from here, and the responses
    /// arrive here.
    pub send: HostPort,
}

/// Where commands about a payload are taken.
///
/// Two bases and one arithmetic rule: a payload's port on each side is that
/// side's base plus the payload's own id. See `HostPort::at`, which is the one
/// place the sum is taken, so that both ends of a link compute the same port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadCommand {
    /// The ground's end, which the MOC binds.
    pub source_base: HostPort,
    /// The vehicle's end, which tcspecial binds.
    pub dest_base: HostPort,
}

/// Where a payload's telemetry comes from and goes to.
///
/// Counted from these bases by the payload's own id, exactly as its commands
/// are. A separate pair of bases because what the ground needs in a hurry must
/// never be queued behind what it asked for at leisure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadTelemetry {
    /// Where a payload's telemetry leaves tcspecial.
    pub source_base: HostPort,
    /// And where it arrives at the ground.
    pub dest_base: HostPort,
}

/// The beacon link and its timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beacon {
    /// How long between beacons, from the section's `beacon.interval_ms`: a
    /// duration here, because that is what it is and what every timer wants.
    pub interval: Duration,
    /// The socket tcspecial sends beacons from.
    pub send: HostPort,
    /// The socket tcsmoc receives them on.
    pub recv: HostPort,
}

/// The telemetry log: where it is written, and in what sized pieces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    /// The directory holding the log's segment files, which must already
    /// exist. `None` is a vehicle that runs without a telemetry log, which is
    /// a configuration somebody may mean: one with no writable storage.
    pub directory: Option<PathBuf>,
    /// Payload bytes per segment file. The segment file header is added to
    /// this, so it is the space available for telemetry records.
    pub segment_size: u32,
}

"#;

/// This configuration's own values, as the function that builds them.
fn tcspecial_of(settings: &CIConfig) -> String {
    let tcs_cmd_recv = host_port(&HostPort {
        address: settings.address.clone(),
        port: settings.port,
    });
    let tcs_cmd_send = host_port(&settings.moc_cmd_send);
    let command_source = host_port(&settings.payload_command_source_base);
    let command_dest = host_port(&settings.payload_command_dest_base);
    let telemetry_source = host_port(&settings.payload_source_base);
    let telemetry_dest = host_port(&settings.payload_dest_base);
    let interval = settings.beacon_interval.0;
    let beacon_send = host_port(&settings.beacon_send);
    let beacon_recv = host_port(&settings.beacon_recv);
    let directory = match &settings.log_dir {
        Some(directory) => format!("Some(PathBuf::from({}))", quoted(directory)),
        None => "None".to_string(),
    };
    let segment_size = settings.log_segment_bytes;

    format!(
        r#"/// What this configuration says tcspecial is configured with.
pub fn tcspecial() -> TcSpecial {{
    TcSpecial {{
        tcs_cmd: TcsCmd {{
            recv: {tcs_cmd_recv},
            send: {tcs_cmd_send},
        }},
        payload_command: PayloadCommand {{
            source_base: {command_source},
            dest_base: {command_dest},
        }},
        payload_telemetry: PayloadTelemetry {{
            source_base: {telemetry_source},
            dest_base: {telemetry_dest},
        }},
        beacon: Beacon {{
            interval: Duration::from_millis({interval}),
            send: {beacon_send},
            recv: {beacon_recv},
        }},
        log: Log {{
            directory: {directory},
            segment_size: {segment_size},
        }},
    }}
}}
"#
    )
}

/// A socket as Rust spells it: the address it was written with, and the port.
fn host_port(interface: &HostPort) -> String {
    format!(
        "HostPort {{ address: {}.to_string(), port: {} }}",
        quoted(&interface.address),
        interface.port
    )
}

/// A string as Rust spells it.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository's root, for the shipped configurations.
    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    /// The worked example, as a graph.
    fn thermostat() -> Graph {
        Graph::load(root().join("tests/manual/thermostat.yaml")).expect("the worked example")
    }

    /// The generated file says what it is and where it came from.
    ///
    /// A derived file that does not say so is a file somebody edits.
    #[test]
    fn the_generated_file_says_it_is_generated() {
        let from = root().join("tests/manual/thermostat.yaml");
        let rust = rust_of(&thermostat(), &from).expect("the worked example generates");

        assert!(rust.starts_with("//!"), "{}", &rust[..40]);
        assert!(
            rust.contains("Generated by tcsgen-rust from `thermostat.yaml`"),
            "{rust}"
        );
        assert!(
            rust.contains("nothing written here by hand survives"),
            "a generated file says not to edit it"
        );
    }

    /// Where the code goes is where the caller said, and the directory is made.
    #[test]
    fn it_writes_where_it_was_told_and_makes_the_directory() {
        let where_ = tempfile::tempdir().expect("a directory");
        let output = where_.path().join("generated/vehicle.rs");

        generate(&root().join("tests/manual/thermostat.yaml"), &output)
            .expect("the worked example generates");
        let rust = std::fs::read_to_string(&output).expect("the generated code");
        assert!(rust.contains("pub struct TcSpecial"), "{rust}");
        assert!(rust.contains("pub fn tcspecial() -> TcSpecial"), "{rust}");
        // Where it went is not in it: that is where the code goes, not
        // something the vehicle is told.
        assert!(!rust.contains("rust-code-path"), "{rust}");
    }

    /// Every attribute is generated as what it means.
    ///
    /// A socket as an address and a port, the beacon's interval as a duration,
    /// the log's directory as a path: the splitting and the reading happen
    /// here, so what is compiled in is the answer and not the text.
    #[test]
    fn the_section_is_generated_as_what_it_means() {
        let from = root().join("tests/manual/thermostat.yaml");
        let rust = rust_of(&thermostat(), &from).expect("the worked example generates");

        for said in [
            // tcs_cmd.recv, which the worked example writes as 0.0.0.0:4000.
            "recv: HostPort { address: \"0.0.0.0\".to_string(), port: 4000 }",
            "send: HostPort { address: \"127.0.0.1\".to_string(), port: 4100 }",
            // beacon.interval_ms: five seconds, as a duration.
            "interval: Duration::from_millis(5000)",
            "directory: Some(PathBuf::from(\"/var/log/tcspecial\"))",
            "segment_size: 65536,",
        ] {
            assert!(rust.contains(said), "{said} is not in it:\n{rust}");
        }
        // And nothing is left as the text a file holds it in: a port that is
        // still part of a string is a port something aboard has to split.
        assert!(!rust.contains("\"0.0.0.0:4000\""), "{rust}");
    }

    /// A section that is not finished is refused, and the attribute named.
    ///
    /// A typed field cannot be written from an attribute nobody stated, and
    /// the useful thing to say about it is which of the section's attributes
    /// it was -- which is what the ground tool and the flight software say
    /// about the same section, in the same words.
    #[test]
    fn a_section_that_is_not_finished_is_refused() {
        let from = root().join("tests/manual/thermostat.yaml");

        // One attribute taken out of the worked example.
        let mut graph = thermostat();
        let mut flat = parameters::flatten(graph.tcspecial.as_ref().expect("the section"));
        flat.remove("beacon.send").expect("the worked example states it");
        graph.tcspecial = Some(parameters::nest(&flat));

        let said = rust_of(&graph, &from).expect_err("a section with no beacon.send");
        assert!(said.contains("beacon.send"), "{said}");

        // And no section at all, which is `Graph::check`'s own sentence.
        graph.tcspecial = None;
        let said = rust_of(&graph, &from).expect_err("a configuration with no section");
        assert!(said.contains("no tcspecial section"), "{said}");
    }

    /// And nothing is written when it refuses.
    ///
    /// Not the file and not the directory it was to go in: the generated file
    /// from before is still the generated file from before, and a build
    /// reading it reads what it read yesterday.
    #[test]
    fn a_configuration_it_refuses_leaves_nothing_behind() {
        let where_ = tempfile::tempdir().expect("a directory");
        let configuration = where_.path().join("half-done.yaml");
        std::fs::write(
            &configuration,
            "version: \"2.0\"\n\
             \n\
             tcspecial:\n  \
               tcs_cmd:\n    \
                 recv: \"0.0.0.0:4000\"\n  \
               beacon:\n    \
                 interval_ms: \"1000\"\n\
             \n\
             moc:\n  \
               - name: moc\n    \
                 type: moc\n    \
                 address: \"10.0.0.1\"\n    \
                 port: \"5000\"\n",
        )
        .expect("the configuration");
        let output = where_.path().join("generated/vehicle.rs");

        let said = generate(&configuration, &output).expect_err("a half-finished section");
        assert!(said.contains("half-done.yaml"), "{said}");
        assert!(said.contains("tcs_cmd.send"), "{said}");
        assert!(!output.exists(), "something was written anyway");
        assert!(
            !output.parent().expect("a directory").exists(),
            "the directory was made for a file that was never written"
        );
    }

    /// A configuration that cannot be read is said so, and nothing is written.
    #[test]
    fn a_configuration_that_cannot_be_read_is_reported() {
        let where_ = tempfile::tempdir().expect("a directory");
        let output = where_.path().join("vehicle.rs");
        let said = generate(&where_.path().join("nothing-here.yaml"), &output)
            .expect_err("a file that is not there");

        assert!(said.contains("nothing-here.yaml"), "{said}");
        assert!(!output.exists(), "something was written anyway");
    }

    /// The committed example is what this generator writes.
    ///
    /// A generated file that nothing compiles is a generator that can emit
    /// something which does not build, and nobody would find out until a build
    /// of the flight software failed. So the generated form of the shipped
    /// thermostat is committed and compiled by `tests/generated.rs`, which
    /// then reads what it builds.
    ///
    /// Set TCSGUI_WRITE_EXAMPLE to write it out after a deliberate change to
    /// the generator, as the shipped examples are written.
    #[test]
    fn the_committed_example_is_what_the_generator_writes() {
        let from = root().join("tests/manual/thermostat.yaml");
        let written = rust_of(&thermostat(), &from).expect("the worked example generates");
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generated/thermostat.rs");

        if std::env::var("TCSGUI_WRITE_EXAMPLE").is_ok() {
            std::fs::create_dir_all(path.parent().expect("a directory"))
                .expect("the directory is writable");
            std::fs::write(&path, &written).expect("the example is writable");
            return;
        }

        let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}. Run the tests with TCSGUI_WRITE_EXAMPLE=1 to write it out",
                path.display()
            )
        });
        assert_eq!(
            committed,
            written,
            "{} is not what the generator writes any more",
            path.display()
        );
    }
}
