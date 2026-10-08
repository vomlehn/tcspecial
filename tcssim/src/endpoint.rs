//! What a data handler's endpoint means to the payload at its far end.
//!
//! The payload file describes the handler's side of the link: what kind of
//! endpoint it is and where. The simulator takes the other end of that, so
//! this is where an endpoint is read -- turned into the configuration a
//! simulated payload runs on, and into the line a panel shows for it.
//!
//! It is also where the kinds of endpoint tcssim cannot stand in for are
//! refused. A Unix socket handler is a configuration the simulator has
//! nothing to be, and saying so is better than simulating something else at
//! that address.
//!
//! What a payload then does with the endpoint is in the file for its kind:
//! [`crate::payload_tcp`], [`crate::payload_udp`], [`crate::payload_device`].

use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use tcslibgs::{DHConfig, EndpointConfig, NetworkProtocol};

use crate::payload::{PayloadConfig, PayloadProtocol};
use tcslibgs::ResolvedSim;

/// Turn a data handler and its simulator settings into the simulator's own
/// configuration.
///
/// The payload file describes what tcspecial expects to talk to, so the
/// simulator takes the other end of it: the handler's endpoint becomes the
/// address the simulated payload uses. Everything about how the payload
/// behaves comes from `sim`, which is what the simulator file settled.
pub fn payload_config_from(dh: &DHConfig, sim: &ResolvedSim) -> Result<PayloadConfig, String> {
    let (protocol, address, port, bus_address) = match &dh.endpoint {
        EndpointConfig::Network(net) => {
            // A Unix socket divides as TCP and UDP divide, and which end binds
            // its path divides with it: the simulator listens at the
            // configured path for a stream, and sends to it from a path of its
            // own for a datagram. See payload_unix.
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => PayloadProtocol::Tcp,
                NetworkProtocol::Udp => PayloadProtocol::Udp,
                NetworkProtocol::UnixStream => PayloadProtocol::UnixStream,
                NetworkProtocol::UnixDgram => PayloadProtocol::UnixDgram,
            };
            (protocol, net.address.clone(), net.port, 0)
        }
        EndpointConfig::Device(dev) => (PayloadProtocol::Device, dev.path.clone(), 0, 0),
        // A line the simulator can be: a pty's slave is a device file with a
        // line discipline behind it, so a handler opens it, sets its terms and
        // reads it exactly as it would a port. The path the handler was told
        // to open is made to lead there; see payload_serial.
        //
        // The line's terms do not come this way. A pty takes a data rate and
        // ignores it, so what the handler sets them to is between the handler
        // and the kernel, and the simulator has only the path to be at.
        EndpointConfig::Serial(serial) => (PayloadProtocol::Serial, serial.path.clone(), 0, 0),
        // A bus the simulator can be on, though not by being a device: the
        // kernel's i2c-stub is an adapter whose chips are a bank of registers
        // in memory, and every master on that bus reads and writes the same
        // bank. So the bank is the payload, and the simulator fills it; see
        // payload_i2c. The bus the handler was told to open is made to lead to
        // the stub, as a line's path is made to lead to a pty.
        EndpointConfig::I2c(i2c) => (
            PayloadProtocol::I2c,
            i2c.bus.clone(),
            0,
            i2c.address,
        ),
        // A peripheral the simulator can carry the bytes of, though not be:
        // nothing emulates one, a peripheral being a thing a controller
        // clocks, so what stands in is a pty as for a serial line. The bytes
        // and their pacing are real; the clock, the mode and the word are not
        // simulated at all. See payload_spi, which says what that does and
        // does not test.
        //
        // Nothing of the terms comes this way, for the same reason a line's
        // do not: what the handler sets is between the handler and the
        // kernel, and a pty has nowhere to put it.
        EndpointConfig::Spi(spi) => (PayloadProtocol::Spi, spi.path.clone(), 0, 0),
    };

    let packet_size = u32::try_from(dh.packet_size)
        .map_err(|_| format!("{} has a packet size too large to simulate", dh.name.0))?;

    // A payload that answers requests has to be able to hear one. Two kinds
    // cannot: tcspecial opens a device and reads it, so there is nothing for
    // the simulator to be written to, and an I2C master's read is the trigger
    // itself, which a second master on the bus cannot see. Refused here
    // rather than simulated as a payload that answers nothing, which would
    // look exactly like a handler whose polling had stopped working.
    if dh.mode.polling().is_some() {
        let cannot = match protocol {
            PayloadProtocol::Device => Some("a device, which tcspecial reads directly"),
            PayloadProtocol::I2c => {
                Some("a bus, where the master's own read is the trigger")
            }
            _ => None,
        };
        if let Some(cannot) = cannot {
            return Err(format!(
                "{} is triggered, but the simulator cannot stand in for a triggered \
                 payload on {cannot}",
                dh.name.0
            ));
        }
    }

    Ok(PayloadConfig {
        bus_address,
        triggered: dh.mode.polling().is_some(),
        faults: sim.faults,
        // The socket this payload answers from, where the simulator file
        // states one. Refused there for the kinds that have none to bind, so
        // what arrives here is for a kind that has.
        own_address: sim.payload_address.clone(),
        own_port: sim.payload_port,
        _id: dh.dh_id.0,
        protocol,
        address,
        port,
        packet_size: Arc::new(AtomicU32::new(packet_size)),
        segment_size: Arc::new(AtomicU32::new(sim.segment_size)),
        packet_interval_ms: Arc::new(AtomicU32::new(sim.packet_interval_ms)),
        segment_interval_ms: Arc::new(AtomicU32::new(sim.segment_interval_ms)),
    })
}

/// How a data handler's endpoint reads in its panel.
pub fn endpoint_description(endpoint: &EndpointConfig) -> String {
    match endpoint {
        EndpointConfig::Network(net) => {
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => "TCP",
                NetworkProtocol::Udp => "UDP",
                NetworkProtocol::UnixStream => "Unix stream",
                NetworkProtocol::UnixDgram => "Unix datagram",
            };

            match net.protocol {
                // A Unix socket is named by a path; its port means nothing.
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                    format!("{} {}", protocol, net.address)
                }
                _ => format!("{} {}:{}", protocol, net.address, net.port),
            }
        }
        EndpointConfig::Device(dev) => format!("Device {}", dev.path),
        EndpointConfig::Serial(serial) => {
            format!("Serial {} at {}", serial.path, serial.datarate)
        }
        EndpointConfig::I2c(i2c) => format!("I2C {} at {:#04X}", i2c.bus, i2c.address),
        EndpointConfig::Spi(spi) => format!("SPI {} {}", spi.path, spi.mode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::ResolvedSim;
    use std::sync::atomic::Ordering;
    use tcslibgs::{
        BitOrder, CsActive, DHId, DHMode, DHName, DeviceConfig, I2cConfig, NetworkConfig,
        NetworkProtocol, SerialConfig, SpiConfig, SpiMode, StopBits,
    };

    fn sim(packet_interval_ms: u32, segment_interval_ms: u32, segment_size: u32) -> ResolvedSim {
        ResolvedSim {
            packet_interval_ms,
            segment_interval_ms,
            segment_size,
            triggered: false,
            faults: Default::default(),
            payload_address: None,
            payload_port: None,
        }
    }

    #[test]
    fn a_device_handler_simulates_against_its_path() {
        let dh = DHConfig {
            dh_id: DHId(7),
            name: DHName::new("DH7"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 4,
            oc: None,
                    mode: Default::default(),
        };

        let config = payload_config_from(&dh, &sim(250, 100, 2)).unwrap();
        assert_eq!(config.address, "/dev/urandom");
        assert!(config.protocol == PayloadProtocol::Device);
        // The packet size is the payload file's; everything else is the
        // simulator file's.
        assert_eq!(config.packet_size.load(Ordering::SeqCst), 4);
        assert_eq!(config.segment_size.load(Ordering::SeqCst), 2);
        assert_eq!(config.packet_interval_ms.load(Ordering::SeqCst), 250);
        assert_eq!(config.segment_interval_ms.load(Ordering::SeqCst), 100);
    }

    /// A triggered payload the simulator cannot be the asked end of is
    /// refused by name, rather than simulated as one that answers nothing.
    ///
    /// Two kinds cannot hear a request. Tcspecial opens a device and reads
    /// it, so there is nothing for the simulator to be written to; and an I2C
    /// master's own read is the trigger, which a second master on the bus
    /// cannot see. A payload that answered nothing would look exactly like a
    /// handler whose polling had stopped working, which is the confusion
    /// worth refusing.
    #[test]
    fn a_triggered_payload_the_simulator_cannot_hear_is_refused() {
        let triggered = |name: &str, endpoint: EndpointConfig| DHConfig {
            dh_id: DHId(12),
            name: DHName::new(name),
            endpoint,
            packet_size: 4,
            oc: None,
            mode: DHMode::Triggered {
                trigger: b"READ".to_vec(),
                interval_ms: 500,
            },
        };

        let deaf = [
            (
                "DH12",
                EndpointConfig::Device(DeviceConfig {
                    path: "/dev/urandom".to_string(),
                }),
                "device",
            ),
            (
                "DH13",
                EndpointConfig::I2c(I2cConfig {
                    bus: "/tmp/i2c-sim".to_string(),
                    address: 0x48,
                    ten_bit: false,
                    pec: false,
                }),
                "bus",
            ),
        ];

        for (name, endpoint, what) in deaf {
            let e = match payload_config_from(&triggered(name, endpoint), &sim(0, 0, 4)) {
                Ok(_) => panic!("{name}: the simulator cannot be asked on a {what}"),
                Err(e) => e,
            };
            assert!(
                e.contains(name) && e.contains(what),
                "the refusal says neither which handler nor why: {e}"
            );
        }
    }

    /// A triggered payload of a kind that can hear one is simulated, and the
    /// simulator is told which kind it is.
    #[test]
    fn a_triggered_payload_reaches_the_simulator_as_triggered() {
        let dh = DHConfig {
            dh_id: DHId(14),
            name: DHName::new("DH14"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 8,
            oc: None,
            mode: DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval_ms: 500,
            },
        };

        let config = payload_config_from(&dh, &sim(0, 100, 2)).expect("a stream can be asked");
        assert!(config.triggered, "the simulator was not told to wait to be asked");
        // What to send and how often are tcspecial's, and do not come this
        // way at all: the simulator only has to answer.
        assert_eq!(config.packet_interval_ms.load(Ordering::SeqCst), 0);
    }

    /// A Unix socket of either flavour becomes a payload at its path, which
    /// leaves nothing the simulator refuses.
    ///
    /// It used to be refused, and was the last thing that was: not for want of
    /// anything to be -- a socket is a socket -- but because nobody had
    /// written it. Which end binds the path still differs by protocol, and
    /// that is payload_unix's business rather than this one's; what reaches it
    /// from here is the path and the protocol.
    #[test]
    fn a_unix_socket_becomes_a_payload_at_its_path() {
        for (protocol, expected) in [
            (NetworkProtocol::UnixStream, PayloadProtocol::UnixStream),
            (NetworkProtocol::UnixDgram, PayloadProtocol::UnixDgram),
        ] {
            let dh = DHConfig {
                dh_id: DHId(8),
                name: DHName::new("DH8"),
                endpoint: EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: "/tmp/dh8.sock".to_string(),
                    port: 0,
                }),
                packet_size: 4,
                oc: None,
                            mode: Default::default(),
            };

            let config =
                payload_config_from(&dh, &sim(250, 250, 4)).expect("a socket is simulable");
            assert!(config.protocol == expected, "{protocol:?}");
            assert_eq!(config.address, "/tmp/dh8.sock");
            // A Unix socket is named by a path, so its port means nothing.
            assert_eq!(config.port, 0);
        }
    }

    /// A serial line is a kind the simulator can be: it becomes a payload at
    /// the path the handler was told to open, which the simulator makes lead
    /// to a pty of its own.
    #[test]
    fn a_serial_line_becomes_a_payload_at_the_configured_path() {
        let dh = DHConfig {
            dh_id: DHId(9),
            name: DHName::new("DH9"),
            endpoint: EndpointConfig::Serial(SerialConfig {
                path: "/tmp/ttyS0".to_string(),
                datarate: 9600,
                stop_bits: StopBits::One,
                byte_length: 8,
            }),
            packet_size: 4,
            oc: None,
                    mode: Default::default(),
        };

        let config = payload_config_from(&dh, &sim(250, 100, 2)).expect("a line is simulable");
        assert!(config.protocol == PayloadProtocol::Serial);
        assert_eq!(config.address, "/tmp/ttyS0");
        // The line's terms are not the simulator's business: a pty takes a
        // data rate and ignores it, and what the handler sets is between the
        // handler and the kernel.
        assert_eq!(config.packet_size.load(Ordering::SeqCst), 4);
        assert_eq!(config.segment_size.load(Ordering::SeqCst), 2);
    }

    /// A device on an I2C bus becomes a payload at the bus the handler was
    /// told to open, carrying the address on it.
    ///
    /// The address is not the port -- a bus has no port -- and not the
    /// address, which for a bus is the bus itself. It is the one thing about
    /// an I2C payload that neither of the other two fields could hold, and
    /// losing it would have the simulator filling the registers of whichever
    /// device the bus was last pointed at.
    #[test]
    fn an_i2c_device_becomes_a_payload_on_its_bus_at_its_address() {
        let dh = DHConfig {
            dh_id: DHId(10),
            name: DHName::new("DH10"),
            endpoint: EndpointConfig::I2c(I2cConfig {
                bus: "/tmp/i2c-sim".to_string(),
                address: 0x48,
                ten_bit: false,
                pec: false,
            }),
            packet_size: 8,
            oc: None,
                    mode: Default::default(),
        };

        let config = payload_config_from(&dh, &sim(250, 100, 2)).expect("a bus is simulable");
        assert!(config.protocol == PayloadProtocol::I2c);
        assert_eq!(config.address, "/tmp/i2c-sim");
        assert_eq!(config.bus_address, 0x48);
        assert_eq!(config.port, 0, "a bus has no port");
    }

    /// A SPI peripheral becomes a payload at the configured path too, which
    /// leaves no kind of endpoint the simulator refuses.
    ///
    /// It is not the same quality of stand-in as the others, and the file for
    /// it says so: a pty carries a peripheral's bytes and has none of its
    /// clocking. With the Unix sockets above, nothing is refused here any
    /// more -- which puts the burden on each kind's own file to say what its
    /// stand-in is and is not.
    #[test]
    fn a_spi_peripheral_becomes_a_payload_at_the_configured_path() {
        let dh = DHConfig {
            dh_id: DHId(11),
            name: DHName::new("DH11"),
            endpoint: EndpointConfig::Spi(SpiConfig {
                path: "/tmp/spidev0.0".to_string(),
                max_speed: 1_000_000,
                mode: SpiMode::Mode0,
                bits_per_word: 8,
                bit_order: BitOrder::MsbFirst,
                cs_active: CsActive::Low,
            }),
            packet_size: 8,
            oc: None,
            mode: Default::default(),
        };

        let config =
            payload_config_from(&dh, &sim(250, 100, 2)).expect("a peripheral is stood in for");
        assert!(config.protocol == PayloadProtocol::Spi);
        assert_eq!(config.address, "/tmp/spidev0.0");
        // None of the clocking comes this way: there is nothing at the other
        // end of a pty for it to be applied to.
        assert_eq!(config.port, 0);
        assert_eq!(config.bus_address, 0);
    }
}
