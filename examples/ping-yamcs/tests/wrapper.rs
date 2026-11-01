//! The Yamcs wrapper reads what the vehicle frames, and frames what the
//! vehicle reads.
//!
//! `yamcs/tcs-ccsds.xtce.in` tells Yamcs how a packet on the link is laid
//! out: a CCSDS primary header, and after it, by APID, what follows. The
//! vehicle frames and unframes with `tcslibgs::ccsds`. Nothing but a run
//! against Yamcs itself would show the two disagreeing -- a header width, a
//! length off by one, a restriction on the wrong APID -- so this reads the
//! wrapper, as `ping_yamcs.py` fills it in from the configuration, and works
//! it as Yamcs would on the vehicle's own packets:
//!
//! * each packet the vehicle sends down -- a beacon, a ping's answer,
//!   CONNECT's answer and another command's -- framed by `tcslibgs` and
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

use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use tcslibgs::ccsds::{read_header, PacketType, Sender, HEADER_BYTES};
use tcslibgs::node::vehicle::ping;
use tcslibgs::telemetry::{ConnectTelemetry, PingTelemetry, Telemetry};
use tcslibgs::types::CommandStatus;
use tcslibgs::{Beacon, Command, ConfigDigest, ConfigVersion};

mod common;

use common::{here, worked_out_of};

/// One element of the wrapper, with what is in it.
#[derive(Debug, Default)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Element>,
    text: String,
}

impl Element {
    /// One of its attributes.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute's name.
    ///
    /// # Returns
    ///
    /// Its value, or `None` where it has none of that name.
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(said, _)| said == name)
            .map(|(_, value)| value.as_str())
    }

    /// Its first child of a kind.
    ///
    /// # Arguments
    ///
    /// * `name` - The child's element name.
    ///
    /// # Returns
    ///
    /// The child, or `None`.
    fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Everything of a kind within it, at any depth, in document order.
    ///
    /// # Arguments
    ///
    /// * `name` - The element name.
    /// * `found` - Where they are put.
    fn within<'a>(&'a self, name: &str, found: &mut Vec<&'a Element>) {
        for child in &self.children {
            if child.name == name {
                found.push(child);
            }
            child.within(name, found);
        }
    }

    /// The one element of a kind within it with a `name` attribute.
    ///
    /// # Arguments
    ///
    /// * `kind` - The element name.
    /// * `name` - Its `name` attribute.
    ///
    /// # Returns
    ///
    /// The element.
    ///
    /// # Panics
    ///
    /// There is none.
    fn named(&self, kind: &str, name: &str) -> &Element {
        let mut found = Vec::new();
        self.within(kind, &mut found);
        found
            .into_iter()
            .find(|element| element.attribute("name") == Some(name))
            .unwrap_or_else(|| panic!("the wrapper has no {kind} {name}"))
    }
}

/// An element's name and attributes, as an [`Element`] with nothing in it.
fn opened(start: &BytesStart<'_>) -> Element {
    let text = |said: &str| unescape(said).expect("well escaped").into_owned();
    Element {
        name: text(start.name().as_ref()),
        attributes: start
            .attributes()
            .map(|attribute| {
                let attribute = attribute.expect("an attribute");
                (text(attribute.key.as_ref()), text(&attribute.value))
            })
            .collect(),
        ..Element::default()
    }
}

/// The wrapper, filled in as `ping_yamcs.py` fills it in, from the APIDs the
/// Rust works out of the example's configuration -- which `agrees.rs` holds
/// the Python's to.
///
/// # Returns
///
/// Its root element.
///
/// # Panics
///
/// It does not read, or a name in it is left unfilled.
fn wrapper() -> Element {
    let mut text =
        std::fs::read_to_string(here().join("yamcs/tcs-ccsds.xtce.in"))
            .expect("the wrapper");
    for name in ["beacon_apid", "cmd_apid", "uplink_apid", "reply_apid"] {
        text = text.replace(&format!("${name}"), &worked_out_of(name));
    }
    assert!(!text.contains('$'), "a name left unfilled in the wrapper");

    let mut reader = Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut open = vec![Element::default()];
    loop {
        match reader.read_event().expect("the wrapper is XML") {
            Event::Start(start) => open.push(opened(&start)),
            Event::Empty(start) => {
                let element = opened(&start);
                open.last_mut().expect("a parent").children.push(element);
            }
            Event::End(_) => {
                let element = open.pop().expect("an element");
                open.last_mut().expect("a parent").children.push(element);
            }
            Event::Text(said) => {
                let said: &str = said.as_ref();
                open.last_mut().expect("an element").text.push_str(said);
            }
            // An entity in text, `&lt;` in a comparison, comes apart from
            // the text around it.
            Event::GeneralRef(entity) => {
                let entity: &str = entity.as_ref();
                let said = unescape(&format!("&{entity};"))
                    .expect("an entity XML defines")
                    .into_owned();
                open.last_mut().expect("an element").text.push_str(&said);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut document = open.pop().expect("the document");
    assert!(open.is_empty(), "every element closed");
    document.children.pop().expect("a root element")
}

/// The APID the Rust works out for a name `ping_yamcs.py --describe` uses.
fn apid(name: &str) -> u16 {
    worked_out_of(name).parse().expect("an APID")
}

/// `bits` bits of `packet` from bit `at`, most significant first.
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
/// id, is a PingReplyPacket whose text is the answer whole.
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
    assert_eq!(decoded.container, "PingReplyPacket");
    assert_eq!(decoded.bytes["PingReply"], answer.as_bytes());
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

/// A ping, as Yamcs sends it by the wrapper, is unframed by the vehicle
/// under the gateway node's `apid_base`, its text whole.
#[test]
fn a_ping_encodes_as_its_text() {
    let xtce = wrapper();
    let message = ping::message(4, Duration::new(1_793_562_467, 160_521_331))
        .expect("a ping");
    let packet = encode(
        &xtce,
        "PingText",
        &[
            ("Text", Given::Bytes(message.as_bytes())),
            ("TextBits", Given::Integer(8 * message.len() as u64)),
        ],
    );
    let body =
        read_header(&packet, PacketType::Telecommand, apid("uplink_apid"))
            .expect("the vehicle takes its header");
    assert_eq!(body, message.as_bytes());
    ping::read_message(body).expect("the vehicle reads the ping");
}
