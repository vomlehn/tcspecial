//! A consolidator, as a configuration states it and the checks hold it:
//! its inputs and their links, how many there may be, when it sends, where
//! what it sends may go, and how large its datum is. See "Compute
//! elements" in `docs/tcspecial-design.rst`; how the datum is laid out is
//! `tcsprotocol::consolidation`'s, and tested there.

use tcsprotocol::graph::Graph;

/// A configuration of two payloads, a u32 and a boolean, consolidated and
/// sent to the ground, with `edit` applied to its text first.
///
/// # Arguments
///
/// * `edit` - What to change in the text, as pairs of what is there and
///            what to put there instead.
///
/// # Returns
///
/// The configuration, loaded, and what its check says, each problem as a
/// line.
///
/// # Panics
///
/// If what `edit` names is not in the text, or the file cannot be written.
fn checked(edit: &[(&str, &str)]) -> (Graph, Vec<String>) {
    let mut text = format!(
        "tcspecial:
  beacon:
    default_interval: \"5 s\"
    recv: \"127.0.0.1:7510\"
    reset_interval: \"1 s\"
    send: \"0.0.0.0:7500\"
  gateway: \"radio\"
  interface: \"{}\"
  max_consolidator_inputs: \"32\"
  payload_command:
    dest_base: \"0.0.0.0:7300\"
    source_base: \"127.0.0.1:7310\"
  payload_telemetry:
    dest_base: \"127.0.0.1:7410\"
    source_base: \"0.0.0.0:7400\"
  state_file: \"/tmp/tcspecial-consolidator.state\"
  tcs_cmd:
    recv: \"0.0.0.0:7200\"
    send: \"127.0.0.1:7210\"
payloads:
  - type network:
      - payload: temp
        address: \"127.0.0.1:7801\"
        protocol: \"udp\"
        world-out-type: \"u32\"
        world-out: [pack.temp]
      - payload: valve
        address: \"127.0.0.1:7803\"
        protocol: \"udp\"
        world-out-type: \"boolean\"
        world-out: [pack.valve]
vehicle:
  - type compute:
      - compute: pack
        function: consolidator
        send: [all_new, interval]
        interval: \"1 s\"
        defaults: [temp=0, valve=0]
        world-in: [temp, valve]
        world-out: [gateway.pack]
gateways:
  - type gateway_radio:
      - gateway: gateway
        address: \"127.0.0.1:7600\"
        ground_base: \"127.0.0.1:7700\"
        world-in: [pack]
",
        tcsprotocol::InterfaceDigest::of_this_build()
    );
    for (was, now) in edit {
        assert!(text.contains(was), "the configuration has no {was:?}");
        text = text.replacen(was, now, 1);
    }
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("pack.ytcs");
    tcsos::fs::write(&path, text).expect("written");
    let graph = Graph::load(&path).expect("it reads");
    let said = graph.check().iter().map(|p| p.describe()).collect();
    (graph, said)
}

/// The sample is sound, and its datum is the states, four bits, the u32,
/// thirty-two, and the boolean, one: thirty-seven bits, five bytes.
#[test]
fn a_consolidator_is_sound_and_sized_by_its_inputs() {
    let (graph, said) = checked(&[]);
    assert!(said.is_empty(), "{said:?}");
    let pack = graph.node("pack").expect("the consolidator");
    let largest = tcsprotocol::graph::largest::largest_datum(&graph, pack);
    assert_eq!(largest.bytes, 5);
    assert!(largest.bounded);
}

/// A consolidator whose datum could be larger than a datagram of its
/// gateway's link carries is refused, though each input alone would fit:
/// its largest datum is the sum of its inputs' with their lengths and
/// states.
#[test]
fn a_datum_that_could_be_too_large_is_refused() {
    let (graph, said) = checked(&[
        (
            "world-out-type: \"u32\"",
            "world-out-type: \"bytes(40000)\"",
        ),
        (
            "world-out-type: \"boolean\"",
            "world-out-type: \"bytes(40000)\"",
        ),
        (
            "defaults: [temp=0, valve=0]",
            "defaults: [\"temp=[]\", \"valve=[]\"]",
        ),
    ]);
    let pack = graph.node("pack").expect("the consolidator");
    assert!(
        tcsprotocol::graph::largest::largest_datum(&graph, pack).bytes > 80_000
    );
    assert!(
        said.iter()
            .any(|s| s.contains("\"pack\" can send")
                && s.contains("bytes at once")),
        "{said:?}"
    );
}

/// A default must be given for every input, as a value of its type, and
/// one that is not is refused, naming the input.
#[test]
fn every_input_has_a_default_of_its_type() {
    let (_, said) =
        checked(&[("defaults: [temp=0, valve=0]", "defaults: [temp=0]")]);
    assert!(
        said.iter().any(|s| s.contains("no entry for \"valve\"")),
        "{said:?}"
    );
    let (_, said) = checked(&[(
        "defaults: [temp=0, valve=0]",
        "defaults: [temp=0, valve=2]",
    )]);
    assert!(
        said.iter()
            .any(|s| s.contains("\"valve\"") && s.contains("boolean")),
        "{said:?}"
    );
    let (_, said) = checked(&[(
        "defaults: [temp=0, valve=0]",
        "defaults: [temp=0, valve=0, gauge=1]",
    )]);
    assert!(
        said.iter().any(|s| s.contains("\"gauge\" is no input")),
        "{said:?}"
    );
}

/// More inputs than max_consolidator_inputs is refused, naming it.
#[test]
fn too_many_inputs_are_refused() {
    let (_, said) = checked(&[(
        "max_consolidator_inputs: \"32\"",
        "max_consolidator_inputs: \"2\"",
    )]);
    assert!(said.is_empty(), "two of two is allowed: {said:?}");
    let (_, said) = checked(&[
        (
            "max_consolidator_inputs: \"32\"",
            "max_consolidator_inputs: \"2\"",
        ),
        ("world-in: [temp, valve]", "world-in: [temp, valve, again]"),
        (
            "defaults: [temp=0, valve=0]",
            "defaults: [temp=0, valve=0, again=0]",
        ),
        (
            "world-out: [pack.temp]",
            "world-out: [pack.temp, pack.again]",
        ),
    ]);
    assert!(
        said.iter()
            .any(|s| s.contains("max_consolidator_inputs is 2")),
        "{said:?}"
    );
    let (_, said) = checked(&[(
        "max_consolidator_inputs: \"32\"",
        "max_consolidator_inputs: \"1\"",
    )]);
    assert!(said.iter().any(|s| s.contains("at least 2")), "{said:?}");
}

/// When it sends: all_new alone, an interval without the trigger, the
/// trigger without an interval, and a trigger that is none are refused.
#[test]
fn when_it_sends_is_held_together() {
    for (edit, why) in [
        (
            ("send: [all_new, interval]", "send: [all_new]"),
            "all_new alone",
        ),
        (
            ("send: [all_new, interval]", "send: [moc]"),
            "interval is given",
        ),
        (("        interval: \"1 s\"\n", ""), "no interval is given"),
        (
            ("send: [all_new, interval]", "send: [soon]"),
            "no trigger called",
        ),
    ] {
        let (_, said) = checked(&[edit]);
        assert!(said.iter().any(|s| s.contains(why)), "{edit:?}: {said:?}");
    }
    let (_, said) = checked(&[
        ("send: [all_new, interval]", "send: [moc]"),
        ("        interval: \"1 s\"\n", ""),
    ]);
    assert!(said.is_empty(), "moc alone: {said:?}");
}

/// What it sends may go to a gateway, a log or a tee, and nowhere else.
#[test]
fn what_it_sends_goes_only_to_a_gateway_a_log_or_a_tee() {
    let (_, said) = checked(&[
        (
            "world-out: [gateway.pack]",
            "world-out: [gateway.pack, keep.pack]",
        ),
        (
            "gateways:",
            "  - type log:
      - compute: keep
        directory: \"/tmp/telemetry\"
        format: \"fixed:5\"
        segment_bytes: \"65536\"
        world-in: [pack]
gateways:",
        ),
    ]);
    assert!(said.is_empty(), "a log of its width: {said:?}");

    let (_, said) = checked(&[
        (
            "world-out: [gateway.pack]",
            "world-out: [gateway.pack, keep.pack]",
        ),
        (
            "gateways:",
            "  - type log:
      - compute: keep
        directory: \"/tmp/telemetry\"
        format: \"fixed:4\"
        segment_bytes: \"65536\"
        world-in: [pack]
gateways:",
        ),
    ]);
    assert!(
        said.iter().any(|s| s.contains("records of 5 bytes")),
        "a log of another width: {said:?}"
    );

    let (_, said) = checked(&[
        (
            "world-out: [gateway.pack]",
            "world-out: [gateway.pack, think.pack]",
        ),
        (
            "gateways:",
            "      - compute: think
        function: process
        executable: \"/bin/cat\"
        timeout: \"1 s\"
        restart: \"never\"
        type-in: \"bytes(16)\"
        type-out: \"bytes(16)\"
        world-in: [pack]
gateways:",
        ),
    ]);
    assert!(
        said.iter().any(|s| s.contains("a gateway, a log or a tee")),
        "{said:?}"
    );
}

/// Each input it lists is linked, once: one listed and not linked is
/// refused.
#[test]
fn each_input_is_linked() {
    let (_, said) = checked(&[(
        "world-in: [temp, valve]",
        "world-in: [temp, valve, lost]",
    )]);
    assert!(said.iter().any(|s| s.contains("\"lost\"")), "{said:?}");
}
