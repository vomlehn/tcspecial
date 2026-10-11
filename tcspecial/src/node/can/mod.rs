//! The CAN endpoint: one raw socket on one interface, filtered to one
//! identifier.
//!
//! **Experimental, and not yet supported.** Run against the kernel's virtual
//! bus and against no CAN controller.
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
//! The structures and constants are the kernel's; `libc` has them for CAN, so
//! unlike the GPIO endpoint nothing is declared here.

use std::io;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{node::can::frame_bytes, CanConfig, TcsError, TcsResult};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// A raw CAN socket, read and written by two conduits.
pub struct CanEndpoint {
    /// The handler this endpoint belongs to, as the device configuration
    /// names it, so what the endpoint says of itself says which handler it is
    /// for.
    what: String,
    socket: OwnedFd,
    /// The identifier frames are sent with, which is the one they are
    /// filtered on.
    can_id: u32,
    /// Whether frames are CAN FD, which decides both the structure the kernel
    /// exchanges and how much a frame carries.
    fd: bool,
}

impl CanEndpoint {
    /// Open a socket on the configured interface and filter it to the
    /// configured identifier.
    pub fn new(what: &str, config: &CanConfig) -> TcsResult<Self> {
        let index = interface_index(what, &config.interface)?;

        // SAFETY: a socket call with constant arguments; the result is
        // checked before it is used.
        let fd = unsafe { libc::socket(libc::PF_CAN, libc::SOCK_RAW, libc::CAN_RAW) };
        if fd < 0 {
            return Err(TcsError::Endpoint(format!(
                "{what}: cannot open a CAN socket: {}. A kernel without CAN support \
                 has no socket to give",
                io::Error::last_os_error()
            )));
        }
        // From here the descriptor is owned, so every path below closes it.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };

        // Only this device's frames. A bus carries everybody's.
        set_filter(what, socket.as_raw_fd(), config.can_id)?;

        if config.fd {
            set_flag(what, socket.as_raw_fd(), libc::CAN_RAW_FD_FRAMES, 1, "CAN FD")?;
        }

        // Off is the unusual case and the one worth asking for: a bus with
        // real hardware on it needs no loopback, where two programs on one
        // host do.
        if !config.loopback {
            set_flag(what, socket.as_raw_fd(), libc::CAN_RAW_LOOPBACK, 0, "loopback")?;
        }

        bind_to(what, socket.as_raw_fd(), &config.interface, index)?;

        // Non-blocking, as every other endpoint is: a conduit waits in poll
        // and reads what is there.
        set_nonblocking(what, socket.as_raw_fd())?;

        Ok(Self {
            what: what.to_string(),
            socket,
            can_id: config.can_id,
            fd: config.fd,
        })
    }

    /// A second endpoint on the same socket.
    ///
    /// One socket, as the other kinds do it: a conduit pair reads and writes
    /// one endpoint, and a second socket would be a second member of the bus
    /// with its own filter.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            socket: self.socket.try_clone()?,
            can_id: self.can_id,
            fd: self.fd,
        })
    }

    /// How many bytes one of this endpoint's frames carries.
    pub fn frame_bytes(&self) -> usize {
        frame_bytes(self.fd)
    }
}

/// The kernel's index for a named interface.
///
/// A socket is bound to an index rather than to a name, and the name is what a
/// file states. A name the kernel does not know is the common failure -- a bus
/// that has not been brought up, or a virtual one that was never created --
/// and the error says so, with the command that would make one, as the I2C
/// stand-in does for its module.
fn interface_index(what: &str, interface: &str) -> TcsResult<libc::c_uint> {
    let name = std::ffi::CString::new(interface).map_err(|_| {
        TcsError::Config(format!(
            "{what}: \"{interface}\" is not a name an interface can have"
        ))
    })?;

    // SAFETY: the name is a valid C string for the length of the call.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return Err(TcsError::Config(format!(
            "{what}: this host has no interface called \"{interface}\": {}. A virtual \
             bus is made with \"ip link add dev {interface} type vcan\" and brought up \
             with \"ip link set dev {interface} up\"",
            io::Error::last_os_error()
        )));
    }

    Ok(index)
}

/// Accept this device's frames and nothing else on the bus.
///
/// One filter, on the identifier and every bit of it. The extended-format
/// flag is part of the identifier the kernel compares, so it goes in the mask
/// as well: without it a base identifier would also match the extended frame
/// with the same number, which is a different frame.
fn set_filter(what: &str, fd: RawFd, can_id: u32) -> TcsResult<()> {
    let id = if tcslibgs::node::can::is_extended(can_id) {
        can_id | libc::CAN_EFF_FLAG
    } else {
        can_id
    };

    let filter = libc::can_filter {
        can_id: id,
        can_mask: libc::CAN_EFF_MASK | libc::CAN_EFF_FLAG,
    };

    // SAFETY: the option is a single can_filter, which is what the kernel
    // reads for CAN_RAW_FILTER, and it lives for the call.
    let set = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_CAN_RAW,
            libc::CAN_RAW_FILTER,
            &filter as *const libc::can_filter as *const libc::c_void,
            mem::size_of::<libc::can_filter>() as libc::socklen_t,
        )
    };

    if set < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot filter this socket to can_id {can_id:#x}: {}",
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// Set one of the raw socket's flags, naming it if the kernel refuses.
fn set_flag(what: &str, fd: RawFd, option: libc::c_int, to: libc::c_int, name: &str) -> TcsResult<()> {
    // SAFETY: an int option of the size the kernel expects, living for the
    // call.
    let set = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_CAN_RAW,
            option,
            &to as *const libc::c_int as *const libc::c_void,
            mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };

    if set < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: this socket will not take {name}: {}",
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// Bind the socket to the interface, which is what puts it on that bus.
fn bind_to(what: &str, fd: RawFd, interface: &str, index: libc::c_uint) -> TcsResult<()> {
    // SAFETY: zeroed and then filled in; sockaddr_can is the address a CAN
    // socket is bound with, and only its family and interface index are read
    // for a raw socket.
    let mut addr: libc::sockaddr_can = unsafe { mem::zeroed() };
    addr.can_family = libc::AF_CAN as libc::sa_family_t;
    addr.can_ifindex = index as libc::c_int;

    // SAFETY: the address is of the length given and lives for the call.
    let bound = unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_can as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_can>() as libc::socklen_t,
        )
    };

    if bound < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot bind a CAN socket to \"{interface}\": {}",
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// A socket a conduit can poll and then read without waiting.
fn set_nonblocking(what: &str, fd: RawFd) -> TcsResult<()> {
    // SAFETY: both calls take the descriptor by value and return a status.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot make this CAN socket non-blocking: {}",
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// What a frame carries, as the bytes a conduit moves.
///
/// Classic and FD frames differ in their structure and in the length field's
/// name, and in nothing else that matters here: an identifier, a length, and
/// that many bytes.
fn data_of(frame: &[u8], fd: bool) -> Option<(u32, usize)> {
    if fd {
        if frame.len() < mem::size_of::<libc::canfd_frame>() {
            return None;
        }
        // SAFETY: the buffer is at least as long as the structure, which is
        // what the kernel wrote into it.
        let got: libc::canfd_frame = unsafe { std::ptr::read_unaligned(frame.as_ptr() as *const _) };
        Some((got.can_id, got.len as usize))
    } else {
        if frame.len() < mem::size_of::<libc::can_frame>() {
            return None;
        }
        // SAFETY: as above.
        let got: libc::can_frame = unsafe { std::ptr::read_unaligned(frame.as_ptr() as *const _) };
        Some((got.can_id, got.can_dlc as usize))
    }
}

/// Where a frame's data begins, which is the same offset in both structures:
/// the identifier, the length, and the padding before the device.
const DATA_AT: usize = 8;

impl EndpointWaitable for CanEndpoint {
    fn io_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for CanEndpoint {
    /// One frame's data, or nothing.
    ///
    /// A frame at a time, because that is what the socket delivers: a read
    /// gets one whole frame or none of one, so there is no partial frame to
    /// keep and no stream rule to say where a read ends.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        let mut frame = [0u8; mem::size_of::<libc::canfd_frame>()];
        let room = if self.fd {
            mem::size_of::<libc::canfd_frame>()
        } else {
            mem::size_of::<libc::can_frame>()
        };

        // SAFETY: reading into a buffer of at least `room` bytes.
        let got = unsafe { libc::read(self.io_fd(), frame.as_mut_ptr() as *mut libc::c_void, room) };
        if got < 0 {
            let e = io::Error::last_os_error();
            return match e.kind() {
                io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(TcsError::Io(e)),
            };
        }

        let Some((_, length)) = data_of(&frame[..got as usize], self.fd) else {
            // A frame shorter than the structure is not a frame. Counted as
            // nothing read rather than reported, since the bus may carry a
            // kind this socket was not set up for.
            return Ok(0);
        };

        let length = length.min(self.frame_bytes()).min(buffer.len());
        buffer[..length].copy_from_slice(&frame[DATA_AT..DATA_AT + length]);
        Ok(length)
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
                "{}: {} bytes will not fit in a {} frame, which carries {room}, and \
                 CAN does not fragment",
                self.what,
                data.len(),
                if self.fd { "CAN FD" } else { "classic CAN" }
            )));
        }

        let id = if tcslibgs::node::can::is_extended(self.can_id) {
            self.can_id | libc::CAN_EFF_FLAG
        } else {
            self.can_id
        };

        let mut frame = [0u8; mem::size_of::<libc::canfd_frame>()];
        frame[..4].copy_from_slice(&id.to_ne_bytes());
        frame[4] = data.len() as u8;
        frame[DATA_AT..DATA_AT + data.len()].copy_from_slice(data);

        let length = if self.fd {
            mem::size_of::<libc::canfd_frame>()
        } else {
            mem::size_of::<libc::can_frame>()
        };

        // SAFETY: writing `length` bytes from a buffer at least that long,
        // laid out as the kernel's frame structure.
        let sent = unsafe { libc::write(self.io_fd(), frame.as_ptr() as *const libc::c_void, length) };
        if sent < 0 {
            let e = io::Error::last_os_error();
            return match e.kind() {
                io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(TcsError::Io(e)),
            };
        }

        // What the device got, not what the frame took: a conduit counts the
        // bytes it moved, and the padding is the kernel's business.
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::OpenOptions;

    /// The data of both frames begins where this writes it.
    ///
    /// The one piece of the kernel's layout this module relies on: an
    /// identifier, a length, and the data eight bytes in. A structure that
    /// moved it would have this reading the length as data and the data as
    /// nothing.
    #[test]
    fn a_frames_data_begins_where_the_kernel_puts_it() {
        assert_eq!(mem::size_of::<libc::can_frame>(), 16);
        assert_eq!(mem::size_of::<libc::canfd_frame>(), 72);

        // Built as `write` builds one, and read back as `read` reads one.
        let mut frame = [0u8; mem::size_of::<libc::canfd_frame>()];
        frame[..4].copy_from_slice(&0x123u32.to_ne_bytes());
        frame[4] = 3;
        frame[DATA_AT..DATA_AT + 3].copy_from_slice(&[0xDE, 0xAD, 0xBE]);

        let (id, length) = data_of(&frame, false).expect("a classic frame");
        assert_eq!(id, 0x123);
        assert_eq!(length, 3);
        assert_eq!(&frame[DATA_AT..DATA_AT + length], &[0xDE, 0xAD, 0xBE]);

        let (id, length) = data_of(&frame, true).expect("an FD frame");
        assert_eq!(id, 0x123, "the identifier is in the same place either way");
        assert_eq!(length, 3, "and so is the length");
    }

    /// A buffer shorter than a frame is not read as one.
    #[test]
    fn a_short_read_is_not_a_frame() {
        assert_eq!(data_of(&[0u8; 4], false), None);
        assert_eq!(data_of(&[0u8; 16], true), None, "a classic frame is not an FD one");
    }

    /// An interface this host has not got is reported, with the command that
    /// would make one.
    ///
    /// The common failure by a distance: a virtual bus has to be created and
    /// brought up, which needs root, so a device set that runs on one
    /// machine fails on the next -- and the message is the whole of what
    /// tells an operator which it is.
    #[test]
    fn an_interface_that_is_not_there_says_how_to_make_one() {
        let said = interface_index("device", "vcan-nonesuch")
            .expect_err("no such interface");
        let said = format!("{said}");
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
            bitrate: None,
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
            bitrate: None,
            loopback: true,
        };

        let mut handler = CanEndpoint::new("handler", &config).expect("a socket on the bus");
        let mut device = CanEndpoint::new("device", &config).expect("another socket on it");

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

    /// An endpoint whose socket is not a bus at all.
    ///
    /// For the checks that happen before anything is written, which need a
    /// constructed endpoint and not a bus. They were gated on a virtual bus
    /// because they were written beside the test above, and so ran on no
    /// machine that had not been set up by hand -- which is most of them.
    fn not_on_a_bus(fd: bool) -> CanEndpoint {
        CanEndpoint {
            what: "device".to_string(),
            // Writable, because the lengths that are not refused go on to
            // be written: /dev/null takes them and says nothing, which is
            // exactly what is wanted of a descriptor that is not a bus.
            socket: OwnedFd::from(
                OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .expect("a descriptor"),
            ),
            can_id: 1,
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
            "the error should say why it cannot be split, and of which frame: {said}"
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
            Ok(n) => panic!("sixty-five bytes went into a sixty-four-byte frame as {n}"),
        };
        assert!(
            said.contains("CAN FD") && said.contains("64"),
            "the error should say which frame and how much it carries: {said}"
        );
    }

    /// Whether an interface is up, from the flags the kernel keeps for it.
    ///
    /// An interface that exists is not one a frame can go out on: `ip link
    /// add` makes a virtual bus and leaves it down, and a socket bound to a
    /// down bus binds without complaint and then fails the send with
    /// `ENETDOWN`. The operational state says nothing either -- a virtual bus
    /// has no carrier and reports "unknown" whether it is up or not -- so
    /// this reads `IFF_UP` itself.
    fn is_up(interface: &str) -> bool {
        let Ok(flags) = std::fs::read_to_string(format!("/sys/class/net/{interface}/flags")) else {
            return false;
        };
        let Ok(flags) = u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16) else {
            return false;
        };
        flags & libc::IFF_UP as u32 != 0
    }

    /// The first virtual CAN interface this host has up, if it has one.
    fn a_virtual_bus() -> Option<String> {
        (0..4)
            .map(|n| format!("vcan{n}"))
            .find(|name| interface_index("probe", name).is_ok() && is_up(name))
    }
}
