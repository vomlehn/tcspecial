//! The Yamcs wrapper reads what the vehicle frames as the configuration
//! says it is.
//!
//! The mission database's wrapper, which `tcslibgs/yamcs.py` generates from the
//! configuration, gives each node drawn into the gateway node a parameter of
//! the type the configuration says its data are: a value as a float in its
//! unit, read from four bytes of a signed count of the unit's steps through a
//! calibrator, a decision as a boolean, read from the character `1` or `0` -- a
//! thermostat's, and what a heater's switch was told -- and a GPIO payload's
//! lines read for their levels as each line's record, its level a boolean.
//! Nothing but a run against Yamcs itself would show the wrapper and the
//! vehicle disagreeing -- a byte order, a scale off by a thousand, a sign lost
//! -- so this generates the wrapper, as `thermostat_yamcs.py` does, and works
//! it as Yamcs would, with `yamcs_common`'s interpreter, on what the vehicle
//! sends: each value made by
//! `tcslib_protocol::node::vehicle::compute::value_bytes`, each line's record
//! as tcspecial writes it, framed by `tcslib_protocol::ccsds` under the APID
//! `tcslib-protocol` counts for its source. And so for every unit there is, in
//! copies of the example whose readings are in that unit.

use std::path::{Path, PathBuf};

use tcslib_protocol::ccsds::HEADER_BYTES;
use tcslib_protocol::graph::telemetry;
use tcslib_protocol::graph::Graph;
use tcslib_protocol::node::vehicle::compute::value_bytes;
use tcslib_protocol::unit::Unit;
use tcslib_protocol::{Beacon, ConfigDigest, ConfigVersion};

use yamcs_common::{decode, framed, generated_mdb, Element};

/// Where the example is.
///
/// # Returns
///
/// Its directory.
fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The APID each source's data carries, as `tcslib-protocol` counts it.
///
/// # Arguments
///
/// * `path` - The configuration.
///
/// # Returns
///
/// Each source's name and APID.
fn apids(path: &Path) -> Vec<(String, u16)> {
    let graph = Graph::load(path).expect("the configuration reads");
    telemetry::of(&graph)
        .expect("its links work out")
        .into_iter()
        .map(|link| (link.source, link.apid.expect("framed")))
        .collect()
}

/// The APID of one source.
///
/// # Arguments
///
/// * `apids`  - What [`apids`] gave.
/// * `source` - The source's name.
///
/// # Returns
///
/// Its APID.
fn apid_of(apids: &[(String, u16)], source: &str) -> u16 {
    apids
        .iter()
        .find(|(name, _)| name == source)
        .unwrap_or_else(|| panic!("{source} reaches the ground"))
        .1
}

/// Decodes a value the vehicle sends, as Yamcs would by the wrapper.
///
/// # Arguments
///
/// * `xtce`   - The wrapper.
/// * `apid`   - The source's APID.
/// * `source` - The source's name, which its parameter is called.
/// * `number` - The value, in `unit`.
/// * `unit`   - Its unit.
///
/// # Returns
///
/// The value Yamcs would show, and its units.
fn value_read(
    xtce: &Element,
    apid: u16,
    source: &str,
    number: f64,
    unit: Unit,
) -> (f64, Vec<String>) {
    let data = value_bytes(number, unit).expect("a value");
    let packet = framed(apid, &data);
    let decoded = decode(xtce, &packet);
    assert_eq!(decoded.container, format!("{source}Packet"));
    assert_eq!(decoded.end, Some(packet.len() * 8), "all of it is read");
    decoded.floats[source].clone()
}

/// The readings and the vote, framed under their APIDs, are temperatures in
/// kelvin, as the vehicle meant them, to the micro-kelvin a value carries:
/// below freezing, at the setpoint, and at either end of what a value can
/// say.
#[test]
fn readings_and_the_vote_decode_as_temperatures() {
    let path = here().join("thermostat-yamcs.xtcs");
    let xtce = generated_mdb(&path);
    let apids = apids(&path);
    let (least, most) = Unit::Kelvin.range();
    for source in ["to_k1", "to_k2", "to_k3", "consensus_temp"] {
        for number in [0.0, 255.37, 293.15, 291.234567, -40.5, least, most] {
            let (read, units) = value_read(
                &xtce,
                apid_of(&apids, source),
                source,
                number,
                Unit::Kelvin,
            );
            assert!((read - number).abs() < 1e-6, "{source}: {read}");
            assert_eq!(units, ["K"], "{source}");
        }
    }
}

/// The heaters' switches, each reporting what it was last told, framed
/// under its own APID, are each the number its line was told: a u8, read
/// as an integer.
#[test]
fn the_heaters_switches_decode_as_what_they_were_told() {
    let path = here().join("thermostat-yamcs.xtcs");
    let xtce = generated_mdb(&path);
    let apids = apids(&path);
    for heater in ["gpio-out-tmass1", "gpio-out-tmass2"] {
        for told in [1u8, 0] {
            let packet = framed(apid_of(&apids, heater), &[told]);
            let decoded = decode(&xtce, &packet);
            assert_eq!(decoded.container, format!("{heater}Packet"));
            assert_eq!(decoded.end, Some(packet.len() * 8), "all of it");
            assert_eq!(decoded.integers[heater], u64::from(told), "{heater}");
        }
    }
}

/// The beacon is read as it is in every configuration that answers to
/// Yamcs: its bytes whole, after the header.
#[test]
fn a_beacon_decodes_as_the_vehicle_framed_it() {
    let path = here().join("thermostat-yamcs.ytcs");
    let graph = Graph::load(&path).expect("the example reads");
    let section = graph.tcspecial.as_ref().expect("a tcspecial section");
    let apid = tcslib_protocol::graph::parameters::ci_config_of(
        &tcslib_protocol::graph::parameters::flatten(section),
    )
    .expect("its section reads")
    .to_ci_config()
    .expect("it settles")
    .sockets()
    .expect("sockets")
    .framing
    .expect("framed")
    .beacon_apid;
    let body =
        Beacon::new(3, ConfigVersion::of_this_build(), ConfigDigest([7; 16]))
            .to_bytes();
    let decoded = decode(&generated_mdb(&path), &framed(apid, &body));
    assert_eq!(decoded.container, "BeaconPacket");
    assert_eq!(decoded.bytes["BeaconBody"], body);
    assert_eq!(decoded.refers[0].1, HEADER_BYTES * 8);
}

/// A copy of the example whose readings are `to_value` conversions to a
/// unit's value type, and whose vote carries that type too.
///
/// The readings are in the file the example includes, so that is copied
/// with them changed, into a `thermostat-common` beside the copy of the
/// example, as in `examples`; both in a directory of the unit's own. Its own
/// XTCE is written beside it, as `make xtce` writes the example's.
///
/// # Arguments
///
/// * `dir`  - Where to write it.
/// * `unit` - The unit.
///
/// # Returns
///
/// The copy's path.
fn in_unit(dir: &Path, unit: Unit) -> PathBuf {
    let common = here().join("../thermostat-common/thermostat.xtcs");
    let said = std::fs::read_to_string(&common).expect("the common file reads");
    let changed = said
        .replace(
            "<function>to_kelvin</function>",
            &format!(
                "<function>to_value</function><type-out>{}</type-out>",
                unit.type_name()
            ),
        )
        .replace(
            "<carries>kelvin</carries>",
            &format!("<carries>{}</carries>", unit.type_name()),
        );
    assert_ne!(said, changed, "the readings are to_kelvin conversions");
    let at = dir.join(format!("{unit:?}"));
    for sub in ["thermostat-yamcs", "thermostat-common"] {
        std::fs::create_dir_all(at.join(sub)).expect("a directory");
    }
    std::fs::write(at.join("thermostat-common/thermostat.xtcs"), changed)
        .expect("written");
    let path = at.join("thermostat-yamcs/in.xtcs");
    std::fs::copy(here().join("thermostat-yamcs.xtcs"), &path).expect("copied");
    let graph =
        tcslib_protocol::graph::Graph::load(&path).expect("the copy reads");
    let own = tcslib_protocol::graph::own_xtce::written(&graph, "in")
        .expect("its own XTCE");
    std::fs::write(path.with_extension("xtce"), own).expect("written");
    path
}

/// In every unit there is, a reading and the vote decode as values in that
/// unit, at its own scale -- a millionth of a volt, a thousandth of a metre
/// -- and with its symbol, as `types.xtce` gives it.
#[test]
fn values_in_every_unit_decode_in_it() {
    let dir = tempfile::tempdir().expect("a directory");
    for unit in Unit::ALL {
        let path = in_unit(dir.path(), unit);
        let xtce = generated_mdb(&path);
        let apids = apids(&path);
        let (_, most) = unit.range();
        for source in ["to_k1", "consensus_temp"] {
            for number in [1.5, -2.25, most] {
                let (read, units) = value_read(
                    &xtce,
                    apid_of(&apids, source),
                    source,
                    number,
                    unit,
                );
                assert!(
                    (read - number).abs() <= 0.5 / unit.per_unit(),
                    "{unit:?} {source}: {read} for {number}"
                );
                assert_eq!(units, [unit.symbol()], "{unit:?}");
            }
        }
    }
}
