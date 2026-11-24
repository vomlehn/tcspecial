//! The I2C endpoint where the system has no i2c-dev: a web page. Built so
//! that a configuration with I2C devices loads, and its nodes run, where they are
//! simulated -- reached over UDP at their `sim_address`, as on Linux in a
//! simulated run -- and refused, saying so, where one would be opened.

use tcsprotocol::{I2cConfig, TcsError, TcsResult};

use crate::endpoint::{
    EndpointReadable, EndpointWaitable, EndpointWritable, WaitResult,
};
use tcsos::wait::Source;

/// A I2C device: none can be had here.
pub enum I2cEndpoint {}

impl I2cEndpoint {
    /// Refuses: a I2C device is Linux's i2c-dev.
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
    pub fn new(what: &str, _config: &I2cConfig) -> TcsResult<Self> {
        Err(TcsError::Endpoint(format!(
            "{what}: a I2C device is Linux's i2c-dev, which a web page \
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

impl EndpointWaitable for I2cEndpoint {
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

impl EndpointReadable for I2cEndpoint {
    fn read(&mut self, _buffer: &mut [u8]) -> TcsResult<usize> {
        match *self {}
    }
}

impl EndpointWritable for I2cEndpoint {
    fn write(&mut self, _data: &[u8]) -> TcsResult<usize> {
        match *self {}
    }
}
