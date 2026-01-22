//! Endpoint implementation for tcspecial
//!
//! Endpoints handle low-level I/O and each one is associated with a thread.
//! The EndpointWaitable trait defines wait_for_event() as a function that waits
//! for an event to occur, similar to the Linux poll() system call.

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::time::Duration;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use tcslibgs::{TcsError, TcsResult, EndpointDelayConfig};

/// Events that can occur on an endpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointEvent {
    /// Data is ready to read
    ReadReady,
    /// Ready to write
    WriteReady,
    /// Command received from CI
    CommandReady,
    /// Timeout occurred
    Timeout,
    /// Error occurred
    Error,
}

/// Trait for endpoints that can wait for events
pub trait EndpointWaitable {
    /// Wait for an event on this endpoint
    fn wait_for_event(&self, timeout: Option<Duration>) -> TcsResult<EndpointEvent>;

    /// Get the I/O file descriptor
    fn io_fd(&self) -> RawFd;

    /// Get the command file descriptor for CI notifications
    fn cmd_fd(&self) -> RawFd;
}

/// Trait for readable endpoints
pub trait EndpointReadable: EndpointWaitable {
    /// Read data from the endpoint
    fn read(&mut self, buf: &mut [u8]) -> TcsResult<usize>;
}

/// Trait for writable endpoints
pub trait EndpointWritable: EndpointWaitable {
    /// Write data to the endpoint
    fn write(&mut self, buf: &[u8]) -> TcsResult<usize>;
}

/// Base endpoint with I/O and command file descriptors
pub struct Endpoint {
    /// I/O file descriptor
    io_fd: RawFd,
    /// Command pipe file descriptor (read end)
    cmd_fd: RawFd,
    /// Delay configuration for retries
    delay_config: EndpointDelayConfig,
    /// Whether this is a stream endpoint
    is_stream: bool,
    /// Stream delay in milliseconds
    stream_delay_ms: u64,
}

impl Endpoint {
    /// Create a new endpoint
    pub fn new(
        io_fd: RawFd,
        cmd_fd: RawFd,
        delay_config: EndpointDelayConfig,
        is_stream: bool,
        stream_delay_ms: u64,
    ) -> Self {
        Self {
            io_fd,
            cmd_fd,
            delay_config,
            is_stream,
            stream_delay_ms,
        }
    }

    /// Wait for events using poll()
    pub fn wait(&self, for_read: bool, timeout: Option<Duration>) -> TcsResult<EndpointEvent> {
        let io_flags = if for_read {
            PollFlags::POLLIN
        } else {
            PollFlags::POLLOUT
        };

        // SAFETY: We're borrowing the file descriptors for the duration of the poll call
        let io_poll_fd = unsafe {
            PollFd::new(BorrowedFd::borrow_raw(self.io_fd), io_flags)
        };
        let cmd_poll_fd = unsafe {
            PollFd::new(BorrowedFd::borrow_raw(self.cmd_fd), PollFlags::POLLIN)
        };

        let mut poll_fds = [io_poll_fd, cmd_poll_fd];

        let timeout_ms = timeout
            .map(|d| PollTimeout::try_from(d).unwrap_or(PollTimeout::MAX))
            .unwrap_or(PollTimeout::NONE);

        match poll(&mut poll_fds, timeout_ms) {
            Ok(0) => Ok(EndpointEvent::Timeout),
            Ok(_) => {
                // Check command fd first (as per requirement)
                if let Some(revents) = poll_fds[1].revents() {
                    if revents.contains(PollFlags::POLLIN) {
                        return Ok(EndpointEvent::CommandReady);
                    }
                }

                // Then check I/O fd
                if let Some(revents) = poll_fds[0].revents() {
                    if revents.contains(PollFlags::POLLERR) || revents.contains(PollFlags::POLLHUP) {
                        return Ok(EndpointEvent::Error);
                    }
                    if for_read && revents.contains(PollFlags::POLLIN) {
                        return Ok(EndpointEvent::ReadReady);
                    }
                    if !for_read && revents.contains(PollFlags::POLLOUT) {
                        return Ok(EndpointEvent::WriteReady);
                    }
                }

                Ok(EndpointEvent::Timeout)
            }
            Err(e) => Err(TcsError::Io(io::Error::from_raw_os_error(e as i32))),
        }
    }

    /// Read with retry logic
    pub fn read_with_retry<R: Read>(&self, reader: &mut R, buf: &mut [u8]) -> TcsResult<usize> {
        let mut delay_ms = self.delay_config.init_ms;

        loop {
            // For stream endpoints, wait for stream_delay to collect more data
            if self.is_stream && self.stream_delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(self.stream_delay_ms));
            }

            match reader.read(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if delay_ms >= self.delay_config.max_ms {
                        return Err(TcsError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    delay_ms *= 2;
                }
                Err(e) => return Err(TcsError::Io(e)),
            }
        }
    }

    /// Write with retry logic
    pub fn write_with_retry<W: Write>(&self, writer: &mut W, buf: &[u8]) -> TcsResult<usize> {
        let mut delay_ms = self.delay_config.init_ms;

        loop {
            match writer.write(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if delay_ms >= self.delay_config.max_ms {
                        return Err(TcsError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    delay_ms *= 2;
                }
                Err(e) => return Err(TcsError::Io(e)),
            }
        }
    }

    /// Get the I/O file descriptor
    pub fn io_fd(&self) -> RawFd {
        self.io_fd
    }

    /// Get the command file descriptor
    pub fn cmd_fd(&self) -> RawFd {
        self.cmd_fd
    }

    /// Check if this is a stream endpoint
    pub fn is_stream(&self) -> bool {
        self.is_stream
    }
}

/// Stream endpoint implementation
pub struct StreamEndpoint {
    inner: Endpoint,
}

impl StreamEndpoint {
    pub fn new(io_fd: RawFd, cmd_fd: RawFd, delay_config: EndpointDelayConfig, stream_delay_ms: u64) -> Self {
        Self {
            inner: Endpoint::new(io_fd, cmd_fd, delay_config, true, stream_delay_ms),
        }
    }
}

impl EndpointWaitable for StreamEndpoint {
    fn wait_for_event(&self, timeout: Option<Duration>) -> TcsResult<EndpointEvent> {
        self.inner.wait(true, timeout)
    }

    fn io_fd(&self) -> RawFd {
        self.inner.io_fd()
    }

    fn cmd_fd(&self) -> RawFd {
        self.inner.cmd_fd()
    }
}

/// Datagram endpoint implementation
pub struct DatagramEndpoint {
    inner: Endpoint,
}

impl DatagramEndpoint {
    pub fn new(io_fd: RawFd, cmd_fd: RawFd, delay_config: EndpointDelayConfig) -> Self {
        Self {
            inner: Endpoint::new(io_fd, cmd_fd, delay_config, false, 0),
        }
    }
}

impl EndpointWaitable for DatagramEndpoint {
    fn wait_for_event(&self, timeout: Option<Duration>) -> TcsResult<EndpointEvent> {
        self.inner.wait(true, timeout)
    }

    fn io_fd(&self) -> RawFd {
        self.inner.io_fd()
    }

    fn cmd_fd(&self) -> RawFd {
        self.inner.cmd_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_event() {
        assert_eq!(EndpointEvent::ReadReady, EndpointEvent::ReadReady);
        assert_ne!(EndpointEvent::ReadReady, EndpointEvent::WriteReady);
    }
}
