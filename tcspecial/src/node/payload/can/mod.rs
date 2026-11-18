//! The CAN endpoint: one raw socket on one interface, filtered to one
//! identifier.
//!
//! **Supported.** Run against the kernel's virtual bus, by
//! `tests/kernel-dummies.rs`, and against no CAN controller.
//!
//! A CAN controller is a network interface, so this is a socket rather than an
//! open device file: `PF_CAN`, `SOCK_RAW`, `CAN_RAW`, bound to the interface
//! the configuration names. What arrives is frames, and a frame is its own
//! length -- the controller delivers whole frames or nothing -- so a read
//! hands up one frame's data and a write puts one packet into one frame.
//! There is no fragmentation anywhere in CAN, which is why the device
//! configuration refuses a packet size a frame cannot carry rather than
//! leaving it to be discovered here.
//!
//! The socket is filtered to the identifier the device's frames carry.
//! Without that a handler on a shared bus would read every other device's
//! traffic and count it as its own, which is the one thing a bus makes easy to
//! get wrong. Its own frames are not received back, so a handler never reads
//! the trigger it just sent.
//!
//! The socket itself -- the kernel's structures, its options, its frames --
//! is `tcsos::linux::can`'s, which tcs-sim's CAN device shares.

use tcsos::linux::can::{Bus, Refused, Settings};
use tcsos::wait::{Ready, Source, Waitable};

use tcsprotocol::{
    node::payload::can::frame_bytes, CanConfig, TcsError, TcsResult,
};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

/// A raw CAN socket, read and written by two conduits.
pub struct CanEndpoint {
    /// The handler this endpoint belongs to, as the device configuration
    /// names it, so what the endpoint says of itself says which handler it is
    /// for.
    what: String,
    /// The socket, on its bus.
    bus: Bus,
    /// Whether frames are CAN FD, which decides how much a frame carries.
    fd: bool,
}

impl CanEndpoint {
    /// Opens a socket on the configured interface and filters it to the
    /// configured identifier.
    ///
    /// Only this device's frames: a bus carries everybody's. Loopback off is
    /// the unusual case and the one worth asking for: a bus with real
    /// hardware on it needs no loopback, where two programs on one host do.
    /// Non-blocking, as every other endpoint is: a conduit waits and reads
    /// what is there.
    ///
    /// # Arguments
    ///
    /// * `what`   - The handler this endpoint belongs to, as the device
    ///              configuration names it, for what a complaint says.
    /// * `config` - The interface, the identifier, whether frames are CAN
    ///              FD and whether the socket's own frames loop back.
    ///
    /// # Returns
    ///
    /// The endpoint, its socket bound to the bus and non-blocking.
    ///
    /// # Errors
    ///
    /// `TcsError::Config` if there is no such interface or its name is
    /// not one an interface can have; `TcsError::Endpoint` if the socket
    /// cannot be opened, filtered, flagged, bound or made non-blocking.
    pub fn new(what: &str, config: &CanConfig) -> TcsResult<Self> {
        let interface = &config.interface;
        let settings = Settings {
            can_id: config.can_id,
            extended: tcsprotocol::node::payload::can::is_extended(
                config.can_id,
            ),
            fd: config.fd,
            loopback: config.loopback,
        };
        let bus = Bus::join(interface, &settings)
            .map_err(|refused| refused_for(what, interface, config, refused))?;
        Ok(Self {
            what: what.to_string(),
            bus,
            fd: config.fd,
        })
    }

    /// A second endpoint on the same socket.
    ///
    /// One socket, as the other kinds do it: a conduit pair reads and writes
    /// one endpoint, and a second socket would be a second member of the bus
    /// with its own filter.
    ///
    /// # Returns
    ///
    /// The second endpoint, on a duplicate of this one's descriptor.
    ///
    /// # Errors
    ///
    /// `TcsError::Io` if the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            bus: self.bus.try_clone()?,
            fd: self.fd,
        })
    }

    /// How many bytes one of this endpoint's frames carries.
    pub fn frame_bytes(&self) -> usize {
        frame_bytes(self.fd)
    }
}

/// What a socket that could not be put on its bus is said to be.
///
/// A name the kernel does not know is the common failure -- a bus that has
/// not been brought up, or a virtual one that was never created -- and the
/// error says so, with the command that would make one.
///
/// # Arguments
///
/// * `what`      - The handler, for what a complaint says.
/// * `interface` - The interface's name.
/// * `config`    - The node's configuration, for the identifier.
/// * `refused`   - What could not be done.
///
/// # Returns
///
/// `TcsError::Config` for an interface that is not there or a name that
/// cannot be one, `TcsError::Endpoint` for the rest.
fn refused_for(
    what: &str,
    interface: &str,
    config: &CanConfig,
    refused: Refused,
) -> TcsError {
    let can_id = config.can_id;
    match refused {
        Refused::Name => TcsError::Config(format!(
            "{what}: \"{interface}\" is not a name an interface can have"
        )),
        Refused::NoInterface(e) => TcsError::Config(format!(
            "{what}: this host has no interface called \"{interface}\": {e}. \
             A virtual bus is made with \"ip link add dev {interface} type \
             vcan\" and brought up with \"ip link set dev {interface} up\""
        )),
        Refused::Socket(e) => TcsError::Endpoint(format!(
            "{what}: cannot open a CAN socket: {e}. A kernel without CAN \
             support has no socket to give"
        )),
        Refused::Filter(e) => TcsError::Endpoint(format!(
            "{what}: cannot filter this socket to can_id {can_id:#x}: {e}"
        )),
        Refused::FdFrames(e) => TcsError::Endpoint(format!(
            "{what}: this socket will not take CAN FD: {e}"
        )),
        Refused::Loopback(e) => TcsError::Endpoint(format!(
            "{what}: this socket will not take loopback: {e}"
        )),
        Refused::Bind(e) => TcsError::Endpoint(format!(
            "{what}: cannot bind a CAN socket to \"{interface}\": {e}"
        )),
        Refused::NonBlocking(e) => TcsError::Endpoint(format!(
            "{what}: cannot make this CAN socket non-blocking: {e}"
        )),
    }
}

impl EndpointWaitable for CanEndpoint {
    fn io_source(&self) -> Source {
        self.bus.source()
    }

    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
    }
}

impl EndpointReadable for CanEndpoint {
    /// One frame's data, or nothing.
    ///
    /// A frame at a time, because that is what the socket delivers: a read
    /// gets one whole frame or none of one, so there is no partial frame to
    /// keep and no stream rule to say where a read ends. A frame shorter
    /// than the structure is not a frame, and is counted as nothing read
    /// rather than reported, since the bus may carry a kind this socket was
    /// not set up for.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        self.bus.receive(buffer).map_err(TcsError::Io)
    }
}

impl EndpointWritable for CanEndpoint {
    /// One packet in one frame.
    ///
    /// A packet longer than a frame is refused rather than split: there is no
    /// fragmentation in CAN, so the far end would receive a frame and have no
    /// way of knowing the rest was coming. The configuration refuses such a
    /// device where the file is read, so reaching this means a trigger
    /// longer than a frame, which is the one write a file does not size.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        let room = self.frame_bytes();
        if data.len() > room {
            return Err(TcsError::Endpoint(format!(
                "{}: {} bytes will not fit in a {} frame, which carries \
                 {room}, and CAN does not fragment",
                self.what,
                data.len(),
                if self.fd { "CAN FD" } else { "classic CAN" }
            )));
        }
        self.bus.send(data).map_err(TcsError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An interface this host has not got is reported, with the command that
    /// would make one.
    ///
    /// The common failure by a distance: a virtual bus has to be created and
    /// brought up, which needs root, so a device set that runs on one
    /// machine fails on the next -- and the message is the whole of what
    /// tells an operator which it is.
    #[test]
    fn an_interface_that_is_not_there_says_how_to_make_one() {
        let config = CanConfig {
            interface: "vcan-nonesuch".to_string(),
            can_id: 0x100,
            fd: false,
            loopback: true,
        };
        let said = match CanEndpoint::new("device", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has an interface called vcan-nonesuch"),
        };
        assert!(
            said.contains("vcan-nonesuch") && said.contains("ip link add"),
            "the error should name the interface and how to make one: {said}"
        );
    }

    /// Opening a device on an interface that is not there fails with that
    /// same message rather than a socket error.
    #[test]
    fn a_device_on_a_bus_that_is_not_there_is_reported() {
        let config = CanConfig {
            interface: "vcan-nonesuch".to_string(),
            can_id: 0x100,
            fd: false,
            loopback: true,
        };

        let said = match CanEndpoint::new("device", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has an interface called vcan-nonesuch"),
        };
        assert!(
            said.contains("vcan-nonesuch"),
            "the error should name the interface: {said}"
        );
    }

    /// A device's frames carry what it was asked to send, over the kernel's
    /// virtual bus.
    ///
    /// The whole of the endpoint, where a bus exists to carry it: two sockets
    /// on one interface, one filtered to the identifier, a write at one end
    /// and a read at the other. Skipped where there is no virtual bus to use,
    /// which is most machines -- creating one needs root -- and said so
    /// rather than passed silently.
    #[test]
    fn a_frame_carries_a_packet_over_a_virtual_bus() {
        let Some(interface) = a_virtual_bus() else {
            eprintln!(
                "skipped: no virtual CAN bus up on this host. Make one with \
                 \"ip link add dev vcan0 type vcan; ip link set dev vcan0 up\""
            );
            return;
        };

        let config = CanConfig {
            interface: interface.clone(),
            can_id: 0x321,
            fd: false,
            loopback: true,
        };

        let mut handler =
            CanEndpoint::new("handler", &config).expect("a socket on the bus");
        let mut device =
            CanEndpoint::new("device", &config).expect("another socket on it");

        // The device sends; the handler reads what arrived.
        assert_eq!(
            device.write(&[1, 2, 3, 4]).expect("a frame goes out"),
            4,
            "a write moves the packet, not the frame"
        );

        let mut buffer = [0u8; 64];
        let got = (0..50)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(20));
                match handler.read(&mut buffer) {
                    Ok(0) => None,
                    Ok(n) => Some(n),
                    Err(e) => panic!("the handler could not read its bus: {e}"),
                }
            })
            .expect("the frame arrived");
        assert_eq!(&buffer[..got], &[1, 2, 3, 4]);
    }

    /// Makes an endpoint whose socket is not a bus at all.
    ///
    /// For the checks that happen before anything is written, which need a
    /// constructed endpoint and not a bus. They were gated on a virtual bus
    /// because they were written beside the test above, and so ran on no
    /// machine that had not been set up by hand -- which is most of them.
    ///
    /// # Arguments
    ///
    /// * `fd` - Whether the endpoint sends CAN FD frames.
    ///
    /// # Returns
    ///
    /// An endpoint for identifier 1 whose descriptor is `/dev/null`.
    ///
    /// # Panics
    ///
    /// If `/dev/null` cannot be opened for writing.
    fn not_on_a_bus(fd: bool) -> CanEndpoint {
        CanEndpoint {
            what: "device".to_string(),
            // Writable, because the lengths that are not refused go on to
            // be written: /dev/null takes them and says nothing, which is
            // exactly what is wanted of a descriptor that is not a bus.
            bus: Bus::not_on_a_bus(fd).expect("a descriptor"),
            fd,
        }
    }

    /// A packet longer than a frame is refused rather than split.
    ///
    /// Before the write is attempted, so the descriptor here need not be a
    /// bus: what is being tested is which lengths are turned away, and what
    /// happens to the ones that are not is the bus test's business.
    #[test]
    fn a_packet_longer_than_a_frame_is_refused() {
        let mut classic = not_on_a_bus(false);

        let said = match classic.write(&[0u8; 9]) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("nine bytes went into an eight-byte frame as {n}"),
        };
        assert!(
            said.contains("does not fragment") && said.contains("classic CAN"),
            "the error should say why it cannot be split, and of which frame: \
             {said}"
        );

        // Eight is the most a classic frame carries, and is not refused.
        assert_eq!(classic.write(&[0u8; 8]).expect("a full frame"), 8);

        // The room is the kind's, not a constant: the same nine bytes are
        // nothing to an FD frame, and sixty-five are too many for one.
        let mut fd = not_on_a_bus(true);
        assert_eq!(fd.write(&[0u8; 9]).expect("room in an FD frame"), 9);
        assert_eq!(fd.write(&[0u8; 64]).expect("a full FD frame"), 64);

        let said = match fd.write(&[0u8; 65]) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!(
                "sixty-five bytes went into a sixty-four-byte frame as {n}"
            ),
        };
        assert!(
            said.contains("CAN FD") && said.contains("64"),
            "the error should say which frame and how much it carries: {said}"
        );
    }

    /// The first virtual CAN interface this host has up, if it has one.
    fn a_virtual_bus() -> Option<String> {
        (0..4).map(|n| format!("vcan{n}")).find(|name| {
            tcsos::linux::can::interface_index(name).is_ok()
                && tcsos::linux::can::is_up(name)
        })
    }
}
