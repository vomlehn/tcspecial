//! A simulated device on a CAN bus.
//!
//! **Experimental, and not yet supported.** Run against the kernel's virtual
//! bus and against no CAN controller.
//!
//! The one experimental kind whose stand-in is as faithful as a socket. A CAN
//! controller carries every member's frames to every other member, so the
//! simulator does not have to stand in for anything: it opens a socket on the
//! same interface the handler is on, sends the device's frames with the
//! device's identifier, and reads the handler's triggers off the same bus.
//! Both ends are real, which is what lets a triggered device work here where
//! one on an I2C bus cannot -- nothing in user space can answer when a master
//! addresses it, and on a bus of peers there is nothing to answer.
//!
//! What the simulator cannot do is make the bus. A virtual one is created with
//! `ip link add dev vcan0 type vcan` and brought up with `ip link set dev
//! vcan0 up`, both of which need root, so this finds the interface and says
//! what would make one if it is not there -- as [`crate::node::i2c`] does
//! for its kernel module.
//!
//! A frame is its own length and carries eight bytes, or sixty-four as CAN
//! FD. A packet larger than that cannot be sent at all, which the device
//! configuration refuses where the file is read; the segment settings still
//! apply, so a packet may be sent as several frames, each a segment.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::io;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet, ignore_this_request, lateness, send_in_segments, wait_out_packet,
    wait_while_running, Pacing, PayloadConfig, PayloadStats, Produce,
};

/// Where a frame's data begins in both of the kernel's frame structures: the
/// identifier, the length, and the padding before the device.
const DATA_AT: usize = 8;

/// A socket on the bus, as this end of the link.
struct BusMember {
    socket: OwnedFd,
    /// The identifier this device's frames carry, which is also the one the
    /// handler sends its triggers with.
    can_id: u32,
    /// Whether frames are CAN FD, which decides the structure the kernel
    /// exchanges and how much a frame carries.
    fd: bool,
}

impl BusMember {
    /// Join the bus the handler is on, filtered to this device's identifier.
    ///
    /// Filtered for the same reason the handler filters: a bus carries
    /// everybody's frames, and a device that read the lot would answer
    /// triggers meant for another device on the same interface.
    fn join(config: &PayloadConfig) -> Result<Self, String> {
        let interface = config.address.clone();
        let index = index_of(&interface).ok_or_else(|| {
            format!(
                "{}: this host has no interface called \"{interface}\". Make a virtual \
                 bus with \"ip link add dev {interface} type vcan\" and bring it up \
                 with \"ip link set dev {interface} up\"",
                config.name
            )
        })?;

        // SAFETY: a socket call with constant arguments, checked before use.
        let fd = unsafe { libc::socket(libc::PF_CAN, libc::SOCK_RAW, libc::CAN_RAW) };
        if fd < 0 {
            return Err(format!(
                "{}: cannot open a CAN socket: {}",
                config.name,
                io::Error::last_os_error()
            ));
        }
        // Owned from here, so every path below closes it.
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };

        let can_id = config.can_id;
        let extended = can_id > 0x7FF;
        let filter = libc::can_filter {
            can_id: if extended { can_id | libc::CAN_EFF_FLAG } else { can_id },
            can_mask: libc::CAN_EFF_MASK | libc::CAN_EFF_FLAG,
        };
        // SAFETY: one can_filter, which is what CAN_RAW_FILTER reads, living
        // for the call.
        let filtered = unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::SOL_CAN_RAW,
                libc::CAN_RAW_FILTER,
                &filter as *const libc::can_filter as *const libc::c_void,
                mem::size_of::<libc::can_filter>() as libc::socklen_t,
            )
        };
        if filtered < 0 {
            return Err(format!(
                "{}: cannot filter this socket to can_id {can_id:#x}: {}",
                config.name,
                io::Error::last_os_error()
            ));
        }

        if config.can_fd {
            let on: libc::c_int = 1;
            // SAFETY: an int option of the size the kernel expects.
            let set = unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_CAN_RAW,
                    libc::CAN_RAW_FD_FRAMES,
                    &on as *const libc::c_int as *const libc::c_void,
                    mem::size_of::<libc::c_int>() as libc::socklen_t,
                )
            };
            if set < 0 {
                return Err(format!(
                    "{}: this socket will not take CAN FD: {}",
                    config.name,
                    io::Error::last_os_error()
                ));
            }
        }

        // SAFETY: zeroed and filled in; only the family and the interface
        // index are read for a raw CAN socket.
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
            return Err(format!(
                "{}: cannot bind a CAN socket to \"{interface}\": {}",
                config.name,
                io::Error::last_os_error()
            ));
        }

        // Read without waiting: the loop looks for a trigger every pass and
        // sends on its own clock, as every other kind's does.
        // SAFETY: both calls take the descriptor by value.
        let flags = unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                < 0
        {
            return Err(format!(
                "{}: cannot make this CAN socket non-blocking: {}",
                config.name,
                io::Error::last_os_error()
            ));
        }

        Ok(Self {
            socket,
            can_id,
            fd: config.can_fd,
        })
    }

    /// How many bytes one frame carries.
    fn frame_bytes(&self) -> usize {
        if self.fd {
            64
        } else {
            8
        }
    }

    /// Put one segment on the bus as one frame.
    fn send(&self, data: &[u8]) -> io::Result<usize> {
        if data.len() > self.frame_bytes() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} bytes will not fit in a frame carrying {}",
                    data.len(),
                    self.frame_bytes()
                ),
            ));
        }

        let extended = self.can_id > 0x7FF;
        let id = if extended {
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

        // SAFETY: writing `length` bytes of a buffer at least that long, laid
        // out as the kernel's frame.
        let sent = unsafe {
            libc::write(
                self.socket.as_raw_fd(),
                frame.as_ptr() as *const libc::c_void,
                length,
            )
        };
        if sent < 0 {
            return Err(io::Error::last_os_error());
        }

        // The device's bytes, not the frame's: what a simulated device
        // counts is what it sent, and the padding is the kernel's.
        Ok(data.len())
    }

    /// One frame's data, or nothing where no frame is waiting.
    fn receive(&self, into: &mut [u8]) -> usize {
        let mut frame = [0u8; mem::size_of::<libc::canfd_frame>()];
        let room = if self.fd {
            mem::size_of::<libc::canfd_frame>()
        } else {
            mem::size_of::<libc::can_frame>()
        };

        // SAFETY: reading into a buffer of at least `room` bytes.
        let got = unsafe {
            libc::read(
                self.socket.as_raw_fd(),
                frame.as_mut_ptr() as *mut libc::c_void,
                room,
            )
        };
        if got < 0 || (got as usize) < room {
            return 0;
        }

        // The length field, which is in the same place in both structures.
        let length = (frame[4] as usize).min(self.frame_bytes()).min(into.len());
        into[..length].copy_from_slice(&frame[DATA_AT..DATA_AT + length]);
        length
    }
}

/// The kernel's index for an interface name, if it has one.
fn index_of(interface: &str) -> Option<libc::c_uint> {
    let name = std::ffi::CString::new(interface).ok()?;
    // SAFETY: a valid C string for the length of the call.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    if index == 0 {
        None
    } else {
        Some(index)
    }
}

/// Run a simulated device on a CAN bus.
///
/// The same loop every other kind has: read whatever the handler sent, send a
/// packet if one is due, wait out the rest of the interval. What differs is
/// only that each segment is a frame, so a packet in four segments arrives at
/// the handler as four frames.
pub fn run_can_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let bus = match BusMember::join(&config) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    let mut rng = rand::thread_rng();
    let mut sent = 0u64;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // What the handler has put on the bus, which is a trigger for a
        // device that answers requests.
        let mut buf = vec![0u8; 64];
        let asked = bus.receive(&mut buf);
        if asked > 0 {
            let mut guard = stats.lock().unwrap();
            guard.a_packet_has_come(&buf[..asked]);
            guard.bytes_recv += asked as u64;
        }

        if pacing.a_packet_is_due(config.triggered, asked)
            && !ignore_this_request(&config, &mut rng)
        {
            let late = lateness(&config, &mut rng);
            let packet = match a_packet(&config, &pacing, sent, &mut rng) {
                Produce::Send(packet) => packet,
                Produce::Nothing => Vec::new(),
            };
            wait_while_running(late, &running);

            // A segment per frame, and a segment no larger than a frame: a
            // file that asked for more is refused where it is read, so what
            // arrives here fits.
            let (bytes, whole) = send_in_segments(
                &packet,
                pacing.segment_size.min(bus.frame_bytes()),
                pacing.segment_interval,
                &running,
                &mut |segment| bus.send(segment),
            );

            if bytes > 0 {
                let mut guard = stats.lock().unwrap();
                guard.bytes_sent += bytes;
                if whole {
                    guard.a_packet_has_gone(&packet);
                    sent += 1;
                }
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{PayloadProtocol, SimulatedPayload};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use tcslibgs::Faults;

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
            .find(|name| index_of(name).is_some() && is_up(name))
    }

    /// A device on the given bus, under the given identifier, sending a
    /// packet every 50 ms.
    ///
    /// The identifier is a parameter rather than a constant because the tests
    /// here run as threads of one process and share the one bus the host has.
    /// Two devices on one identifier is the arrangement a configuration is
    /// refused for, and for the same reason: each handler filters on that
    /// identifier and so reads the other's frames. A test asserting that
    /// nothing has arrived yet then sees a frame another test sent and fails
    /// intermittently, nowhere near the cause. So each test that touches the
    /// bus takes an identifier of its own, and the device and the handler
    /// within one test share it, which is how they talk at all.
    fn device_on(interface: &str, can_id: u32, triggered: bool) -> PayloadConfig {
        PayloadConfig {
            name: "can-device".to_string(),
            protocol: PayloadProtocol::Can,
            address: interface.to_string(),
            port: 0,
            bus_address: 0,
            can_id,
            can_fd: false,
            gpio_lines: Vec::new(),
            triggered,
            faults: Faults::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(8)),
            segment_size: Arc::new(AtomicU32::new(8)),
            packet_interval_ms: Arc::new(AtomicU32::new(50)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        }
    }

    /// A bus that is not there is reported with the command that would make
    /// one, and the device stops rather than spinning.
    ///
    /// The case every machine without a virtual bus is in, which is most of
    /// them: making one needs root. A device that said nothing would be a
    /// panel with every counter at nought and no reason given.
    #[test]
    fn a_bus_that_is_not_there_says_how_to_make_one() {
        let said = match BusMember::join(&device_on("vcan-nonesuch", 0x200, false)) {
            Err(e) => e,
            Ok(_) => panic!("this host has an interface called vcan-nonesuch"),
        };
        assert!(
            said.contains("vcan-nonesuch") && said.contains("ip link add"),
            "the error should name the bus and how to make one: {said}"
        );
    }

    /// A frame carries what the device sent, to another member of the bus.
    ///
    /// Needs a bus, and says so rather than passing in silence where there is
    /// none. What can be checked without one is beside this.
    #[test]
    fn a_frame_carries_what_the_device_sends() {
        let Some(interface) = a_virtual_bus() else {
            eprintln!(
                "skipped: no virtual CAN bus up on this host. Make one with \
                 \"ip link add dev vcan0 type vcan; ip link set dev vcan0 up\""
            );
            return;
        };

        let device =
            BusMember::join(&device_on(&interface, 0x201, false)).expect("a member of the bus");
        let handler =
            BusMember::join(&device_on(&interface, 0x201, false)).expect("another member");

        assert_eq!(
            device.send(&[9, 8, 7]).expect("a frame goes out"),
            3,
            "a send moves the device's bytes, not the frame's"
        );

        let mut buf = [0u8; 64];
        let got = (0..50)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                match handler.receive(&mut buf) {
                    0 => None,
                    n => Some(n),
                }
            })
            .expect("the frame arrived at the other member");
        assert_eq!(&buf[..got], &[9, 8, 7]);

    }

    /// A member of the bus whose socket is not a bus at all.
    ///
    /// For the checks that happen before anything is sent, which need a
    /// member and not a bus. They were written beside the test above and
    /// gated with it, and so ran on no machine that had not been set up by
    /// hand.
    fn not_on_a_bus(fd: bool) -> BusMember {
        BusMember {
            // Writable, because the lengths that are not refused go on to be
            // written: /dev/null takes them and says nothing.
            socket: std::os::unix::io::OwnedFd::from(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .expect("a descriptor"),
            ),
            can_id: 1,
            fd,
        }
    }

    /// A packet larger than a frame is refused rather than split, and how
    /// large that is follows from the kind of frame.
    ///
    /// Before the send is attempted, so this needs no bus. The device
    /// configuration refuses such a packet where the file is read, which is
    /// what keeps it from reaching here in a real run; this is the stand-in's
    /// own guard, for the trigger a file does not size.
    #[test]
    fn a_packet_larger_than_a_frame_is_refused() {
        let classic = not_on_a_bus(false);

        let said = match classic.send(&[0u8; 9]) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("nine bytes went into an eight-byte frame as {n}"),
        };
        assert!(
            said.contains("will not fit") && said.contains("8"),
            "the error should say it cannot be sent and how much a frame takes: {said}"
        );

        // The most each kind of frame carries, neither of them refused.
        assert_eq!(classic.send(&[0u8; 8]).expect("a full frame"), 8);
        let fd = not_on_a_bus(true);
        assert_eq!(fd.send(&[0u8; 64]).expect("a full FD frame"), 64);

        let said = match fd.send(&[0u8; 65]) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("sixty-five bytes went into a sixty-four-byte frame as {n}"),
        };
        assert!(
            said.contains("64"),
            "the error should say what an FD frame carries: {said}"
        );
    }

    /// A triggered device on a bus answers a request and says nothing
    /// otherwise.
    ///
    /// Which is the thing a bus of peers can do and an I2C bus cannot: both
    /// ends here are sockets, so the simulator really does hear the handler
    /// ask.
    #[test]
    fn a_triggered_device_on_a_bus_answers_a_request() {
        let Some(interface) = a_virtual_bus() else {
            eprintln!("skipped: no virtual CAN bus up on this host");
            return;
        };

        // The handler's end, which asks.
        let handler = BusMember::join(&device_on(&interface, 0x202, false)).expect("a member");

        let mut config = device_on(&interface, 0x202, true);
        // A triggered device sends when it is asked and at no rate of its
        // own, which is what the device configuration holds it to.
        config.packet_interval_ms = Arc::new(AtomicU32::new(0));
        let mut device = SimulatedPayload::new(config);
        device.start().expect("the device starts");

        // Nothing until it is asked.
        let mut buf = [0u8; 64];
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            handler.receive(&mut buf),
            0,
            "a triggered device sent without being asked"
        );

        // Asked, and the answer arrives.
        handler.send(b"READ").expect("the trigger goes out");
        let got = (0..50)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                match handler.receive(&mut buf) {
                    0 => None,
                    n => Some(n),
                }
            })
            .expect("the request was answered");
        assert_eq!(got, 8, "one packet of the configured size");

        device.stop();
    }
}
