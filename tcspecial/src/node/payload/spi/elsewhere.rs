//! The SPI endpoint where the system has no spidev: a web page. Built so
//! that a configuration with SPI peripherals loads, and its nodes run, where they are
//! simulated -- reached over UDP at their `sim_address`, as on Linux under
//! `--simulate` -- and refused, saying so, where one would be opened.

use tcsprotocol::{SpiConfig, TcsError, TcsResult};

use crate::endpoint::{
    EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};
use tcsos::wait::Source;

/// A SPI device: none can be had here.
pub enum SpiEndpoint {}

impl SpiEndpoint {
    /// Refuses: a SPI device is Linux's spidev.
    ///
    /// # Arguments
    ///
    /// * `what`    - The node's name, for the refusal.
    /// * `_config` - The node's settings.
    ///
    /// # Returns
    ///
    /// Never.
    ///
    /// # Errors
    ///
    /// Always `TcsError::Endpoint`, saying that only a simulated device can
    /// be reached here.
    pub fn new(what: &str, _config: &SpiConfig) -> TcsResult<Self> {
        Err(TcsError::Endpoint(format!(
            "{what}: a SPI device is Linux's spidev, which a web page \
             does not have: only a simulated one, at its sim_address, can \
             be reached here"
        )))
    }

    /// A second endpoint on the same device: there is none.
    ///
    /// # Returns
    ///
    /// Never.
    ///
    /// # Errors
    ///
    /// Never.
    pub fn try_clone(&self) -> TcsResult<Self> {
        match *self {}
    }
}

impl EndpointWaitable for SpiEndpoint {
    fn io_source(&self) -> Source {
        match *self {}
    }

    fn wait_for_event(
        &self,
        _cmd: Source,
        _timeout_ms: i32,
    ) -> TcsResult<WaitResult> {
        match *self {}
    }
}

impl EndpointReadable for SpiEndpoint {
    fn read(&mut self, _buffer: &mut [u8]) -> TcsResult<usize> {
        match *self {}
    }
}

impl EndpointWritable for SpiEndpoint {
    fn write(&mut self, _data: &[u8]) -> TcsResult<usize> {
        match *self {}
    }
}
