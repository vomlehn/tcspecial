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
use crate::sim_config::ResolvedSim;

/// Turn a data handler and its simulator settings into the simulator's own
/// configuration.
///
/// The payload file describes what tcspecial expects to talk to, so the
/// simulator takes the other end of it: the handler's endpoint becomes the
/// address the simulated payload uses. Everything about how the payload
/// behaves comes from `sim`, which is what the simulator file settled.
pub fn payload_config_from(dh: &DHConfig, sim: &ResolvedSim) -> Result<PayloadConfig, String> {
    let (protocol, address, port) = match &dh.endpoint {
        EndpointConfig::Network(net) => {
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => PayloadProtocol::Tcp,
                NetworkProtocol::Udp => PayloadProtocol::Udp,
                // The simulator speaks only TCP, UDP, and devices. A Unix
                // socket handler is a configuration the simulator cannot
                // stand in for, so say so rather than simulating the wrong
                // thing.
                other => {
                    return Err(format!(
                        "{} uses {:?}, which the simulator cannot simulate",
                        dh.name.0, other
                    ))
                }
            };
            (protocol, net.address.clone(), net.port)
        }
        EndpointConfig::Device(dev) => (PayloadProtocol::Device, dev.path.clone(), 0),
        // The simulator has nothing to be at the far end of these. A serial
        // line needs a port to open, a bus needs a device that answers an
        // address on it, and a SPI peripheral needs to be clocked: standing in
        // for any of them means being the hardware, not a program on a
        // socket. Refused here rather than simulated as something else, which
        // is what a plain device would be.
        EndpointConfig::Serial(serial) => {
            return Err(format!(
                "{} is a serial line at {}, which the simulator cannot stand in for",
                dh.name.0, serial.path
            ))
        }
        EndpointConfig::I2c(i2c) => {
            return Err(format!(
                "{} is I2C device {:#04X} on {}, which the simulator cannot stand in for",
                dh.name.0, i2c.address, i2c.bus
            ))
        }
        EndpointConfig::Spi(spi) => {
            return Err(format!(
                "{} is a SPI peripheral at {}, which the simulator cannot stand in for",
                dh.name.0, spi.path
            ))
        }
    };

    let packet_size = u32::try_from(dh.packet_size)
        .map_err(|_| format!("{} has a packet size too large to simulate", dh.name.0))?;

    Ok(PayloadConfig {
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
    use crate::sim_config::ResolvedSim;
    use std::sync::atomic::Ordering;
    use tcslibgs::{
        BitOrder, CsActive, DHId, DHName, DeviceConfig, I2cConfig, NetworkConfig, SerialConfig,
        SpiConfig, SpiMode, StopBits,
    };

    fn sim(packet_interval_ms: u32, segment_interval_ms: u32, segment_size: u32) -> ResolvedSim {
        ResolvedSim {
            packet_interval_ms,
            segment_interval_ms,
            segment_size,
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

    #[test]
    fn a_unix_socket_handler_is_refused_rather_than_mis_simulated() {
        let dh = DHConfig {
            dh_id: DHId(8),
            name: DHName::new("DH8"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::UnixStream,
                address: "/tmp/dh8".to_string(),
                port: 0,
            }),
            packet_size: 4,
            oc: None,
        };

        assert!(payload_config_from(&dh, &sim(250, 250, 4)).is_err());
    }

    /// A line, a bus and a clocked peripheral are refused the same way, and
    /// each refusal says which handler and where.
    ///
    /// They used all to arrive here as plain devices, so the simulator would
    /// cheerfully stand in for a serial line by writing to its node at
    /// whatever rate the port was left at. Being told that the simulator
    /// cannot be the far end of a bus is more use than being simulated as
    /// something else.
    #[test]
    fn the_kinds_the_simulator_cannot_be_are_refused_by_name() {
        let hardware = [
            (
                "DH9",
                EndpointConfig::Serial(SerialConfig {
                    path: "/dev/ttyS0".to_string(),
                    datarate: 9600,
                    stop_bits: StopBits::One,
                    byte_length: 8,
                }),
                "/dev/ttyS0",
            ),
            (
                "DH10",
                EndpointConfig::I2c(I2cConfig {
                    bus: "/dev/i2c-1".to_string(),
                    address: 0x48,
                    ten_bit: false,
                    pec: false,
                }),
                "/dev/i2c-1",
            ),
            (
                "DH11",
                EndpointConfig::Spi(SpiConfig {
                    path: "/dev/spidev0.0".to_string(),
                    max_speed: 1_000_000,
                    mode: SpiMode::Mode0,
                    bits_per_word: 8,
                    bit_order: BitOrder::MsbFirst,
                    cs_active: CsActive::Low,
                }),
                "/dev/spidev0.0",
            ),
        ];

        for (name, endpoint, where_it_is) in hardware {
            let dh = DHConfig {
                dh_id: DHId(9),
                name: DHName::new(name),
                endpoint,
                packet_size: 4,
                oc: None,
            };

            let e = match payload_config_from(&dh, &sim(250, 250, 4)) {
                Ok(_) => panic!("{name}: the simulator cannot be hardware"),
                Err(e) => e,
            };
            assert!(
                e.contains(name) && e.contains(where_it_is),
                "the refusal names neither the handler nor where it is: {e}"
            );
        }
    }
}
