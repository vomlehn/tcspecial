//! A synchronous line, which Linux drives as a network interface through
//! generic HDLC: its wiring and framing set and read back with
//! `SIOCWANDEV`, and its frames over a packet socket. Linux alone: in a
//! browser, a line is only ever simulated.
//!
//! `SIOCWANDEV` is the one ioctl generic HDLC has, and it carries whichever
//! structure the `type` field names: `IF_IFACE_*` carries the port's wiring,
//! `IF_PROTO_HDLC` what the bits mean, and `IF_GET_IFACE` and `IF_GET_PROTO`
//! read the two back. A port may take a call and keep less than it was
//! given, so both are read back and compared.

use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use crate::linux::wait::{Source, Waitable};

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

/// The line's electrical standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standard {
    /// V.35.
    V35,
    /// V.24.
    V24,
    /// X.21.
    X21,
    /// X.21 with dual clocks.
    X21D,
}

/// Where the line's clock comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// The far end drives it.
    External,
    /// This end does.
    Internal,
    /// This end drives the transmit clock, the far end the receive.
    TxInternal,
    /// The transmit clock is taken from the receive clock.
    TxFromRx,
}

/// How bits are put on the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// NRZ.
    Nrz,
    /// NRZI.
    Nrzi,
    /// FM, mark.
    FmMark,
    /// FM, space.
    FmSpace,
    /// Manchester.
    Manchester,
}

/// The check on each frame: the kernel calls it the line's parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameCheck {
    /// None.
    None,
    /// CRC-16, preset to nought.
    Crc16Pr0,
    /// CRC-16, preset to one.
    Crc16Pr1,
    /// CRC-16 CCITT, preset to nought.
    Crc16Pr0Ccitt,
    /// CRC-16 CCITT, preset to one.
    Crc16Pr1Ccitt,
    /// CRC-32 CCITT, preset to nought.
    Crc32Pr0Ccitt,
    /// CRC-32 CCITT, preset to one.
    Crc32Pr1Ccitt,
}

/// The port's wiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wiring {
    /// Its standard.
    pub standard: Standard,
    /// Where its clock comes from.
    pub clock: Clock,
    /// The clock's rate, nought where the far end drives it.
    pub clock_rate: u32,
    /// Whether the line loops back.
    pub loopback: bool,
}

/// What the bits mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Framing {
    /// How they are encoded.
    pub encoding: Encoding,
    /// The check on each frame.
    pub frame_check: FrameCheck,
}

/// `sync_serial_settings`: the port's wiring, as the kernel holds it.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SyncSerialSettings {
    /// The clock's rate.
    clock_rate: libc::c_uint,
    /// Where the clock comes from.
    clock_type: libc::c_uint,
    /// Whether the line loops back.
    loopback: libc::c_ushort,
}

/// `raw_hdlc_proto`: what the bits mean, as the kernel holds it.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct RawHdlcProto {
    /// The encoding.
    encoding: libc::c_ushort,
    /// The frame check.
    parity: libc::c_ushort,
}

/// `struct if_settings`, which `ifreq` carries for this ioctl. The union in
/// the kernel's version is a union of pointers, every arm the same size, so
/// one pointer field is the whole of it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IfSettings {
    /// What `settings` points at.
    type_: libc::c_uint,
    /// Its size.
    size: libc::c_uint,
    /// The structure.
    settings: *mut libc::c_void,
}

/// `struct ifreq` as this ioctl uses it: a name and the settings.
#[repr(C)]
#[derive(Debug)]
struct IfReq {
    /// The interface's name, NUL terminated.
    name: [libc::c_char; libc::IF_NAMESIZE],
    /// The settings.
    settings: IfSettings,
}

/// What could not be done to open a line.
#[derive(Debug)]
pub enum Refused {
    /// The interface's name is not one an interface can have.
    Name,
    /// The interface's name is longer than one may be.
    TooLong,
    /// The host has no interface by that name.
    NoInterface(std::io::Error),
    /// No socket to make the ioctls through could be opened.
    Asking(std::io::Error),
    /// The card refused the wiring.
    Wiring(std::io::Error),
    /// The card refused the framing.
    Framing(std::io::Error),
    /// The wiring could not be read back.
    ReadWiring(std::io::Error),
    /// The card kept other wiring than it was given: what it reports.
    WiringKept(String),
    /// The framing could not be read back.
    ReadFraming(std::io::Error),
    /// The card kept other framing than it was given: what it reports.
    FramingKept(String),
    /// No packet socket could be opened.
    PacketSocket(std::io::Error),
    /// The packet socket could not be bound to the interface.
    Bind(std::io::Error),
}

/// The `IF_IFACE_*` a standard is.
fn iface_of(standard: Standard) -> libc::c_uint {
    match standard {
        Standard::V35 => IF_IFACE_V35,
        Standard::V24 => IF_IFACE_V24,
        Standard::X21 => IF_IFACE_X21,
        Standard::X21D => IF_IFACE_X21D,
    }
}

/// The port's wiring, as the kernel numbers it.
fn wiring_of(wiring: &Wiring) -> SyncSerialSettings {
    SyncSerialSettings {
        clock_rate: wiring.clock_rate,
        clock_type: match wiring.clock {
            Clock::External => CLOCK_EXT,
            Clock::Internal => CLOCK_INT,
            Clock::TxInternal => CLOCK_TXINT,
            Clock::TxFromRx => CLOCK_TXFROMRX,
        },
        loopback: u16::from(wiring.loopback),
    }
}

/// What the bits mean, as the kernel numbers it.
fn framing_of(framing: &Framing) -> RawHdlcProto {
    RawHdlcProto {
        encoding: match framing.encoding {
            Encoding::Nrz => ENCODING_NRZ,
            Encoding::Nrzi => ENCODING_NRZI,
            Encoding::FmMark => ENCODING_FM_MARK,
            Encoding::FmSpace => ENCODING_FM_SPACE,
            Encoding::Manchester => ENCODING_MANCHESTER,
        },
        parity: match framing.frame_check {
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

/// Builds a request for one of the four calls, with the name filled in.
///
/// # Arguments
///
/// * `interface` - The interface's name.
/// * `type_`     - What `settings` points at.
/// * `settings`  - The structure the kernel reads or fills, which must
///                 outlive every call made with the request.
/// * `size`      - Its size in bytes.
///
/// # Returns
///
/// The request.
///
/// # Errors
///
/// [`Refused::TooLong`] where the name does not fit with its terminator: a
/// request for an interface nobody has is worse than a line not opened.
fn request_for(
    interface: &str,
    type_: libc::c_uint,
    settings: *mut libc::c_void,
    size: usize,
) -> Result<IfReq, Refused> {
    if interface.len() >= libc::IF_NAMESIZE {
        return Err(Refused::TooLong);
    }
    let mut request = IfReq {
        name: [0; libc::IF_NAMESIZE],
        settings: IfSettings {
            type_,
            size: size as libc::c_uint,
            settings,
        },
    };
    for (n, byte) in interface.bytes().enumerate() {
        request.name[n] = byte as libc::c_char;
    }
    Ok(request)
}

/// Makes one `SIOCWANDEV` call.
///
/// # Arguments
///
/// * `asking`    - Any socket, through which the call is made.
/// * `interface` - The interface's name.
/// * `type_`     - What `settings` is.
/// * `settings`  - The structure, read or filled.
///
/// # Errors
///
/// [`Refused::TooLong`] for a name too long, and what the kernel said, as
/// `Ok(Err)`, where the call failed.
fn wandev<T>(
    asking: &OwnedFd,
    interface: &str,
    type_: libc::c_uint,
    settings: &mut T,
) -> Result<std::io::Result<()>, Refused> {
    let mut request = request_for(
        interface,
        type_,
        settings as *mut T as *mut libc::c_void,
        mem::size_of::<T>(),
    )?;
    // SAFETY: a request laid out as the kernel's own -- the tests below hold
    // it to the kernel's sizes -- pointing at a structure that outlives the
    // call, on a socket `asking` holds open.
    if unsafe { libc::ioctl(asking.as_raw_fd(), SIOCWANDEV, &mut request) } < 0
    {
        return Ok(Err(std::io::Error::last_os_error()));
    }
    Ok(Ok(()))
}

/// A packet socket address for an interface, every frame it carries.
fn address_of(index: u32) -> libc::sockaddr_ll {
    // SAFETY: sockaddr_ll is a C struct of integers and a byte array, for
    // which all zeroes is a valid value.
    let mut address: libc::sockaddr_ll = unsafe { mem::zeroed() };
    address.sll_family = libc::AF_PACKET as u16;
    address.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
    address.sll_ifindex = index as i32;
    address
}

/// A synchronous line, set up, and a packet socket for its frames.
#[derive(Debug)]
pub struct Line {
    /// The packet socket the frames come and go on.
    socket: OwnedFd,
    /// The interface's index, which a frame sent is addressed to.
    index: u32,
}

impl Line {
    /// Sets a line up -- its wiring, then its framing, each read back -- and
    /// binds a packet socket to it for its frames.
    ///
    /// The wiring first, then what the bits mean: the framing is attached to
    /// a port that is already clocked, which is the order `sethdlc` uses and
    /// the order the drivers expect.
    ///
    /// # Arguments
    ///
    /// * `interface` - The interface's name, such as `hdlc0`.
    /// * `wiring`    - The port's wiring.
    /// * `framing`   - What the bits mean.
    ///
    /// # Returns
    ///
    /// The line.
    ///
    /// # Errors
    ///
    /// What could not be done.
    pub fn open(
        interface: &str,
        wiring: &Wiring,
        framing: &Framing,
    ) -> Result<Line, Refused> {
        let name =
            std::ffi::CString::new(interface).map_err(|_| Refused::Name)?;
        // SAFETY: the name is a valid C string for the length of the call.
        let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
        if index == 0 {
            return Err(Refused::NoInterface(std::io::Error::last_os_error()));
        }

        // A socket of its own for the ioctls. Any socket will do: the call
        // acts on the interface the request names.
        // SAFETY: a socket call with constant arguments, checked.
        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if fd < 0 {
            return Err(Refused::Asking(std::io::Error::last_os_error()));
        }
        // SAFETY: a descriptor the call above just gave, owned from here.
        let asking = unsafe { OwnedFd::from_raw_fd(fd) };

        let mut asked = wiring_of(wiring);
        wandev(&asking, interface, iface_of(wiring.standard), &mut asked)?
            .map_err(Refused::Wiring)?;
        let mut asked_framing = framing_of(framing);
        wandev(&asking, interface, IF_PROTO_HDLC, &mut asked_framing)?
            .map_err(Refused::Framing)?;

        let mut got = SyncSerialSettings::default();
        wandev(&asking, interface, IF_GET_IFACE, &mut got)?
            .map_err(Refused::ReadWiring)?;
        if got != wiring_of(wiring) {
            return Err(Refused::WiringKept(format!(
                "a clock of {got:?} after being set to {:?}",
                wiring_of(wiring)
            )));
        }
        let mut got_framing = RawHdlcProto::default();
        wandev(&asking, interface, IF_GET_PROTO, &mut got_framing)?
            .map_err(Refused::ReadFraming)?;
        if got_framing != framing_of(framing) {
            return Err(Refused::FramingKept(format!(
                "{got_framing:?} after being set to {:?}",
                framing_of(framing)
            )));
        }

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
            return Err(Refused::PacketSocket(std::io::Error::last_os_error()));
        }
        // SAFETY: as above.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };
        let address = address_of(index);
        // SAFETY: an address of the family the socket was opened in, of its
        // own size, living across the call.
        let bound = unsafe {
            libc::bind(
                socket.as_raw_fd(),
                &address as *const libc::sockaddr_ll as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if bound < 0 {
            return Err(Refused::Bind(std::io::Error::last_os_error()));
        }
        Ok(Line { socket, index })
    }

    /// A second handle on the same socket: the line is set up once.
    ///
    /// # Returns
    ///
    /// The line again.
    ///
    /// # Errors
    ///
    /// What the system said where the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> std::io::Result<Line> {
        Ok(Line {
            socket: self.socket.try_clone()?,
            index: self.index,
        })
    }

    /// Reads one frame's payload, without waiting.
    ///
    /// # Arguments
    ///
    /// * `into` - Where it goes.
    ///
    /// # Returns
    ///
    /// How many bytes it was: nought where none was waiting.
    ///
    /// # Errors
    ///
    /// What the kernel said, other than that nothing was waiting.
    pub fn receive(&self, into: &mut [u8]) -> std::io::Result<usize> {
        // SAFETY: reading into a buffer of its own length.
        let got = unsafe {
            libc::recv(
                self.socket.as_raw_fd(),
                into.as_mut_ptr() as *mut libc::c_void,
                into.len(),
                0,
            )
        };
        if got < 0 {
            let e = std::io::Error::last_os_error();
            return match e.kind() {
                std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::Interrupted => Ok(0),
                _ => Err(e),
            };
        }
        Ok(got as usize)
    }

    /// Sends one payload in one frame, addressed to the interface rather
    /// than to anybody on it: a raw HDLC frame carries no address.
    ///
    /// # Arguments
    ///
    /// * `data` - The payload.
    ///
    /// # Returns
    ///
    /// How many bytes went: nought where the socket had no room.
    ///
    /// # Errors
    ///
    /// What the kernel said, other than that it had no room.
    pub fn send(&self, data: &[u8]) -> std::io::Result<usize> {
        let address = address_of(self.index);
        // SAFETY: a buffer of its own length, and an address of its own
        // size, both living across the call.
        let sent = unsafe {
            libc::sendto(
                self.socket.as_raw_fd(),
                data.as_ptr() as *const libc::c_void,
                data.len(),
                0,
                &address as *const libc::sockaddr_ll as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };
        if sent < 0 {
            let e = std::io::Error::last_os_error();
            return match e.kind() {
                std::io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(e),
            };
        }
        Ok(sent as usize)
    }
}

impl Waitable for Line {
    fn source(&self) -> Source {
        self.socket.source()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The simplest wiring and framing: V.35, clocked from the far end,
    /// NRZ, no frame check.
    fn simplest() -> (Wiring, Framing) {
        (
            Wiring {
                standard: Standard::V35,
                clock: Clock::External,
                clock_rate: 0,
                loopback: false,
            },
            Framing {
                encoding: Encoding::Nrz,
                frame_check: FrameCheck::None,
            },
        )
    }

    /// Every structure is the size the kernel's own is: twelve bytes of
    /// sync settings, four of framing, and an `if_settings` of two words and
    /// a pointer.
    #[test]
    fn the_structures_are_what_the_kernel_reads() {
        assert_eq!(mem::size_of::<SyncSerialSettings>(), 12);
        assert_eq!(mem::size_of::<RawHdlcProto>(), 4);
        assert_eq!(
            mem::size_of::<IfSettings>(),
            8 + mem::size_of::<*mut libc::c_void>()
        );
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

    /// The wiring is told the kernel by its numbers, each clock its own.
    #[test]
    fn the_wiring_is_the_kernels_numbers() {
        let (mut wiring, _) = simplest();
        assert_eq!(wiring_of(&wiring).clock_type, CLOCK_EXT);
        assert_eq!(wiring_of(&wiring).clock_rate, 0);
        assert_eq!(wiring_of(&wiring).loopback, 0);
        wiring.loopback = true;
        wiring.clock_rate = 2_048_000;
        assert_eq!(wiring_of(&wiring).loopback, 1);
        assert_eq!(wiring_of(&wiring).clock_rate, 2_048_000);
        for (clock, number) in [
            (Clock::External, CLOCK_EXT),
            (Clock::Internal, CLOCK_INT),
            (Clock::TxInternal, CLOCK_TXINT),
            (Clock::TxFromRx, CLOCK_TXFROMRX),
        ] {
            wiring.clock = clock;
            assert_eq!(wiring_of(&wiring).clock_type, number, "{clock:?}");
        }
        assert_eq!(iface_of(Standard::V35), IF_IFACE_V35);
        assert_eq!(iface_of(Standard::V24), IF_IFACE_V24);
        assert_eq!(iface_of(Standard::X21), IF_IFACE_X21);
        assert_eq!(iface_of(Standard::X21D), IF_IFACE_X21D);
    }

    /// The framing is the encoding and the frame check, by the kernel's
    /// numbers.
    #[test]
    fn the_framing_is_the_kernels_numbers() {
        let (_, mut framing) = simplest();
        for (encoding, number) in [
            (Encoding::Nrz, ENCODING_NRZ),
            (Encoding::Nrzi, ENCODING_NRZI),
            (Encoding::FmMark, ENCODING_FM_MARK),
            (Encoding::FmSpace, ENCODING_FM_SPACE),
            (Encoding::Manchester, ENCODING_MANCHESTER),
        ] {
            framing.encoding = encoding;
            assert_eq!(framing_of(&framing).encoding, number, "{encoding:?}");
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
            framing.frame_check = check;
            assert_eq!(framing_of(&framing).parity, number, "{check:?}");
        }
    }

    /// A request carries the interface's name and the size of what it
    /// points at, and a name too long is refused rather than cut.
    #[test]
    fn a_request_names_the_interface() {
        let (wiring, _) = simplest();
        let mut settings = wiring_of(&wiring);
        let request = request_for(
            "hdlc0",
            IF_IFACE_V35,
            &mut settings as *mut SyncSerialSettings as *mut libc::c_void,
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
        assert!(matches!(
            request_for(
                "an-interface-name-far-too-long",
                IF_IFACE_V35,
                std::ptr::null_mut(),
                0
            ),
            Err(Refused::TooLong)
        ));
    }
}
