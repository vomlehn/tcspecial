//! Configuration loading for TCSpecial

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tcslibgs::{CIConfig, DHConfig, PayloadConfig, TcsError, TcsResult};

/// Load payload configuration from a JSON file
pub fn load_config<P: AsRef<Path>>(path: P) -> TcsResult<(CIConfig, Vec<DHConfig>)> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let payload_config: PayloadConfig = serde_json::from_reader(reader)?;

    let ci_config = payload_config.ci_config.to_ci_config()
        .map_err(|e| TcsError::Config(e))?;

    let dh_configs: Result<Vec<DHConfig>, String> = payload_config
        .data_handlers
        .iter()
        .map(|dh| dh.to_dh_config())
        .collect();

    let dh_configs = dh_configs.map_err(|e| TcsError::Config(e))?;

    Ok((ci_config, dh_configs))
}

/// Configuration constants
pub mod constants {
    use std::time::Duration;

    pub const BEACON_DEFAULT_MS: Duration = Duration::new(1000, 0);

    // FIXME: use getaddrinfo()
    pub const BEACON_NETADDR: &str = "localhost:5550";

    /// Initial delay for endpoint retry
    pub const ENDPOINT_DELAY_INIT: Duration = Duration::from_millis(100);

    /// Maximum delay for endpoint retry
    pub const ENDPOINT_DELAY_MAX: Duration = Duration::from_secs(10);

    /// Maximum number of endpoint retries
    pub const ENDPOINT_MAX_RETRIES: u32 = 10;

    /// Stream endpoint delay for collecting bytes
    pub const STREAM_EP_DELAY: Duration = Duration::from_millis(50);

    /// Default buffer size for endpoints
    pub const ENDPOINT_BUFFER_SIZE: usize = 4096;

    /// Restart arm timeout
    pub const RESTART_ARM_TIMEOUT: Duration = Duration::from_secs(60);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config() {
        let config_json = r#"{
            "version": "1.0",
            "description": "Test config",
            "data_handlers": [
                {
                    "dh_id": 0,
                    "name": "DH0",
                    "type": "network",
                    "protocol": "udp",
                    "address": "localhost",
                    "port": 5000,
                    "packet_size": 12,
                    "packet_interval_ms": 1000
                }
            ],
            "ci_config": {
                "address": "0.0.0.0",
                "port": 4000,
                "protocol": "udp",
                "beacon_interval_ms": 5000
            }
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_json.as_bytes()).unwrap();

        let result = load_config(temp_file.path());
        assert!(result.is_ok());

        let (ci_config, dh_configs) = result.unwrap();
        assert_eq!(ci_config.port, 4000);
        assert_eq!(dh_configs.len(), 1);
    }
}
