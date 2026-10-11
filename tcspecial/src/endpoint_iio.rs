//! The IIO endpoint: a sensor's buffer, read and never written.
//!
//! **Experimental, and not yet supported.** Run against a pseudo-terminal
//! standing in for a buffer, and against no sensor.
//!
//! What a buffered IIO device offers user space is a character device that
//! hands over packed samples: open it, poll it, read it, and what comes back
//! is as many whole sample sets as were ready. That is exactly the shape a
//! conduit wants, so this endpoint is a short one -- open, poll, read.
//!
//! What it does not offer is anywhere to write. A buffer is a stream from the
//! sensor; the knobs that would change what it produces are in `sysfs`, and a
//! handler that wrote there would be reconfiguring a sensor other programs may
//! be reading. So this endpoint refuses writes rather than failing at them,
//! and the payload configuration refuses a triggered IIO payload: there is
//! nowhere to send a request.
//!
//! Opened read-only for the same reason. A read-write open of a buffer
//! succeeds on some drivers and fails on others, and an endpoint that asked
//! for a permission it will never use would be refused by the strict ones for
//! nothing.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::io::{AsRawFd, RawFd};

use nix::poll::PollFlags;
use tcslibgs::{IioConfig, TcsError, TcsResult};

use crate::config::constants::ENDPOINT_BUFFER_SIZE;
use crate::endpoint::{
    wait_for_fds, EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};

/// A sensor's sample buffer, open for reading.
pub struct IioEndpoint {
    /// The handler this endpoint belongs to, as the payload configuration
    /// names it, so what the endpoint says of itself says which handler it is
    /// for.
    what: String,
    file: File,
    /// Where the buffer is, for the error that says a write has nowhere to
    /// go.
    path: String,
    _buffer: Vec<u8>,
}

impl IioEndpoint {
    /// Open the device the configuration names.
    ///
    /// Nothing is set here: how many channels are enabled and how fast they
    /// are sampled belong to whoever brought the sensor up, which is why the
    /// configuration records them rather than applying them. What this can
    /// say is whether the device is there and readable, which is the failure
    /// a payload set gets wrong.
    pub fn new(what: &str, config: &IioConfig) -> TcsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .open(&config.path)
            .map_err(|e| {
                TcsError::Endpoint(format!(
                    "{what}: cannot open the sample buffer {}: {e}. An IIO buffer is a \
                     character device, and one that is not there is commonly a sensor \
                     whose channels and buffer nobody enabled",
                    config.path
                ))
            })?;

        Ok(Self {
            what: what.to_string(),
            file,
            path: config.path.clone(),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }

    /// A second endpoint on the same open buffer.
    ///
    /// One open description, as a device payload has: the conduit that reads
    /// the buffer and the conduit that would write it share it, and the
    /// second one has nothing to write.
    pub fn try_clone(&self) -> TcsResult<Self> {
        Ok(Self {
            what: self.what.clone(),
            file: self.file.try_clone()?,
            path: self.path.clone(),
            _buffer: vec![0u8; ENDPOINT_BUFFER_SIZE],
        })
    }
}

impl EndpointWaitable for IioEndpoint {
    fn io_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn wait_for_event(&self, cmd_fd: RawFd, timeout_ms: i32) -> TcsResult<WaitResult> {
        wait_for_fds(self.io_fd(), cmd_fd, PollFlags::POLLIN, timeout_ms)
    }
}

impl EndpointReadable for IioEndpoint {
    /// Whatever sample sets are ready.
    ///
    /// Whole sets: the driver hands over a multiple of the sample size and
    /// never part of one, which is why this kind needs no rule for where a
    /// read ends.
    fn read(&mut self, buffer: &mut [u8]) -> TcsResult<usize> {
        match self.file.read(buffer) {
            Ok(n) => Ok(n),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(TcsError::Io(e)),
        }
    }
}

impl EndpointWritable for IioEndpoint {
    /// Nothing is written to a sample buffer.
    ///
    /// Reported rather than attempted. The write would fail anyway -- the
    /// device is open for reading -- but the error a driver gives says
    /// nothing about why, where this says what an IIO payload is: a stream
    /// out of a sensor, with its settings in sysfs and not on this
    /// descriptor.
    ///
    /// A triggered IIO payload is refused where the file is read, so nothing
    /// reaches this in a run that got as far as starting: what does reach it
    /// is the ground sending data to a sensor, which is a command that should
    /// be reported rather than quietly dropped.
    fn write(&mut self, _data: &[u8]) -> TcsResult<usize> {
        Err(TcsError::Endpoint(format!(
            "{}: {} is an IIO sample buffer, which is read and not written: a \
             sensor's channels and rate are set through sysfs, and nothing the \
             ground sends has anywhere to go here",
            self.what, self.path
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A buffer that is not there is reported, and the message says what is
    /// usually wrong.
    #[test]
    fn a_buffer_that_is_not_there_is_reported() {
        let config = IioConfig {
            path: "/dev/iio:device-nonesuch".to_string(),
            channels: Some(2),
            sample_rate: Some(100),
        };

        let said = match IioEndpoint::new("sensor", &config) {
            Err(e) => format!("{e}"),
            Ok(_) => panic!("this host has a device called iio:device-nonesuch"),
        };
        assert!(
            said.contains("iio:device-nonesuch") && said.contains("nobody enabled"),
            "the error should name the device and the usual cause: {said}"
        );
    }

    /// Samples are read as the buffer hands them over.
    ///
    /// A file standing in for the character device, which is what a buffer is
    /// as far as reading goes: the test is that the endpoint reads what the
    /// device produced, byte for byte.
    #[test]
    fn the_samples_a_buffer_offers_are_read() {
        let mut file = tempfile::NamedTempFile::new().expect("a temporary file");
        // Two channels of two bytes, twice: what a packed sample set looks
        // like.
        file.write_all(&[0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00])
            .expect("the samples are written");
        file.flush().expect("and flushed");

        let mut endpoint = IioEndpoint::new(
            "sensor",
            &IioConfig {
                path: file.path().display().to_string(),
                channels: Some(2),
                sample_rate: None,
            },
        )
        .expect("the buffer opens");

        let mut buffer = [0u8; 64];
        let got = endpoint.read(&mut buffer).expect("the samples are read");
        assert_eq!(&buffer[..got], &[0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00]);
    }

    /// A write says what an IIO payload is rather than failing at a
    /// descriptor.
    #[test]
    fn a_sample_buffer_refuses_a_write() {
        let file = tempfile::NamedTempFile::new().expect("a temporary file");
        let mut endpoint = IioEndpoint::new(
            "sensor",
            &IioConfig {
                path: file.path().display().to_string(),
                channels: None,
                sample_rate: None,
            },
        )
        .expect("the buffer opens");

        let said = match endpoint.write(b"anything") {
            Err(e) => format!("{e}"),
            Ok(n) => panic!("a sample buffer took {n} bytes"),
        };
        assert!(
            said.contains("read and not written") && said.contains("sysfs"),
            "the error should say what an IIO payload is: {said}"
        );
    }
}
