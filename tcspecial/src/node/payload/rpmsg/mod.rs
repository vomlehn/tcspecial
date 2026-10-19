//! The rpmsg endpoint: a channel to software on another core.
//!
//! **Experimental, and not yet supported.** Run against a pseudo-terminal
//! standing in for the channel, and against no remote processor.
//!
//! What Linux gives user space for an rpmsg channel is a character device that
//! carries whole messages: a write is one message to the remote, and a read
//! hands over one message from it. Both directions, so unlike a sample buffer
//! this device can be triggered -- the handler writes a request and the
//! remote answers.
//!
//! The one thing worth doing beyond opening it is checking which channel it
//! is. A node's number is assigned as channels announce themselves, so
//! `/dev/rpmsg0` is whichever came first, and a device set that names a node
//! may be pointed at another core's channel after a change in boot order --
//! the same hazard an I2C stub adapter has. The kernel says what a node is:
//! `/sys/class/rpmsg/<node>/name` holds the service name the remote announced.
//! So the endpoint reads it and refuses a channel that is not the one the file
//! asked for.
//!
//! Where that file does not exist, nothing is refused. A pty standing in for
//! the channel has no `sysfs` entry, and neither has an rpmsg device on a
//! kernel that does not publish the name; the check exists to catch a node
//! whose number moved, which only a real rpmsg device can do.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;

use nix::poll::PollFlags;
use tcslibgs::{node::payload::rpmsg::message_bytes, RpmsgConfig, TcsError, TcsResult};

use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// Where the kernel publishes what an rpmsg node is.
///
/// A constant so that a test can look at a tree of its own, as the I2C
/// stand-in's scan does.
const RPMSG_CLASS: &str = "/sys/class/rpmsg";

/// An rpmsg channel, open both ways.
pub struct RpmsgEndpoint {
    /// The handler this endpoint belongs to, as the device configuration
    /// names it.
    what: String,
    file: File,
    /// The greatest number of bytes one message carries, which a write longer
    /// than is refused rather than split.
    room: u32,
}

impl RpmsgEndpoint {
    /// Open the channel the configuration names, and hold it to the service
    /// it was supposed to be.
    pub fn new(what: &str, config: &RpmsgConfig) -> TcsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)
            .map_err(|e| {
                TcsError::Endpoint(format!(
                    "{what}: cannot open the rpmsg channel {}: {e}. A node exists only \\
                     while the remote has announced its channel, so one that is not \\
                     there is commonly a remote processor that has not been started",
                    config.path
                ))
            })?;

        is_that_service(what, Path::new(RPMSG_CLASS), &config.path, &config.service)?;

        Ok(Self {
            what: what.to_string(),
            file,
            room: message_bytes(config.max_message_bytes),
        })
    }

    /// A second endpoint on the same open channel.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            file: self.file.try_clone()?,
            room: self.room,
        })
    }
}

/// Hold a node against the service the file said it would be.
///
/// `class` is where the kernel publishes it, given so that a test can hand
/// over a tree of its own. Nothing is refused where the entry is absent; see
/// the module's own note for why.
fn is_that_service(what: &str, class: &Path, path: &str, service: &str) -> TcsResult<()> {
    let Some(node) = Path::new(path).file_name() else {
        return Ok(());
    };
    let named = class.join(node).join("name");

    let Ok(announced) = std::fs::read_to_string(&named) else {
        return Ok(());
    };
    let announced = announced.trim();

    if announced != service {
        return Err(TcsError::Config(format!(
            "{what}: {path} is the channel \"{announced}\", not \"{service}\". A node's \
             number is assigned as channels announce themselves, so the node a file \
             names can become another core's channel"
        )));
    }

    Ok(())
}

impl EndpointWaitable for RpmsgEndpoint {
    fn io_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for RpmsgEndpoint {
    /// One message from the remote, or nothing.
    ///
    /// A whole message: the driver hands over one per read and never part of
    /// one, which is why this kind needs no rule for where a read ends.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.file.read(buffer) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for RpmsgEndpoint {
    /// One message to the remote.
    ///
    /// Refused rather than split where it will not fit. rpmsg messages go in
    /// buffers the transport fixes the size of and there is no fragmentation,
    /// so the remote would receive the first part and have no way of knowing
    /// the rest was coming. The configuration refuses such a packet size
    /// where the file is read, so what reaches this is a trigger longer than
    /// a message -- the one write a file does not size.
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        if data.len() > self.room as usize {
            return Err(TcsError::Endpoint(format!(
                "{}: {} bytes will not fit in an rpmsg message, which carries {}, and \
                 rpmsg does not fragment",
                self.what,
                data.len(),
                self.room
            )));
        }

        match self.file.write(data) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A channel that is not there is reported, with the usual cause.
    #[test]
    fn a_channel_that_is_not_there_is_reported() {
        let config = RpmsgConfig {
            path: "/dev/rpmsg-nonesuch".to_string(),
            service: "device".to_string(),
            max_message_bytes: None,
        };

        let said = match RpmsgEndpoint::new("remote", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has a device called rpmsg-nonesuch"),
        };
        assert!(
            said.contains("rpmsg-nonesuch") && said.contains("not been started"),
            "the error should name the node and the usual cause: {said}"
        );
    }

    /// A node that is another channel is refused, and the right one is not.
    ///
    /// The whole reason a file states a service name: node numbers are
    /// assigned as channels appear, so the node a set names can become
    /// another core's channel after a change in boot order -- and a handler
    /// would then be exchanging telemetry with the wrong software.
    #[test]
    fn a_node_that_is_another_channel_is_refused() {
        let class = tempfile::tempdir().expect("a class directory");
        let node = class.path().join("rpmsg0");
        std::fs::create_dir(&node).expect("the node's directory");
        std::fs::write(node.join("name"), "payload-telemetry\n").expect("what it is");

        is_that_service("remote", class.path(), "/dev/rpmsg0", "payload-telemetry")
            .expect("the channel the file asked for");

        let said = match is_that_service("remote", class.path(), "/dev/rpmsg0", "something-else") {
            Err(e) => format!("{e}"),
            Ok(()) => panic!("the wrong channel was accepted"),
        };
        assert!(
            said.contains("payload-telemetry") && said.contains("something-else"),
            "the error should name both channels: {said}"
        );
    }

    /// A node the kernel says nothing about is not refused.
    ///
    /// Which is the case a simulated channel is in: a pty has no entry under
    /// /sys/class/rpmsg, and the check exists to catch a node whose number
    /// moved -- something only a real rpmsg device can do.
    #[test]
    fn a_node_the_kernel_says_nothing_about_is_left_alone() {
        let class = tempfile::tempdir().expect("an empty class directory");
        is_that_service("remote", class.path(), "/tmp/a-pty-slave", "device")
            .expect("a stand-in is not a mismatch");
    }

    /// Messages go both ways, and one too large is refused rather than split.
    #[test]
    fn a_channel_carries_messages_both_ways() {
        // A pty, which is what the simulator stands in with: two ends of one
        // character device.
        let mut master = 0;
        let mut slave = 0;
        // SAFETY: both ends are written by the call and checked below.
        let made = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(made, 0, "a pty: {}", std::io::Error::last_os_error());

        // Raw, so the line discipline does not edit the messages: a channel
        // carries bytes, and a pty starts in canonical mode.
        // SAFETY: the descriptor is an open pty slave; both calls take a
        // termios this frame owns.
        unsafe {
            let mut terms: libc::termios = std::mem::zeroed();
            libc::tcgetattr(slave, &mut terms);
            libc::cfmakeraw(&mut terms);
            libc::tcsetattr(slave, libc::TCSANOW, &terms);
        }

        let name = {
            let mut buffer = [0i8; 128];
            // SAFETY: a buffer of the length given, filled by the call.
            assert_eq!(
                unsafe { libc::ttyname_r(slave, buffer.as_mut_ptr(), buffer.len()) },
                0
            );
            unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        };

        let mut endpoint = RpmsgEndpoint::new(
            "remote",
            &RpmsgConfig {
                path: name,
                service: "device".to_string(),
                max_message_bytes: Some(16),
            },
        )
        .expect("the channel opens");

        // The handler's message reaches the remote.
        assert_eq!(endpoint.write(b"READ").expect("a message goes out"), 4);
        let mut got = [0u8; 32];
        // SAFETY: reading into a buffer of the length given.
        let read = unsafe { libc::read(master, got.as_mut_ptr() as *mut libc::c_void, got.len()) };
        assert!(read >= 4, "the remote got {read} bytes");
        assert_eq!(&got[..4], b"READ");

        // And the remote's answer reaches the handler.
        // SAFETY: writing the slice's own length from the slice.
        let sent = unsafe { libc::write(master, b"DATA".as_ptr() as *const libc::c_void, 4) };
        assert_eq!(sent, 4);
        let answered = (0..50)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(20));
                match endpoint.read(&mut got) {
                    Ok(0) => None,
                    Ok(n) => Some(n),
                    Err(e) => panic!("the channel could not be read: {e}"),
                }
            })
            .expect("the answer arrived");
        assert_eq!(&got[..answered], b"DATA");

        // A message larger than the transport carries is refused with the
        // reason, rather than being split into two the remote cannot join.
        let said = match endpoint.write(&[0u8; 17]) {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("seventeen bytes went into a sixteen-byte message as {n}"),
        };
        assert!(
            said.contains("does not fragment"),
            "the error should say why it cannot be split: {said}"
        );

        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }
}
