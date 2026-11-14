//! The Yamcs wrapper reads what the vehicle frames, and frames what the
//! vehicle reads.
//!
//! The mission database's wrapper, which `tcsprotocol/yamcs.py` generates
//! from the configuration and `python/tcsprotocol/yamcs.xtce`, tells Yamcs
//! how a packet on the link is laid out: a CCSDS primary header, and after it,
//! by APID, what follows. The vehicle frames and unframes with
//! `tcsprotocol::ccsds`. Nothing but a run against Yamcs itself would show
//! the two disagreeing -- a header width, a length off by one, a restriction on
//! the wrong APID -- so this generates the wrapper, as `ping_yamcs.py` does,
//! and works it as Yamcs would on the vehicle's own packets:
//!
//! * each packet the vehicle sends down -- a beacon, a ping's answer,
//!   CONNECT's answer and another command's, and in a copy of the example
//!   with more nodes, a payload's data through the gateway node and a
//!   payload's telemetry -- framed by `tcsprotocol` and
//!   decoded by the wrapper, must land in the container meant for it, with
//!   the header's fields and the body where the wrapper says: a ping's
//!   answer field by field, as the ping node's interface document
//!   (`tcsprotocol/xtce/ping.xtce`) lays it out, every byte of it read;
//! * each command Yamcs sends up -- CONNECT and a ping, the ping as the
//!   document's fields -- encoded by the wrapper, with its length filled in as
//!   Yamcs fills it in, must be unframed by `tcsprotocol` and read as the
//!   command it is: the ping as the text `tcsprotocol` writes.
//!
//! The interpreter, `yamcs_common`'s, which thermostat-yamcs's tests share,
//! does what the wrapper uses of XTCE and no more: a wrapper that grows past
//! it fails there, saying what it does not know, rather than passing unread.

use std::time::Duration;

use tcsprotocol::ccsds::{read_header, PacketType, HEADER_BYTES};
use tcsprotocol::graph::telemetry;
use tcsprotocol::graph::Graph;
use tcsprotocol::node::vehicle::ping;
use tcsprotocol::telemetry::{ConnectTelemetry, PingTelemetry, Telemetry};
use tcsprotocol::types::CommandStatus;
use tcsprotocol::{Beacon, Command, ConfigDigest, ConfigVersion};

mod common;

use common::{here, with_more_nodes, worked_out_of};
use yamcs_common::{
    counted, decode, encode, framed, generated_mdb, Element, Given,
};

/// The wrapper `tcsprotocol/yamcs.py` generates for the example's
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

/// The text of each of a ping's answer's fields, as the ping node's
/// interface document names them, for an answer `tcsprotocol` writes.
///
/// # Arguments
///
/// * `count`    - The ping's count.
/// * `sent`     - When it was sent.
/// * `answered` - When it was answered.
///
/// # Returns
///
/// The answer, and each field's name and text.
fn answer_and_fields(
    count: u64,
    sent: Duration,
    answered: Duration,
) -> (Vec<u8>, Vec<(&'static str, String)>) {
    let message = ping::message(count, sent).expect("a ping");
    let answer = ping::reply(message.as_bytes(), answered)
        .expect("an answer")
        .as_bytes()
        .to_vec();
    let nanoseconds = |at: Duration| format!("{:09}", at.subsec_nanos());
    let fields = vec![
        ("ReplyPrefix", "ping #".to_string()),
        ("ReplyCount", count.to_string()),
        ("ReplySentSeconds", sent.as_secs().to_string()),
        ("ReplySentNanoseconds", nanoseconds(sent)),
        ("ReplySpace", " ".to_string()),
        ("ReplyAnsweredSeconds", answered.as_secs().to_string()),
        ("ReplyAnsweredNanoseconds", nanoseconds(answered)),
    ];
    (answer, fields)
}

/// A ping's answer, framed under `apid_base` plus the ping node's device
/// id, is the ping node's container, its parameters the answer's text field
/// by field, as the ping node's interface document lays it out, and every
/// byte of it read.
#[test]
fn a_pings_answer_decodes_as_its_text() {
    let xtce = wrapper();
    let largest = Duration::new(u64::MAX, 999_999_999);
    let sent = Duration::new(1_793_562_467, 79_578_927);
    for (count, sent, answered) in [
        (1, sent, sent + Duration::from_millis(5)),
        (1, Duration::ZERO, Duration::ZERO),
        (u64::MAX, largest, largest),
    ] {
        let (answer, fields) = answer_and_fields(count, sent, answered);
        let packet = framed(apid("reply_apid"), &counted(&answer));
        let decoded = decode(&xtce, &packet);
        assert_eq!(decoded.container, "pingPacket");
        for (name, text) in &fields {
            assert_eq!(
                String::from_utf8_lossy(&decoded.bytes[*name]),
                *text,
                "{name}"
            );
        }
        assert_eq!(decoded.end, Some(packet.len() * 8), "all of it is read");
    }
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

/// A ping, as Yamcs sends it by the wrapper as the ping node's document's
/// command, its fields as text, is unframed by the vehicle under the gateway
/// node's `apid_base` and read as the ping `tcsprotocol` writes; and the
/// vehicle's answer to it decodes back into the same fields.
#[test]
fn a_ping_encodes_as_its_text() {
    let xtce = wrapper();
    let largest = Duration::new(u64::MAX, 999_999_999);
    for (count, sent) in [
        (4, Duration::new(1_793_562_467, 160_521_331)),
        (1, Duration::ZERO),
        (u64::MAX, largest),
    ] {
        let message = ping::message(count, sent).expect("a ping");
        let seconds = sent.as_secs().to_string();
        let nanoseconds = format!("{:09}", sent.subsec_nanos());
        let count_text = count.to_string();
        let packet = encode(
            &xtce,
            "Ping",
            &[
                ("Count", Given::Text(&count_text)),
                ("SentSeconds", Given::Text(&seconds)),
                ("SentNanoseconds", Given::Text(&nanoseconds)),
            ],
        );
        let body =
            read_header(&packet, PacketType::Telecommand, apid("uplink_apid"))
                .expect("the vehicle takes its header");
        assert_eq!(body, message.as_bytes());
        let read = ping::read_message(body).expect("the vehicle reads it");
        assert_eq!((read.count, read.sent), (count, sent));

        // And round again: the vehicle's answer, as Yamcs reads it.
        let answered = sent.saturating_add(Duration::from_millis(1));
        let answer = ping::reply(body, answered).expect("an answer");
        let decoded = decode(
            &xtce,
            &framed(apid("reply_apid"), &counted(answer.as_bytes())),
        );
        for (name, said) in [
            ("ReplyCount", &count_text),
            ("ReplySentSeconds", &seconds),
            ("ReplySentNanoseconds", &nanoseconds),
        ] {
            assert_eq!(String::from_utf8_lossy(&decoded.bytes[name]), *said);
        }
    }
}

/// In a copy of the example with more nodes, the data of each, framed under the
/// APID `tcsprotocol` counts for it -- "pong"'s through the gateway node,
/// "sensor"'s and "tank"'s over `payload_telemetry` -- is that node's
/// container, its parameter the data whole; and the ping node's answers and the
/// ping are as they were.
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
        let apid = link.apid.expect("framed");
        if link.source == "ping" {
            let at = Duration::new(1_793_562_467, 1);
            let (answer, fields) = answer_and_fields(2, at, at);
            let decoded = decode(&xtce, &framed(apid, &counted(&answer)));
            assert_eq!(decoded.container, "pingPacket");
            assert_eq!(decoded.bytes["ReplyCount"], fields[1].1.as_bytes());
            continue;
        }
        let data = format!("{} says hello", link.source).into_bytes();
        let decoded = decode(&xtce, &framed(apid, &counted(&data)));
        assert_eq!(decoded.container, format!("{}Packet", link.source));
        assert_eq!(decoded.integers["CcsdsApid"], u64::from(apid));
        assert_eq!(decoded.bytes[link.source.as_str()], data);
    }
    let packet = encode(
        &xtce,
        "Ping",
        &[
            ("Count", Given::Text("3")),
            ("SentSeconds", Given::Text("1793562467")),
            ("SentNanoseconds", Given::Text("000000001")),
        ],
    );
    let body =
        read_header(&packet, PacketType::Telecommand, apid("uplink_apid"))
            .expect("the vehicle takes its header");
    assert_eq!(body, b"ping #3 1793562467.000000001");
}
