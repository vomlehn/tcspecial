//! The device endpoint: a device file, opened once and read and written.
//!
//! Neither bound nor connected -- opened. That is why a device handler was
//! the only kind that ever worked while both ends of a network link were
//! binding the same address: two opens of a device are fine where two binds
//! of a socket are not.
//!
//! See [`crate::endpoint`] for the traits.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{DeviceConfig, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// Device endpoint for device file I/O
pub struct DeviceEndpoint {
    file: File,
    _buffer: Vec<u8>,
}

impl DeviceEndpoint {
    pub fn new(what: &str, config: &DeviceConfig) -> TcsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            // Not this process's terminal. A device that is a tty -- a real
            // serial port, or a pty standing in for one -- becomes the
            // controlling terminal of a process that opens it without this,
            // and then a read from a background process group raises SIGTTIN
            // and a hangup raises SIGHUP: a handler stopped or killed for
            // reasons that have nothing to do with its payload.
            .custom_flags(libc::O_NOCTTY)
            // Named, because a bare `No such file or directory` says neither
            // which handler could not start nor what it was reaching for.
            .open(&config.path)
            .map_err(|e| {
                TcsError::Endpoint(format!("{what}: {} cannot be opened: {e}", config.path))
            })?;

        Ok(Self {
            file,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same open file.
    ///
    /// A device could be opened twice where a socket could not, but one
    /// description is shared so that both ends of a conduit pair see one file
    /// position and one set of open flags.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for DeviceEndpoint {
    fn io_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for DeviceEndpoint {
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.file.read(buffer) {
            Ok(n) => Ok(n),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for DeviceEndpoint {
    fn write(&mut self, data: &[u8]) -> TcsResult<usize> {
        match self.file.write(data) {
            Ok(n) => Ok(n),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

