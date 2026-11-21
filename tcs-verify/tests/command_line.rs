//! The program itself, run as a person or a Makefile runs it.
//!
//! The library's tests say what a run finds under each limit; these say that
//! the command line reaches them -- that an option is taken as an option and
//! not as a file to check, which nothing short of running the program shows.

use std::path::{Path, PathBuf};
use tcsos::process::Command;

/// Writes a configuration with this many errors in it, one per payload: each
/// sends to a node that is not there.
///
/// # Arguments
///
/// * `dir`   - The directory it is written in, as `set.ytcs`.
/// * `count` - How many errors.
///
/// # Returns
///
/// The file's path.
fn with_errors(dir: &Path, count: usize) -> PathBuf {
    let mut text = "\
description: many
tcspecial:
  gateway: radio
  interface: \"b03f49878bcc9c45e7b854c7da1bce47\"
  state_file: /tmp/tcspecial-test.state
  max_consolidator_inputs: 32
  beacon:
    default_interval: \"5 s\"
    reset_interval: \"5 s\"
    recv: \"127.0.0.1:5650\"
    send: \"0.0.0.0:5550\"
  log:
    directory: \"/tmp/telemetry\"
    segment_size: \"65536\"
  payload_command:
    dest_base: \"0.0.0.0:4200\"
    source_base: \"127.0.0.1:4300\"
  payload_telemetry:
    dest_base: \"127.0.0.1:6100\"
    source_base: \"0.0.0.0:6000\"
  tcs_cmd:
    recv: \"0.0.0.0:4000\"
    send: \"127.0.0.1:4100\"
payloads:
  - type device-file:
"
    .to_string();
    for id in 0..count {
        text.push_str(&format!(
            "      - payload: DH{id}\n        path: /dev/null{id}\n        \
             read_size: \"16\"\n        \
             world-out: [nowhere{id}.DH{id}]\n"
        ));
    }
    let path = dir.join("set.ytcs");
    tcsos::fs::write(&path, text).expect("written");
    path
}

/// Runs tcs-verify with these arguments.
///
/// # Arguments
///
/// * `args` - Its command line, after the program's name.
///
/// # Returns
///
/// The lines it printed on stdout and its exit status.
///
/// # Panics
///
/// If it cannot be run, prints something that is not text, or is killed by
/// a signal.
fn run(args: &[&str]) -> (Vec<String>, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_tcs-verify"))
        .args(args)
        .output()
        .expect("tcs-verify runs");
    let printed = String::from_utf8(out.stdout).expect("text");
    (
        printed.lines().map(str::to_string).collect(),
        out.status.code().expect("an exit status"),
    )
}

/// `--interface-digest` prints the digest of the interface this build
/// speaks, alone, as a configuration's `interface` writes it; with anything
/// beside it the command line is none of the shapes.
#[test]
fn the_interface_digest_is_printed_alone() {
    let (lines, status) = run(&["--interface-digest"]);
    assert_eq!(status, 0);
    assert_eq!(
        lines,
        vec![tcsprotocol::InterfaceDigest::of_this_build().to_string()]
    );
    assert_eq!(lines[0].len(), 32);
    assert!(
        lines[0]
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase()),
        "{lines:?}"
    );

    // And it is what the shipped configurations name.
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/ping/ping.ytcs");
    let text = tcsos::fs::read_to_string(example).expect("the example");
    assert!(
        text.contains(&format!("interface: \"{}\"", lines[0])),
        "the ping example names another interface"
    );

    let (_, status) = run(&["--interface-digest", "a.ytcs"]);
    assert_eq!(status, 2, "given alone");
}

/// `--ambient` prints a simulator file's world's ambient temperature in
/// kelvin, a number alone, given the file or a configuration it is beside;
/// a file with no world, one whose figure does not read, or a configuration
/// with none beside it prints nothing and fails; and anything beside the
/// one file is none of the shapes.
#[test]
fn the_worlds_ambient_is_printed_in_kelvin() {
    let dir = tempfile::tempdir().expect("a directory");
    let write = |name: &str, text: &str| {
        let path = dir.path().join(name);
        tcsos::fs::write(&path, text).expect("written");
        path.to_str().expect("a path").to_string()
    };
    let good = write("good-sim.yaml", "world:\n  ambient_temp: \"288.15 K\"\n");
    assert_eq!(run(&["--ambient", &good]), (vec!["288.15".to_string()], 0));
    // A configuration is answered for by the simulator file beside it.
    let beside = write("good.ytcs", "description: unread\n");
    assert_eq!(
        run(&["--ambient", &beside]),
        (vec!["288.15".to_string()], 0)
    );
    let alone = write("alone.ytcs", "description: unread\n");
    assert_eq!(run(&["--ambient", &alone]), (Vec::new(), 1));
    let none = write("none-sim.yaml", "groups: []\n");
    assert_eq!(run(&["--ambient", &none]), (Vec::new(), 1));
    let wrong = write("wrong-sim.yaml", "world:\n  ambient_temp: \"20 C\"\n");
    assert_eq!(run(&["--ambient", &wrong]), (Vec::new(), 1));
    assert_eq!(run(&["--ambient"]).1, 2, "no file");
    assert_eq!(run(&["--ambient", &good, &none]).1, 2, "two files");
    assert_eq!(run(&[&good, "--ambient"]).1, 2, "the flag second");
}

/// `--keep=<directory>` keeps a configuration with nothing wrong with it
/// under its checksum, read-only, and says where; again, it writes nothing
/// and says it is kept already. One with errors is not kept, and the store
/// is what tcspecial loads from. The thermostat example, copied, includes a
/// file in a directory beside its own, so its top file is kept one `top`
/// down in its tree.
#[test]
fn a_checked_configuration_is_kept_under_its_checksum() {
    let dir = tempfile::tempdir().expect("a directory");
    let store = dir.path().join("kept");
    tcsos::fs::create_dir(&store).expect("a store");
    let keep = format!("--keep={}", store.display());
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    for file in [
        "thermostat/thermostat.ytcs",
        "thermostat-common/thermostat.ytcs",
    ] {
        let to = dir.path().join(file);
        tcsos::fs::create_dir_all(to.parent().expect("a directory"))
            .expect("a directory");
        tcsos::fs::copy(examples.join(file), &to).expect("copied");
    }
    let example = dir.path().join("thermostat/thermostat.ytcs");
    let example = example.to_str().expect("a path");
    let checksum = tcsprotocol::config_digest::digest_of_file(example)
        .expect("it digests");
    let tree = store.join(checksum.to_string());
    let kept = tree.join("top").join(format!("{checksum}.ytcs"));

    let (lines, status) = run(&[&keep, example]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(
        lines,
        vec![format!(
            "{example}: md5 {checksum}, kept as {}",
            kept.display()
        )]
    );
    let mode = tcsos::fs::mode(&kept).expect("kept");
    assert_eq!(mode & 0o777, 0o444, "read-only");
    assert!(tcsos::fs::is_file(
        tree.join("thermostat-common/thermostat.ytcs")
    ));
    let (graph, _) = tcsprotocol::store::Store::at(&store)
        .load(checksum)
        .expect("it loads by its checksum");
    assert_eq!(
        graph.nodes.len(),
        tcsprotocol::graph::Graph::load(example)
            .expect("the example reads")
            .nodes
            .len()
    );

    let (lines, status) = run(&[&keep, example]);
    assert_eq!(status, 0);
    assert!(lines[0].contains("already kept as"), "{lines:?}");

    let wrong = with_errors(dir.path(), 2);
    let (_, status) = run(&[&keep, wrong.to_str().expect("a path")]);
    assert_eq!(status, 1, "a configuration with errors");
    assert_eq!(
        tcsprotocol::store::Store::at(&store)
            .held()
            .expect("it reads")
            .len(),
        1,
        "is not kept"
    );

    let (_, status) = run(&["--keep=", example]);
    assert_eq!(status, 2, "a directory is named");
}

/// `--fprime-model=<directory>` writes the fprime example's bridge model,
/// says which files it wrote, and again writes nothing and says so. A
/// configuration with no gateway_fprime node has no model, and one with
/// errors is not written from.
#[test]
fn the_fprime_example_has_its_bridge_model_written() {
    let dir = tempfile::tempdir().expect("a directory");
    let model = dir.path().join("model");
    let flag = format!("--fprime-model={}", model.display());
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/fprime/fprime.ytcs");
    let example = example.to_str().expect("a path");

    let (lines, status) = run(&[&flag, example]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("the F´ model of fsw"), "{lines:?}");
    let fppi = tcsos::fs::read_to_string(model.join("TcsBridgeModel.fppi"))
        .expect("the FPP is written");
    let checksum = tcsprotocol::config_digest::digest_of_file(example)
        .expect("it digests");
    assert!(
        fppi.contains(&format!("CONFIGURATION_DIGEST = \"{checksum}\"")),
        "{fppi}"
    );
    assert!(fppi.contains("telemetry $thermometer: I16 id 3"), "{fppi}");
    assert!(tcsos::fs::is_file(model.join("TcsBridgeModel.cpp")));

    let (lines, status) = run(&[&flag, example]);
    assert_eq!(status, 0);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("says this already"), "{lines:?}");

    let ping = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/ping/ping.ytcs");
    let (_, status) = run(&[&flag, ping.to_str().expect("a path")]);
    assert_eq!(status, 1, "no gateway_fprime node");

    let wrong = with_errors(dir.path(), 1);
    let (_, status) = run(&[&flag, wrong.to_str().expect("a path")]);
    assert_eq!(status, 1, "a configuration with errors");

    let (_, status) = run(&["--fprime-model=", example]);
    assert_eq!(status, 2, "a directory is named");
}

/// `--set-interface` writes this build's interface into a stale
/// configuration, changing that and nothing else, says what it changed, and
/// changes nothing in one that is current.
#[test]
fn set_interface_rewrites_only_the_interface() {
    let dir = tempfile::tempdir().expect("a directory");
    let ours = tcsprotocol::InterfaceDigest::of_this_build().to_string();
    let stale = "0123456789abcdef0123456789abcdef";
    let mut paths = Vec::new();
    // An example that includes no other file, so that its copy reads.
    for name in ["minimal.ytcs", "minimal.xtcs"] {
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/minimal")
            .join(name);
        let good = tcsos::fs::read_to_string(&shipped).expect("the example");
        let path = dir.path().join(name);
        tcsos::fs::write(&path, good.replace(&ours, stale)).expect("written");
        paths.push((path, good));
    }
    let args: Vec<String> = paths
        .iter()
        .map(|(path, _)| path.display().to_string())
        .collect();
    let mut command = vec!["--set-interface"];
    command.extend(args.iter().map(String::as_str));

    let (lines, status) = run(&command);
    assert_eq!(status, 0, "{lines:?}");
    for ((path, good), line) in paths.iter().zip(&lines) {
        assert_eq!(
            &tcsos::fs::read_to_string(path).expect("it reads"),
            good,
            "every other byte as it was"
        );
        assert!(
            line.contains(&format!("set to {ours}, from {stale}")),
            "{line}"
        );
    }
    let (lines, status) = run(&command);
    assert_eq!(status, 0);
    assert!(
        lines.iter().all(|line| line.contains("already")),
        "{lines:?}"
    );

    let (_, status) = run(&["--set-interface"]);
    assert_eq!(status, 2, "a configuration is named");
}

#[test]
fn the_options_are_taken_as_options() {
    let dir = tempfile::tempdir().expect("a directory");
    let set = with_errors(dir.path(), 12);
    let set = set.to_str().expect("a path");

    // Every one unless asked.
    let (lines, status) = run(&[set]);
    assert_eq!(lines.len(), 12, "{lines:?}");
    assert_eq!(status, 1);

    // Before the file or after it, either way.
    for args in [["--error-limit=2", set], [set, "--error-limit=2"]] {
        let (lines, status) = run(&args);
        assert_eq!(lines.len(), 2, "{args:?}: {lines:?}");
        assert!(
            lines.iter().all(|line| line.contains(": error: ")),
            "{lines:?}"
        );
        assert_eq!(status, 1);
    }

    // Nought prints nothing and passes.
    let (lines, status) = run(&["--error-limit=0", set]);
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(status, 0);

    // Every one, and what is done with warnings does not touch the errors.
    let (lines, status) =
        run(&["--error-limit=unlimited", "--warnings=off", set]);
    assert_eq!(lines.len(), 12, "{lines:?}");
    assert_eq!(status, 1);
}

/// The digest is printed for the configurations, and for them only.
///
/// The first pathname of two and the first two of three: what is left is the
/// payload simulator file, which has no digest of this kind -- it is not a
/// graph, and nothing aboard reads it, so there is nothing on the other end
/// to compare one with.
///
/// What the digest *is* is checked in `tcsprotocol`, which takes it.
/// Checked here: that the flag is taken as a flag, which file it is taken of,
/// and that the two spellings of one configuration come out alike -- that last
/// being the whole point of digesting what a file says rather than its bytes,
/// and the one thing a reader of this output relies on.
#[test]
fn the_digest_is_printed_for_the_configurations() {
    let example = |name: &str| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/thermostat")
            .join(name)
            .to_str()
            .expect("a path")
            .to_string()
    };
    let (yaml, xml, sim) = (
        example("thermostat.ytcs"),
        example("thermostat.xtcs"),
        example("thermostat-sim.yaml"),
    );
    let digest = |line: &str| line.split("md5 ").nth(1).map(str::to_string);

    // One configuration: its digest, and the flag was not taken for a file.
    let (lines, status) = run(&["--digest", &yaml]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with(&yaml), "{lines:?}");
    let said = digest(&lines[0]).unwrap_or_else(|| panic!("{lines:?}"));
    assert_eq!(said.len(), 32, "a digest is sixteen bytes of hex: {said}");
    assert!(said.chars().all(|c| c.is_ascii_hexdigit()), "{said}");

    // A configuration and its simulator file: the first of the two.
    let (lines, status) = run(&["--digest", &yaml, &sim]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(digest(&lines[0]), Some(said.clone()));

    // Two spellings and their simulator file: both configurations, and the
    // two digests are one digest.
    let (lines, status) = run(&["--digest", &xml, &yaml, &sim]);
    assert_eq!(status, 0, "{lines:?}");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with(&xml) && lines[1].starts_with(&yaml),
        "{lines:?}"
    );
    assert_eq!(
        digest(&lines[0]),
        digest(&lines[1]),
        "the two spellings of one configuration are one configuration"
    );
    assert_eq!(digest(&lines[0]), Some(said));
}

/// A configuration with problems still has a digest; one that cannot be read
/// has none, and saying so is a failed run.
///
/// The second half is why the status is checked under `--error-limit=0`: that
/// limit prints no errors and passes, so without this a run that was asked
/// for a digest and could not take one would end silently and successfully
/// having answered nothing.
#[test]
fn a_digest_that_cannot_be_taken_is_a_failed_run() {
    let dir = tempfile::tempdir().expect("a directory");

    // Twelve errors in it, and it still parses: a digest is of what a file
    // says, and this file says something.
    let set = with_errors(dir.path(), 12);
    let set = set.to_str().expect("a path");
    let (lines, status) = run(&["--digest", set]);
    assert_eq!(status, 1, "{lines:?}");
    assert_eq!(lines.len(), 13, "the digest and the twelve: {lines:?}");
    assert!(
        lines[0].contains(": md5 "),
        "the digest comes first: {lines:?}"
    );

    // And one that does not parse has nothing to digest.
    let unreadable = dir.path().join("unreadable.ytcs");
    tcsos::fs::write(&unreadable, "description: nonsense\npayloads: oops\n")
        .expect("written");
    let unreadable = unreadable.to_str().expect("a path");

    let (lines, status) = run(&["--digest", unreadable]);
    assert_eq!(status, 1, "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.contains("md5")),
        "there is no digest to print: {lines:?}"
    );

    // Even where nothing was counted against it.
    let (lines, status) = run(&["--digest", "--error-limit=0", unreadable]);
    assert_eq!(status, 1, "a digest was asked for and not given: {lines:?}");
    assert!(lines.is_empty(), "{lines:?}");
}

#[test]
fn a_bad_option_is_a_command_line_of_none_of_the_shapes() {
    let dir = tempfile::tempdir().expect("a directory");
    let set = with_errors(dir.path(), 1);
    let set = set.to_str().expect("a path");
    for bad in [
        "--error-limit=five",
        "--error-limit=",
        "--warnings=maybe",
        "--warnings=",
    ] {
        let (lines, status) = run(&[bad, set]);
        assert_eq!(status, 2, "{bad}");
        assert!(lines.is_empty(), "{bad}: nothing is checked: {lines:?}");
    }
}

/// Writes a configuration whose one vehicle node is written as `node`,
/// beside a ground link it is joined to both ways.
///
/// # Arguments
///
/// * `dir`       - The directory it is written in, as `one.ytcs`.
/// * `node_type` - The node's type.
/// * `node`      - The node's attributes after its name, as indented YAML
///                 lines.
///
/// # Returns
///
/// The file's path.
fn with_vehicle_node(dir: &Path, node_type: &str, node: &str) -> PathBuf {
    let text = format!(
        "\
description: one vehicle node
tcspecial:
  gateway: radio
  interface: \"b03f49878bcc9c45e7b854c7da1bce47\"
  state_file: /tmp/tcspecial-test.state
  max_consolidator_inputs: 32
  beacon:
    default_interval: \"5 s\"
    reset_interval: \"5 s\"
    recv: \"127.0.0.1:5650\"
    send: \"0.0.0.0:5550\"
  log:
    directory: \"/tmp/telemetry\"
    segment_size: \"65536\"
  payload_command:
    dest_base: \"0.0.0.0:4200\"
    source_base: \"127.0.0.1:4300\"
  payload_telemetry:
    dest_base: \"127.0.0.1:6100\"
    source_base: \"0.0.0.0:6000\"
  tcs_cmd:
    recv: \"0.0.0.0:4000\"
    send: \"127.0.0.1:4100\"
gateways:
  - type gateway_radio:
      - gateway: ground
        address: \"127.0.0.1:5000\"
        ground_base: \"127.0.0.1:5100\"
        moc-out: [pinger.ground]
        world-in: [pinger]
vehicle:
  - type {node_type}:
      - compute: pinger
{node}
        world-out: [ground.pinger]
        moc-in: [ground]
"
    );
    let path = dir.join("one.ytcs");
    tcsos::fs::write(&path, text).expect("written");
    path
}

/// Warnings and no errors is a run that passes: the warnings are printed,
/// and the exit status is success.
#[test]
fn warnings_alone_are_a_run_that_passes() {
    let dir = tempfile::tempdir().expect("a directory");
    let file =
        with_vehicle_node(dir.path(), "ping", "        interval: \"1 s\"");

    let (lines, status) = run(&[file.to_str().expect("a path")]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(": warning: "), "{lines:?}");
    assert_eq!(status, 0);
}

/// A node of a type that does not exist is an error, and fails the run.
#[test]
fn a_type_that_does_not_exist_fails_the_run() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = with_vehicle_node(dir.path(), "bogus", "");

    let (lines, status) = run(&[file.to_str().expect("a path")]);
    assert!(
        lines.iter().any(|line| line.contains(": error: ")
            && line.contains("bogus is not a type of node")),
        "{lines:?}"
    );
    assert_eq!(status, 1);
}

/// Writes a configuration of network payloads, each at a port of this host
/// and joined to nothing, so that each reports over `payload_telemetry`.
///
/// # Arguments
///
/// * `dir`   - The directory it is written in, as `ports.ytcs`.
/// * `ports` - Each payload's name and port.
///
/// # Returns
///
/// The file's path.
fn with_payloads_at(dir: &Path, ports: &[(&str, u16)]) -> PathBuf {
    let whole = tcsos::fs::read_to_string(with_errors(dir, 0)).expect("read");
    let mut text = whole
        .split("payloads:")
        .next()
        .expect("a section")
        .to_string();
    text.push_str("payloads:\n  - type network:\n");
    for (name, port) in ports {
        text.push_str(&format!(
            "      - payload: {name}\n        \
             address: \"127.0.0.1:{port}\"\n        protocol: \"udp\"\n"
        ));
    }
    let path = dir.join("ports.ytcs");
    tcsos::fs::write(&path, text).expect("written");
    path
}

/// A gateway node's ground port counted past 65535 -- its ground_base at
/// the last port, and a second source counted on from it -- is a
/// configuration complete and unflyable: an error, which fails the run, and
/// not the warning a node half filled in is. Without `Problem::Clash` it
/// was such a warning, and this run passed.
#[test]
fn a_gateway_port_past_the_last_fails_the_run() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = with_payloads_at(dir.path(), &[("one", 7001), ("two", 7002)]);
    let mut text = tcsos::fs::read_to_string(&file).expect("read");
    for name in ["one", "two"] {
        let port = if name == "one" { 7001 } else { 7002 };
        let at = format!("        address: \"127.0.0.1:{port}\"\n");
        text = text.replace(
            &at,
            &format!("{at}        world-out:\n          - ground.{name}\n"),
        );
    }
    text.push_str(
        "gateways:\n  - type gateway_radio:\n      - gateway: ground\n        \
         ground_base: \"127.0.0.1:65535\"\n        \
         world-in: [one, two]\n",
    );
    tcsos::fs::write(&file, &text).expect("written");
    let (lines, status) = run(&[file.to_str().expect("path")]);
    assert!(
        lines
            .iter()
            .any(|l| l.contains(": error: ") && l.contains("65535")),
        "{lines:?}"
    );
    assert_eq!(status, 1, "{lines:?}");
}

/// `--summary` lists, after checking, the thermostat's gateway node's far
/// end at its `ground_base` plus each source's device id, by source; and
/// the rates each link carries, with the beacon's and the total.
#[test]
fn the_summary_lists_the_gateway_ports_by_source() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/thermostat/thermostat.ytcs");
    let (lines, status) = run(&["--summary", example.to_str().expect("path")]);
    assert_eq!(status, 0, "{lines:?}");
    let graph = tcsprotocol::graph::Graph::load(&example).expect("it loads");
    let ids = tcsprotocol::graph::telemetry::ids(&graph);
    for source in ["consensus_temp", "to_k1", "to_k2", "to_k3"] {
        let port = 5100 + ids[source];
        assert!(
            lines
                .iter()
                .any(|line| line.contains(&format!("127.0.0.3:{port}"))
                    && line.contains(&format!("what \"{source}\" sends"))),
            "{source} at {port}: {lines:?}"
        );
    }
    for heading in [
        "Addresses and ports",
        "Device ids",
        "Message IDs",
        "APIDs",
        "Rates to the ground",
    ] {
        assert!(lines.iter().any(|l| l == heading), "{heading}: {lines:?}");
    }
    assert!(
        lines.iter().any(|l| l.starts_with("  beacon ")),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l.starts_with("  total ")), "{lines:?}");

    // One configuration, as a model is made of one.
    let (_, status) = run(&["--summary", "a.xtcs", "a.ytcs", "a-sim.yaml"]);
    assert_eq!(status, 2);
}

/// `--summary` lists each consolidator: its datum's size, and its inputs
/// with where each is in the datum, the bytes it takes and its default, an
/// input whose source is drawn into the gateway node itself too being said
/// to be sent twice; and its inputs have no ports or APIDs of their own.
#[test]
fn the_summary_lists_a_consolidators_inputs() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/consolidator/yamcs.ytcs");
    let (lines, status) = run(&["--summary", fixture.to_str().expect("path")]);
    assert_eq!(status, 0, "{lines:?}");
    let has = |want: &[&str]| {
        lines
            .iter()
            .any(|line| want.iter().all(|part| line.contains(part)))
    };
    assert!(lines.iter().any(|l| l == "Consolidators"), "{lines:?}");
    assert!(
        has(&[
            "\"pack\": 3 inputs, 4 to 12 bytes a datum",
            "to \"gateway\""
        ]),
        "{lines:?}"
    );
    assert!(has(&["temp", "u16", " 0 ", " 2 ", "0000"]), "{lines:?}");
    assert!(has(&["valve", "boolean", " 2 ", " 1 ", "00"]), "{lines:?}");
    assert!(
        has(&["name", "label", "end", "1 + up to 8", "empty", "sent twice"]),
        "{lines:?}"
    );
    assert!(!has(&["127.0.0.1:26703"]), "{lines:?}");
    assert!(!has(&["0x203"]), "{lines:?}");
}

/// The cfs example's summary lists its message IDs, inside its allotment:
/// the gateway node's cmd_msgid and what the thermometer sends to it.
#[test]
fn the_summary_lists_the_message_ids_on_a_bus() {
    let example =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/cfs/cfs.ytcs");
    let (lines, status) = run(&["--summary", example.to_str().expect("path")]);
    assert_eq!(status, 0, "{lines:?}");
    assert!(
        lines.iter().any(|l| l == "  msgid_range 0x4200 to 0x42ff"),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(
            |l| l.contains("0x4281") && l.contains("cmd_msgid of \"fsw\"")
        ),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("0x4242")
            && l.contains("what \"thermometer\" sends to \"fsw\"")),
        "{lines:?}"
    );
}

/// Two payloads on one port are an error, said with the summary, which
/// fails the run: the rule is the existing one for two nodes that want one
/// thing.
#[test]
fn two_payloads_on_one_port_fail_the_summary() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = with_payloads_at(dir.path(), &[("one", 7001), ("two", 7001)]);
    let (lines, status) = run(&["--summary", file.to_str().expect("path")]);
    assert!(
        lines.iter().any(|l| l.contains(": error: ")
            && l.contains("\"one\" and \"two\" both want localhost:7001")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l == "Addresses and ports"),
        "{lines:?}"
    );
    assert_eq!(status, 1);
}

/// A payload at the port the vehicle takes commands on is one socket with it:
/// an error, naming both, which fails the run. The rule is
/// `tcsprotocol::graph::sockets`'s, and without it this configuration
/// passes.
#[test]
fn a_payload_on_the_command_port_fails_the_run() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = with_payloads_at(dir.path(), &[("one", 4000)]);
    let (lines, status) = run(&[file.to_str().expect("path")]);
    assert!(
        lines.iter().any(|l| l.contains(": error: ")
            && l.contains("tcs_cmd.recv")
            && l.contains("\"one\" address and port")
            && l.contains("one socket")),
        "{lines:?}"
    );
    assert_eq!(status, 1);

    // And moved to a port of its own, it passes.
    let file = with_payloads_at(dir.path(), &[("one", 7001)]);
    let (lines, status) = run(&[file.to_str().expect("path")]);
    assert_eq!(status, 0, "{lines:?}");
}

/// Every example's configuration but the files they include, and the
/// vehicle each is part of: an example on its own, or the stages of
/// Building a Vehicle, the two landers or the two thermostats, which are
/// each one vehicle run one at a time.
///
/// # Returns
///
/// Each configuration's path and the name of the vehicle it is.
fn every_example() -> Vec<(PathBuf, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = vec![(root.join("example.ytcs"), "example".to_string())];
    let mut add = |dir: PathBuf| {
        let entries = tcsos::fs::read_dir(&dir).expect("the directory reads");
        for entry in entries {
            let path = entry.expect("an entry").path();
            let name = path.to_string_lossy().to_string();
            if path.extension().is_some_and(|e| e == "ytcs")
                && !name.contains("-common/")
                && !name.contains("/common/")
            {
                let vehicle = if name.contains("building-a-vehicle") {
                    "building-a-vehicle"
                } else if name.contains("/lander-") {
                    "lander"
                } else if name.contains("/thermostat") {
                    "thermostat"
                } else {
                    &name
                };
                found.push((path, vehicle.to_string()));
            }
        }
    };
    for parent in ["examples", "docs/building-a-vehicle"] {
        let dirs = tcsos::fs::read_dir(root.join(parent)).expect("it reads");
        for dir in dirs {
            let dir = dir.expect("an entry").path();
            if tcsos::fs::metadata(&dir).is_ok_and(|m| m.is_dir()) {
                add(dir);
            }
        }
    }
    found
}

/// No two examples bind one socket, so that any two may run at once: each
/// has its own pair of loopback hosts, the MOC on the odd one and the
/// vehicle on the even one after it, and binds nothing at `0.0.0.0`, which
/// would hold its port on every host. A port reached as a base plus a device
/// id counts, as `--summary` lists it. Examples that are one vehicle -- the
/// stages of Building a Vehicle, the two landers, the two thermostats --
/// share theirs, and run one at a time.
#[test]
fn no_two_examples_bind_one_socket() {
    let mut taken: Vec<(String, u16, String, String)> = Vec::new();
    for (path, vehicle) in every_example() {
        let shown = path.to_string_lossy().to_string();
        let (lines, status) = run(&["--summary", &shown]);
        assert_eq!(status, 0, "{shown}: {lines:?}");
        let start = lines
            .iter()
            .position(|l| l == "Addresses and ports")
            .unwrap_or_else(|| panic!("{shown}: no addresses"));
        let mut here = Vec::new();
        for line in lines[start + 2..].iter().take_while(|l| !l.is_empty()) {
            let at = line.split_whitespace().next().expect("an address");
            let (host, port) = at.rsplit_once(':').expect("host:port");
            let port: u16 = port.parse().expect("a port");
            assert_ne!(host, "0.0.0.0", "{shown} binds {at}");
            if port == 0 {
                continue;
            }
            for (h, p, other, by) in &taken {
                assert!(
                    *p != port || *h != host || *other == vehicle,
                    "{shown} and {by} both bind {at}"
                );
            }
            here.push((host.to_string(), port, vehicle.clone(), shown.clone()));
        }
        taken.extend(here);
    }
}

/// No two examples that are not one vehicle hold anything one would take
/// from the other -- a socket, a log directory, a state file -- so that any
/// two may run at once, each logging in a directory of its own under
/// `/tmp/telemetry`. What tcs-conflict says of two configurations is what is
/// asked. A device under `/dev` is left out: the examples are written for
/// one board, a desk simulates every device they name, and two of them are
/// never flown on one board.
#[test]
fn no_two_examples_hold_one_thing() {
    use tcsprotocol::graph::check::Severity;
    use tcsprotocol::graph::conflict::{between, holdings};
    let held: Vec<(String, String, Vec<_>)> = every_example()
        .into_iter()
        .map(|(path, vehicle)| {
            let graph = tcsprotocol::graph::Graph::load(&path)
                .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
            let shown = path.to_string_lossy().to_string();
            (shown, vehicle, holdings(&graph, None))
        })
        .collect();
    for (i, (one, one_vehicle, one_holds)) in held.iter().enumerate() {
        for (two, two_vehicle, two_holds) in &held[i + 1..] {
            if one_vehicle == two_vehicle {
                continue;
            }
            for conflict in between(one_holds, two_holds) {
                assert!(
                    !matches!(conflict.severity, Severity::Error)
                        || conflict.why.contains("/dev/"),
                    "{one} and {two}: {}",
                    conflict.why
                );
            }
        }
    }
}
