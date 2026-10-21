//! What a data handler's endpoint means to the payload at its far end.
//!
//! The tcspecial configuration describes the handler's side of the link: what
//! kind of endpoint it is and where. The simulator takes the other end of that,
//! so this is where an endpoint is read -- turned into the configuration a
//! simulated payload runs on, and into the line a panel shows for it.
//!
//! The kinds of payload tcssim cannot stand in for are refused by
//! `tcslibgs::simulable`, which this asks first: the rule is there so that
//! `tcsverify` asks it of the same files, and saying so is better than
//! simulating something else at that address.
//!
//! What a payload then does with the endpoint is in the file for its kind:
//! [`crate::node::payload::network::tcp`], [`crate::node::payload::network::udp`], [`crate::node::payload::device_file`].

use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use tcslibgs::{DHConfig, EndpointConfig, NetworkProtocol};

use crate::payload::{PayloadConfig, PayloadProtocol};
use tcslibgs::ResolvedSim;

/// Turn a data handler and its simulator settings into the simulator's own
/// configuration.
///
/// The tcspecial configuration describes what tcspecial expects to talk to, so
/// the simulator takes the other end of it: the handler's endpoint becomes the
/// address the simulated payload uses. Everything about how the payload behaves
/// comes from `sim`, which is what the simulator file settled.
///
/// Attached to nothing, which the simulator itself never is any more -- it
/// asks for the attachment because a payload may have one -- so this is the
/// form the tests of the pacing, the faults and the addresses use, and saying
/// `None` in each of them would say nothing to them.
#[cfg(test)]
pub fn payload_config_from(dh: &DHConfig, sim: &ResolvedSim) -> Result<PayloadConfig, String> {
    payload_config_attached(dh, sim, None)
}

/// And the same, with the thermal mass this payload is attached to.
///
/// `thermal` is what [`crate::thermal`] made of the attachment the simulator
/// file states: `None` for a payload attached to nothing, which is every
/// payload of a file that declares no mass.
///
/// Apart from the function above so that every test of the pacing, the
/// faults and the addresses goes on saying nothing about thermal masses:
/// what they are about has not changed.
pub fn payload_config_attached(
    dh: &DHConfig,
    sim: &ResolvedSim,
    thermal: Option<crate::thermal::Thermal>,
) -> Result<PayloadConfig, String> {
    // What the simulator cannot be, which is tcslibgs's rule so that tcsverify
    // asks it too: see `tcslibgs::simulable`.
    tcslibgs::simulable(dh)?;

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
                // Refused by `simulable` above, which says why; here only so
                // that a raw payload reaching this is refused and not run.
                NetworkProtocol::Raw(_) => {
                    return Err(format!(
                        "{} is reached on a raw socket, which the simulator does not \
                         stand in for",
                        dh.name.0
                    ))
                }
            };
            (protocol, net.address.clone(), net.port, 0)
        }
        EndpointConfig::Device(dev) => (PayloadProtocol::Device, dev.path.clone(), 0, 0),
        // A bus the simulator can simply be on: a CAN controller carries
        // every member's frames to every other, so the simulator opens a
        // socket on the same interface and sends the payload's frames with
        // the payload's identifier. Nothing has to stand in for anything --
        // which makes this the one experimental kind whose stand-in is as
        // faithful as a socket.
        EndpointConfig::Can(can) => (PayloadProtocol::Can, can.interface.clone(), 0, 0),
        // A sensor's buffer the simulator can be: a pty's slave is a
        // character device a handler reads the same way, so the path the
        // handler was told to open is made to lead there. What it does not
        // stand in for is the sysfs around it -- the channels and the buffer
        // -- which a handler does not touch either; see payload_iio.
        EndpointConfig::Iio(iio) => (PayloadProtocol::Iio, iio.path.clone(), 0, 0),
        // A channel to another core, which a pty stands in for both ways: a
        // handler writes a message and reads an answer, and that is what a
        // pty carries. What it does not reproduce is the channel's identity
        // -- there is no sysfs entry saying which service it is -- which the
        // handler's own check allows for; see payload_rpmsg.
        EndpointConfig::Rpmsg(rpmsg) => (PayloadProtocol::Rpmsg, rpmsg.path.clone(), 0, 0),
        // Discrete lines, which nothing in user space can drive: what stands
        // in is the kernel's gpio-sim, whose lines the simulator pulls one way
        // and the other. Found rather than made, as the I2C stub is; see
        // payload_gpio.
        EndpointConfig::Gpio(gpio) => (PayloadProtocol::Gpio, gpio.path.clone(), 0, 0),
        // A synchronous line, which nothing in the kernel stands in for.
        // Refused by `simulable` above, where the reason is written; said
        // again here rather than assumed.
        EndpointConfig::Hdlc(_) => {
            return Err(format!(
                "simulated payload \"{}\" is on a synchronous line, which \
                 nothing stands in for",
                dh.name.0
            ))
        }
        // A line the simulator can be: a pty's slave is a device file with a
        // line discipline behind it, so a handler opens it, sets its terms and
        // reads it exactly as it would a port. The path the handler was told
        // to open is made to lead there; see node::payload::serial.
        //
        // The line's terms do not come this way. A pty takes a data rate and
        // ignores it, so what the handler sets them to is between the handler
        // and the kernel, and the simulator has only the path to be at.
        EndpointConfig::Serial(serial) => (PayloadProtocol::Serial, serial.path.clone(), 0, 0),
        // A bus the simulator can be on, though not by being a device: the
        // kernel's i2c-stub is an adapter whose chips are a bank of registers
        // in memory, and every master on that bus reads and writes the same
        // bank. So the bank is the payload, and the simulator fills it; see
        // node::payload::i2c. The bus the handler was told to open is made to lead to
        // the stub, as a line's path is made to lead to a pty.
        EndpointConfig::I2c(i2c) => (PayloadProtocol::I2c, i2c.bus.clone(), 0, i2c.address),
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

    // What a CAN payload is identified by, which no other kind has: taken
    // from the endpoint rather than from `bus_address`, a CAN identifier
    // being twenty-nine bits where an I2C address is ten.
    let (can_id, can_fd) = match &dh.endpoint {
        EndpointConfig::Can(can) => (can.can_id, can.fd),
        _ => (0, false),
    };

    // And which lines a set of discretes moves, which is the one thing a GPIO
    // payload produces: not bytes a file sized.
    let gpio_lines = match &dh.endpoint {
        EndpointConfig::Gpio(gpio) => gpio.lines.clone(),
        _ => Vec::new(),
    };

    Ok(PayloadConfig {
        thermal,
        bus_address,
        can_id,
        can_fd,
        gpio_lines,
        triggered: dh.mode.polling().is_some(),
        faults: sim.faults,
        // The socket this payload answers from, where the simulator file
        // states one. Refused there for the kinds that have none to bind, so
        // what arrives here is for a kind that has.
        own_address: sim.payload_address.clone(),
        own_port: sim.payload_port,
        name: dh.name.0.clone(),
        protocol,
        address,
        port,
        packet_size: Arc::new(AtomicU32::new(packet_size)),
        segment_size: Arc::new(AtomicU32::new(sim.segment_size)),
        packet_interval_ms: Arc::new(AtomicU32::new(sim.packet_interval.as_millis() as u32)),
        segment_interval_ms: Arc::new(AtomicU32::new(sim.segment_interval.as_millis() as u32)),
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
                NetworkProtocol::Raw(_) => "Raw IP",
            };

            match net.protocol {
                // A Unix socket is named by a path; its port means nothing.
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                    format!("{} {}", protocol, net.address)
                }
                // Nor has an IP datagram a port: its protocol number says
                // which are the payload's.
                NetworkProtocol::Raw(number) => {
                    format!("{} {} protocol {}", protocol, net.address, number)
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
        EndpointConfig::Can(can) => format!("CAN {} id {:#x}", can.interface, can.can_id),
        EndpointConfig::Iio(iio) => format!("IIO {}", iio.path),
        EndpointConfig::Rpmsg(rpmsg) => format!("rpmsg {} ({})", rpmsg.path, rpmsg.service),
        EndpointConfig::Gpio(gpio) => {
            let offsets: Vec<String> = gpio.lines.iter().map(|n| n.to_string()).collect();
            format!("GPIO {} lines {}", gpio.path, offsets.join(","))
        }
        // A synchronous line: the interface, and whose clock it is on, which
        // is the fact an operator compares against the equipment.
        EndpointConfig::Hdlc(hdlc) => format!(
            "HDLC {} as {}, clock {}",
            hdlc.interface, hdlc.standard, hdlc.clock_type
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use tcslibgs::ResolvedSim;
    use tcslibgs::{
        BitOrder, CsActive, DHMode, DHName, DeviceConfig, I2cConfig, NetworkConfig,
        NetworkProtocol, SerialConfig, SpiConfig, SpiMode, StopBits,
    };

    fn sim(packet_interval_ms: u32, segment_interval_ms: u32, segment_size: u32) -> ResolvedSim {
        ResolvedSim {
            attached: None,
            packet_interval: Duration::from_millis(u64::from(packet_interval_ms)),
            segment_interval: Duration::from_millis(u64::from(segment_interval_ms)),
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
            name: DHName::new("DH7"),
            endpoint: EndpointConfig::Device(DeviceConfig {
                path: "/dev/urandom".to_string(),
            }),
            packet_size: 4,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
            sends_from: None,
        };

        let config = payload_config_from(&dh, &sim(250, 100, 2)).unwrap();
        assert_eq!(config.address, "/dev/urandom");
        assert!(config.protocol == PayloadProtocol::Device);
        // The packet size is the tcspecial configuration's; everything else is
        // the simulator file's.
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
            name: DHName::new(name),
            endpoint,
            packet_size: 4,
            oc: None,
            ground: None,
            mode: DHMode::Triggered {
                trigger: b"READ".to_vec(),
                interval: Duration::from_millis(500),
            },
            stale_interval: None,
            sends_from: None,
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
            name: DHName::new("DH14"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Tcp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 8,
            oc: None,
            ground: None,
            mode: DHMode::Triggered {
                trigger: b"READ\r".to_vec(),
                interval: Duration::from_millis(500),
            },
            stale_interval: None,
            sends_from: None,
        };

        let config = payload_config_from(&dh, &sim(0, 100, 2)).expect("a stream can be asked");
        assert!(
            config.triggered,
            "the simulator was not told to wait to be asked"
        );
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
                name: DHName::new("DH8"),
                endpoint: EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: "/tmp/dh8.sock".to_string(),
                    port: 0,
                }),
                packet_size: 4,
                oc: None,
                ground: None,
                mode: Default::default(),
                stale_interval: None,
                sends_from: None,
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
            name: DHName::new("DH9"),
            endpoint: EndpointConfig::Serial(SerialConfig {
                path: "/tmp/ttyS0".to_string(),
                datarate: 9600,
                parity: tcslibgs::Parity::None,
                stop_bits: StopBits::One,
                byte_length: 8,
                rs485: None,
            }),
            packet_size: 4,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
            sends_from: None,
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
            name: DHName::new("DH10"),
            endpoint: EndpointConfig::I2c(I2cConfig {
                bus: "/tmp/i2c-sim".to_string(),
                address: 0x48,
                ten_bit: false,
                pec: false,
            }),
            packet_size: 8,
            oc: None,
            ground: None,
            mode: Default::default(),
            stale_interval: None,
            sends_from: None,
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
            ground: None,
            mode: Default::default(),
            stale_interval: None,
            sends_from: None,
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
