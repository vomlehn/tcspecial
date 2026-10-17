//! What a version 2 node says about the thing it reaches.
//!
//! A node of a graph is text: a type and a map of attributes as somebody wrote
//! them. What the flight software opens is an [`EndpointConfig`] -- a bus and
//! an address, a chip and three line numbers, a line and the framing of it.
//! This is the piece between, and it is deliberately thin: the rules about
//! what each type of interface may be told, and what each term means, are the
//! version 1 rules in [`crate::endpoint_config`] and the per-type modules
//! under [`crate::node`], and they are reached from here rather than written
//! again.
//!
//! That is the whole argument for the shape of this module. Those rules are
//! the ones that have been tested against real buses; a second set of them
//! for a second schema would be two statements of every rule, drifting apart
//! one flight at a time, and the first symptom would be a configuration that
//! verifies on the ground and opens something else aboard.
//!
//! # What this refuses
//!
//! An attribute it does not recognise, by name, for the type of node it is
//! on. That is the point of doing it here: a term belonging to another type --
//! a data rate on an I2C node -- is passed on to that type's own rules, which
//! have always refused it in their own words, but a term belonging to no type
//! at all would otherwise vanish. `max_speeed` silently ignored is a
//! peripheral clocked at whatever the last program left the bus at, and
//! nothing in the telemetry would ever say so.

use crate::endpoint_config::{
    a_link_term, endpoint_of, parse_u32, ByteList, EndpointLocation, LinkTerms, Scalar,
    StreamTerms,
};
use crate::graph::Node;
use crate::types::{host_port_of, DeviceConfig, EndpointConfig, HostPort, NetworkConfig};
use crate::{DHType, NetworkProtocol};

/// Everything a node's attributes say about reaching it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reach {
    /// What to open, and how to set it up once open.
    pub endpoint: EndpointConfig,
    /// How many bytes one read asks for, where the node says.
    ///
    /// A bus master clocks exactly as many bytes as it asks for, so what a
    /// read asks for is a property of the part: the forward sensor answers in
    /// two bytes and the one in the equipment bay in three. Absent for a node
    /// whose reads are bounded by the thing itself -- a whole CAN frame, a
    /// whole rpmsg message, a line event.
    pub read_size: Option<u32>,
    /// What to write to make this node answer, where it must be asked.
    ///
    /// A node with a trigger is a node that is asked; there is no separate
    /// word for the mode, because a trigger and nothing else is what made a
    /// version 1 payload triggered and the extra word could only disagree
    /// with it.
    pub trigger: Option<Vec<u8>>,
    /// Where a ground link's data goes: the `moc` node's `ground_base`.
    ///
    /// For a `moc` node, and only for one. What each node linked into the
    /// `moc` node produces is sent to this host, at this port plus that
    /// node's device id -- see `crate::graph::telemetry` -- so the ground
    /// tells the nodes apart by the port their data arrives at, and the
    /// vehicle sends from the first reading without being spoken to.
    pub ground: Option<HostPort>,
}

/// What this node reaches, or what is wrong with what it says.
///
/// The name is in every complaint: a configuration is read by somebody who
/// needs to know which of forty nodes is the one being refused.
pub fn reach_of(node: &Node) -> Result<Reach, String> {
    let what = node.name.as_str();
    let kind = kind_of(node)?;

    let mut terms = LinkTerms::default();
    let mut stream = StreamTerms {
        max_length: None,
        timeout: None,
        terminators: None,
    };
    let mut stream_stated = false;
    let mut read_size = None;
    let mut trigger = None;

    for (attribute, value) in &node.attributes {
        // The terms of a link go to the rules for the type, whichever type
        // they belong to: one on the wrong type is refused by that type's own
        // rules, in the words it has always used.
        if put_term(&mut terms, attribute, value) {
            continue;
        }

        match attribute.as_str() {
            // Where it is. Which attributes locate a node depends on its
            // type, and one that locates another type is refused rather than
            // ignored: a port on a node reached through a device file is a
            // file that looks configured and is not.
            other if LOCATORS.contains(&other) => {
                if !locators(kind).contains(&other) {
                    return Err(format!(
                        "\"{what}\" is {} {} node, so {other} does not locate it: \
                         it is located by {}",
                        kind.article(),
                        node.node_type,
                        locators(kind).join(" and ")
                    ));
                }
            }
            // Which group reads it, which is the schedule's business and not
            // this one's.
            "group" | "interval" => {}
            // Where a ground link's data goes, which only a ground link has:
            // read below.
            "ground_base" if node.node_type == "moc" => {}
            "ground_base" => {
                return Err(format!(
                    "\"{what}\" is {} {} node, and ground_base says where a ground \
                     link's data goes: only a moc node has one",
                    kind.article(),
                    node.node_type
                ))
            }
            "read_size" => {
                let size = number(what, "read_size", value)?;
                if size == 0 {
                    return Err(format!(
                        "\"{what}\": read_size must be more than nought -- a read of no \
                         bytes is not a read, and a node that is not read says nothing \
                         about it"
                    ));
                }
                read_size = Some(size);
            }
            "trigger" => {
                trigger = Some(
                    crate::trigger::trigger_bytes(value.as_str())
                        .map_err(|why| format!("\"{what}\": trigger: {why}"))?,
                );
            }
            // Where one read of a stream ends. Three attributes rather than a
            // section of its own, because a node's attributes are flat: a
            // version 1 file nested them under `stream`, and a version 2 node
            // has nowhere to nest them.
            "max_length" => {
                stream.max_length = Some(value.clone());
                stream_stated = true;
            }
            "read_timeout" => {
                stream.timeout = Some(value.clone());
                stream_stated = true;
            }
            "terminators" => {
                stream.terminators = Some(ByteList::of(value.as_str()));
                stream_stated = true;
            }
            other => {
                return Err(format!(
                    "\"{what}\": there is nothing called \"{other}\" on a \
                     {} node. An attribute nothing reads is worse than one that \
                     is refused: it looks like something that was set",
                    node.node_type
                ))
            }
        }
    }

    if stream_stated {
        terms.stream = Some(stream);
    }

    // The two types that are not on a link have no terms, so a term on one of
    // them is refused here: the per-type rules that would have refused it are
    // the ones that are never consulted for these two.
    if matches!(kind, DHType::Network | DHType::Device) {
        if let Some(term) = a_link_term(&terms, &[]) {
            return Err(format!(
                "\"{what}\" is {} {} node, which is not on a link, so {term} does \
                 not apply to it",
                kind.article(),
                node.node_type
            ));
        }
        if stream_stated {
            return Err(format!(
                "\"{what}\" is {} {} node, and where a read ends is a property of a \
                 line: max_length, read_timeout and terminators do not apply to it",
                kind.article(),
                node.node_type
            ));
        }
    }

    let endpoint = match kind {
        // A socket: a host and a port, and which protocol is spoken to it.
        // The MOC link is one of these, and a `moc` node needs no protocol --
        // a ground link is a datagram link, as the one tcspecial has always
        // had is.
        DHType::Network => {
            let protocol = match node.attributes.get("protocol") {
                Some(stated) => {
                    NetworkProtocol::from_spelling(stated.as_str()).ok_or_else(|| {
                        format!(
                            "\"{what}\": \"{}\" is not a protocol: there are {}",
                            stated.as_str(),
                            NetworkProtocol::spellings()
                        )
                    })?
                }
                None if node.node_type == "moc" => NetworkProtocol::Udp,
                None => {
                    return Err(format!(
                        "\"{what}\" is a network node, so it needs the protocol spoken \
                         to it: one of {}",
                        NetworkProtocol::spellings()
                    ))
                }
            };

            // A ground link that states neither address nor port binds
            // nothing: it only sends, to its ground_base, and the ground has
            // nowhere aboard to send to. One that receives -- commands, or a
            // line out of it -- must state both, which `Graph::check` holds it
            // to, knowing the links. Both or neither, whichever: half an
            // address is a mistake on any node.
            let has = |a: &str| node.attributes.contains_key(a);
            if node.node_type == "moc" && !has("address") && !has("port") {
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: UNBOUND.to_string(),
                    port: 0,
                })
            } else {
                EndpointConfig::Network(NetworkConfig {
                    protocol,
                    address: required(
                        what,
                        node,
                        "address",
                        "a network node",
                        "the host it is at",
                    )?
                    .as_str()
                    .to_string(),
                    port: port_of(what, node)?,
                })
            }
        }
        // A file, opened: see node::payload::device_file for why that is a type of its
        // own and not every other type as well.
        DHType::Device => EndpointConfig::Device(DeviceConfig {
            path: required(
                what,
                node,
                "path",
                "a device file node",
                "the file it opens",
            )?
            .as_str()
            .to_string(),
        }),
        // Everything else is on a link with terms of its own -- a rate, a
        // bus address, a clock mode -- and the rules for that type settle
        // them.
        kind => {
            let on_a_link = crate::types::kind_of_terms(what, kind, terms)?;
            endpoint_of(what, &location_of(what, node, kind)?, &on_a_link)?
        }
    };

    let ground = ground_of(what, node)?;

    Ok(Reach {
        endpoint,
        read_size,
        trigger,
        ground,
    })
}

/// What a `moc` node that binds nothing is given as its address: see
/// [`binds`].
const UNBOUND: &str = "0.0.0.0";

/// Whether a node's endpoint is bound aboard for the far end to reach.
///
/// False only for a `moc` node that states no address and no port, which only
/// sends. Everything else opens what it describes.
pub fn binds(node: &Node) -> bool {
    node.node_type != "moc"
        || node.attributes.contains_key("address")
        || node.attributes.contains_key("port")
}

/// Where a `moc` node's data goes: its `ground_base`, `address:port`.
///
/// Required: a ground link with nowhere written down to send would be one
/// that waits to be spoken to before it can report anything.
fn ground_of(what: &str, node: &Node) -> Result<Option<HostPort>, String> {
    if node.node_type != "moc" {
        return Ok(None);
    }
    let stated = required(
        what,
        node,
        "ground_base",
        "a ground link",
        "where the ground station is, as address:port -- each node linked into it \
         sends to that port plus its own device id",
    )?;
    host_port_of("ground_base", stated.as_str())
        .map(Some)
        .map_err(|why| format!("\"{what}\": {why}"))
}

/// Every attribute that says where some type of node is.
///
/// One list, so that an attribute which locates nothing is an unknown
/// attribute rather than one silently accepted on every type.
const LOCATORS: &[&str] = &["path", "address", "port", "protocol", "interface", "can_id"];

/// What locates a node of this type.
fn locators(kind: DHType) -> &'static [&'static str] {
    match kind {
        // A host and a port, and which protocol is spoken to it.
        DHType::Network => &["address", "port", "protocol"],
        // A bus and the address of one device on it.
        DHType::I2c => &["path", "address"],
        // An interface and the identifier the frames carry: a CAN controller
        // is a network interface and not a device file.
        DHType::Can => &["interface", "can_id"],
        // An interface and nothing else: a raw HDLC frame carries no address,
        // so one line is one node.
        DHType::Hdlc => &["interface"],
        // Everything else is a device node.
        _ => &["path"],
    }
}

/// Which of the version 1 types this node's type is.
///
/// The type names differ in two places and both are deliberate. `device-file`
/// is what version 2 calls a file opened in the filesystem, the word `device`
/// having meant that and a piece of hardware on a bus at once. And `moc` is a
/// network link named for what is at the far end of it rather than for how it
/// is reached, because that is the thing a reader of the configuration cares
/// about.
fn kind_of(node: &Node) -> Result<DHType, String> {
    match node.node_type.as_str() {
        "i2c" => Ok(DHType::I2c),
        "spi" => Ok(DHType::Spi),
        "serial" => Ok(DHType::Serial),
        "can" => Ok(DHType::Can),
        "iio" => Ok(DHType::Iio),
        "rpmsg" => Ok(DHType::Rpmsg),
        "gpio" => Ok(DHType::Gpio),
        "hdlc" => Ok(DHType::Hdlc),
        "device-file" => Ok(DHType::Device),
        "network" | "moc" => Ok(DHType::Network),
        "compute" => Err(format!(
            "\"{}\" is a compute node: it is a computation and not something \
             that is opened",
            node.name
        )),
        "log" => Err(format!(
            "\"{}\" is a log node: what it writes to is a directory, not an \
             interface",
            node.name
        )),
        // An app node is a program of its own, and what is opened for it is a
        // pair of sockets held by this process rather than an interface
        // reached by address: see `crate::node::app` and `tcsapp`.
        other if crate::node::app::Kind::of(other).is_some() => Err(format!(
            "\"{}\" is an app node: it is a program of its own, and what it is \
             reached over is made when it is started rather than named here",
            node.name
        )),
        other => Err(format!(
            "\"{}\": there is no node type called \"{other}\". The interfaces are \
             i2c, spi, serial, hdlc, can, iio, rpmsg, gpio, device-file and \
             network; a \
             ground link is moc; and compute, log, null and the app nodes are \
             the rest",
            node.name
        )),
    }
}

/// Where this node is, by the rules for its type.
fn location_of(
    what: &str,
    node: &Node,
    kind: DHType,
) -> Result<EndpointLocation, String> {
    // A CAN controller is a network interface and not a device file, and a
    // payload on it is told apart from the others by the identifier its
    // frames carry -- as a device on a bus is told apart by its address.
    if kind == DHType::Can {
        let interface = required(
            what,
            node,
            "interface",
            "a can node",
            "the interface its bus is reached through",
        )?;
        let can_id = required(
            what,
            node,
            "can_id",
            "a can node",
            "the identifier its frames carry",
        )?;
        let can_id = parse_u32(what, "can_id", can_id)
            .and_then(|id| crate::node::payload::can::check_can_id(what, id))
            .map_err(|e| format!("{e}"))?;
        return Ok(EndpointLocation::Can {
            interface: interface.as_str().to_string(),
            can_id,
        });
    }

    // A synchronous line is a network interface and nothing else.
    if kind == DHType::Hdlc {
        let interface = required(
            what,
            node,
            "interface",
            "an hdlc node",
            "the interface its line is",
        )?;
        return Ok(EndpointLocation::Interface {
            interface: interface.as_str().to_string(),
        });
    }

    let path = required(
        what,
        node,
        "path",
        &format!("{} {} node", kind.article(), kind.spelling()),
        "the device it is reached through",
    )?
    .as_str()
    .to_string();

    if kind != DHType::I2c {
        return Ok(EndpointLocation::Path { path });
    }

    // A bus is shared by design and an address on it is not, so both are
    // needed to say which device this is.
    let address = required(
        what,
        node,
        "address",
        "a node on an I2C bus",
        "the address of the device on it",
    )?;
    let ten_bit = node
        .attributes
        .get("ten_bit")
        .map(|t| t.as_str() == "true")
        .unwrap_or(false);

    Ok(EndpointLocation::I2c {
        bus: path,
        address: crate::node::payload::i2c::parse_i2c_address(what, address.as_str(), ten_bit)
            .map_err(|e| format!("{e}"))?,
    })
}

/// An attribute this type cannot do without.
fn required<'a>(
    what: &str,
    node: &'a Node,
    attribute: &str,
    is: &str,
    wants: &str,
) -> Result<&'a Scalar, String> {
    node.attributes.get(attribute).ok_or_else(|| {
        format!("\"{what}\" is {is}, so it needs {wants}: say {attribute}")
    })
}

/// A whole number an attribute states, in decimal or in hexadecimal.
fn number(what: &str, attribute: &str, value: &Scalar) -> Result<u32, String> {
    parse_u32(what, attribute, value).map_err(|e| format!("{e}"))
}

/// The port a network node is at.
fn port_of(what: &str, node: &Node) -> Result<u16, String> {
    port_named(what, node, "port", "a network node", "the port it is at")
}

/// A port, under whichever attribute states it.
fn port_named(
    what: &str,
    node: &Node,
    attribute: &str,
    is: &str,
    wants: &str,
) -> Result<u16, String> {
    let stated = required(what, node, attribute, is, wants)?;
    let port = number(what, attribute, stated)?;

    // Nought is the one number a socket would accept and a configuration
    // cannot mean: it asks the kernel for whichever port is free, so a
    // configuration that stated it would describe a link nothing could be
    // told where to find.
    if port == 0 || port > u16::MAX as u32 {
        return Err(format!(
            "\"{what}\": {attribute} {port} is not a port -- a port is 1 to 65535, and 0 \
             is \"whichever one is free\", which a configuration cannot mean"
        ));
    }

    Ok(port as u16)
}

/// Put one attribute in the term it names, saying whether it was one.
///
/// Every term of every type, in one place: a term is handed to the rules for
/// the node's own type whichever type it belongs to, and those rules refuse
/// the ones that are not theirs. The alternative -- a list per type here --
/// would be a second copy of what each type accepts, and the two would
/// disagree the first time a type gained a term.
fn put_term(terms: &mut LinkTerms, attribute: &str, value: &Scalar) -> bool {
    let value = Some(value.clone());
    match attribute {
        // A line.
        "datarate" => terms.datarate = value,
        "stop_bits" => terms.stop_bits = value,
        // A synchronous line.
        "standard" => terms.standard = value,
        "clock_rate" => terms.clock_rate = value,
        "frame_check" => terms.frame_check = value,
        "parity" => terms.parity = value,
        "clock_type" => terms.clock_type = value,
        "encoding" => terms.encoding = value,
        "loopback" => terms.loopback = value,
        "byte_length" => terms.byte_length = value,
        // A bus, and a device on one.
        "ten_bit" => terms.ten_bit = value,
        "pec" => terms.pec = value,
        "retries" => terms.retries = value,
        "timeout" => terms.timeout = value,
        // A peripheral on a SPI bus.
        "max_speed" => terms.max_speed = value,
        "spi_mode" => terms.spi_mode = value,
        "bits_per_word" => terms.bits_per_word = value,
        "bit_order" => terms.bit_order = value,
        "cs_active" => terms.cs_active = value,
        // A half-duplex RS-485 line.
        "rs485" => terms.rs485 = value,
        "rs485_rts_on_send" => terms.rs485_rts_on_send = value,
        "rs485_delay_before_send_ms" => terms.rs485_delay_before_send_ms = value,
        "rs485_delay_after_send_ms" => terms.rs485_delay_after_send_ms = value,
        // A CAN bus.
        "can_fd" => terms.can_fd = value,
        // A channel to another core.
        "service" => terms.service = value,
        "max_message_bytes" => terms.max_message_bytes = value,
        // Discrete lines.
        "lines" => terms.lines = value,
        "edge" => terms.edge = value,
        "bias" => terms.bias = value,
        "debounce_us" => terms.debounce_us = value,
        "direction" => terms.direction = value,
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use crate::node::payload::gpio::Direction;
    use crate::node::payload::spi::SpiMode;
    use std::collections::BTreeMap;
    use std::path::Path;

    /// The shipped example, which is what this has to be able to open.
    fn thermostat() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads")
    }

    /// A node built here rather than read, so a test can make one wrong.
    fn node(name: &str, node_type: &str, attributes: &[(&str, &str)]) -> Node {
        Node {
            name: name.to_string(),
            node_type: node_type.to_string(),
            group: None,
            attributes: attributes
                .iter()
                .map(|(a, v)| (a.to_string(), Scalar::of(*v)))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    /// A ground link says where the ground is, as one address and port.
    #[test]
    fn a_ground_link_says_where_the_ground_is() {
        let moc = node(
            "moc",
            "moc",
            &[("ground_base", "127.0.0.1:5100")],
        );
        let reach = reach_of(&moc).expect("it reads");
        let ground = reach.ground.expect("a ground");
        assert_eq!((ground.address.as_str(), ground.port), ("127.0.0.1", 5100));
        // And with no address of its own, it binds nothing.
        assert!(!binds(&moc));

        let mut without = moc.clone();
        without.attributes.remove("ground_base");
        let said = reach_of(&without).expect_err("a ground link says where");
        assert!(said.contains("ground_base"), "{said}");

        let mut portless = moc.clone();
        portless
            .attributes
            .insert("ground_base".to_string(), Scalar::of("127.0.0.1"));
        let said = reach_of(&portless).expect_err("an address and a port");
        assert!(said.contains("ground_base"), "{said}");
    }

    /// A ground link that receives states where, both halves of it.
    #[test]
    fn a_ground_link_binds_both_halves_or_neither() {
        let both = node(
            "moc",
            "moc",
            &[
                ("address", "127.0.0.1"),
                ("port", "5000"),
                ("ground_base", "127.0.0.1:5100"),
            ],
        );
        assert!(binds(&both));
        assert!(reach_of(&both).is_ok());

        for (missing, said_of) in [("address", "address"), ("port", "port")] {
            let mut half = both.clone();
            half.attributes.remove(missing);
            let said = reach_of(&half).expect_err("half an address");
            assert!(said.contains(said_of), "{said}");
        }
    }

    /// Only a ground link has a ground_base.
    #[test]
    fn a_ground_base_is_refused_anywhere_else() {
        let network = node(
            "sensor",
            "network",
            &[
                ("protocol", "udp"),
                ("address", "127.0.0.1"),
                ("port", "5001"),
                ("ground_base", "127.0.0.1:5100"),
            ],
        );
        let said = reach_of(&network).expect_err("not a ground link");
        assert!(said.contains("only a moc node"), "{said}");
    }

    /// Every device of the worked example becomes what the flight software
    /// opens.
    ///
    /// The example and not an arrangement resembling it: these are the four
    /// types the thermostat actually uses, read out of the shipped file.
    #[test]
    fn the_worked_example_opens() {
        let graph = thermostat();
        let reach = |name: &str| {
            reach_of(graph.node(name).expect("a node of the example"))
                .unwrap_or_else(|why| panic!("{name}: {why}"))
        };

        // A device on a bus: the bus, and the address that tells it from the
        // other sensor on the same wires.
        let fore = reach("temp_fore");
        match &fore.endpoint {
            EndpointConfig::I2c(i2c) => {
                assert_eq!(i2c.bus, "/dev/i2c-1");
                assert_eq!(i2c.address, 0x48, "0x48 and not the 72 a YAML reader makes of it");
                assert!(!i2c.ten_bit);
                assert!(!i2c.pec);
            }
            other => panic!("the forward sensor should be on a bus: {other:?}"),
        }
        assert_eq!(
            fore.read_size,
            Some(2),
            "two bytes, which is what the part answers in"
        );
        assert_eq!(fore.trigger, None);

        // And the one in the equipment bay, which is a peripheral on a SPI
        // bus and answers in three.
        let bay = reach("temp_bay");
        match &bay.endpoint {
            EndpointConfig::Spi(spi) => {
                assert_eq!(spi.path, "/dev/spidev0.0");
                assert_eq!(spi.max_speed, 1_000_000);
                assert_eq!(spi.mode, SpiMode::Mode0);
            }
            other => panic!("the bay sensor should be on a SPI bus: {other:?}"),
        }
        assert_eq!(bay.read_size, Some(3));

        // The heaters: three lines of one chip, driven.
        match &reach("heaters").endpoint {
            EndpointConfig::Gpio(gpio) => {
                assert_eq!(gpio.path, "/dev/gpiochip0");
                assert_eq!(gpio.lines, vec![3, 4, 5]);
                assert_eq!(gpio.direction, Direction::Output);
                assert_eq!(gpio.debounce_us, None);
            }
            other => panic!("the heaters should be discrete lines: {other:?}"),
        }

        // And the ground link, which needs no protocol: a ground link is a
        // datagram link.
        match &reach("moc").endpoint {
            EndpointConfig::Network(net) => {
                assert_eq!(net.protocol, NetworkProtocol::Udp);
                assert_eq!(net.address, "10.0.0.1");
                assert_eq!(net.port, 5000);
            }
            other => panic!("the ground link should be a socket: {other:?}"),
        }
    }

    /// What is not reached over an interface says so.
    #[test]
    fn a_computation_and_a_log_are_not_opened() {
        let graph = thermostat();

        let said = reach_of(graph.node("thermostat").expect("the thermostat"))
            .expect_err("a computation opened");
        assert!(
            said.contains("thermostat") && said.contains("not something that is opened"),
            "{said}"
        );

        let said = reach_of(graph.node("housekeeping").expect("the log"))
            .expect_err("a log opened");
        assert!(said.contains("directory"), "{said}");

        let said = reach_of(&node("mystery", "quantum", &[])).expect_err("a type that is not one");
        assert!(
            said.contains("quantum") && said.contains("device-file"),
            "the complaint should list the types there are: {said}"
        );
    }

    /// An attribute nothing reads is refused, not ignored.
    ///
    /// The reason this bridge checks names at all. A term belonging to
    /// another type reaches that type's own rules and is refused in their
    /// words; a term belonging to no type would simply vanish, and a
    /// peripheral clocked at whatever the last program left the bus at looks
    /// exactly like a configured one.
    #[test]
    fn an_attribute_nothing_reads_is_refused() {
        let said = reach_of(&node(
            "temp_bay",
            "spi",
            &[("path", "/dev/spidev0.0"), ("max_speeed", "1000000")],
        ))
        .expect_err("a misspelled term");
        assert!(
            said.contains("max_speeed") && said.contains("looks like something that was set"),
            "{said}"
        );
    }

    /// A term of another type of link is refused by that type's own rules.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = reach_of(&node(
            "temp_fore",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("datarate", "9600"),
            ],
        ))
        .expect_err("a line's rate on a bus");
        assert!(
            said.contains("datarate"),
            "the complaint should name the term: {said}"
        );
    }

    /// An attribute that locates another type of node is refused.
    #[test]
    fn an_attribute_that_locates_another_type_is_refused() {
        let said = reach_of(&node(
            "heaters",
            "gpio",
            &[("path", "/dev/gpiochip0"), ("lines", "3"), ("port", "5000")],
        ))
        .expect_err("a port on a set of lines");
        assert!(
            said.contains("port does not locate it") && said.contains("path"),
            "the complaint should say what does locate it: {said}"
        );

        // And the other way round: a socket has no device file.
        let said = reach_of(&node(
            "moc",
            "moc",
            &[("address", "10.0.0.1"), ("port", "5000"), ("path", "/dev/null")],
        ))
        .expect_err("a path on a socket");
        assert!(said.contains("path does not locate it"), "{said}");
    }

    /// A synchronous line is located by an interface and nothing else.
    #[test]
    fn a_synchronous_line_is_an_interface() {
        let reach = reach_of(&node(
            "link",
            "hdlc",
            &[
                ("interface", "hdlc0"),
                ("standard", "v35"),
                ("clock_type", "internal"),
                ("clock_rate", "2048000"),
                ("frame_check", "crc16_pr1_ccitt"),
            ],
        ))
        .expect("a line");

        match reach.endpoint {
            EndpointConfig::Hdlc(hdlc) => {
                assert_eq!(hdlc.interface, "hdlc0");
                assert_eq!(hdlc.clock_rate, Some(2_048_000));
            }
            other => panic!("expected a synchronous line: {other:?}"),
        }

        // A path does not locate one: the kernel drives such a line as a
        // network interface, so there is no file to open.
        let said = reach_of(&node(
            "link",
            "hdlc",
            &[
                ("path", "/dev/ttyS1"),
                ("standard", "v35"),
                ("clock_type", "external"),
            ],
        ))
        .expect_err("a path on a synchronous line");
        assert!(said.contains("path does not locate it"), "{said}");

        // And a start-stop line's words are not its.
        let said = reach_of(&node(
            "link",
            "hdlc",
            &[
                ("interface", "hdlc0"),
                ("standard", "v35"),
                ("clock_type", "external"),
                ("stop_bits", "1"),
            ],
        ))
        .expect_err("stop bits on a clocked line");
        assert!(said.contains("stop_bits"), "{said}");
    }

    /// A node that does not say where it is cannot be opened.
    #[test]
    fn a_node_that_says_nowhere_is_refused() {
        for (name, node_type, attributes, wanted) in [
            ("temp_fore", "i2c", vec![("path", "/dev/i2c-1")], "address"),
            ("temp_fore", "i2c", vec![("address", "0x48")], "path"),
            ("link", "moc", vec![("port", "5000")], "address"),
            ("link", "moc", vec![("address", "10.0.0.1")], "port"),
            ("file", "device-file", vec![], "path"),
            ("bus", "can", vec![("can_id", "0x201")], "interface"),
            ("bus", "can", vec![("interface", "vcan0")], "can_id"),
        ] {
            let said = reach_of(&node(name, node_type, &attributes))
                .expect_err("a node that says nowhere");
            assert!(
                said.contains(wanted),
                "the complaint should name what is missing ({wanted}): {said}"
            );
        }
    }

    /// A read of no bytes is not a read.
    #[test]
    fn a_read_size_of_nothing_is_refused() {
        let said = reach_of(&node(
            "temp_fore",
            "i2c",
            &[("path", "/dev/i2c-1"), ("address", "0x48"), ("read_size", "0")],
        ))
        .expect_err("a read of no bytes");
        assert!(said.contains("read_size"), "{said}");
    }

    /// A port is a port, and nought is not one.
    #[test]
    fn a_port_is_a_port() {
        let said = reach_of(&node(
            "link",
            "moc",
            &[("address", "10.0.0.1"), ("port", "70000")],
        ))
        .expect_err("a number that is not a port");
        assert!(said.contains("70000"), "{said}");

        let said = reach_of(&node("link", "moc", &[("address", "10.0.0.1"), ("port", "0")]))
            .expect_err("port nought");
        assert!(said.contains("whichever one is free"), "{said}");
    }

    /// A network node says which protocol is spoken to it; a MOC link need
    /// not.
    #[test]
    fn a_network_node_says_its_protocol() {
        let said = reach_of(&node(
            "payload",
            "network",
            &[("address", "127.0.0.1"), ("port", "5000")],
        ))
        .expect_err("a socket with no protocol");
        assert!(said.contains("protocol"), "{said}");

        let reach = reach_of(&node(
            "payload",
            "network",
            &[("address", "127.0.0.1"), ("port", "5000"), ("protocol", "tcp")],
        ))
        .expect("a TCP payload");
        match reach.endpoint {
            EndpointConfig::Network(net) => assert_eq!(net.protocol, NetworkProtocol::Tcp),
            other => panic!("{other:?}"),
        }

        let said = reach_of(&node(
            "payload",
            "network",
            &[("address", "127.0.0.1"), ("port", "5000"), ("protocol", "smoke")],
        ))
        .expect_err("a protocol that is not one");
        assert!(said.contains("smoke"), "{said}");
    }

    /// A line says where one of its reads ends, as three attributes.
    ///
    /// A version 1 file nests them under `stream`; a node's attributes are
    /// flat, so they are written beside the rest of the line's terms. The
    /// rule they are checked by is the same one.
    #[test]
    fn a_line_says_where_a_read_ends() {
        let reach = reach_of(&node(
            "console",
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("parity", "none"),
                ("byte_length", "8"),
                ("max_length", "512"),
                ("terminators", "0x0a"),
            ],
        ))
        .expect("a line");
        match reach.endpoint {
            EndpointConfig::Serial(serial) => {
                assert_eq!(serial.path, "/dev/ttyS0");
                assert_eq!(serial.datarate, 9600);
            }
            other => panic!("{other:?}"),
        }

        // A line that says nothing about where a read ends is refused, by the
        // rule that has always refused it.
        let said = reach_of(&node(
            "console",
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("parity", "none"),
                ("byte_length", "8"),
            ],
        ))
        .expect_err("a read with no end");
        assert!(said.contains("stream"), "{said}");

        // And those three on something that is not a line.
        let said = reach_of(&node(
            "file",
            "device-file",
            &[("path", "/dev/urandom"), ("max_length", "512")],
        ))
        .expect_err("where a read ends, on a file");
        assert!(
            said.contains("max_length") && said.contains("property of a"),
            "{said}"
        );
    }

    /// A node that must be asked carries what asks it.
    #[test]
    fn a_trigger_is_read_as_the_bytes_it_is() {
        let reach = reach_of(&node(
            "temp_fore",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("read_size", "2"),
                ("trigger", "00"),
            ],
        ))
        .expect("a sensor that is asked");
        assert_eq!(reach.trigger, Some(vec![0x00]));

        // A command and a register, which is what an interface document
        // commonly asks for: two values, the second of them two bytes.
        let reach = reach_of(&node(
            "temp_fore",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("trigger", "03 0400"),
            ],
        ))
        .expect("a sensor that is asked");
        assert_eq!(reach.trigger, Some(vec![0x03, 0x04, 0x00]));

        // And a trigger that is text, which is a C string: the quotes are
        // part of the value.
        let reach = reach_of(&node(
            "console",
            "device-file",
            &[("path", "/dev/ttyS0"), ("trigger", "\"READ\\r\"")],
        ))
        .expect("a console that is asked");
        assert_eq!(reach.trigger, Some(b"READ\r".to_vec()));

        let said = reach_of(&node(
            "temp_fore",
            "i2c",
            &[("path", "/dev/i2c-1"), ("address", "0x48"), ("trigger", "zz")],
        ))
        .expect_err("a trigger that is not bytes");
        assert!(said.contains("trigger"), "{said}");
    }

    /// The group a node is read with is the schedule's business, not this
    /// one's.
    #[test]
    fn a_group_is_not_an_attribute_of_the_interface() {
        let reach = reach_of(&node(
            "temp_fore",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("group", "cabin_temps"),
            ],
        ))
        .expect("a node in a group");
        assert!(matches!(reach.endpoint, EndpointConfig::I2c(_)));
    }
}
