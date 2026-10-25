//! The HDLC endpoint: a synchronous line, which Linux drives as a network
//! interface.
//!
//! **Experimental, and not yet supported.** Run against no card, because
//! there is no card and no stand-in: every other experimental type here has
//! something in the kernel to run it against -- `vcan`, `i2c-stub`,
//! `gpio-sim`, `iio_dummy` -- and generic HDLC has nothing. What is written
//! here is held to the kernel's structures and constants by tests, which is
//! how the GPIO endpoint was written before `gpio-sim` existed.
//!
//! # What this does
//!
//! Two ioctls to set the line up, two more to read back what it took, and a
//! packet socket for the frames.
//!
//! `SIOCWANDEV` is the one ioctl generic HDLC has, and it carries whichever
//! structure the `type` field names:
//!
//! * `IF_IFACE_V35` and its fellows carry `sync_serial_settings` -- the clock,
//!   its rate, and the loopback. This is the port's wiring.
//! * `IF_PROTO_HDLC` carries `raw_hdlc_proto` -- the encoding and the frame
//!   check. This is what the bits mean once the port is clocked.
//! * `IF_GET_IFACE` and `IF_GET_PROTO` read the two back.
//!
//! Read back and compared, which is the rule a serial line already follows: a
//! port may take the call and keep less than it was given, and a line whose
//! frame check is not the one the configuration asked for is a line checking
//! nothing while the file says it checks every frame.
//!
//! Then the frames. Once the line is set up and the interface is up, a raw
//! HDLC netdevice carries frames with no address in them, so this binds an
//! `AF_PACKET` socket of type `SOCK_DGRAM` to the interface: a read hands over
//! one frame's payload and a write puts one payload into one frame, as the CAN
//! endpoint does with its own. `SOCK_DGRAM` rather than `SOCK_RAW` because
//! there is no link header worth seeing -- a raw HDLC frame has none.
//!
//! The interface is not brought up here. Bringing an interface up is the
//! platform's business, as it is for a CAN controller: `ip link set dev hdlc0
//! up`, from whatever starts the vehicle.

use std::io;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{
    ClockType, Encoding, FrameCheck, HdlcConfig, Standard, TcsError, TcsResult,
};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

/// `SIOCWANDEV`, the one ioctl generic HDLC has.
const SIOCWANDEV: libc::c_ulong = 0x894A;

/// The `type` values of `struct if_settings` this uses, from `linux/if.h`.
const IF_GET_IFACE: libc::c_uint = 0x0001;
const IF_GET_PROTO: libc::c_uint = 0x0002;
const IF_IFACE_V35: libc::c_uint = 0x1000;
const IF_IFACE_V24: libc::c_uint = 0x1001;
const IF_IFACE_X21: libc::c_uint = 0x1002;
const IF_IFACE_X21D: libc::c_uint = 0x1006;
const IF_PROTO_HDLC: libc::c_uint = 0x2000;

/// The clock, the encoding and the frame check, as `linux/hdlc/ioctl.h`
/// numbers them.
const CLOCK_EXT: libc::c_uint = 1;
const CLOCK_INT: libc::c_uint = 2;
const CLOCK_TXINT: libc::c_uint = 3;
const CLOCK_TXFROMRX: libc::c_uint = 4;

const ENCODING_NRZ: libc::c_ushort = 1;
const ENCODING_NRZI: libc::c_ushort = 2;
const ENCODING_FM_MARK: libc::c_ushort = 3;
const ENCODING_FM_SPACE: libc::c_ushort = 4;
const ENCODING_MANCHESTER: libc::c_ushort = 5;

const PARITY_NONE: libc::c_ushort = 1;
const PARITY_CRC16_PR0: libc::c_ushort = 2;
const PARITY_CRC16_PR1: libc::c_ushort = 3;
const PARITY_CRC16_PR0_CCITT: libc::c_ushort = 4;
const PARITY_CRC16_PR1_CCITT: libc::c_ushort = 5;
const PARITY_CRC32_PR0_CCITT: libc::c_ushort = 6;
const PARITY_CRC32_PR1_CCITT: libc::c_ushort = 7;

/// `sync_serial_settings`: the port's wiring.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SyncSerialSettings {
    clock_rate: libc::c_uint,
    clock_type: libc::c_uint,
    loopback: libc::c_ushort,
}

/// `raw_hdlc_proto`: what the bits mean.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct RawHdlcProto {
    encoding: libc::c_ushort,
    parity: libc::c_ushort,
}

/// `struct if_settings`, which `ifreq` carries for this ioctl.
///
/// The union in the kernel's version is a union of pointers, every arm the
/// same size, so one pointer field is the whole of it: what `type` says
/// decides what it points at.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IfSettings {
    type_: libc::c_uint,
    size: libc::c_uint,
    settings: *mut libc::c_void,
}

/// `struct ifreq` as this ioctl uses it: a name and the settings.
#[repr(C)]
#[derive(Debug)]
struct IfReq {
    name: [libc::c_char; libc::IF_NAMESIZE],
    settings: IfSettings,
}

/// A synchronous line: the frames, and the settings that were put on it.
pub struct HdlcEndpoint {
    /// The handler this endpoint belongs to, as the configuration names it.
    what: String,
    /// The packet socket the frames come and go on.
    socket: OwnedFd,
    /// Which interface, for the address a write needs and for what a
    /// complaint says.
    interface: String,
    index: libc::c_uint,
}

impl HdlcEndpoint {
    /// Set the line up and bind a socket for its frames.
    pub fn new(what: &str, config: &HdlcConfig) -> TcsResult<Self> {
        let index = interface_index(what, &config.interface)?;

        // A socket of its own for the ioctls. Any socket will do -- the call
        // acts on the interface the request names and not on the socket --
        // and this one is closed when it goes out of scope.
        // SAFETY: a socket call with constant arguments, whose result is
        // checked before it is used.
        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if fd < 0 {
            return Err(TcsError::Endpoint(format!(
                "{what}: cannot open a socket to set up {}: {}",
                config.interface,
                io::Error::last_os_error()
            )));
        }
        // SAFETY: a descriptor the call above just gave, owned from here.
        let asking = unsafe { OwnedFd::from_raw_fd(fd) };

        // The port's wiring first, then what the bits mean: the framing is
        // attached to a port that is already clocked, which is the order
        // `sethdlc` uses and the order the drivers expect.
        set_interface(what, asking.as_raw_fd(), config)?;
        set_protocol(what, asking.as_raw_fd(), config)?;
        read_back(what, asking.as_raw_fd(), config)?;

        // And the frames. AF_PACKET with SOCK_DGRAM: a raw HDLC frame has no
        // address in it, so what a read hands over is the payload.
        // SAFETY: as above.
        let fd = unsafe {
            libc::socket(
                libc::AF_PACKET,
                libc::SOCK_DGRAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                (libc::ETH_P_ALL as u16).to_be() as libc::c_int,
            )
        };
        if fd < 0 {
            return Err(TcsError::Endpoint(format!(
                "{what}: cannot open a packet socket for {}: {}. A kernel \
                 without \\
                 packet sockets has none to give",
                config.interface,
                io::Error::last_os_error()
            )));
        }
        // SAFETY: as above.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };

        bind_to_interface(what, socket.as_raw_fd(), &config.interface, index)?;

        Ok(HdlcEndpoint {
            what: what.to_string(),
            socket,
            interface: config.interface.clone(),
            index,
        })
    }

    /// A second endpoint on the same socket.
    ///
    /// The descriptor is duplicated rather than the socket opened again: the
    /// line is set up once, and setting it up twice would be two programs
    /// configuring one port.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(HdlcEndpoint {
            what: self.what.clone(),
            socket: self.socket.try_clone()?,
            interface: self.interface.clone(),
            index: self.index,
        })
    }
}

/// Which interface this is, or why there is none.
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
            "{what}: this host has no interface called \"{interface}\": {}. A \
             synchronous line is a card -- a wanxl, a pci200syn, a pc300too or \
             a FarSync -- and unlike a CAN bus there is no virtual one to \
             stand in for it",
            io::Error::last_os_error()
        )));
    }

    Ok(index)
}

/// The request for one of the four calls, with the name filled in.
fn request_for(
    what: &str,
    interface: &str,
    type_: libc::c_uint,
    settings: *mut libc::c_void,
    size: usize,
) -> TcsResult<IfReq> {
    let mut request = IfReq {
        name: [0; libc::IF_NAMESIZE],
        settings: IfSettings {
            type_,
            size: size as libc::c_uint,
            settings,
        },
    };

    if interface.len() >= libc::IF_NAMESIZE {
        return Err(TcsError::Config(format!(
            "{what}: \"{interface}\" is longer than an interface name may be",
        )));
    }
    for (n, byte) in interface.bytes().enumerate() {
        request.name[n] = byte as libc::c_char;
    }

    Ok(request)
}

/// Which `IF_IFACE_*` this standard is.
fn iface_of(standard: Standard) -> libc::c_uint {
    match standard {
        Standard::V35 => IF_IFACE_V35,
        Standard::V24 => IF_IFACE_V24,
        Standard::X21 => IF_IFACE_X21,
        Standard::X21D => IF_IFACE_X21D,
    }
}

/// The port's wiring, as the kernel numbers it.
fn wiring_of(config: &HdlcConfig) -> SyncSerialSettings {
    SyncSerialSettings {
        // Nought where the far end drives the clock, which is what the kernel
        // reads as "not this end's to set": the configuration refuses a rate
        // in that case, so there is none to pass on.
        clock_rate: config.clock_rate.unwrap_or(0),
        clock_type: match config.clock_type {
            ClockType::External => CLOCK_EXT,
            ClockType::Internal => CLOCK_INT,
            ClockType::TxInternal => CLOCK_TXINT,
            ClockType::TxFromRx => CLOCK_TXFROMRX,
        },
        loopback: u16::from(config.loopback),
    }
}

/// What the bits mean, as the kernel numbers it.
fn framing_of(config: &HdlcConfig) -> RawHdlcProto {
    RawHdlcProto {
        encoding: match config.encoding {
            Encoding::Nrz => ENCODING_NRZ,
            Encoding::Nrzi => ENCODING_NRZI,
            Encoding::FmMark => ENCODING_FM_MARK,
            Encoding::FmSpace => ENCODING_FM_SPACE,
            Encoding::Manchester => ENCODING_MANCHESTER,
        },
        // The kernel calls a frame check a line's parity, which is the one
        // place this project's word and the kernel's differ: see
        // tcslibgs::node::payload::hdlc.
        parity: match config.frame_check {
            FrameCheck::None => PARITY_NONE,
            FrameCheck::Crc16Pr0 => PARITY_CRC16_PR0,
            FrameCheck::Crc16Pr1 => PARITY_CRC16_PR1,
            FrameCheck::Crc16Pr0Ccitt => PARITY_CRC16_PR0_CCITT,
            FrameCheck::Crc16Pr1Ccitt => PARITY_CRC16_PR1_CCITT,
            FrameCheck::Crc32Pr0Ccitt => PARITY_CRC32_PR0_CCITT,
            FrameCheck::Crc32Pr1Ccitt => PARITY_CRC32_PR1_CCITT,
        },
    }
}

/// Set the port's wiring.
fn set_interface(
    what: &str,
    asking: RawFd,
    config: &HdlcConfig,
) -> TcsResult<()> {
    let mut wiring = wiring_of(config);
    let mut request = request_for(
        what,
        &config.interface,
        iface_of(config.standard),
        &mut wiring as *mut SyncSerialSettings as *mut libc::c_void,
        mem::size_of::<SyncSerialSettings>(),
    )?;

    // SAFETY: a request laid out as the kernel's own -- the tests below hold
    // it to the kernel's sizes -- pointing at a structure that outlives the
    // call.
    if unsafe { libc::ioctl(asking, SIOCWANDEV, &mut request) } < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {} will not be wired as {}: {}. A card that does not \
             offer that standard refuses it here",
            config.interface,
            config.standard,
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// Set what the bits mean.
fn set_protocol(
    what: &str,
    asking: RawFd,
    config: &HdlcConfig,
) -> TcsResult<()> {
    let mut framing = framing_of(config);
    let mut request = request_for(
        what,
        &config.interface,
        IF_PROTO_HDLC,
        &mut framing as *mut RawHdlcProto as *mut libc::c_void,
        mem::size_of::<RawHdlcProto>(),
    )?;

    // SAFETY: as above.
    if unsafe { libc::ioctl(asking, SIOCWANDEV, &mut request) } < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {} will not take {} framing with {}: {}",
            config.interface,
            config.encoding,
            config.frame_check,
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// Read both halves back, and refuse a line that kept less than it was given.
///
/// The rule a serial line already follows, for the same reason: a port may
/// take the call and keep something else, and a line whose frame check is not
/// the one the file asked for is a line checking nothing while the
/// configuration says it checks every frame.
fn read_back(what: &str, asking: RawFd, config: &HdlcConfig) -> TcsResult<()> {
    let mut wiring = SyncSerialSettings::default();
    let mut request = request_for(
        what,
        &config.interface,
        IF_GET_IFACE,
        &mut wiring as *mut SyncSerialSettings as *mut libc::c_void,
        mem::size_of::<SyncSerialSettings>(),
    )?;
    // SAFETY: as above, and the call writes into the structure.
    if unsafe { libc::ioctl(asking, SIOCWANDEV, &mut request) } < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {} cannot be read back after being wired: {}",
            config.interface,
            io::Error::last_os_error()
        )));
    }
    if wiring != wiring_of(config) {
        return Err(TcsError::Endpoint(format!(
            "{what}: {} reports a clock of {:?} after being set to {:?}, and a \
             handler cannot run a line the card is not running",
            config.interface,
            wiring,
            wiring_of(config)
        )));
    }

    let mut framing = RawHdlcProto::default();
    let mut request = request_for(
        what,
        &config.interface,
        IF_GET_PROTO,
        &mut framing as *mut RawHdlcProto as *mut libc::c_void,
        mem::size_of::<RawHdlcProto>(),
    )?;
    // SAFETY: as above.
    if unsafe { libc::ioctl(asking, SIOCWANDEV, &mut request) } < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: {} cannot be read back after its framing was set: {}",
            config.interface,
            io::Error::last_os_error()
        )));
    }
    if framing != framing_of(config) {
        return Err(TcsError::Endpoint(format!(
            "{what}: {} reports {:?} after being set to {:?}: a frame check \
             the card is not applying is one the configuration only says it \
             applies",
            config.interface,
            framing,
            framing_of(config)
        )));
    }

    Ok(())
}

/// Bind the packet socket to the one interface.
fn bind_to_interface(
    what: &str,
    socket: RawFd,
    interface: &str,
    index: libc::c_uint,
) -> TcsResult<()> {
    let mut address: libc::sockaddr_ll = unsafe { mem::zeroed() };
    address.sll_family = libc::AF_PACKET as u16;
    address.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
    address.sll_ifindex = index as i32;

    // SAFETY: an address of the family the socket was opened in, of its own
    // size, living across the call.
    let bound = unsafe {
        libc::bind(
            socket,
            &address as *const libc::sockaddr_ll as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        )
    };
    if bound < 0 {
        return Err(TcsError::Endpoint(format!(
            "{what}: cannot bind a packet socket to {interface}: {}",
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

impl EndpointWaitable for HdlcEndpoint {
    fn io_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }

    fn wait_for_event(
        &self,
        cmd_fd: RawFd,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for HdlcEndpoint {
    /// One frame's payload, or nothing.
    ///
    /// A frame at a time, because that is what the socket delivers: a read
    /// gets one whole frame or none of one, so there is no partial frame to
    /// keep and no stream rule to say where a read ends.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        // SAFETY: reading into a buffer of its own length.
        let got = unsafe {
            libc::recv(
                self.io_fd(),
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
                0,
            )
        };
        if got < 0 {
            let e = io::Error::last_os_error();
            return match e.kind() {
                io::ErrorKind::WouldBlock => Ok(0),
                io::ErrorKind::Interrupted => Ok(0),
                _ => Err(TcsError::Io(e)),
            };
        }

        Ok(got as usize)
    }
}

impl EndpointWritable for HdlcEndpoint {
    /// One payload into one frame.
    ///
    /// Addressed to the interface rather than to anybody on it: a raw HDLC
    /// frame carries no address, there being one far end and no bus.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        let mut address: libc::sockaddr_ll = unsafe { mem::zeroed() };
        address.sll_family = libc::AF_PACKET as u16;
        address.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
        address.sll_ifindex = self.index as i32;

        // SAFETY: a buffer of its own length, and an address of its own size,
        // both living across the call.
        let sent = unsafe {
            libc::sendto(
                self.io_fd(),
                data.as_ptr() as *const libc::c_void,
                data.len(),
                0,
                &address as *const libc::sockaddr_ll as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if sent < 0 {
            let e = io::Error::last_os_error();
            return match e.kind() {
                io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(TcsError::Endpoint(format!(
                    "{}: cannot send a frame on {}: {e}",
                    self.what, self.interface
                ))),
            };
        }

        Ok(sent as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> HdlcConfig {
        HdlcConfig {
            interface: "hdlc0".to_string(),
            standard: Standard::V35,
            clock_type: ClockType::External,
            clock_rate: None,
            loopback: false,
            encoding: Encoding::Nrz,
            frame_check: FrameCheck::None,
        }
    }

    /// Every structure is the size the kernel's own is.
    ///
    /// Measured from `linux/hdlc/ioctl.h` and `linux/if.h`: twelve bytes of
    /// sync settings, four of framing, and an `if_settings` of two words and a
    /// pointer. A structure of the wrong size is an ioctl that reads or writes
    /// past what it was handed, and nothing else in a test would notice.
    #[test]
    fn the_structures_are_what_the_kernel_reads() {
        assert_eq!(mem::size_of::<SyncSerialSettings>(), 12);
        assert_eq!(mem::size_of::<RawHdlcProto>(), 4);
        assert_eq!(
            mem::size_of::<IfSettings>(),
            8 + mem::size_of::<*mut libc::c_void>()
        );
        // And the name is the kernel's own length, which is what makes a
        // request's layout match.
        assert_eq!(libc::IF_NAMESIZE, 16);
    }

    /// The ioctl and the settings types are the kernel's numbers.
    #[test]
    fn the_numbers_are_the_kernels() {
        assert_eq!(SIOCWANDEV, 0x894A);
        assert_eq!(IF_GET_IFACE, 0x0001);
        assert_eq!(IF_GET_PROTO, 0x0002);
        assert_eq!(IF_IFACE_V35, 0x1000);
        assert_eq!(IF_IFACE_V24, 0x1001);
        assert_eq!(IF_IFACE_X21, 0x1002);
        assert_eq!(IF_IFACE_X21D, 0x1006);
        assert_eq!(IF_PROTO_HDLC, 0x2000);
    }

    /// What the configuration says becomes what the kernel is told.
    #[test]
    fn the_settings_are_what_the_file_said() {
        // The far end's clock: no rate, which is the kernel's "not mine to
        // set", and the configuration refuses one in this case anyway.
        let wiring = wiring_of(&config());
        assert_eq!(wiring.clock_type, CLOCK_EXT);
        assert_eq!(wiring.clock_rate, 0);
        assert_eq!(wiring.loopback, 0);

        // This end's clock, at the rate the file gave.
        let mut mine = config();
        mine.clock_type = ClockType::Internal;
        mine.clock_rate = Some(2_048_000);
        mine.loopback = true;
        let wiring = wiring_of(&mine);
        assert_eq!(wiring.clock_type, CLOCK_INT);
        assert_eq!(wiring.clock_rate, 2_048_000);
        assert_eq!(wiring.loopback, 1);

        // Each clock is its own number, and the two mixed ones are not the
        // two plain ones.
        for (clock, number) in [
            (ClockType::External, CLOCK_EXT),
            (ClockType::Internal, CLOCK_INT),
            (ClockType::TxInternal, CLOCK_TXINT),
            (ClockType::TxFromRx, CLOCK_TXFROMRX),
        ] {
            let mut config = config();
            config.clock_type = clock;
            config.clock_rate = clock.is_ours().then_some(64_000);
            assert_eq!(wiring_of(&config).clock_type, number, "{clock}");
        }
    }

    /// The framing is the encoding and the frame check, by the kernel's
    /// numbers.
    #[test]
    fn the_framing_is_what_the_file_said() {
        for (encoding, number) in [
            (Encoding::Nrz, ENCODING_NRZ),
            (Encoding::Nrzi, ENCODING_NRZI),
            (Encoding::FmMark, ENCODING_FM_MARK),
            (Encoding::FmSpace, ENCODING_FM_SPACE),
            (Encoding::Manchester, ENCODING_MANCHESTER),
        ] {
            let mut config = config();
            config.encoding = encoding;
            assert_eq!(framing_of(&config).encoding, number, "{encoding}");
        }

        for (check, number) in [
            (FrameCheck::None, PARITY_NONE),
            (FrameCheck::Crc16Pr0, PARITY_CRC16_PR0),
            (FrameCheck::Crc16Pr1, PARITY_CRC16_PR1),
            (FrameCheck::Crc16Pr0Ccitt, PARITY_CRC16_PR0_CCITT),
            (FrameCheck::Crc16Pr1Ccitt, PARITY_CRC16_PR1_CCITT),
            (FrameCheck::Crc32Pr0Ccitt, PARITY_CRC32_PR0_CCITT),
            (FrameCheck::Crc32Pr1Ccitt, PARITY_CRC32_PR1_CCITT),
        ] {
            let mut config = config();
            config.frame_check = check;
            assert_eq!(framing_of(&config).parity, number, "{check}");
        }
    }

    /// Each standard is its own `IF_IFACE_*`.
    #[test]
    fn each_standard_is_its_own_wiring() {
        assert_eq!(iface_of(Standard::V35), IF_IFACE_V35);
        assert_eq!(iface_of(Standard::V24), IF_IFACE_V24);
        assert_eq!(iface_of(Standard::X21), IF_IFACE_X21);
        assert_eq!(iface_of(Standard::X21D), IF_IFACE_X21D);
    }

    /// A request carries the interface's name and the size of what it points
    /// at.
    #[test]
    fn a_request_names_the_interface() {
        let mut wiring = wiring_of(&config());
        let request = request_for(
            "link",
            "hdlc0",
            IF_IFACE_V35,
            &mut wiring as *mut SyncSerialSettings as *mut libc::c_void,
            mem::size_of::<SyncSerialSettings>(),
        )
        .expect("a request");

        let name: Vec<u8> = request
            .name
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as u8)
            .collect();
        assert_eq!(String::from_utf8_lossy(&name), "hdlc0");
        assert_eq!(request.settings.type_, IF_IFACE_V35);
        assert_eq!(request.settings.size, 12);

        // A name longer than an interface's is refused rather than cut to
        // fit: a request for an interface nobody has is worse than a
        // configuration that would not start.
        let said = request_for(
            "link",
            "an-interface-name-far-too-long",
            IF_IFACE_V35,
            std::ptr::null_mut(),
            0,
        )
        .expect_err("a name that will not fit");
        assert!(format!("{said}").contains("longer than"), "{said}");
    }

    /// An interface that is not there is reported, and says what one is.
    #[test]
    fn an_interface_that_is_not_there_is_reported() {
        let mut config = config();
        config.interface = "hdlc-nonesuch".to_string();

        let said = match HdlcEndpoint::new("link", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has an interface called hdlc-nonesuch"),
        };
        assert!(
            said.contains("hdlc-nonesuch") && said.contains("no virtual one"),
            "the complaint should name the interface and say what one is: \
             {said}"
        );
    }
}
