//! The endpoint configuration examples in `docs/design.rst` are these files.
//!
//! Keeping the documented examples in `tests/actual`, where a test parses
//! them, is what stops the manual and the parser from drifting apart: an
//! example that stops being valid fails the build rather than quietly
//! misleading a reader.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tcslibgs::endpoint_config::{
    self, BitOrder, CsActive, EndpointLocation, GroupKind, SpiMode, StopBits,
};

fn actual(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/actual")
        .join(name)
}

#[test]
fn the_documented_examples_parse() {
    endpoint_config::load(actual("endpoints.yaml")).expect("endpoints.yaml");
    endpoint_config::load(actual("endpoints.xml")).expect("endpoints.xml");
    endpoint_config::load(actual("endpoints.json")).expect("endpoints.json");
}

#[test]
fn the_documented_examples_describe_the_same_configuration() {
    // The claim design.rst makes about the formats, checked.
    let yaml = endpoint_config::load(actual("endpoints.yaml")).unwrap();
    let xml = endpoint_config::load(actual("endpoints.xml")).unwrap();
    let json = endpoint_config::load(actual("endpoints.json")).unwrap();
    assert_eq!(yaml, xml, "the YAML and XML examples have drifted apart");
    assert_eq!(yaml, json, "the YAML and JSON examples have drifted apart");
}

#[test]
fn the_examples_contain_what_the_manual_says_they_do() {
    // Guards against both examples being identically wrong -- every field
    // silently defaulting, say.
    let doc = endpoint_config::load(actual("endpoints.yaml")).unwrap();

    assert_eq!(doc.general.version.as_deref(), Some("1.0"));
    assert_eq!(doc.groups.len(), 7);
    assert_eq!(doc.endpoints.len(), 9);

    // Every group type the format defines appears in the examples, so that
    // none of them can rot unnoticed.
    let types: Vec<&str> = doc.groups.iter().map(|g| g.kind.type_name()).collect();
    for wanted in ["serial", "network", "i2c", "spi"] {
        assert!(types.contains(&wanted), "no {wanted} group in the examples");
    }

    // packet_size is shared by every type, so check one group of each and
    // the one group that states none -- the examples' proof it is optional.
    for (group, wanted) in [
        ("rs422_payload", Some(512)),
        ("rs422_blockmode", Some(64)),
        ("payload_tcp", Some(1024)),
        ("payload_udp", Some(256)),
        ("payload_i2c", Some(32)),
        ("payload_spi", Some(64)),
        ("payload_unix", None),
    ] {
        assert_eq!(
            doc.group(group).unwrap().packet_size,
            wanted,
            "{group} has the wrong packet size"
        );
    }

    // A group holds the shared attributes...
    match &doc.group("rs422_payload").unwrap().kind {
        GroupKind::Serial(s) => {
            assert_eq!(s.datarate, 115_200);
            assert_eq!(s.stop_bits, StopBits::One);
            assert_eq!(s.byte_length.bits(), 8);
            assert_eq!(s.stream.max_length, 512);
            assert_eq!(s.stream.timeout, Some(Duration::from_millis(250)));
            assert_eq!(s.stream.terminators, vec![0x0D, 0x0A]);
        }
        other => panic!("expected a serial group, got {other:?}"),
    }

    // ...and two endpoints share it, differing only in device name.
    let shared: Vec<_> = doc
        .endpoints
        .iter()
        .filter(|e| e.group == "rs422_payload")
        .collect();
    assert_eq!(shared.len(), 2);
    assert_eq!(
        shared[0].location,
        EndpointLocation::Device { path: "/dev/ttyS0".into() }
    );
    assert_eq!(
        shared[1].location,
        EndpointLocation::Device { path: "/dev/ttyS1".into() }
    );

    // The fixed-length case the manual calls out.
    let block = doc.group("rs422_blockmode").unwrap().kind.stream().unwrap();
    assert!(block.is_fixed_length());
    assert_eq!(block.max_length, 64);

    // A datagram group has no stream section at all.
    assert!(doc.group("payload_udp").unwrap().kind.stream().is_none());

    // A network endpoint carries an address and port instead of a device.
    let camera = doc.endpoints.iter().find(|e| e.name == "camera").unwrap();
    assert_eq!(
        camera.location,
        EndpointLocation::Network { address: "10.0.0.20".into(), port: 5000 }
    );

    // A Unix-domain socket is a network group whose endpoint gives a path.
    let recorder = doc.endpoints.iter().find(|e| e.name == "recorder").unwrap();
    assert_eq!(
        recorder.location,
        EndpointLocation::Device { path: "/run/tcspecial/recorder.sock".into() }
    );

    // An I2C group holds how the master drives the bus...
    match &doc.group("payload_i2c").unwrap().kind {
        GroupKind::I2c(p) => {
            assert!(p.pec);
            assert!(!p.ten_bit, "the example addresses with seven bits");
            assert_eq!(p.retries, 2);
            assert_eq!(p.timeout, Some(Duration::from_millis(50)));
            assert_eq!(p.bus_speed, Some(400_000));
        }
        other => panic!("expected an i2c group, got {other:?}"),
    }

    // ...and the two sensors on that bus differ only in their address, which
    // is the whole reason an endpoint carries one.
    let sensors: Vec<_> = doc
        .endpoints
        .iter()
        .filter(|e| e.group == "payload_i2c")
        .collect();
    assert_eq!(sensors.len(), 2);
    assert_eq!(
        sensors[0].location,
        EndpointLocation::I2c { bus: "/dev/i2c-2".into(), address: 0x48 }
    );
    assert_eq!(
        sensors[1].location,
        EndpointLocation::I2c { bus: "/dev/i2c-2".into(), address: 0x49 }
    );

    // A SPI group, including the two attributes the file left to default.
    match &doc.group("payload_spi").unwrap().kind {
        GroupKind::Spi(p) => {
            assert_eq!(p.max_speed, 10_000_000);
            assert_eq!(p.mode, SpiMode::Mode3);
            assert_eq!(p.bits_per_word.bits(), 8);
            assert_eq!(p.bit_order, BitOrder::MsbFirst);
            assert_eq!(p.cs_active, CsActive::Low);
        }
        other => panic!("expected a spi group, got {other:?}"),
    }

    // Neither bus type has a stream section: a transfer is already bounded.
    assert!(doc.group("payload_i2c").unwrap().kind.stream().is_none());
    assert!(doc.group("payload_spi").unwrap().kind.stream().is_none());

    // A SPI device node names bus and chip select together, so no address.
    let imu = doc.endpoints.iter().find(|e| e.name == "imu").unwrap();
    assert_eq!(
        imu.location,
        EndpointLocation::Device { path: "/dev/spidev0.1".into() }
    );
}

/// The documented examples describe how to reach devices, not data handlers.
///
/// They assign no `dh_id`, because an endpoint configuration need not, so
/// converting one is refused rather than guessing. This pins which of the two
/// things the examples are, and keeps the conversion honest about what it
/// requires: if a reader ever adds ids to the examples, this test is what says
/// the examples changed purpose.
#[test]
fn the_documented_examples_assign_no_data_handler_ids() {
    for name in ["endpoints.yaml", "endpoints.xml", "endpoints.json"] {
        let doc = endpoint_config::load(actual(name)).expect(name);

        assert!(
            doc.endpoints.iter().all(|e| e.dh_id.is_none()),
            "{name}: an example endpoint assigns a dh_id"
        );

        let message = doc
            .to_dh_configs()
            .expect_err(&format!("{name} should not convert"))
            .to_string();
        assert!(
            message.contains("dh_id"),
            "{name}: the error should say an id is missing, but said: {message}"
        );
    }
}
