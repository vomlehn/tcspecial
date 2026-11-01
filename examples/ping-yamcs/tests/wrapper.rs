//! The Yamcs wrapper reads what the vehicle frames, and frames what the
//! vehicle reads.
//!
//! The mission database's wrapper, which `tcslibgs/yamcs.py` generates from
//! the configuration and `python/tcslibgs/yamcs.xtce`, tells Yamcs how a
//! packet on the link is laid out: a CCSDS primary header, and after it, by
//! APID, what follows. The vehicle frames and unframes with
//! `tcslibgs::ccsds`. Nothing but a run against Yamcs itself would show the
//! two disagreeing -- a header width, a length off by one, a restriction on
//! the wrong APID -- so this generates the wrapper, as `ping_yamcs.py` does,
//! and works it as Yamcs would on the vehicle's own packets:
//!
//! * each packet the vehicle sends down -- a beacon, a ping's answer,
//!   CONNECT's answer and another command's, and in a copy of the example
//!   with more nodes, a payload's data through the gateway node and a
//!   payload's telemetry -- framed by `tcslibgs` and
//!   decoded by the wrapper, must land in the container meant for it, with
//!   the header's fields and the body where the wrapper says;
//! * each command Yamcs sends up -- CONNECT and a ping -- encoded by the
//!   wrapper, with its length filled in as Yamcs fills it in, must be
//!   unframed by `tcslibgs` and read as the command it is.
//!
//! The interpreter here does what the wrapper uses of XTCE and no more: a
//! wrapper that grows past it fails here, saying what it does not know,
//! rather than passing unread.

use std::collections::BTreeMap;
use std::time::Duration;

use tcslibgs::ccsds::{read_header, PacketType, Sender, HEADER_BYTES};
use tcslibgs::graph::telemetry;
use tcslibgs::graph::Graph;
use tcslibgs::node::vehicle::ping;
use tcslibgs::telemetry::{ConnectTelemetry, PingTelemetry, Telemetry};
use tcslibgs::types::CommandStatus;
use tcslibgs::{Beacon, Command, ConfigDigest, ConfigVersion};

mod common;

use common::{generated_mdb, here, with_more_nodes, worked_out_of, Element};

/// The wrapper `tcslibgs/yamcs.py` generates for the example's
/// configuration, as `ping_yamcs.py` writes it for Yamcs.
///
/// # Returns
///
/// Its root element.
///
/// # Panics
///
/// It is not generated, or does not read.
fn wrapper() -> Element {
    generated_mdb(&here().join("ping-yamcs.xtcs"))
}

/// The APID the Rust works out for a name `ping_yamcs.py --describe` uses.
fn apid(name: &str) -> u16 {
    worked_out_of(name).parse().expect("an APID")
}

/// `bits` bits of `packet` from bit `at`, most significant first.
///
/// # Arguments
///
/// * `packet` - The packet.
/// * `at`     - The first bit, counting from nought.
/// * `bits`   - How many.
///
/// # Returns
///
/// Their value.
fn bits(packet: &[u8], at: usize, bits: usize) -> u64 {
    assert!(at + bits <= packet.len() * 8, "{bits} bits at {at} overrun");
    (at..at + bits).fold(0, |value, bit| {
        let set = packet[bit / 8] >> (7 - bit % 8) & 1;
        value << 1 | u64::from(set)
    })
}

/// The bits a parameter's or argument's encoding takes, given the values
/// already decoded that a dynamic size may name.
///
/// # Arguments
///
/// * `kind`   - Its type.
/// * `values` - The integers known so far, by name.
///
/// # Returns
///
/// How many bits it takes.
///
/// # Panics
///
/// The type is not one of those the wrapper uses.
fn width(kind: &Element, values: &BTreeMap<String, u64>) -> usize {
    if let Some(encoding) = kind.child("IntegerDataEncoding") {
        return encoding
            .attribute("sizeInBits")
            .expect("an integer's size")
            .parse()
            .expect("a number");
    }
    let mut fixed = Vec::new();
    kind.within("FixedValue", &mut fixed);
    if let [fixed] = fixed.as_slice() {
        return fixed.text.trim().parse().expect("a number");
    }
    let mut dynamic = Vec::new();
    kind.within("DynamicValue", &mut dynamic);
    let [dynamic] = dynamic.as_slice() else {
        panic!("{} has no size this test reads", kind.name);
    };
    let reference = dynamic
        .child("ParameterInstanceRef")
        .and_then(|r| r.attribute("parameterRef"))
        .or_else(|| {
            dynamic
                .child("ArgumentInstanceRef")
                .and_then(|r| r.attribute("argumentRef"))
        })
        .expect("what the size is read from");
    let value = values[reference] as i64;
    let (slope, intercept) =
        dynamic
            .child("LinearAdjustment")
            .map_or((1, 0), |adjustment| {
                let number = |name| {
                    adjustment
                        .attribute(name)
                        .unwrap_or("0")
                        .parse::<i64>()
                        .expect("a whole number")
                };
                (number("slope"), number("intercept"))
            });
    (slope * value + intercept) as usize
}

/// Where an entry starts: where it says, from the start of the packet, or
/// where the entry before it ended.
///
/// # Arguments
///
/// * `entry` - The entry.
/// * `after` - Where the entry before it ended, or `None` where that is
///             not known.
///
/// # Returns
///
/// The bit it starts at.
fn start_of(entry: &Element, after: Option<usize>) -> usize {
    match entry.child("LocationInContainerInBits") {
        Some(location) => {
            assert_eq!(
                location.attribute("referenceLocation"),
                Some("containerStart"),
                "a location this test reads"
            );
            location
                .child("FixedValue")
                .expect("a fixed location")
                .text
                .trim()
                .parse()
                .expect("a number")
        }
        None => after.expect("an entry after a container placed nowhere"),
    }
}

/// What a packet decoded to.
#[derive(Debug, Default)]
struct Decoded {
    /// The container it ended in: the most derived whose restrictions held.
    container: String,
    /// Each integer parameter.
    integers: BTreeMap<String, u64>,
    /// Each string or binary parameter, as its bytes.
    bytes: BTreeMap<String, Vec<u8>>,
    /// Each container of another document it refers to, and the bit it
    /// starts at.
    refers: Vec<(String, usize)>,
}

/// Whether a comparison holds.
///
/// # Arguments
///
/// * `left`     - The value compared.
/// * `operator` - The comparison, as XTCE writes it.
/// * `right`    - What it is compared with.
///
/// # Returns
///
/// Whether it holds.
fn holds(left: u64, operator: &str, right: u64) -> bool {
    match operator {
        "==" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        other => panic!("a comparison this test does not know: {other}"),
    }
}

/// Whether a container's restrictions hold of what has been decoded.
///
/// # Arguments
///
/// * `criteria` - Its `RestrictionCriteria`.
/// * `values`   - The integers decoded so far.
///
/// # Returns
///
/// Whether they hold.
fn restricted(criteria: &Element, values: &BTreeMap<String, u64>) -> bool {
    let compare = |comparison: &Element| {
        let name = comparison.attribute("parameterRef").expect("a parameter");
        let value = comparison
            .attribute("value")
            .expect("a value")
            .parse()
            .expect("a number");
        let operator =
            comparison.attribute("comparisonOperator").unwrap_or("==");
        holds(values[name], operator, value)
    };
    let condition = |condition: &Element| {
        let name = condition
            .child("ParameterInstanceRef")
            .and_then(|r| r.attribute("parameterRef"))
            .expect("a parameter");
        let operator = condition
            .child("ComparisonOperator")
            .expect("an operator")
            .text
            .trim();
        let value = condition
            .child("Value")
            .expect("a value")
            .text
            .trim()
            .parse()
            .expect("a number");
        holds(values[name], operator, value)
    };
    let [kind] = criteria.children.as_slice() else {
        panic!("one restriction at a time");
    };
    match kind.name.as_str() {
        "Comparison" => compare(kind),
        "ComparisonList" => kind.children.iter().all(compare),
        "BooleanExpression" => {
            let ored = kind.child("ORedConditions").expect("conditions");
            ored.children.iter().any(condition)
        }
        other => panic!("a restriction this test does not know: {other}"),
    }
}

/// Decodes a packet from the link as Yamcs would by the wrapper: from its
/// root container down through whichever container's restrictions hold.
///
/// # Arguments
///
/// * `xtce`   - The wrapper.
/// * `packet` - The packet as the vehicle sent it.
///
/// # Returns
///
/// What it decoded to.
///
/// # Panics
///
/// Two containers' restrictions both hold, or the wrapper uses what this
/// does not read.
fn decode(xtce: &Element, packet: &[u8]) -> Decoded {
    let mut containers = Vec::new();
    xtce.within("SequenceContainer", &mut containers);
    let mut decoded = Decoded {
        container: "CCSDSPacket".to_string(),
        ..Decoded::default()
    };
    let mut after = Some(0);
    loop {
        let container = xtce.named("SequenceContainer", &decoded.container);
        for entry in &container.child("EntryList").expect("entries").children {
            let at = start_of(entry, after);
            match entry.name.as_str() {
                "ParameterRefEntry" => {
                    let name =
                        entry.attribute("parameterRef").expect("a parameter");
                    let parameter = xtce.named("Parameter", name);
                    let kind_name = parameter
                        .attribute("parameterTypeRef")
                        .expect("a type");
                    let mut kinds = Vec::new();
                    xtce.within("ParameterTypeSet", &mut kinds);
                    let kind = kinds[0]
                        .children
                        .iter()
                        .find(|k| k.attribute("name") == Some(kind_name))
                        .expect("its type");
                    let size = width(kind, &decoded.integers);
                    if kind.name == "IntegerParameterType" {
                        decoded
                            .integers
                            .insert(name.to_string(), bits(packet, at, size));
                    } else {
                        assert!(
                            at.is_multiple_of(8) && size.is_multiple_of(8),
                            "{name}: whole octets"
                        );
                        assert!(
                            (at + size) / 8 <= packet.len(),
                            "{name} runs past the packet"
                        );
                        decoded.bytes.insert(
                            name.to_string(),
                            packet[at / 8..(at + size) / 8].to_vec(),
                        );
                    }
                    after = Some(at + size);
                }
                "ContainerRefEntry" => {
                    let name =
                        entry.attribute("containerRef").expect("a container");
                    decoded.refers.push((name.to_string(), at));
                    // How long another document's container is, this does
                    // not read; whatever follows says where it is.
                    after = None;
                }
                other => panic!("an entry this test does not know: {other}"),
            }
        }
        let mut next = containers.iter().filter(|candidate| {
            candidate.child("BaseContainer").is_some_and(|base| {
                base.attribute("containerRef")
                    == Some(decoded.container.as_str())
                    && base
                        .child("RestrictionCriteria")
                        .is_some_and(|c| restricted(c, &decoded.integers))
            })
        });
        let Some(derived) = next.next() else {
            return decoded;
        };
        let name = derived.attribute("name").expect("a name").to_string();
        if let Some(also) = next.next() {
            panic!(
                "{name} and {} both match one packet",
                also.attribute("name").unwrap_or("?")
            );
        }
        decoded.container = name;
    }
}

/// What an argument is given.
enum Given<'a> {
    Integer(u64),
    Bytes(&'a [u8]),
}

/// Encodes a command as Yamcs would by the wrapper: its base command's
/// container, then its own, each argument as given or assigned or by its
/// initial value; and then the length field filled in, as Yamcs's command
/// postprocessor fills it in.
///
/// # Arguments
///
/// * `xtce`    - The wrapper.
/// * `command` - The command's name.
/// * `given`   - Its arguments, by name.
///
/// # Returns
///
/// The packet as it goes on the link.
fn encode(xtce: &Element, command: &str, given: &[(&str, Given)]) -> Vec<u8> {
    // The command, and the commands it is built on, base first.
    let mut chain = vec![xtce.named("MetaCommand", command)];
    while let Some(base) = chain
        .last()
        .and_then(|c| c.child("BaseMetaCommand"))
        .and_then(|b| b.attribute("metaCommandRef"))
    {
        chain.push(xtce.named("MetaCommand", base));
    }
    chain.reverse();

    // Every argument's value, and its type.
    let mut integers: BTreeMap<String, u64> = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for meta in &chain {
        for argument in meta
            .child("ArgumentList")
            .map_or(&[][..], |list| list.children.as_slice())
        {
            let name = argument.attribute("name").expect("a name");
            kinds.insert(
                name.to_string(),
                argument.attribute("argumentTypeRef").expect("a type"),
            );
            if let Some(initial) = argument.attribute("initialValue") {
                integers.insert(name.to_string(), initial.parse().unwrap());
            }
        }
        if let Some(assigned) = meta
            .child("BaseMetaCommand")
            .and_then(|b| b.child("ArgumentAssignmentList"))
        {
            for assignment in &assigned.children {
                let name = assignment.attribute("argumentName").unwrap();
                let value = assignment.attribute("argumentValue").unwrap();
                integers.insert(name.to_string(), value.parse().unwrap());
            }
        }
    }
    let mut bytes = BTreeMap::new();
    for (name, value) in given {
        match value {
            Given::Integer(value) => {
                integers.insert(name.to_string(), *value);
            }
            Given::Bytes(value) => {
                bytes.insert(name.to_string(), *value);
            }
        }
    }

    let mut types = Vec::new();
    xtce.within("ArgumentTypeSet", &mut types);
    let mut bits: Vec<bool> = Vec::new();
    for meta in &chain {
        let container = meta.child("CommandContainer").expect("a container");
        for entry in &container.child("EntryList").expect("entries").children {
            let name = entry.attribute("argumentRef").expect("an argument");
            let kind = types[0]
                .children
                .iter()
                .find(|k| k.attribute("name") == Some(kinds[name]))
                .expect("its type");
            let size = width(kind, &integers);
            match (integers.get(name), bytes.get(name)) {
                (Some(&value), _) if kind.name.starts_with("Integer") => {
                    assert!(size == 64 || value < 1 << size, "{name} fits");
                    bits.extend((0..size).rev().map(|b| value >> b & 1 == 1));
                }
                (_, Some(value)) => {
                    assert_eq!(value.len() * 8, size, "{name}'s size");
                    for byte in value.iter() {
                        bits.extend((0..8).rev().map(|b| byte >> b & 1 == 1));
                    }
                }
                _ => panic!("{name} is given no value"),
            }
        }
    }
    assert_eq!(bits.len() % 8, 0, "{command} is whole octets");
    let mut packet: Vec<u8> = bits
        .chunks(8)
        .map(|byte| byte.iter().fold(0, |b, &bit| b << 1 | u8::from(bit)))
        .collect();
    // The length field: what follows the header, less one.
    let length = (packet.len() - HEADER_BYTES - 1) as u16;
    packet[4..6].copy_from_slice(&length.to_be_bytes());
    packet
}

/// Frames a packet's data as the vehicle does, under an APID.
///
/// # Arguments
///
/// * `apid` - The APID.
/// * `data` - What follows the header.
///
/// # Returns
///
/// The packet.
fn framed(apid: u16, data: &[u8]) -> Vec<u8> {
    let mut sender =
        Sender::new(PacketType::Telemetry, apid).expect("a sender");
    let mut room = vec![0u8; HEADER_BYTES + data.len()];
    sender.frame(&mut room, data).expect("framed").to_vec()
}

/// A beacon, framed by the vehicle under `beacon.apid`, is a BeaconPacket:
/// its header read field by field, the beacon's container after the header,
/// and the beacon's bytes whole, which the window's beacon panel reads.
#[test]
fn a_beacon_decodes_as_the_vehicle_framed_it() {
    let xtce = wrapper();
    let body = Beacon::new(
        5,
        ConfigVersion::of_this_build(),
        ConfigDigest([0x5a; 16]),
    )
    .to_bytes();
    let packet = framed(apid("beacon_apid"), &body);
    let decoded = decode(&xtce, &packet);
    assert_eq!(decoded.container, "BeaconPacket");
    let header = |name: &str| decoded.integers[name];
    assert_eq!(header("CcsdsVersion"), 0);
    assert_eq!(header("CcsdsType"), 0, "telemetry");
    assert_eq!(header("CcsdsSecHdr"), 0);
    assert_eq!(header("CcsdsApid"), u64::from(apid("beacon_apid")));
    assert_eq!(header("CcsdsSeqFlags"), 3, "unsegmented");
    assert_eq!(header("CcsdsLength"), body.len() as u64 - 1);
    assert_eq!(
        decoded.refers,
        vec![("/TcsBeacon/Beacon".to_string(), HEADER_BYTES * 8)]
    );
    assert_eq!(decoded.bytes["BeaconBody"], body);
}

/// A ping's answer, framed under `apid_base` plus the ping node's device
/// id, is the ping node's container, its parameter the answer whole.
#[test]
fn a_pings_answer_decodes_as_its_text() {
    let xtce = wrapper();
    let sent = Duration::new(1_793_562_467, 79_578_927);
    let message = ping::message(1, sent).expect("a ping");
    let answer =
        ping::reply(message.as_bytes(), sent + Duration::from_millis(5))
            .expect("an answer");
    let packet = framed(apid("reply_apid"), answer.as_bytes());
    let decoded = decode(&xtce, &packet);
    assert_eq!(decoded.container, "pingPacket");
    assert_eq!(decoded.bytes["ping"], answer.as_bytes());
}

/// CONNECT's answer, on `tcs_cmd.apid`, is a FixedAnswerPacket, the fixed
/// answer after the header; another command's answer there is a
/// LinkAnswerPacket.
#[test]
fn answers_decode_by_their_opcode() {
    let xtce = wrapper();
    let connect = Telemetry::Connect(ConnectTelemetry::new(
        7,
        ConfigVersion::of_this_build(),
        ConfigDigest([1; 16]),
        7,
    ))
    .to_bytes();
    let decoded = decode(&xtce, &framed(apid("cmd_apid"), &connect));
    assert_eq!(decoded.container, "FixedAnswerPacket");
    assert_eq!(decoded.integers["AnswerOpcode"], u64::from(connect[0]));
    assert_eq!(
        decoded.refers,
        vec![("/TcsFixed/FixedAnswer".to_string(), HEADER_BYTES * 8)]
    );

    let other = Telemetry::Ping(PingTelemetry::new(3, CommandStatus::Success))
        .to_bytes();
    let decoded = decode(&xtce, &framed(apid("cmd_apid"), &other));
    assert_eq!(decoded.container, "LinkAnswerPacket");
    assert_eq!(
        decoded.refers,
        vec![("/TcsLink/AnswerLayout".to_string(), HEADER_BYTES * 8)]
    );
}

/// CONNECT, as Yamcs sends it by the wrapper, is unframed by the vehicle
/// under `tcs_cmd.apid` and read as the CONNECT it is.
#[test]
fn connect_encodes_as_the_vehicle_reads_it() {
    let xtce = wrapper();
    let checksum = [
        0x27, 0xbf, 0x79, 0xb2, 0x37, 0x06, 0x9f, 0xc4, 0xc5, 0xd1, 0x33, 0x25,
        0xe1, 0xac, 0x0b, 0xb6,
    ];
    let packet = encode(
        &xtce,
        "Connect",
        &[
            ("Sequence", Given::Integer(9)),
            ("Checksum", Given::Bytes(&checksum)),
        ],
    );
    let body = read_header(&packet, PacketType::Telecommand, apid("cmd_apid"))
        .expect("the vehicle takes its header");
    match Command::read(body).expect("the vehicle reads it") {
        Command::Connect(connect) => {
            assert_eq!(connect.header.sequence, 9);
            assert_eq!(connect.digest, ConfigDigest(checksum));
        }
        other => panic!("CONNECT was read as {other:?}"),
    }
}

/// A ping, as Yamcs sends it by the wrapper as the gateway node's command,
/// is unframed by the vehicle under the gateway node's `apid_base`, its
/// text whole.
#[test]
fn a_ping_encodes_as_its_text() {
    let xtce = wrapper();
    let message = ping::message(4, Duration::new(1_793_562_467, 160_521_331))
        .expect("a ping");
    let packet = encode(
        &xtce,
        "gateway",
        &[
            ("Data", Given::Bytes(message.as_bytes())),
            ("DataBits", Given::Integer(8 * message.len() as u64)),
        ],
    );
    let body =
        read_header(&packet, PacketType::Telecommand, apid("uplink_apid"))
            .expect("the vehicle takes its header");
    assert_eq!(body, message.as_bytes());
    ping::read_message(body).expect("the vehicle reads the ping");
}

/// In a copy of the example with more nodes, the data of each, framed under
/// the APID `tcslibgs` counts for it -- "pong"'s through the gateway node,
/// "sensor"'s and "tank"'s over `payload_telemetry` -- is that node's
/// container, its parameter the data whole; and the ping node's answers
/// and the gateway node's command are as they were.
#[test]
fn added_nodes_decode_into_their_own_containers() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = with_more_nodes(dir.path());
    let xtce = generated_mdb(&path);
    let graph = Graph::load(&path).expect("the copy reads");
    let links = telemetry::of(&graph).expect("its links work out");
    let mut sources: Vec<&str> =
        links.iter().map(|link| link.source.as_str()).collect();
    sources.sort_unstable();
    assert_eq!(sources, ["ping", "pong", "sensor", "tank"]);
    for link in &links {
        let data = format!("{} says hello", link.source).into_bytes();
        let apid = link.apid.expect("framed");
        let decoded = decode(&xtce, &framed(apid, &data));
        assert_eq!(decoded.container, format!("{}Packet", link.source));
        assert_eq!(decoded.integers["CcsdsApid"], u64::from(apid));
        assert_eq!(decoded.bytes[link.source.as_str()], data);
    }
    let packet = encode(
        &xtce,
        "gateway",
        &[
            ("Data", Given::Bytes(b"up")),
            ("DataBits", Given::Integer(16)),
        ],
    );
    let body =
        read_header(&packet, PacketType::Telecommand, apid("uplink_apid"))
            .expect("the vehicle takes its header");
    assert_eq!(body, b"up");
}
