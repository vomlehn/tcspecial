//! Configuration for tcspecial

use std::net::SocketAddr;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tcslibgs::{BeaconTime, EndpointDelayConfig, StreamEPDelay};

/// Main configuration for tcspecial
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcspecialConfig {
    /// Address to listen for commands from OC
    pub listen_addr: SocketAddr,
    /// Beacon interval
    pub beacon_interval: BeaconTime,
    /// Endpoint delay configuration
    pub endpoint_delay: EndpointDelayConfig,
    /// Stream endpoint delay
    pub stream_delay: StreamEPDelay,
    /// Maximum number of data handlers
    pub max_data_handlers: usize,
    /// Command queue size
    pub command_queue_size: usize,
    /// Telemetry queue size
    pub telemetry_queue_size: usize,
}

impl Default for TcspecialConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:5000".parse().unwrap(),
            beacon_interval: BeaconTime::default(),
            endpoint_delay: EndpointDelayConfig::default(),
            stream_delay: StreamEPDelay::default(),
            max_data_handlers: 8,
            command_queue_size: 16,
            telemetry_queue_size: 64,
        }
    }
}

impl TcspecialConfig {
    /// Get beacon interval as Duration
    pub fn beacon_duration(&self) -> Duration {
        Duration::from_millis(self.beacon_interval.0)
    }

    /// Get initial endpoint delay as Duration
    pub fn endpoint_delay_init(&self) -> Duration {
        Duration::from_millis(self.endpoint_delay.init_ms)
    }

    /// Get maximum endpoint delay as Duration
    pub fn endpoint_delay_max(&self) -> Duration {
        Duration::from_millis(self.endpoint_delay.max_ms)
    }

    /// Get stream delay as Duration
    pub fn stream_delay_duration(&self) -> Duration {
        Duration::from_millis(self.stream_delay.0)
    }
}

/// Data handler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DHConfig {
    /// Buffer size for reads
    pub read_buffer_size: usize,
    /// Buffer size for writes
    pub write_buffer_size: usize,
    /// Whether to use stream semantics
    pub is_stream: bool,
    /// Stream delay (only used if is_stream is true)
    pub stream_delay_ms: Option<u64>,
}

impl Default for DHConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: 4096,
            write_buffer_size: 4096,
            is_stream: false,
            stream_delay_ms: None,
        }
    }
}

impl DHConfig {
    pub fn stream() -> Self {
        Self {
            is_stream: true,
            stream_delay_ms: Some(10),
            ..Default::default()
        }
    }

    pub fn datagram() -> Self {
        Self {
            is_stream: false,
            stream_delay_ms: None,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TcspecialConfig::default();
        assert_eq!(config.max_data_handlers, 8);
        assert_eq!(config.beacon_interval.0, 10000);
    }

    #[test]
    fn test_dh_config() {
        let stream_config = DHConfig::stream();
        assert!(stream_config.is_stream);

        let dgram_config = DHConfig::datagram();
        assert!(!dgram_config.is_stream);
    }
}
