//! The device file node: an entry in the filesystem, opened once and read and
//! written.
//!
//! A *device file* and not a device, which is the distinction this module is
//! named for. "Device" meant two things here -- a piece of hardware on a bus,
//! and an entry in the filesystem such as `/dev/urandom` or a named pipe --
//! and it meant them in exactly the places that had to be exact. What this
//! opens is the second thing. Hardware has a type of its own for whichever
//! interface it arrives on.
//!
//! Neither bound nor connected -- opened. That is why a device file handler was
//! the only type that ever worked while both ends of a network link were
//! binding the same address: two opens of a file are fine where two binds of a
//! socket are not.
//!
//! See [`crate::endpoint`] for the traits.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tcsos::wait::{Ready, Source, Waitable};

use tcsprotocol::{DeviceConfig, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable,
    WaitResult,
};

/// A node reached by opening a device file.
pub struct DeviceEndpoint {
    file: File,
    _buffer: Vec<u8>,
}

impl DeviceEndpoint {
    /// Opens the device file a node names, for reading and writing.
    ///
    /// # Arguments
    ///
    /// * `what`   - The node's name, for the complaint.
    /// * `config` - The node's settings, which give the device file's path.
    ///
    /// # Returns
    ///
    /// The endpoint, the file open.
    ///
    /// # Errors
    ///
    /// `TcsError::Endpoint`, naming the node and the path, if the file
    /// cannot be opened.
    pub fn new(what: &str, config: &DeviceConfig) -> TcsResult<Self> {
        // Not this process's terminal: see tcsos::linux::fs::open_device.
        // Named, because a bare `No such file or directory` says neither
        // which handler could not start nor what it was reaching for.
        let file =
            tcsos::linux::fs::open_device(Path::new(&config.path), true, true)
                .map_err(|e| {
                    TcsError::Endpoint(format!(
                        "{what}: {} cannot be opened: {e}",
                        config.path
                    ))
                })?;
        crate::exclusive::device(what, &config.path, &file)?;

        Ok(Self {
            file,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// Makes a second endpoint on the same open file.
    ///
    /// A device could be opened twice where a socket could not, but one
    /// description is shared so that both ends of a conduit pair see one file
    /// position and one set of open flags.
    ///
    /// # Returns
    ///
    /// The second endpoint.
    ///
    /// # Errors
    ///
    /// The I/O error if the descriptor cannot be duplicated.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for DeviceEndpoint {
    fn io_source(&self) -> Source {
        self.file.source()
    }

    fn wait_for_event(
        &self,
        cmd: Source,
        timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_source(), cmd, Ready::READ, timeout_ms)
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
