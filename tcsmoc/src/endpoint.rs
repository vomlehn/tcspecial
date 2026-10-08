//! What a data handler's endpoint means to the window.
//!
//! The MOC does no endpoint I/O of its own -- a handler's endpoints are
//! tcspecial's, and the MOC only ever talks to the command interpreter. What
//! it needs of them is what to say about one in a panel, and which kind of
//! handler a START_DH is asking for, since both follow from the endpoint the
//! payload configuration file gives.
//!
//! One place for both, so that a kind of endpoint added to the configuration
//! has one file here to be taught about rather than a window to be searched.

use tcslibgs::{DHType, EndpointConfig, NetworkProtocol};

/// How a data handler's endpoint reads in its panel.
pub fn endpoint_description(endpoint: &EndpointConfig) -> String {
    match endpoint {
        EndpointConfig::Network(net) => {
            let protocol = match net.protocol {
                NetworkProtocol::Tcp => "TCP",
                NetworkProtocol::Udp => "UDP",
                NetworkProtocol::UnixStream => "Unix stream",
                NetworkProtocol::UnixDgram => "Unix datagram",
            };

            match net.protocol {
                // A Unix socket is named by a path; its port means nothing.
                NetworkProtocol::UnixStream | NetworkProtocol::UnixDgram => {
                    format!("{} {}", protocol, net.address)
                }
                _ => format!("{} {}:{}", protocol, net.address, net.port),
            }
        }
        EndpointConfig::Device(dev) => format!("Device {}", dev.path),
        // The kinds that are device files with terms of their own. The panel
        // shows the term that would be looked for first if the link were not
        // working -- the line rate, the address on the bus, the clock -- since
        // the path alone is what every one of them has in common.
        EndpointConfig::Serial(serial) => {
            format!("Serial {} at {}", serial.path, serial.datarate)
        }
        EndpointConfig::I2c(i2c) => format!("I2C {} at {:#04X}", i2c.bus, i2c.address),
        EndpointConfig::Spi(spi) => format!("SPI {} {}", spi.path, spi.mode),
    }
}

/// Which kind of data handler tcspecial is being asked to start.
///
/// START_DH carries the type, and the configuration file is what knows it: a
/// device handler started as a network handler is a command tcspecial cannot
/// carry out.
pub fn dh_type_of(endpoint: &EndpointConfig) -> DHType {
    match endpoint {
        EndpointConfig::Network(_) => DHType::Network,
        EndpointConfig::Device(_) => DHType::Device,
        EndpointConfig::Serial(_) => DHType::Serial,
        EndpointConfig::I2c(_) => DHType::I2c,
        EndpointConfig::Spi(_) => DHType::Spi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{DeviceConfig, NetworkConfig};

    /// A device handler is started as a device handler, not as whatever the
    /// first panel happens to be.
    #[test]
    fn a_handler_is_started_as_the_type_its_endpoint_makes_it() {
        let device = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });
        assert!(dh_type_of(&device) == DHType::Device);
        assert_eq!(endpoint_description(&device), "Device /dev/urandom");

        let network = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: "localhost".to_string(),
            port: 5000,
        });
        assert!(dh_type_of(&network) == DHType::Network);
        assert_eq!(endpoint_description(&network), "TCP localhost:5000");
    }

    /// A Unix socket is named by a path, so its panel does not append a port.
    #[test]
    fn a_unix_socket_panel_shows_no_port() {
        let unix = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::UnixStream,
            address: "/tmp/dh8".to_string(),
            port: 0,
        });
        assert_eq!(endpoint_description(&unix), "Unix stream /tmp/dh8");
    }
}
