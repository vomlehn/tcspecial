//! What a data handler's endpoint means to the window.
//!
//! The MOC does no endpoint I/O of its own -- a handler's endpoints are
//! tcspecial's, and the MOC only ever talks to the command interpreter. What
//! it needs of them is what to say about one in a panel, and which kind of
//! handler a START_DH is asking for, since both follow from the endpoint the
//! payload configuration file gives.
//!
//! Which kind of handler a START_DH asks for is not worked out here: it is
//! `EndpointConfig::kind`, in tcslibgs, because tcspecial checks the kind a
//! command names against the same mapping. Two copies of it would be two
//! chances for the ground to ask for something the spacecraft refuses.

use tcslibgs::{EndpointConfig, NetworkProtocol};

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

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::{DeviceConfig, NetworkConfig};

    /// What a panel says about an endpoint, which is the kind and where it
    /// is. Which kind of handler to ask for is `EndpointConfig::kind` and is
    /// tested where it lives.
    #[test]
    fn a_panel_names_the_kind_of_endpoint_and_where_it_is() {
        let device = EndpointConfig::Device(DeviceConfig {
            path: "/dev/urandom".to_string(),
        });
        assert_eq!(endpoint_description(&device), "Device /dev/urandom");

        let network = EndpointConfig::Network(NetworkConfig {
            protocol: NetworkProtocol::Tcp,
            address: "localhost".to_string(),
            port: 5000,
        });
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
