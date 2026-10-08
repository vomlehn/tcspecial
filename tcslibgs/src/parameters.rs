//! What a payload set says about one payload, as a window shows it.
//!
//! Both GUIs have a button per payload that shows this. They read the same
//! two files -- the payload configuration and the simulator configuration
//! beside it -- so there is one description of what those files said, and the
//! two windows cannot come to describe the same payload differently.
//!
//! Every value here was read from a file. Nothing is a run-time measurement:
//! what a payload has actually done is on the panel itself, and mixing the
//! two would leave no way to tell a configured value from an observed one.
//! A value a file did not state is shown as what the program will use instead,
//! marked as a default rather than passed off as a statement.

use std::fmt::Write;

use crate::sim_config::ResolvedSim;
use crate::{DHConfig, DHMode, EndpointConfig, NetworkProtocol};

/// The parameters of one payload, a line each.
///
/// `sim` is what the simulator configuration settled for this payload, where
/// the program has it. Tcssim always does; tcsmoc reads the file if it is
/// there, and says plainly when it is not rather than leaving the simulation
/// half of the set unexplained.
pub fn payload_parameters(dh: &DHConfig, sim: Option<&ResolvedSim>) -> String {
    let mut out = String::new();

    // From the payload configuration: what the payload is and how it is
    // reached. This half is flight configuration -- tcspecial serves it and
    // the ground controls it -- which is why it is first.
    let _ = writeln!(out, "From the payload configuration");
    let _ = writeln!(out, "  dh_id            {}", dh.dh_id.0);
    let _ = writeln!(out, "  name             {}", dh.name.0);
    let _ = writeln!(out, "  type             {}", dh.endpoint.kind().spelling());
    endpoint_parameters(&mut out, &dh.endpoint);
    let _ = writeln!(out, "  packet_size      {} bytes", dh.packet_size);

    match &dh.oc {
        Some(oc) => {
            let _ = writeln!(out, "  oc_address       {}", oc.address);
            let _ = writeln!(out, "  oc_port          {}", oc.port);
        }
        // A handler cannot be started without one, so its absence is worth a
        // line of its own rather than two lines missing.
        None => {
            let _ = writeln!(out, "  oc_address       none stated, so it cannot be started");
        }
    }

    match &dh.mode {
        DHMode::Periodic => {
            let _ = writeln!(out, "  mode             periodic, so it sends on its own");
        }
        DHMode::Triggered {
            trigger,
            interval_ms,
        } => {
            let _ = writeln!(out, "  mode             triggered, so it answers requests");
            let _ = writeln!(out, "  trigger          {:?}", trigger);
            let _ = writeln!(out, "  trigger_interval {interval_ms} ms");
        }
    }

    // And from the simulator configuration: how a stand-in for this payload
    // behaves. None of it reaches tcspecial.
    let _ = writeln!(out);
    match sim {
        Some(sim) => simulator_parameters(&mut out, dh, sim),
        None => {
            let _ = writeln!(out, "From the simulator configuration");
            let _ = writeln!(
                out,
                "  not read by this program, so what a simulated payload does is \
                 not shown here"
            );
        }
    }

    out
}

/// The attributes that belong to this kind of endpoint and to no other.
fn endpoint_parameters(out: &mut String, endpoint: &EndpointConfig) {
    match endpoint {
        EndpointConfig::Network(net) => {
            let _ = writeln!(out, "  protocol         {}", net.protocol.spelling());
            match net.protocol {
                // A Unix socket is named by a path, and its port means
                // nothing: shown as the address it is, not as a port of zero.
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                    let _ = writeln!(out, "  address          {} (a socket path)", net.address);
                }
                NetworkProtocol::Tcp | NetworkProtocol::Udp => {
                    let _ = writeln!(out, "  address          {}", net.address);
                    let _ = writeln!(out, "  port             {}", net.port);
                }
            }
        }
        EndpointConfig::Device(device) => {
            let _ = writeln!(out, "  path             {}", device.path);
        }
        EndpointConfig::Serial(serial) => {
            let _ = writeln!(out, "  path             {}", serial.path);
            let _ = writeln!(out, "  datarate         {} bit/s", serial.datarate);
            let _ = writeln!(out, "  byte_length      {} data bits", serial.byte_length);
            let _ = writeln!(out, "  stop_bits        {}", serial.stop_bits);
        }
        EndpointConfig::I2c(i2c) => {
            let _ = writeln!(out, "  bus              {}", i2c.bus);
            let _ = writeln!(out, "  address          {:#04X}", i2c.address);
            let _ = writeln!(
                out,
                "  ten_bit          {}",
                if i2c.ten_bit { "yes" } else { "no" }
            );
            let _ = writeln!(out, "  pec              {}", if i2c.pec { "yes" } else { "no" });
        }
        EndpointConfig::Spi(spi) => {
            let _ = writeln!(out, "  path             {}", spi.path);
            let _ = writeln!(out, "  max_speed        {} Hz", spi.max_speed);
            let _ = writeln!(out, "  mode             {}", spi.mode);
            let _ = writeln!(out, "  bits_per_word    {}", spi.bits_per_word);
            let _ = writeln!(
                out,
                "  bit_order        {}",
                match spi.bit_order {
                    crate::BitOrder::MsbFirst => "most significant first",
                    crate::BitOrder::LsbFirst => "least significant first",
                }
            );
            let _ = writeln!(
                out,
                "  cs_active        {}",
                match spi.cs_active {
                    crate::CsActive::Low => "low",
                    crate::CsActive::High => "high",
                }
            );
        }
    }
}

/// What the simulator configuration settled for this payload.
fn simulator_parameters(out: &mut String, dh: &DHConfig, sim: &ResolvedSim) {
    let _ = writeln!(out, "From the simulator configuration");

    if sim.triggered {
        // A triggered payload has no rate of its own, and the file may not
        // state one: the interval that governs it is the trigger interval
        // above, which is flight behaviour.
        let _ = writeln!(
            out,
            "  packet_interval  none, this payload answering requests rather than \
             sending on its own"
        );
    } else {
        let _ = writeln!(
            out,
            "  packet_interval  {} ms{}",
            sim.packet_interval_ms,
            if sim.packet_interval_ms == 0 {
                ", as fast as the payload can be driven"
            } else {
                ""
            }
        );
    }

    let _ = writeln!(out, "  segment_interval {} ms", sim.segment_interval_ms);
    let _ = writeln!(
        out,
        "  segment_size     {} bytes{}",
        sim.segment_size,
        if sim.segment_size as usize >= dh.packet_size {
            ", so a packet goes whole"
        } else {
            ""
        }
    );

    let faults = &sim.faults;
    if !faults.any() {
        let _ = writeln!(out, "  faults           none, so the payload works");
        return;
    }

    let _ = writeln!(out, "  faults");
    for (what, value, unit) in [
        ("drop_percent", u64::from(faults.drop_percent), "% of packets"),
        ("corrupt_percent", u64::from(faults.corrupt_percent), "% of packets"),
        ("truncate_percent", u64::from(faults.truncate_percent), "% of packets"),
        ("jitter_ms", u64::from(faults.jitter_ms), " ms late at most"),
        ("silent_after", faults.silent_after, " packets"),
        ("close_after", faults.close_after, " packets"),
        (
            "ignore_trigger_percent",
            u64::from(faults.ignore_trigger_percent),
            "% of requests",
        ),
    ] {
        // Only what was asked for: a list of six noughts and one value says
        // less about what this payload does than the one value does.
        if value != 0 {
            let _ = writeln!(out, "    {what:<22} {value}{unit}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim_config::SimConfigFile;
    use crate::{
        ConfigFormat, DHId, DHName, DeviceConfig, I2cConfig, NetworkConfig, SpiConfig,
    };

    fn udp(name: &str) -> DHConfig {
        DHConfig {
            dh_id: DHId(2),
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5002,
            }),
            packet_size: 12,
            oc: Some(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "127.0.0.1".to_string(),
                port: 6002,
            }),
            mode: DHMode::Periodic,
        }
    }

    fn settled(text: &str, dh: &DHConfig) -> ResolvedSim {
        let file: SimConfigFile = ConfigFormat::Yaml.parse(text).expect("parses");
        file.resolve(std::slice::from_ref(dh))
            .expect("fits the payload")
            .remove(0)
    }

    /// Every value either file stated is shown, and said to come from the
    /// file it came from.
    ///
    /// The two halves are what the window has to keep apart: one is flight
    /// configuration, which tcspecial serves and the ground controls, and the
    /// other is how a stand-in behaves and reaches no spacecraft at all. A
    /// list that mixed them would have someone reading a drop percentage as
    /// something a payload does.
    #[test]
    fn both_files_are_shown_and_told_apart() {
        let dh = udp("DH2");
        let sim = settled(
            "simulated_payloads:\n  - name: DH2\n    type: network\n    protocol: udp\n    \
             packet_interval_ms: 500\n    segment_size: 5\n    segment_interval_ms: 40\n",
            &dh,
        );

        let shown = payload_parameters(&dh, Some(&sim));

        for said in [
            "From the payload configuration",
            "dh_id            2",
            "name             DH2",
            "type             network",
            "protocol         udp",
            "address          localhost",
            "port             5002",
            "packet_size      12 bytes",
            "oc_address       127.0.0.1",
            "oc_port          6002",
            "mode             periodic",
            "From the simulator configuration",
            "packet_interval  500 ms",
            "segment_interval 40 ms",
            "segment_size     5 bytes",
            "faults           none",
        ] {
            assert!(shown.contains(said), "{said:?} is missing from:\n{shown}");
        }

        // The simulator's half is below the payload's, and each value is
        // under the heading of the file it was read from.
        let payload_half = shown.find("From the payload configuration").unwrap();
        let sim_half = shown.find("From the simulator configuration").unwrap();
        assert!(payload_half < sim_half);
        assert!(
            shown.find("packet_size").unwrap() < sim_half,
            "the packet size is the payload file's"
        );
        assert!(
            shown.find("segment_size").unwrap() > sim_half,
            "the segment size is the simulator file's"
        );
    }

    /// A program that has not read the simulator file says so.
    ///
    /// Tcsmoc controls payloads and does not simulate them, so the file may
    /// not be there to read. Showing the payload half alone would leave the
    /// other half looking like a set that stated nothing.
    #[test]
    fn a_missing_simulator_configuration_is_said_to_be_missing() {
        let shown = payload_parameters(&udp("DH2"), None);
        assert!(shown.contains("From the payload configuration"));
        assert!(
            shown.contains("From the simulator configuration")
                && shown.contains("not read by this program"),
            "{shown}"
        );
        assert!(
            !shown.contains("packet_interval"),
            "a file that was not read states no interval:\n{shown}"
        );
    }

    /// A triggered payload's two halves say where its timing comes from.
    #[test]
    fn a_triggered_payload_shows_the_trigger_and_no_interval() {
        let mut dh = udp("DH0");
        dh.endpoint = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: "localhost".to_string(),
            port: 5000,
        });
        dh.mode = DHMode::Triggered {
            trigger: "READ\r".to_string(),
            interval_ms: 500,
        };

        let sim = settled(
            "simulated_payloads:\n  - name: DH0\n    type: network\n    protocol: tcp\n",
            &dh,
        );
        let shown = payload_parameters(&dh, Some(&sim));

        assert!(shown.contains("mode             triggered"), "{shown}");
        // As the file wrote it, escapes and all: a trigger whose carriage
        // return was shown as a line break would read as two triggers.
        assert!(shown.contains(r#"trigger          "READ\r""#), "{shown}");
        assert!(shown.contains("trigger_interval 500 ms"), "{shown}");
        assert!(
            shown.contains("packet_interval  none"),
            "a triggered payload has no rate of its own:\n{shown}"
        );
    }

    /// The faults a file asked for are listed, and the ones it did not are
    /// not.
    #[test]
    fn only_the_faults_that_were_asked_for_are_listed() {
        let dh = udp("DH2");
        let sim = settled(
            "simulated_payloads:\n  - name: DH2\n    type: network\n    protocol: udp\n    \
             packet_interval_ms: 100\n    drop_percent: 10\n    jitter_ms: 25\n",
            &dh,
        );

        let shown = payload_parameters(&dh, Some(&sim));
        assert!(shown.contains("drop_percent           10% of packets"), "{shown}");
        assert!(shown.contains("jitter_ms              25 ms late at most"), "{shown}");
        assert!(
            !shown.contains("corrupt_percent") && !shown.contains("silent_after"),
            "a fault nobody asked for is not worth a line of nought:\n{shown}"
        );
        assert!(!shown.contains("faults           none"), "{shown}");
    }

    /// Each kind of endpoint shows the attributes that belong to it.
    ///
    /// The kinds a payload file cannot describe are the ones worth checking:
    /// a line, a bus and a peripheral come from an endpoint configuration and
    /// carry terms -- a baud rate, a slave address, a clock mode -- that
    /// nothing else in the window shows.
    #[test]
    fn every_kind_shows_what_belongs_to_it() {
        let mut dh = udp("DH0");
        dh.oc = None;

        dh.endpoint = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });
        let shown = payload_parameters(&dh, None);
        assert!(shown.contains("type             device"), "{shown}");
        assert!(shown.contains("path             /dev/urandom"), "{shown}");
        assert!(
            shown.contains("oc_address       none stated"),
            "a handler with no OC address cannot be started, which is worth \
             saying:\n{shown}"
        );
        assert!(
            !shown.contains("protocol") && !shown.contains("port "),
            "a device is not reached by a protocol or a port:\n{shown}"
        );

        dh.endpoint = EndpointConfig::I2c(I2cConfig {
            bus: "/dev/i2c-1".to_string(),
            address: 0x48,
            ten_bit: false,
            pec: true,
        });
        let shown = payload_parameters(&dh, None);
        assert!(shown.contains("bus              /dev/i2c-1"), "{shown}");
        assert!(shown.contains("address          0x48"), "{shown}");
        assert!(shown.contains("pec              yes"), "{shown}");
        assert!(shown.contains("ten_bit          no"), "{shown}");

        dh.endpoint = EndpointConfig::Spi(SpiConfig {
            path: "/dev/spidev0.0".to_string(),
            max_speed: 500_000,
            mode: crate::SpiMode::Mode0,
            bits_per_word: 8,
            bit_order: crate::BitOrder::MsbFirst,
            cs_active: crate::CsActive::Low,
        });
        let shown = payload_parameters(&dh, None);
        assert!(shown.contains("max_speed        500000 Hz"), "{shown}");
        assert!(shown.contains("bits_per_word    8"), "{shown}");
        assert!(shown.contains("bit_order        most significant first"), "{shown}");
        assert!(shown.contains("cs_active        low"), "{shown}");

        // A socket named by a path shows the path and no port, the port a
        // payload file gives it meaning nothing.
        dh.endpoint = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::UnixStream,
            address: "/tmp/dh.sock".to_string(),
            port: 0,
        });
        let shown = payload_parameters(&dh, None);
        assert!(shown.contains("address          /tmp/dh.sock (a socket path)"), "{shown}");
        assert!(!shown.contains("port             0"), "{shown}");
    }

    /// Nothing here is a measurement.
    ///
    /// What a payload has actually done is on the panel; this is what the
    /// files said. A list that mixed the two would leave no way to tell a
    /// configured value from an observed one.
    #[test]
    fn nothing_shown_is_a_measurement() {
        let dh = udp("DH2");
        let sim = settled(
            "simulated_payloads:\n  - name: DH2\n    type: network\n    protocol: udp\n    \
             packet_interval_ms: 100\n",
            &dh,
        );
        let shown = payload_parameters(&dh, Some(&sim));
        for counted in ["packets_sent", "bytes_sent", "Last sent", "Sent:"] {
            assert!(
                !shown.contains(counted),
                "{counted:?} is a measurement, not a parameter:\n{shown}"
            );
        }
    }
}
