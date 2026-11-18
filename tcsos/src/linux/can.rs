//! A CAN bus through SocketCAN: a raw socket on an interface, filtered to
//! one identifier, and a frame read or written at a time. Linux alone: in a
//! browser, a bus is only ever simulated.
//!
//! What a node or a simulated device does with its frames -- what a packet
//! too long for one is, what a short read counts as -- is its own. This is
//! the kernel's part, which tcspecial's CAN node and tcs-sim's CAN device
//! share.

use std::fs::File;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use crate::linux::wait::{Source, Waitable};

/// How many bytes a classic frame carries.
pub const CLASSIC_BYTES: usize = 8;
/// How many bytes a CAN FD frame carries.
pub const FD_BYTES: usize = 64;

/// Where a frame's data begins, which is the same offset in both of the
/// kernel's structures: the identifier, the length, and the padding before
/// the data.
const DATA_AT: usize = 8;

/// How a socket on a bus is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// The identifier frames are sent with, which is the one they are
    /// filtered on.
    pub can_id: u32,
    /// Whether the identifier is in the extended, 29-bit format.
    pub extended: bool,
    /// Whether frames are CAN FD.
    pub fd: bool,
    /// Whether the socket's own frames loop back to other sockets on the
    /// host, as two programs on one host need.
    pub loopback: bool,
}

/// What could not be done to put a socket on a bus.
#[derive(Debug)]
pub enum Refused {
    /// The interface's name is not one an interface can have.
    Name,
    /// The host has no interface by that name.
    NoInterface(std::io::Error),
    /// No CAN socket could be opened: a kernel without CAN support.
    Socket(std::io::Error),
    /// The socket would not take its filter.
    Filter(std::io::Error),
    /// The socket would not take CAN FD.
    FdFrames(std::io::Error),
    /// The socket would not take loopback being turned off.
    Loopback(std::io::Error),
    /// The socket could not be bound to the interface.
    Bind(std::io::Error),
    /// The socket could not be made non-blocking.
    NonBlocking(std::io::Error),
}

/// The kernel's index for a named interface.
///
/// # Arguments
///
/// * `interface` - Its name, such as `vcan0`.
///
/// # Returns
///
/// Its index.
///
/// # Errors
///
/// [`Refused::Name`] where the name holds a NUL, [`Refused::NoInterface`]
/// where the host has no interface by that name.
pub fn interface_index(interface: &str) -> Result<u32, Refused> {
    let name = std::ffi::CString::new(interface).map_err(|_| Refused::Name)?;
    // SAFETY: the name is a valid C string for the length of the call.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return Err(Refused::NoInterface(std::io::Error::last_os_error()));
    }
    Ok(index)
}

/// Whether an interface is up, from its flags in sysfs.
///
/// # Arguments
///
/// * `interface` - Its name.
///
/// # Returns
///
/// Whether `IFF_UP` is set; `false` where the flags cannot be read.
pub fn is_up(interface: &str) -> bool {
    let Ok(flags) =
        std::fs::read_to_string(format!("/sys/class/net/{interface}/flags"))
    else {
        return false;
    };
    let Ok(flags) =
        u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16)
    else {
        return false;
    };
    flags & libc::IFF_UP as u32 != 0
}

/// Sets one of a raw CAN socket's options.
///
/// # Arguments
///
/// * `socket` - The socket.
/// * `option` - The `CAN_RAW_*` option.
/// * `value`  - Its value, as the option takes it.
///
/// # Errors
///
/// What the kernel said.
fn set_option<T>(
    socket: &OwnedFd,
    option: libc::c_int,
    value: &T,
) -> std::io::Result<()> {
    // SAFETY: an option value of the size the kernel expects for it, living
    // for the call, on a socket `socket` holds open.
    let set = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            libc::SOL_CAN_RAW,
            option,
            value as *const T as *const libc::c_void,
            mem::size_of::<T>() as libc::socklen_t,
        )
    };
    if set < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// A raw CAN socket on a bus.
#[derive(Debug)]
pub struct Bus {
    /// The socket.
    socket: OwnedFd,
    /// The identifier frames are sent with, its extended flag set where it
    /// is extended.
    id: u32,
    /// Whether frames are CAN FD, which decides the structure the kernel
    /// exchanges.
    fd: bool,
}

impl Bus {
    /// Puts a socket on a bus: filtered to its identifier, set to the frames
    /// it carries, bound to the interface and non-blocking.
    ///
    /// One filter, on the identifier and every bit of it. The extended
    /// flag is part of the identifier the kernel compares, so it goes in the
    /// mask as well: without it a base identifier would also match the
    /// extended frame with the same number, which is a different frame.
    ///
    /// # Arguments
    ///
    /// * `interface` - The interface's name.
    /// * `settings`  - How the socket is set up.
    ///
    /// # Returns
    ///
    /// The socket, on the bus.
    ///
    /// # Errors
    ///
    /// What could not be done.
    pub fn join(interface: &str, settings: &Settings) -> Result<Bus, Refused> {
        let index = interface_index(interface)?;
        // SAFETY: a socket call with constant arguments; the result is
        // checked before it is used.
        let fd = unsafe {
            libc::socket(libc::PF_CAN, libc::SOCK_RAW, libc::CAN_RAW)
        };
        if fd < 0 {
            return Err(Refused::Socket(std::io::Error::last_os_error()));
        }
        // From here the descriptor is owned, so every path below closes it.
        //
        // SAFETY: socket has just made this descriptor and nothing else has
        // been given it.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };

        let id = if settings.extended {
            settings.can_id | libc::CAN_EFF_FLAG
        } else {
            settings.can_id
        };
        let filter = libc::can_filter {
            can_id: id,
            can_mask: libc::CAN_EFF_MASK | libc::CAN_EFF_FLAG,
        };
        set_option(&socket, libc::CAN_RAW_FILTER, &filter)
            .map_err(Refused::Filter)?;
        if settings.fd {
            set_option(&socket, libc::CAN_RAW_FD_FRAMES, &(1 as libc::c_int))
                .map_err(Refused::FdFrames)?;
        }
        if !settings.loopback {
            set_option(&socket, libc::CAN_RAW_LOOPBACK, &(0 as libc::c_int))
                .map_err(Refused::Loopback)?;
        }

        // SAFETY: zeroed and then filled in; only the family and the
        // interface index are read for a raw socket.
        let mut addr: libc::sockaddr_can = unsafe { mem::zeroed() };
        addr.can_family = libc::AF_CAN as libc::sa_family_t;
        addr.can_ifindex = index as libc::c_int;
        // SAFETY: the address is of the length given and lives for the call.
        let bound = unsafe {
            libc::bind(
                socket.as_raw_fd(),
                &addr as *const libc::sockaddr_can as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_can>() as libc::socklen_t,
            )
        };
        if bound < 0 {
            return Err(Refused::Bind(std::io::Error::last_os_error()));
        }

        // SAFETY: both calls take the descriptor by value and return a
        // status; `socket` holds it open across them.
        let flags = unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            // SAFETY: as for F_GETFL.
            || unsafe {
                libc::fcntl(
                    socket.as_raw_fd(),
                    libc::F_SETFL,
                    flags | libc::O_NONBLOCK,
                )
            } < 0
        {
            return Err(Refused::NonBlocking(std::io::Error::last_os_error()));
        }

        Ok(Bus {
            socket,
            id,
            fd: settings.fd,
        })
    }

    /// A second handle on the same socket: a second socket would be a
    /// second member of the bus, with its own filter.
    ///
    /// # Returns
    ///
    /// The socket again.
    ///
    /// # Errors
    ///
    /// What the system said where the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> std::io::Result<Bus> {
        Ok(Bus {
            socket: self.socket.try_clone()?,
            id: self.id,
            fd: self.fd,
        })
    }

    /// How many bytes one of this socket's frames carries.
    ///
    /// # Returns
    ///
    /// [`FD_BYTES`] for CAN FD, [`CLASSIC_BYTES`] otherwise.
    pub fn frame_bytes(&self) -> usize {
        if self.fd {
            FD_BYTES
        } else {
            CLASSIC_BYTES
        }
    }

    /// How long the kernel's structure for one of this socket's frames is.
    ///
    /// # Returns
    ///
    /// The size of a `canfd_frame` or a `can_frame`.
    fn structure_bytes(&self) -> usize {
        if self.fd {
            mem::size_of::<libc::canfd_frame>()
        } else {
            mem::size_of::<libc::can_frame>()
        }
    }

    /// Reads one frame's data, without waiting.
    ///
    /// # Arguments
    ///
    /// * `into` - Where the data goes: as much as fits.
    ///
    /// # Returns
    ///
    /// How many bytes it carried, nought where no frame was waiting or what
    /// arrived was shorter than a frame.
    ///
    /// # Errors
    ///
    /// What the kernel said, other than that nothing was waiting.
    pub fn receive(&self, into: &mut [u8]) -> std::io::Result<usize> {
        let mut frame = [0u8; mem::size_of::<libc::canfd_frame>()];
        let room = self.structure_bytes();
        // SAFETY: reading into a buffer of at least `room` bytes, from a
        // socket this holds open.
        let got = unsafe {
            libc::read(
                self.socket.as_raw_fd(),
                frame.as_mut_ptr() as *mut libc::c_void,
                room,
            )
        };
        if got < 0 {
            let e = std::io::Error::last_os_error();
            return match e.kind() {
                std::io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(e),
            };
        }
        if (got as usize) < room {
            // Shorter than the structure is not a frame: the bus may carry a
            // kind this socket was not set up for.
            return Ok(0);
        }
        // The length field, which is in the same place in both structures.
        let length =
            (frame[4] as usize).min(self.frame_bytes()).min(into.len());
        into[..length].copy_from_slice(&frame[DATA_AT..DATA_AT + length]);
        Ok(length)
    }

    /// Writes one frame with this socket's identifier.
    ///
    /// # Arguments
    ///
    /// * `data` - What it carries: no more than [`Bus::frame_bytes`].
    ///
    /// # Returns
    ///
    /// How many bytes of `data` went: all of it, or nought where the socket
    /// had no room.
    ///
    /// # Errors
    ///
    /// [`std::io::ErrorKind::InvalidInput`] where `data` is longer than a
    /// frame carries, and what the kernel said where the write failed.
    pub fn send(&self, data: &[u8]) -> std::io::Result<usize> {
        if data.len() > self.frame_bytes() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "{} bytes will not fit in a frame carrying {}",
                    data.len(),
                    self.frame_bytes()
                ),
            ));
        }
        let mut frame = [0u8; mem::size_of::<libc::canfd_frame>()];
        frame[..4].copy_from_slice(&self.id.to_ne_bytes());
        frame[4] = data.len() as u8;
        frame[DATA_AT..DATA_AT + data.len()].copy_from_slice(data);
        // SAFETY: writing the structure's length from a buffer at least that
        // long, laid out as the kernel's frame structure.
        let sent = unsafe {
            libc::write(
                self.socket.as_raw_fd(),
                frame.as_ptr() as *const libc::c_void,
                self.structure_bytes(),
            )
        };
        if sent < 0 {
            let e = std::io::Error::last_os_error();
            return match e.kind() {
                std::io::ErrorKind::WouldBlock => Ok(0),
                _ => Err(e),
            };
        }
        // What the device got, not what the frame took: the padding is the
        // kernel's business.
        Ok(data.len())
    }

    /// A socket that is not on a bus, on `/dev/null`, for a test of what is
    /// refused before anything is written: lengths that are not refused go
    /// on to be written, and `/dev/null` takes them.
    ///
    /// # Arguments
    ///
    /// * `fd` - Whether its frames are CAN FD.
    ///
    /// # Returns
    ///
    /// The socket.
    ///
    /// # Errors
    ///
    /// What the system said where `/dev/null` cannot be opened.
    #[doc(hidden)]
    pub fn not_on_a_bus(fd: bool) -> std::io::Result<Bus> {
        let null = File::options().write(true).open("/dev/null")?;
        Ok(Bus {
            socket: OwnedFd::from(null),
            id: 1,
            fd,
        })
    }
}

impl Waitable for Bus {
    fn source(&self) -> Source {
        self.socket.source()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kernel's frames are the sizes this reads and writes, and their
    /// data begins where both put it.
    #[test]
    fn a_frames_data_begins_where_the_kernel_puts_it() {
        assert_eq!(mem::size_of::<libc::can_frame>(), 16);
        assert_eq!(mem::size_of::<libc::canfd_frame>(), 72);
        assert_eq!(mem::offset_of!(libc::can_frame, data), DATA_AT);
        assert_eq!(mem::offset_of!(libc::canfd_frame, data), DATA_AT);
        assert_eq!(mem::offset_of!(libc::can_frame, can_dlc), 4);
        assert_eq!(mem::offset_of!(libc::canfd_frame, len), 4);
    }

    /// An interface that is not there is said to be not there.
    #[test]
    fn an_interface_that_is_not_there_is_refused() {
        assert!(matches!(
            interface_index("vcan-nonesuch"),
            Err(Refused::NoInterface(_))
        ));
        assert!(matches!(interface_index("a\0b"), Err(Refused::Name)));
    }
}
