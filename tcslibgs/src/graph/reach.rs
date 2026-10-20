//! What a node says about the thing it reaches.
//!
//! A node of a graph is text: a type and a map of attributes as somebody wrote
//! them. What the flight software opens is an [`EndpointConfig`] -- a bus and
//! an address, a chip and three line numbers, a line and the framing of it.
//! This is the piece between, and it is deliberately thin: the rules about
//! what each type of interface may be told, and what each term means, are the
//! rules in [`crate::endpoint_config`] and the per-type modules under
//! [`crate::node`], and they are reached from here rather than written again.
//!
//! That is the whole argument for the shape of this module. Those rules are
//! the ones that have been tested against real buses; a second set of them
//! written here would be two statements of every rule, drifting apart
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
    StreamParams, StreamTerms,
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
    /// word for the mode, because a trigger and nothing else is what makes a
    /// node triggered and the extra word could only disagree with it.
    pub trigger: Option<Vec<u8>>,
    /// How long to wait between asking this node and reading it.
    ///
    /// For a node with a trigger, and for no other: it is the time the part
    /// takes to answer, which its interface document states and nothing here
    /// can guess. With one, a pass asks, waits, and reads what came back; with
    /// none, a pass reads what the last ask produced and asks again, so the
    /// reading is one interval old and nothing waits.
    pub read_delay: Option<std::time::Duration>,
    /// Where a ground link's data goes: the `moc` node's `ground_base`.
    ///
    /// For a `moc` node, and only for one. What each node linked into the
    /// `moc` node produces is sent to this host, at this port plus that
    /// node's device id -- see `crate::graph::telemetry` -- so the ground
    /// tells the nodes apart by the port their data arrives at, and the
    /// vehicle sends from the first reading without being spoken to.
    pub ground: Option<HostPort>,
    /// Where one read of a line ends: the most it may hold, and the timeout
    /// and terminators that may end it sooner.
    ///
    /// For a line, and only for one: a stream delivers bytes with no frame
    /// boundaries of its own, so the vehicle holds what has arrived until one
    /// of these says a read is done. Every other interface's read is already
    /// a frame -- a datagram, a bus transfer, a CAN frame, a line event.
    pub stream: Option<StreamParams>,
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
    let mut read_delay = None;

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
            // When it is read, which is the schedule's business and not
            // this one's: the group it is read with, or the rate it states
            // for itself -- `trigger_interval` where it is asked and
            // `poll_interval` where it is simply read.
            "group" | "interval" | "trigger_interval" | "poll_interval" => {}
            // How long to wait between asking and reading, which is this
            // one's: it belongs to the part rather than to the schedule.
            "read_delay" => {
                read_delay = Some(
                    crate::graph::parse_interval(value.as_str())
                        .map_err(|why| format!("\"{what}\": read_delay: {why}"))?,
                );
            }
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
            // section of its own, because a node's attributes are flat and
            // there is nowhere to nest them.
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
            // One a configuration used to write, told what to say instead:
            // see `retired_term`, and the note there about why it is not a
            // misspelling.
            other if retired_term(other).is_some() => {
                return Err(format!(
                    "\"{what}\": \"{other}\" is no longer an attribute: {}",
                    retired_term(other).expect("just asked")
                ))
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

    // What the rules settle about where a read ends, for a line.
    let mut framing = None;
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
            framing = on_a_link.stream().cloned();
            endpoint_of(what, &location_of(what, node, kind)?, &on_a_link)?
        }
    };

    // A read is delivered as one datum, so one that could hold more than a
    // datum carries would arrive cut short -- and a line read in pieces is a
    // line whose pieces nothing knows how to put back together.
    if let Some(rule) = &framing {
        if rule.max_length as usize > crate::types::DATUM_BYTES {
            return Err(format!(
                "\"{what}\": max_length {} is more than one datum carries: a read \
                 is delivered whole, and the most a datum holds is {} bytes",
                rule.max_length,
                crate::types::DATUM_BYTES
            ));
        }
    }

    // What the interface itself will not do, which is the last thing to ask:
    // both of these read what the kind settled above.
    if trigger.is_some() {
        if let Some(why) = cannot_be_asked(&endpoint) {
            return Err(format!("\"{what}\" states a trigger, and {why}"));
        }
    }
    if let Some(size) = read_size {
        fits_one_read(what, size, &endpoint)?;
    }

    let ground = ground_of(what, node)?;

    Ok(Reach {
        endpoint,
        read_size,
        trigger,
        read_delay,
        ground,
        stream: framing,
    })
}

/// Why this interface cannot be asked for a reading, if it cannot.
///
/// A trigger is bytes written to a part to make it answer, so an interface
/// that cannot be written at all cannot carry one. Two cannot, and each for
/// its own reason:
///
/// A set of discrete lines is read or driven and not both. Writing to lines
/// that are watched would be driving them, which is commanding a device
/// rather than reading it, and the kernel's request is for one direction.
///
/// A sensor's sample buffer is read and never written. What an IIO device is
/// configured with -- its channels, its sample rate, its trigger in the
/// kernel's sense -- is in `sysfs` and is written by whoever brought the
/// sensor up, not by a datum on this link.
fn cannot_be_asked(endpoint: &EndpointConfig) -> Option<&'static str> {
    match endpoint {
        EndpointConfig::Gpio(_) => Some(
            "a set of discrete lines is read or driven and not both: writing to \
             lines that are watched would be commanding a device rather than \
             reading it",
        ),
        EndpointConfig::Iio(_) => Some(
            "a sensor's sample buffer is read and not written: what an IIO device \
             is configured with is in sysfs, written by whoever brought the sensor \
             up",
        ),
        _ => None,
    }
}

/// Whether one read of this size fits what the interface carries at once.
///
/// Two interfaces carry a bounded amount and refuse the rest rather than
/// splitting it, so a `read_size` larger than that is a read that can never
/// come back whole -- which is a configuration fault and not a thing to find
/// out in flight.
///
/// A CAN frame carries eight bytes, or sixty-four where the file says
/// `can_fd: true`; an rpmsg message carries what `max_message_bytes` says,
/// and 496 where it says nothing -- 512-byte buffers less the 16-byte header.
/// Every other interface is a stream or a register and has nothing to say
/// here: where one read ends is the `stream` rule's business.
fn fits_one_read(what: &str, size: u32, endpoint: &EndpointConfig) -> Result<(), String> {
    let (room, carries) = match endpoint {
        EndpointConfig::Can(can) => (
            crate::node::payload::can::frame_bytes(can.fd) as u32,
            if can.fd { "an FD frame" } else { "a frame" },
        ),
        EndpointConfig::Rpmsg(rpmsg) => (
            crate::node::payload::rpmsg::message_bytes(rpmsg.max_message_bytes),
            "a message",
        ),
        _ => return Ok(()),
    };

    if size > room {
        return Err(format!(
            "\"{what}\": read_size is {size} and {carries} carries {room} bytes: \
             the rest is refused rather than split, so a read of {size} could \
             never come back whole"
        ));
    }
    Ok(())
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

/// Which [`DHType`] this node's type is.
///
/// The type names differ from those in two places and both are deliberate.
/// `device-file` is what a configuration calls a file opened in the
/// filesystem, the word `device` having meant that and a piece of hardware on
/// a bus at once. And `moc` is a network link named for what is at the far end
/// of it rather than for how it is reached, because that is the thing a reader
/// of the configuration cares about.
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
            "\"{}\" is an app node: a program tcspecial runs, which is reached \
             over what tcspecial makes for it when it starts it, so it takes no \
             address, port or path -- it names its executable, and the \
             arguments its kind is run with",
            node.name
        )),
        other => Err(format!(
            "\"{}\": there is no node type called \"{other}\". The interfaces are \
             i2c, spi, serial, hdlc, can, iio, rpmsg, gpio, device-file and \
             network; a \
             ground link is moc; and compute, log, null-moc, null-payload, \
             tee-moc, tee-payload, ping and the app nodes are the rest",
            node.name
        )),
    }
}

/// Where this node is, by the rules for its type.
fn location_of(what: &str, node: &Node, kind: DHType) -> Result<EndpointLocation, String> {
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
    node.attributes
        .get(attribute)
        .ok_or_else(|| format!("\"{what}\" is {is}, so it needs {wants}: say {attribute}"))
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
///
/// A term a configuration used to write, and what to write instead.
///
/// The spellings three terms had before each became a length of time.
/// Refused by name rather than as a term nothing knows, because the value
/// changes with the name: a node that says `debounce_us: 50` is not a node
/// with a misspelling on it, it is a node written for the spelling before
/// this one. The files here were renamed with these, so the old spelling is
/// nobody's now.
pub fn retired_term(attribute: &str) -> Option<&'static str> {
    match attribute {
        "debounce_us" => Some("say debounce, as a length of time: 50 us rather than 50"),
        "rs485_delay_before_send_ms" => {
            Some("say rs485_delay_before_send, as a length of time: 2 ms rather than 2")
        }
        "rs485_delay_after_send_ms" => {
            Some("say rs485_delay_after_send, as a length of time: 2 ms rather than 2")
        }
        _ => None,
    }
}

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
        "rs485_delay_before_send" => terms.rs485_delay_before_send = value,
        "rs485_delay_after_send" => terms.rs485_delay_after_send = value,
        // A CAN bus.
        "can_fd" => terms.can_fd = value,
        // A channel to another core.
        "service" => terms.service = value,
        "max_message_bytes" => terms.max_message_bytes = value,
        // Discrete lines.
        "lines" => terms.lines = value,
        "edge" => terms.edge = value,
        "bias" => terms.bias = value,
        "debounce" => terms.debounce = value,
        "state_interval" => terms.state_interval = value,
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
    use std::time::Duration;

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
        let moc = node("moc", "moc", &[("ground_base", "127.0.0.1:5100")]);
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
        let fore = reach("temp_1");
        match &fore.endpoint {
            EndpointConfig::I2c(i2c) => {
                assert_eq!(i2c.bus, "/dev/i2c-1");
                assert_eq!(
                    i2c.address, 0x48,
                    "0x48 and not the 72 a YAML reader makes of it"
                );
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
        let bay = reach("temp_3");
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
        match &reach("heaters_1").endpoint {
            EndpointConfig::Gpio(gpio) => {
                assert_eq!(gpio.path, "/dev/gpiochip0");
                assert_eq!(gpio.lines, vec![3, 4, 5]);
                assert_eq!(gpio.direction, Direction::Output);
                assert_eq!(gpio.debounce, None);
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

        let said =
            reach_of(graph.node("housekeeping").expect("the log")).expect_err("a log opened");
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
            "temp_3",
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
            "temp_1",
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
            &[
                ("address", "10.0.0.1"),
                ("port", "5000"),
                ("path", "/dev/null"),
            ],
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
            ("temp_1", "i2c", vec![("path", "/dev/i2c-1")], "address"),
            ("temp_1", "i2c", vec![("address", "0x48")], "path"),
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

    /// Two kinds of interface cannot be asked, and a trigger on one is
    /// refused.
    ///
    /// A trigger is bytes written to a part to make it answer, and these two
    /// are not written: a set of discrete lines is read or driven and not
    /// both, and a sensor's sample buffer is read and never written -- what an
    /// IIO device is configured with is in `sysfs`. Each says which of the two
    /// reasons it is, because they are different mistakes: one is a direction
    /// and the other is a place.
    #[test]
    fn an_interface_that_cannot_be_asked_refuses_a_trigger() {
        let said = reach_of(&node(
            "asked_lines",
            "gpio",
            &[
                ("path", "/dev/gpiochip0"),
                ("lines", "0"),
                ("trigger", "55"),
            ],
        ))
        .expect_err("a set of discrete lines cannot be asked");
        assert!(
            said.contains("asked_lines") && said.contains("driven"),
            "the complaint should say why it cannot be asked: {said}"
        );

        let said = reach_of(&node(
            "asked_sensor",
            "iio",
            &[("path", "/dev/iio:device0"), ("trigger", "55")],
        ))
        .expect_err("a sample buffer cannot be asked");
        assert!(
            said.contains("asked_sensor") && said.contains("sysfs"),
            "the complaint should say where it is configured instead: {said}"
        );

        // And the kinds that can be asked still are: a register on a bus is
        // read by asking for it.
        reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("read_size", "2"),
                ("trigger", "55"),
            ],
        ))
        .expect("a register is read by asking for it");
    }

    /// A read larger than the interface carries at once is refused.
    ///
    /// Two interfaces carry a bounded amount and refuse the rest rather than
    /// splitting it, so such a read could never come back whole. A CAN frame
    /// carries eight bytes, or sixty-four in an FD frame; an rpmsg message
    /// carries 496 -- 512-byte buffers less the header -- or what
    /// `max_message_bytes` says. Both numbers are in the complaint, because
    /// what a reader has to decide is which of the two to change.
    #[test]
    fn a_read_larger_than_the_interface_carries_is_refused() {
        let frame = |terms: &[(&str, &str)]| {
            let mut all = vec![("interface", "vcan0"), ("can_id", "1")];
            all.extend_from_slice(terms);
            reach_of(&node("big_frame", "can", &all))
        };

        let said = frame(&[("read_size", "12")]).expect_err("twelve bytes in eight");
        assert!(said.contains("12") && said.contains('8'), "{said}");

        // Eight fits, and sixty-four fits an FD frame -- which is what the
        // flag is for.
        frame(&[("read_size", "8")]).expect("eight bytes of frame");
        frame(&[("read_size", "12"), ("can_fd", "true")]).expect("an FD frame has room");
        let said = frame(&[("read_size", "65"), ("can_fd", "true")])
            .expect_err("sixty-five bytes in sixty-four");
        assert!(said.contains("65") && said.contains("64"), "{said}");

        let message = |terms: &[(&str, &str)]| {
            let mut all = vec![("path", "/dev/rpmsg0"), ("service", "payload")];
            all.extend_from_slice(terms);
            reach_of(&node("big_message", "rpmsg", &all))
        };

        let said = message(&[("read_size", "600")]).expect_err("600 bytes in 496");
        assert!(said.contains("600") && said.contains("496"), "{said}");

        // And a platform with larger buffers says so, which makes it fit.
        message(&[("read_size", "600"), ("max_message_bytes", "2048")])
            .expect("a platform with room for it");
        let said = message(&[("read_size", "2049"), ("max_message_bytes", "2048")])
            .expect_err("one byte over what it said");
        assert!(said.contains("2049") && said.contains("2048"), "{said}");
    }

    /// A read of no bytes is not a read.
    #[test]
    fn a_read_size_of_nothing_is_refused() {
        let said = reach_of(&node(
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("read_size", "0"),
            ],
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

        let said = reach_of(&node(
            "link",
            "moc",
            &[("address", "10.0.0.1"), ("port", "0")],
        ))
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
            &[
                ("address", "127.0.0.1"),
                ("port", "5000"),
                ("protocol", "tcp"),
            ],
        ))
        .expect("a TCP payload");
        match reach.endpoint {
            EndpointConfig::Network(net) => assert_eq!(net.protocol, NetworkProtocol::Tcp),
            other => panic!("{other:?}"),
        }

        let said = reach_of(&node(
            "payload",
            "network",
            &[
                ("address", "127.0.0.1"),
                ("port", "5000"),
                ("protocol", "smoke"),
            ],
        ))
        .expect_err("a protocol that is not one");
        assert!(said.contains("smoke"), "{said}");
    }

    /// A line says where one of its reads ends, as three attributes.
    ///
    /// A node's attributes are flat, so they are written beside the rest of
    /// the line's terms.
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
            "temp_1",
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
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("trigger", "\"\\x03\\x04\\x00\""),
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
            "temp_1",
            "i2c",
            &[
                ("path", "/dev/i2c-1"),
                ("address", "0x48"),
                ("trigger", "zz"),
            ],
        ))
        .expect_err("a trigger that is not bytes");
        assert!(said.contains("trigger"), "{said}");
    }

    /// The group a node is read with is the schedule's business, not this
    /// one's.
    #[test]
    fn a_group_is_not_an_attribute_of_the_interface() {
        let reach = reach_of(&node(
            "temp_1",
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

    // -- the rules of a link, as a node states them ---------------------------
    //
    // A line, a bus and a peripheral each have rules of their own: which terms
    // the kind takes, which it must be told, and which belong to another kind.
    // The rules are stated once, in `endpoint_config` and its per-kind
    // modules, and tested there. These are the other half: that a node
    // stating them reaches an endpoint carrying them. A term read and dropped
    // on the way is a line set to something nobody asked for, and nothing in
    // the rule's own tests would notice.

    /// What a node of this type and these attributes reaches.
    fn reached(node_type: &str, attributes: &[(&str, &str)]) -> Result<Reach, String> {
        reach_of(&node("p", node_type, attributes))
    }

    /// The endpoint a node of this type and these attributes opens.
    fn endpoint(node_type: &str, attributes: &[(&str, &str)]) -> Result<EndpointConfig, String> {
        reached(node_type, attributes).map(|reach| reach.endpoint)
    }

    /// What the rules for the link settle, for the terms that never reach an
    /// endpoint -- where a read of a line ends, the knobs of a bus and a
    /// peripheral -- and so cannot be read back from `reach_of`.
    ///
    /// The node is read through `reach_of` first, so a refusal is the one a
    /// configuration gets. Then the terms are gathered as `reach_of` gathers
    /// them -- `put_term`, and the three attributes of where a read ends --
    /// and handed to the same `kind_of_terms` it hands them to.
    fn settled(
        node_type: &str,
        attributes: &[(&str, &str)],
    ) -> Result<crate::endpoint_config::GroupKind, String> {
        let node = node("p", node_type, attributes);
        reach_of(&node)?;
        let mut terms = LinkTerms::default();
        let mut stream = StreamTerms {
            max_length: None,
            timeout: None,
            terminators: None,
        };
        let mut stream_stated = false;
        for (attribute, value) in &node.attributes {
            if put_term(&mut terms, attribute, value) {
                continue;
            }
            match attribute.as_str() {
                "max_length" => stream.max_length = Some(value.clone()),
                "read_timeout" => stream.timeout = Some(value.clone()),
                "terminators" => stream.terminators = Some(ByteList::of(value.as_str())),
                _ => continue,
            }
            stream_stated = true;
        }
        if stream_stated {
            terms.stream = Some(stream);
        }
        crate::types::kind_of_terms("p", kind_of(&node)?, terms)
    }

    /// The terms of a line, which is what the stream rule belongs to.
    const A_LINE: &[(&str, &str)] = &[
        ("path", "/dev/ttyS0"),
        ("datarate", "9600"),
        ("stop_bits", "1"),
        ("byte_length", "8"),
    ];

    /// A line's terms with these beside them.
    fn line_with<'a>(extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
        A_LINE.iter().chain(extra).copied().collect()
    }

    /// An RS-485 bus reaches its endpoint with its turnaround on it.
    #[test]
    fn an_rs485_line_carries_its_turnaround_to_the_endpoint() {
        let bus = line_with(&[
            ("rs485", "true"),
            ("rs485_rts_on_send", "false"),
            ("rs485_delay_after_send", "2 ms"),
            ("max_length", "64"),
            ("read_timeout", "none"),
        ]);
        match endpoint("serial", &bus).expect("it opens") {
            EndpointConfig::Serial(serial) => {
                let rs485 = serial.rs485.expect("the bus reached the endpoint");
                assert!(!rs485.rts_on_send, "RTS is low while sending on this board");
                assert_eq!(
                    rs485.delay_before_send,
                    Duration::ZERO,
                    "nothing was asked for"
                );
                assert_eq!(rs485.delay_after_send, Duration::from_millis(2));
            }
            other => panic!("expected a serial endpoint, got {other:?}"),
        }

        // And a line that says nothing is not a bus.
        let plain = line_with(&[("max_length", "64"), ("read_timeout", "none")]);
        match endpoint("serial", &plain).expect("it opens") {
            EndpointConfig::Serial(serial) => assert_eq!(serial.rs485, None),
            other => panic!("expected a serial endpoint, got {other:?}"),
        }
    }

    /// The turnaround belongs to a line and to no other kind.
    #[test]
    fn rs485_terms_are_refused_on_a_kind_that_is_not_a_line() {
        let said = endpoint(
            "spi",
            &[
                ("path", "/dev/spidev0.0"),
                ("max_speed", "1000000"),
                ("spi_mode", "0"),
                ("rs485", "true"),
            ],
        )
        .expect_err("a peripheral is not an RS-485 bus");
        assert!(
            said.contains("rs485"),
            "the error should name the term: {said}"
        );
    }

    /// A GPIO node reaches its endpoint with the lines it watches and how.
    #[test]
    fn a_gpio_node_is_a_chip_and_the_lines_it_watches() {
        let opened = endpoint(
            "gpio",
            &[
                ("path", "/dev/gpiochip0"),
                ("lines", "3, 7"),
                ("edge", "rising"),
                ("bias", "pull_up"),
                ("debounce", "5000 us"),
            ],
        )
        .expect("it opens");
        match &opened {
            EndpointConfig::Gpio(gpio) => {
                assert_eq!(gpio.path, "/dev/gpiochip0");
                assert_eq!(gpio.lines, vec![3, 7]);
                assert_eq!(gpio.edge, crate::LineEdge::Rising);
                assert_eq!(gpio.bias, crate::Bias::PullUp);
                assert_eq!(gpio.debounce, Some(Duration::from_micros(5_000)));
            }
            other => panic!("expected a GPIO endpoint, got {other:?}"),
        }
        assert_eq!(opened.kind(), DHType::Gpio);
    }

    /// An rpmsg node reaches its endpoint with its channel and its service.
    #[test]
    fn an_rpmsg_node_is_a_channel_and_the_service_it_should_be() {
        let opened = endpoint(
            "rpmsg",
            &[("path", "/dev/rpmsg0"), ("service", "payload-telemetry")],
        )
        .expect("it opens");
        match &opened {
            EndpointConfig::Rpmsg(rpmsg) => {
                assert_eq!(rpmsg.path, "/dev/rpmsg0");
                assert_eq!(rpmsg.service, "payload-telemetry");
                assert_eq!(rpmsg.max_message_bytes, None, "the usual transport");
            }
            other => panic!("expected an rpmsg endpoint, got {other:?}"),
        }
        assert_eq!(opened.kind(), DHType::Rpmsg);

        // The service is what identifies a channel, so a node without one is
        // refused: a channel's number is assigned as channels appear.
        let said =
            endpoint("rpmsg", &[("path", "/dev/rpmsg0")]).expect_err("a channel with no service");
        assert!(
            said.contains("service"),
            "the error should name what is missing: {said}"
        );
    }

    /// A network node may be asked, whatever its protocol, datagrams
    /// included.
    ///
    /// A node states where it is, so the vehicle has somewhere to send a
    /// trigger before the node has said anything: the question goes to the
    /// address and port the node states, and the answer comes back on the
    /// same socket.
    #[test]
    fn a_datagram_node_may_be_triggered() {
        for protocol in ["udp", "unix_dgram", "tcp"] {
            let reach = reached(
                "network",
                &[
                    ("protocol", protocol),
                    ("address", "127.0.0.1"),
                    ("port", "5000"),
                    ("trigger", "\"READ\""),
                ],
            )
            .unwrap_or_else(|e| panic!("{protocol}: {e}"));
            assert_eq!(
                reach.trigger.as_deref(),
                Some(b"READ".as_slice()),
                "{protocol}"
            );
        }
    }

    /// An rpmsg node may be asked: the channel carries both directions.
    #[test]
    fn an_rpmsg_node_may_be_triggered() {
        let asked = endpoint(
            "rpmsg",
            &[
                ("path", "/dev/rpmsg0"),
                ("service", "payload"),
                ("trigger", "\"READ\""),
            ],
        )
        .expect("a channel may be asked");
        assert!(matches!(asked, EndpointConfig::Rpmsg(_)), "{asked:?}");
    }

    /// A CAN node reaches its endpoint with its interface and identifier.
    ///
    /// Which is the whole of where it is: a CAN controller is a network
    /// interface rather than a device file, and the identifier its frames
    /// carry is what tells one node on a shared bus from another.
    #[test]
    fn a_can_node_is_an_interface_and_an_identifier() {
        let opened =
            endpoint("can", &[("interface", "vcan0"), ("can_id", "0x123")]).expect("it opens");
        match &opened {
            EndpointConfig::Can(can) => {
                assert_eq!(can.interface, "vcan0");
                assert_eq!(can.can_id, 0x123);
                assert!(!can.fd, "classic CAN unless the file says otherwise");
                assert!(
                    can.loopback,
                    "and looped back, which is what the kernel does"
                );
            }
            other => panic!("expected a CAN endpoint, got {other:?}"),
        }
        assert_eq!(opened.kind(), DHType::Can);
    }

    /// A CAN node needs both halves of where it is.
    #[test]
    fn a_can_node_without_an_interface_or_an_id_is_refused() {
        for (missing, stated) in [
            ("interface", ("can_id", "1")),
            ("can_id", ("interface", "vcan0")),
        ] {
            let said = endpoint("can", &[stated]).expect_err("half of where it is");
            assert!(
                said.contains(missing),
                "the error should name what is missing: {said}"
            );
        }
    }

    // -- where a read of a line ends ------------------------------------------

    /// The stream rule a line with these attributes settles.
    fn line_stream(
        attributes: &[(&str, &str)],
    ) -> Result<crate::endpoint_config::StreamParams, String> {
        let line = line_with(attributes);
        let terms = settled("serial", &line)?;
        Ok(terms.stream().expect("a line has a stream rule").clone())
    }

    /// A line's reach carries where its reads end, so that the vehicle can
    /// end them there; nothing else's does.
    #[test]
    fn a_line_carries_its_stream_rule_and_nothing_else_does() {
        let line = line_with(&[("max_length", "80"), ("terminators", "0x0A")]);
        let reach = reached("serial", &line).expect("a line");
        let rule = reach.stream.expect("a line's reads end somewhere");
        assert_eq!(rule.max_length, 80);
        assert_eq!(rule.terminators, vec![0x0A]);

        let bus = reached("i2c", &[("path", "/dev/i2c-1"), ("address", "0x48")]).expect("a bus");
        assert_eq!(bus.stream, None, "a bus transfer is already a frame");
    }

    /// A line whose reads could be longer than a datum is refused: a read is
    /// delivered whole, and one cut short is a line read in pieces nothing
    /// knows how to put back together.
    #[test]
    fn a_read_longer_than_a_datum_is_refused() {
        let most = crate::types::DATUM_BYTES.to_string();
        let one_more = (crate::types::DATUM_BYTES + 1).to_string();
        let fits = line_with(&[("max_length", &most), ("read_timeout", "none")]);
        reached("serial", &fits).expect("a datum's worth fits");
        let over = line_with(&[("max_length", &one_more), ("read_timeout", "none")]);
        let said = reached("serial", &over).expect_err("more than a datum");
        assert!(
            said.contains("max_length") && said.contains(&most),
            "{said}"
        );
    }

    #[test]
    fn a_stream_rule_must_say_how_long_a_read_may_be() {
        let said = line_stream(&[("read_timeout", "1 s")]).expect_err("max_length is required");
        assert!(said.contains("max_length"), "{said}");
    }

    #[test]
    fn a_timeout_alone_ends_a_read() {
        let stream =
            line_stream(&[("max_length", "64"), ("read_timeout", "250 ms")]).expect("accepted");
        assert_eq!(stream.timeout, Some(Duration::from_millis(250)));
        assert!(stream.terminators.is_empty());
        assert!(!stream.is_fixed_length());
    }

    #[test]
    fn terminators_alone_end_a_read() {
        let stream = line_stream(&[("max_length", "64"), ("terminators", "10")]).expect("accepted");
        assert_eq!(stream.timeout, None);
        assert_eq!(stream.terminators, vec![10]);
        assert!(!stream.is_fixed_length());
    }

    #[test]
    fn a_timeout_and_terminators_may_both_end_a_read() {
        let stream = line_stream(&[
            ("max_length", "64"),
            ("read_timeout", "1 s"),
            ("terminators", "0x04"),
        ])
        .expect("accepted");
        assert_eq!(stream.timeout, Some(Duration::from_secs(1)));
        assert_eq!(stream.terminators, vec![0x04]);
    }

    #[test]
    fn a_stream_rule_that_ends_no_read_is_refused() {
        let said = line_stream(&[("max_length", "64")]).expect_err("nothing ends the read");
        assert!(
            said.contains("timeout") && said.contains("terminator"),
            "{said}"
        );
    }

    #[test]
    fn timeout_none_is_how_a_fixed_length_read_is_asked_for() {
        // The documented way to say "hand me max_length bytes at a time".
        let stream =
            line_stream(&[("max_length", "12"), ("read_timeout", "none")]).expect("accepted");
        assert_eq!(stream.max_length, 12);
        assert_eq!(stream.timeout, None);
        assert!(stream.terminators.is_empty());
        assert!(stream.is_fixed_length());
    }

    #[test]
    fn an_empty_terminator_list_is_refused() {
        let said = line_stream(&[("max_length", "64"), ("terminators", "")])
            .expect_err("an empty list is not a list");
        assert!(said.contains("terminator"), "{said}");
    }

    #[test]
    fn a_timeouts_units_are_understood() {
        for (text, want) in [
            ("900 us", Duration::from_micros(900)),
            ("250 ms", Duration::from_millis(250)),
            ("3 s", Duration::from_secs(3)),
        ] {
            let stream = line_stream(&[("max_length", "8"), ("read_timeout", text)])
                .unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(stream.timeout, Some(want), "for {text}");
        }
    }

    /// A number with no unit after it is refused, and so is one written
    /// against its unit.
    ///
    /// Two halves of the same rule: a length of time is a number, whitespace
    /// and a unit, so "250" is missing the unit and "250ms" is missing the
    /// whitespace. Each is told which, rather than both being told that the
    /// value is not a length of time.
    #[test]
    fn a_timeout_without_a_unit_is_refused() {
        let said =
            line_stream(&[("max_length", "8"), ("read_timeout", "250")]).expect_err("250 of what");
        assert!(said.contains("a number, a space, and a unit"), "{said}");

        let said = line_stream(&[("max_length", "8"), ("read_timeout", "250ms")])
            .expect_err("written together");
        assert!(said.contains("\"250 ms\""), "{said}");
    }

    #[test]
    fn a_zero_timeout_is_refused_in_favour_of_none() {
        let said = line_stream(&[("max_length", "8"), ("read_timeout", "0 ms")])
            .expect_err("a read that times out at once reads nothing");
        assert!(
            said.contains("no interval at all") && said.contains("none"),
            "{said}"
        );
    }

    /// A timeout takes every unit and every number an interval takes.
    ///
    /// One grammar for a length of time: see `graph::parse_interval`, which
    /// this reads through. What a timeout has of its own is the word `none`.
    #[test]
    fn a_timeout_takes_what_an_interval_takes() {
        for (text, want) in [
            ("1.5 s", Duration::from_millis(1_500)),
            ("5 min", Duration::from_secs(300)),
            ("1 h", Duration::from_secs(3_600)),
            ("2_000 ms", Duration::from_secs(2)),
        ] {
            let stream = line_stream(&[("max_length", "8"), ("read_timeout", text)])
                .unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(stream.timeout, Some(want), "for {text}");
        }

        // And what neither takes: an ambiguous unit, by the name the one
        // grammar refuses it with.
        for text in ["5m", "1y", "6mon", "0x8ms"] {
            let said = line_stream(&[("max_length", "8"), ("read_timeout", text)])
                .expect_err("not a length of time");
            assert!(said.contains("timeout"), "{text}: {said}");
        }
    }

    /// Terminators are one attribute, so a list of them is written as text:
    /// the bytes, separated by commas.
    #[test]
    fn terminators_are_a_string_of_them() {
        let stream =
            line_stream(&[("max_length", "8"), ("terminators", "0x0D, 0x0A")]).expect("a string");
        assert_eq!(stream.terminators, vec![0x0D, 0x0A]);
    }

    #[test]
    fn terminators_are_written_in_decimal_or_hexadecimal() {
        let stream = line_stream(&[("max_length", "8"), ("terminators", "13, 0x0A")])
            .expect("either spelling");
        assert_eq!(stream.terminators, vec![13, 10]);
    }

    #[test]
    fn a_terminator_wider_than_a_byte_is_refused() {
        let said = line_stream(&[("max_length", "8"), ("terminators", "256")])
            .expect_err("a line carries bytes");
        assert!(said.contains("not a byte value"), "{said}");
    }

    // -- the framing of a line ------------------------------------------------

    /// What the rules settle for a line with these terms, beside a path, a
    /// rate, a byte length and a fixed-length read.
    fn line_terms(extra: &[(&str, &str)]) -> Result<crate::endpoint_config::GroupKind, String> {
        let mut line: Vec<(&str, &str)> = vec![
            ("path", "/dev/ttyS0"),
            ("datarate", "9600"),
            ("byte_length", "8"),
            ("max_length", "8"),
            ("read_timeout", "none"),
        ];
        line.extend_from_slice(extra);
        settled("serial", &line)
    }

    /// A line says its framing, and the words of a clocked line are not its.
    ///
    /// This type is start-stop framing and nothing else. A line whose bits are
    /// carried on a clock is an `hdlc` node -- a network interface, with the
    /// clock and the coding set on it -- so the words that belong to one are
    /// refused here.
    #[test]
    fn a_line_is_start_stop_and_says_so() {
        let serial = |extra: &[(&str, &str)]| match line_terms(extra) {
            Ok(crate::endpoint_config::GroupKind::Serial(serial)) => serial,
            Ok(other) => panic!("{other:?}"),
            Err(e) => panic!("{extra:?}: {e}"),
        };

        // Stop bits are required: they are what start-stop framing uses in
        // place of a clock, so a line without them is not one of these.
        let start_stop = serial(&[("stop_bits", "2")]);
        assert_eq!(start_stop.stop_bits, crate::StopBits::Two);
        let said = line_terms(&[]).expect_err("stop bits are required");
        assert!(said.contains("stop_bits"), "{said}");

        // Parity defaults to none, which is what nearly every device link
        // uses.
        assert_eq!(start_stop.parity, crate::Parity::None);
        assert_eq!(
            serial(&[("stop_bits", "1"), ("parity", "even")]).parity,
            crate::Parity::Even
        );

        // A clocked line's words belong to the hdlc type and are refused
        // here, each by name.
        for stated in [
            ("clock_type", "external"),
            ("encoding", "nrzi"),
            ("clock_rate", "64000"),
            ("frame_check", "crc16_pr1"),
            ("standard", "v35"),
            ("loopback", "true"),
        ] {
            let said = line_terms(&[("stop_bits", "1"), stated])
                .expect_err("a clocked line's word on a start-stop line");
            assert!(said.contains(stated.0), "{stated:?}: {said}");
        }

        // And a parity that is a frame check is refused with the five a
        // character can have: a CRC is not one of them.
        let said = line_terms(&[("stop_bits", "1"), ("parity", "crc16_pr1")])
            .expect_err("a character's parity is not a frame check");
        assert!(said.contains("none, even, odd, mark, or space"), "{said}");
    }

    #[test]
    fn stop_bits_are_one_one_and_a_half_or_two() {
        for (text, want) in [
            ("1", crate::StopBits::One),
            ("1.5", crate::StopBits::OnePointFive),
            ("2", crate::StopBits::Two),
        ] {
            match line_terms(&[("stop_bits", text)]).unwrap_or_else(|e| panic!("{text}: {e}")) {
                crate::endpoint_config::GroupKind::Serial(serial) => {
                    assert_eq!(serial.stop_bits, want, "for {text}")
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn a_byte_length_no_uart_offers_is_refused() {
        let said = settled(
            "serial",
            &[
                ("path", "/dev/ttyS0"),
                ("datarate", "9600"),
                ("stop_bits", "1"),
                ("byte_length", "9"),
                ("max_length", "8"),
                ("read_timeout", "none"),
            ],
        )
        .expect_err("a UART offers five to eight bits");
        assert!(said.contains("out of range"), "{said}");
    }

    // -- a bus and what sits on it --------------------------------------------

    /// One device on a bus, with these terms beside its bus and address.
    fn bus(extra: &[(&str, &str)]) -> Result<crate::node::payload::i2c::I2cParams, String> {
        let mut device: Vec<(&str, &str)> = vec![("path", "/dev/i2c-1")];
        device.extend_from_slice(extra);
        match settled("i2c", &device)? {
            crate::endpoint_config::GroupKind::I2c(i2c) => Ok(i2c),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_bus_reads_its_terms() {
        let i2c = bus(&[
            ("ten_bit", "false"),
            ("pec", "true"),
            ("retries", "3"),
            ("timeout", "50 ms"),
            ("address", "0x48"),
        ])
        .expect("every term of a bus");
        assert!(!i2c.ten_bit);
        assert!(i2c.pec);
        assert_eq!(i2c.retries, 3);
        assert_eq!(i2c.timeout, Some(Duration::from_millis(50)));
    }

    #[test]
    fn every_term_of_a_bus_has_a_default() {
        // Nothing but the type and where it is is required: the protocol
        // fixes the framing, and every knob here has a quiet setting.
        let i2c = bus(&[("address", "16")]).expect("a bus states little");
        assert!(!i2c.ten_bit);
        assert!(!i2c.pec);
        assert_eq!(i2c.retries, 0);
        assert_eq!(i2c.timeout, None);
    }

    #[test]
    fn a_bus_timeout_is_rounded_up_to_the_drivers_resolution() {
        let i2c = bus(&[("timeout", "25 ms"), ("address", "8")]).expect("a timeout");
        assert_eq!(i2c.timeout, Some(Duration::from_millis(25)));
        assert_eq!(i2c.effective_timeout(), Some(Duration::from_millis(30)));
    }

    #[test]
    fn a_device_on_a_bus_carries_the_bus_and_its_place_on_it() {
        let place =
            endpoint("i2c", &[("path", "/dev/i2c-2"), ("address", "0x49")]).expect("a place");
        assert_eq!(
            place,
            EndpointConfig::I2c(crate::types::I2cConfig {
                bus: "/dev/i2c-2".to_string(),
                address: 0x49,
                ten_bit: false,
                pec: false,
            })
        );
    }

    #[test]
    fn an_address_too_wide_for_the_addressing_is_refused() {
        let said = endpoint("i2c", &[("path", "/dev/i2c-2"), ("address", "0x90")])
            .expect_err("0x90 needs eight bits");
        assert!(said.contains("does not fit in 7 bits"), "{said}");

        // ...and the same address is fine once the node says so.
        match endpoint(
            "i2c",
            &[
                ("path", "/dev/i2c-2"),
                ("address", "0x90"),
                ("ten_bit", "true"),
            ],
        )
        .expect("ten bits hold it")
        {
            EndpointConfig::I2c(i2c) => {
                assert_eq!(i2c.address, 0x90);
                assert!(i2c.ten_bit);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_address_the_specification_reserves_is_refused() {
        // In range, but the specification keeps these, so no device can
        // answer to one.
        for address in ["0x00", "0x01", "0x07", "0x78", "0x7B", "0x7F"] {
            let said = endpoint("i2c", &[("path", "/dev/i2c-1"), ("address", address)])
                .expect_err(address);
            assert!(
                said.contains("reserved by the I2C specification"),
                "for {address}: {said}"
            );
        }
    }

    #[test]
    fn the_addresses_either_side_of_the_reserved_blocks_are_accepted() {
        for (address, want) in [("0x08", 0x08), ("0x77", 0x77)] {
            match endpoint("i2c", &[("path", "/dev/i2c-1"), ("address", address)])
                .unwrap_or_else(|e| panic!("{address} should be usable: {e}"))
            {
                EndpointConfig::I2c(i2c) => assert_eq!(i2c.address, want, "for {address}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn ten_bit_addressing_has_no_reserved_block() {
        // A 10-bit transfer carries its address after the 0x78 prefix, so the
        // whole space is available and the 7-bit reservations do not apply.
        for address in ["0x00", "0x78", "0x3FF"] {
            endpoint(
                "i2c",
                &[
                    ("path", "/dev/i2c-1"),
                    ("ten_bit", "true"),
                    ("address", address),
                ],
            )
            .unwrap_or_else(|e| panic!("{address} should be usable with ten_bit: {e}"));
        }
    }

    #[test]
    fn a_device_on_a_bus_says_where_on_it_it_is() {
        let said = endpoint("i2c", &[("path", "/dev/i2c-1")]).expect_err("which device on the bus");
        assert!(said.contains("address"), "{said}");

        // And which bus, the two together being what tells one device on a
        // bus from another.
        let said = endpoint("i2c", &[("address", "0x48")]).expect_err("which bus");
        assert!(said.contains("path"), "{said}");
    }

    // -- a peripheral ---------------------------------------------------------

    /// One peripheral with these terms beside its device file.
    fn peripheral(extra: &[(&str, &str)]) -> Result<crate::node::payload::spi::SpiParams, String> {
        let mut device: Vec<(&str, &str)> = vec![("path", "/dev/spidev0.0")];
        device.extend_from_slice(extra);
        match settled("spi", &device)? {
            crate::endpoint_config::GroupKind::Spi(spi) => Ok(spi),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_peripheral_reads_its_terms() {
        let spi = peripheral(&[
            ("max_speed", "10000000"),
            ("spi_mode", "3"),
            ("bits_per_word", "16"),
            ("bit_order", "lsb"),
            ("cs_active", "high"),
        ])
        .expect("every term of a peripheral");
        assert_eq!(spi.max_speed, 10_000_000);
        assert_eq!(spi.mode, SpiMode::Mode3);
        assert_eq!(spi.bits_per_word.bits(), 16);
        assert_eq!(spi.bit_order, crate::BitOrder::LsbFirst);
        assert_eq!(spi.cs_active, crate::CsActive::High);
    }

    #[test]
    fn a_peripherals_defaults_are_eight_bit_msb_first_and_active_low() {
        let spi = peripheral(&[("max_speed", "1000000"), ("spi_mode", "0")]).expect("the usual");
        assert_eq!(spi.bits_per_word.bits(), 8);
        assert_eq!(spi.bit_order, crate::BitOrder::MsbFirst);
        assert_eq!(spi.cs_active, crate::CsActive::Low);
    }

    #[test]
    fn a_peripheral_says_which_mode_it_is_clocked_in() {
        let said = peripheral(&[("max_speed", "1000000")]).expect_err("a mode is required");
        assert!(said.contains("mode"), "{said}");

        let said = peripheral(&[("max_speed", "1000000"), ("spi_mode", "4")])
            .expect_err("there are four of them");
        assert!(said.contains("not one of 0, 1, 2, or 3"), "{said}");
    }

    #[test]
    fn a_bus_is_given_no_stream_rule() {
        // A master clocks exactly as many bytes as it asks for, so there is
        // no rule to give for where a read ends.
        for (kind, rest) in [
            ("i2c", vec![("path", "/dev/i2c-1"), ("address", "0x40")]),
            (
                "spi",
                vec![
                    ("path", "/dev/spidev0.0"),
                    ("max_speed", "1"),
                    ("spi_mode", "0"),
                ],
            ),
        ] {
            let mut stated = rest.clone();
            stated.extend_from_slice(&[("max_length", "8"), ("read_timeout", "none")]);
            let said = endpoint(kind, &stated).expect_err(kind);
            assert!(said.contains("does not apply"), "{kind}: {said}");
        }
    }

    #[test]
    fn a_term_of_another_kind_of_link_is_refused_by_that_kind() {
        // The rule that a misplaced term is reported where it was written,
        // checked across the cross-product rather than one way.
        let i2c = vec![("path", "/dev/i2c-1"), ("address", "0x40")];
        let spi = vec![
            ("path", "/dev/spidev0.0"),
            ("max_speed", "1"),
            ("spi_mode", "0"),
        ];
        let serial = vec![
            ("path", "/dev/ttyS0"),
            ("datarate", "9600"),
            ("stop_bits", "1"),
            ("byte_length", "8"),
            ("max_length", "8"),
            ("read_timeout", "none"),
        ];
        for (kind, rest, foreign) in [
            ("i2c", &i2c, ("datarate", "9600")),
            ("i2c", &i2c, ("spi_mode", "0")),
            ("spi", &spi, ("pec", "true")),
            ("serial", &serial, ("cs_active", "low")),
        ] {
            let mut stated = rest.clone();
            stated.push(foreign);
            let said = endpoint(kind, &stated).expect_err(foreign.0);
            assert!(
                said.contains(foreign.0) && said.contains("does not apply"),
                "{foreign:?} on {kind}: {said}"
            );
        }
    }
}
