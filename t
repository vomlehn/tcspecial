tcspecial/src/beacon_send.rs:            beacon_address: at,
tcspecial/src/beacon_send.rs:            beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/config.rs:            group: beacon_address_of(&section.beacon_address)?,
tcspecial/src/config.rs:            interface: beacon_interface_of(&section.beacon_interface)?,
tcspecial/src/config.rs:            group: config.beacon_address,
tcspecial/src/config.rs:            interface: config.beacon_interface,
tcspecial/src/config.rs:                        beacon_interface: 127.0.0.1\n";
tcspecial/src/config.rs:        <beacon_address>239.255.0.1:5550</beacon_address>\
tcspecial/src/config.rs:        <beacon_interface>127.0.0.1</beacon_interface>\
tcspecial/src/config.rs:        assert_eq!(config.beacon_address.to_string(), "239.255.0.1:5550");
tcspecial/src/config.rs:        assert_eq!(config.beacon_interface.to_string(), "127.0.0.1");
tcspecial/src/config.rs:            "beacon_address: 239.255.0.1:5550\nbeacon_interface: 127.0.0.1\n",
tcspecial/src/config.rs:                "beacon_address: 239.255.0.1:5550",
tcspecial/src/config.rs:                "beacon_address: 239.255.0.9:5550",
tcspecial/src/config.rs:        assert_eq!(from_the_file.beacon_address.to_string(), "239.255.0.9:5550");
tcspecial/src/config.rs:            beacon_address: beacon.to_string(),
tcspecial/src/config.rs:            beacon_interface: "127.0.0.1".to_string(),
tcspecial/src/config.rs:        assert!(said.contains("beacon_address"), "{said}");
tcspecial/src/config.rs:                "beacon_address: 239.255.0.1:5550",
tcspecial/src/config.rs:                &format!("beacon_address: \"{bad}\""),
tcspecial/src/config.rs:                said.contains("beacon_address"),
tcspecial/src/ci.rs:    beacon_address: std::net::SocketAddr,
tcspecial/src/ci.rs:    beacon_interface: std::net::Ipv4Addr,
tcspecial/src/ci.rs:            beacon_address: config.beacon_address,
tcspecial/src/ci.rs:            beacon_interface: config.beacon_interface,
tcspecial/src/ci.rs:            self.beacon_address, self.beacon_interface
tcspecial/src/ci.rs:            self.beacon_address,
tcspecial/src/ci.rs:            self.beacon_interface,
tcspecial/src/ci.rs:            beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:            beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:            beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:            beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/ci.rs:                beacon_address: a_beacon_address(),
tcspecial/src/ci.rs:                beacon_interface: std::net::Ipv4Addr::LOCALHOST,
tcspecial/src/main.rs:    tcspecial_config.beacon_address = beacon.group;
tcspecial/src/main.rs:    tcspecial_config.beacon_interface = beacon.interface;
tcsmoc/src/main.rs:        assert_eq!(mine.group.to_string(), said.beacon_address);
tcsmoc/src/main.rs:        assert_eq!(mine.interface.to_string(), said.beacon_interface);
tcsverify/src/lib.rs:  beacon_address: \"239.255.0.1:5550\"
tcsverify/src/lib.rs:  beacon_interface: \"127.0.0.1\"
tcslibgs/tests/format_equivalence.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/tests/format_equivalence.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/config.rs:                      beacon_interval_ms: 5000\n  beacon_address: 239.255.0.7:7550\n  beacon_interface: 127.0.0.1\n\
tcslibgs/src/config.rs:        assert_eq!(section.beacon_address, "239.255.0.7:7550");
tcslibgs/src/config.rs:        assert_eq!(section.beacon_interface, "127.0.0.1");
tcslibgs/src/config.rs:                     beacon_interval_ms: 5000\n  beacon_address: 239.255.0.1:5550\n  \
tcslibgs/src/config.rs:                     beacon_interface: 127.0.0.1\n\
tcslibgs/src/config.rs:        assert_eq!(section.beacon_address, "239.255.0.1:5550");
tcslibgs/src/config.rs:        assert_eq!(section.beacon_interface, "127.0.0.1");
tcslibgs/src/config.rs:            "  beacon_address: 239.255.0.1:5550\n",
tcslibgs/src/config.rs:            "  beacon_interface: 127.0.0.1\n",
tcslibgs/src/graph/check.rs:            ("beacon_address", "239.255.0.1:5550"),
tcslibgs/src/graph/check.rs:            ("beacon_interface", "127.0.0.1"),
tcslibgs/src/graph/check.rs:            ("beacon_address", "10.0.0.1:5550", "multicast"),
tcslibgs/src/graph/mod.rs:             beacon_address: \"239.255.0.1:5550\"\n  beacon_interface: \"127.0.0.1\"\n\
tcslibgs/src/graph/parameters.rs:    ("beacon_address", true),
tcslibgs/src/graph/parameters.rs:    ("beacon_interface", true),
tcslibgs/src/graph/parameters.rs:        beacon_address: text("beacon_address")?,
tcslibgs/src/graph/parameters.rs:        beacon_interface: text("beacon_interface")?,
tcslibgs/src/graph/parameters.rs:            ("beacon_address", "239.255.0.1:5550"),
tcslibgs/src/graph/parameters.rs:            ("beacon_interface", "127.0.0.1"),
tcslibgs/src/graph/parameters.rs:        assert_eq!(config.beacon_address, "239.255.0.1:5550");
tcslibgs/src/graph/parameters.rs:        assert!(why.contains("beacon_address"), "{why}");
tcslibgs/src/config_digest.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/config_digest.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/config_digest.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/config_digest.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/config_digest.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/config_digest.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/config_digest.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/config_digest.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/types.rs:    /// Optional. `beacon_address` is read from here -- where a set's beacons
tcslibgs/src/types.rs:        .map_err(|e| format!("beacon_address \"{stated}\" is not an address and port: {e}"))?;
tcslibgs/src/types.rs:            "beacon_address \"{stated}\" is not a multicast group: beacons are sent to \
tcslibgs/src/types.rs:        format!("beacon_interface \"{stated}\" is not the address of a local interface: {e}")
tcslibgs/src/types.rs:    pub beacon_address: String,
tcslibgs/src/types.rs:    pub beacon_interface: String,
tcslibgs/src/types.rs:    pub beacon_address: SocketAddr,
tcslibgs/src/types.rs:    pub beacon_interface: Ipv4Addr,
tcslibgs/src/types.rs:        let beacon_address = beacon_address_of(&self.beacon_address)?;
tcslibgs/src/types.rs:        let beacon_interface = beacon_interface_of(&self.beacon_interface)?;
tcslibgs/src/types.rs:            beacon_address,
tcslibgs/src/types.rs:            beacon_interface,
tcslibgs/src/types.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/types.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/types.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/types.rs:  beacon_interface: 127.0.0.1
tcslibgs/src/types.rs:                 beacon_interval_ms: 5000\n  beacon_address: 239.255.0.1:5550\n  \
tcslibgs/src/types.rs:                 beacon_interface: 127.0.0.1\npayloads:\n  - \
tcslibgs/src/types.rs:                 beacon_address: 239.255.0.1:5550\n  \
tcslibgs/src/types.rs:                 beacon_interface: 127.0.0.1\npayloads:\n  - \
tcslibgs/src/types.rs:  beacon_address: 239.255.0.1:5550
tcslibgs/src/types.rs:  beacon_interface: 127.0.0.1
tcsgui/src/document.rs:        ("beacon_address", "239.255.0.1:5550"),
tcsgui/src/document.rs:        ("beacon_interface", "127.0.0.1"),
tcsgui/src/document.rs:            ("beacon_address", "239.255.0.1:5550"),
tcsgui/src/document.rs:            ("beacon_interface", "127.0.0.1"),
tcsgui/src/document.rs:            ("beacon_address", "10.0.0.1:5550", "multicast"),
