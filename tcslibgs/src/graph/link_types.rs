//! Which type each terminal of each node carries, and whether each link's
//! two ends agree.
//!
//! A link has no type of its own: it has the types its two terminals carry, and
//! the two must be the same exactly. Each node says what its terminals carry,
//! outright -- a `to_kelvin` sends `kelvin` -- or by an attribute -- a vote's
//! `carries`, a thermostat's `type-in` and `type-out`, a device's
//! `world-out-type` -- and nothing is inferred from what is upstream. So a link
//! is judged by its two ends alone, and a refusal names one link. A null node,
//! a log and a gateway node take any type. See "Typed links" in
//! `docs/tcspecial-design.rst`, and [`crate::link_type`] for the types
//! themselves.

use super::terminals::{answers, terminal_at, terminals_of};
use super::{Destination, Graph, Node};
use crate::link_type::{Basic, LinkType};
use crate::node::vehicle::compute::{stated_type, Format};
use crate::unit::Unit;

/// What a terminal carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carries {
    /// This type and no other.
    Only(LinkType),
    /// Any type: a null node and a log take what they are given, a gateway
    /// node's `world-in` takes every source drawn into it, and what its
    /// `moc-out` sends is the type of whatever each link reaches.
    Any,
}

/// The attributes a device or an app states its terminals' types in, each
/// with the terminal it is about.
pub const TYPE_ATTRIBUTES: [(&str, &str); 4] = [
    ("world-in-type", "world-in"),
    ("world-out-type", "world-out"),
    ("moc-in-type", "moc-in"),
    ("moc-out-type", "moc-out"),
];

/// Reads the type an attribute states, where it states one.
///
/// # Arguments
///
/// * `node` - The node.
/// * `name` - The attribute's name.
///
/// # Returns
///
/// The type, or `None` where the node does not state the attribute.
///
/// # Errors
///
/// A message naming the node when the attribute names no type.
fn optional(node: &Node, name: &str) -> Result<Option<LinkType>, String> {
    node.attributes
        .get(name)
        .map(|said| {
            LinkType::of(said.as_str())
                .map_err(|why| format!("\"{}\": {name}: {why}", node.name))
        })
        .transpose()
}

/// Reads the type an attribute states, which the node must state.
///
/// # Arguments
///
/// * `node` - The node.
/// * `name` - The attribute's name.
///
/// # Returns
///
/// The type.
///
/// # Errors
///
/// A message naming the node when the attribute is absent or names no type.
fn required(node: &Node, name: &str) -> Result<LinkType, String> {
    stated_type(&node.attributes, name)
        .map_err(|why| format!("\"{}\": {why}", node.name))
}

/// Says how a device's own bytes are laid out, where it says.
///
/// A device states a `format` only where its part's bytes are not its type's
/// own layout -- `i16_le`, or `u24_be` read as a `u32` -- and the vehicle
/// lays them out as the type's as it reads them.
///
/// # Arguments
///
/// * `node` - The device's node.
///
/// # Returns
///
/// Its format, or `None` where it states none.
///
/// # Errors
///
/// A message naming the node for a format that is none of them.
pub fn part_format(node: &Node) -> Result<Option<Format>, String> {
    node.attributes
        .get("format")
        .map(|said| {
            Format::of(said.as_str())
                .map_err(|why| format!("\"{}\": {why}", node.name))
        })
        .transpose()
}

/// Says whether a node is a device whose types its `world-out-type`,
/// `moc-in-type` and `format` state: a payload that is neither a set of
/// lines, which carries what its lines need, nor a program.
///
/// # Arguments
///
/// * `node` - The node.
///
/// # Returns
///
/// True for such a device.
pub fn is_device(node: &Node) -> bool {
    node.is_payload()
        && crate::node::payload::gpio::Kind::of(&node.node_type).is_none()
        && crate::node::app::Kind::of(&node.node_type).is_none()
}

/// Says what type a set of GPIO lines carries.
///
/// # Arguments
///
/// * `node` - The node, whose `lines` say how many.
///
/// # Returns
///
/// The narrowest unsigned whole number with a bit for each line.
///
/// # Errors
///
/// A message when the node's `lines` cannot be read.
fn lines_type(node: &Node) -> Result<LinkType, String> {
    let stated = node.attributes.get("lines").ok_or_else(|| {
        format!(
            "\"{}\" states no lines: a set of lines says which",
            node.name
        )
    })?;
    let lines =
        crate::node::payload::gpio::lines_of(&node.name, "lines", stated)
            .map_err(|why| format!("{why}"))?;
    Ok(LinkType::Basic(Basic::lines(lines.len())))
}

/// Says what a device sends at its `world-out`.
///
/// # Arguments
///
/// * `node` - The device's node.
///
/// # Returns
///
/// Its `world-out-type`; the type its `format` reads where it states only
/// that; `bytes` where it states neither, its readings then being its
/// part's own business.
///
/// # Errors
///
/// A message for a type or format that does not read, and for a format whose
/// type is not the one the node states.
fn device_sends(node: &Node) -> Result<LinkType, String> {
    let stated = optional(node, "world-out-type")?;
    match (stated, part_format(node)?) {
        (Some(stated), Some(format))
            if stated != LinkType::Basic(format.basic()) =>
        {
            Err(format!(
                "\"{}\": its format lays out a {}, and its world-out-type is \
                 {stated}: a format says how the part's bytes are laid out, \
                 and they are a number of the type the node sends",
                node.name,
                format.basic().name()
            ))
        }
        (Some(stated), _) => Ok(stated),
        (None, Some(format)) => Ok(LinkType::Basic(format.basic())),
        (None, None) => Ok(LinkType::Basic(Basic::Bytes)),
    }
}

/// Says what one terminal of a node carries.
///
/// # Arguments
///
/// * `node`     - The node.
/// * `terminal` - The terminal: `world-in`, `world-out`, `moc-in` or
///                `moc-out`.
///
/// # Returns
///
/// What it carries.
///
/// # Errors
///
/// A message naming the node when what it states cannot be read, or a type
/// it must state is not stated.
pub fn carries(node: &Node, terminal: &str) -> Result<Carries, String> {
    let only = |kind: LinkType| Ok(Carries::Only(kind));
    let sink = terminal.ends_with("-in");
    let kind = node.node_type.as_str();

    if kind == "compute" {
        let function = node
            .attributes
            .get("function")
            .map(|said| said.as_str().to_string())
            .unwrap_or_default();
        return match (function.as_str(), sink) {
            ("vote", _) => only(required(node, "carries")?),
            ("to_kelvin", false) => {
                only(LinkType::Basic(Basic::Value(Unit::Kelvin)))
            }
            (_, true) => only(required(node, "type-in")?),
            (_, false) => only(required(node, "type-out")?),
        };
    }
    if crate::gateway::is_gateway(kind)
        || kind == "log"
        || kind == "null-gateway"
        || kind == "null-payload"
    {
        return Ok(Carries::Any);
    }
    if kind == "tee-gateway" || kind == "tee-payload" {
        return only(
            optional(node, "carries")?.unwrap_or(LinkType::Basic(Basic::Bytes)),
        );
    }
    if kind == "ping" {
        return only(LinkType::Basic(Basic::String));
    }
    if let Some(gpio) = crate::node::payload::gpio::Kind::of(kind) {
        // A watched set takes anything at its world-in, as a request for a
        // report of its lines.
        if gpio == crate::node::payload::gpio::Kind::In && sink {
            return Ok(Carries::Any);
        }
        return only(lines_type(node)?);
    }
    if crate::node::app::Kind::of(kind).is_some() {
        let name = format!("{terminal}-type");
        return only(
            optional(node, &name)?.unwrap_or(LinkType::Basic(Basic::Bytes)),
        );
    }
    // A device of any other kind: what it reads, and what it is driven with.
    if sink {
        only(
            optional(node, "moc-in-type")?
                .unwrap_or(LinkType::Basic(Basic::Bytes)),
        )
    } else {
        only(device_sends(node)?)
    }
}

/// Says what unit the values a node sends are in.
///
/// # Arguments
///
/// * `node` - The node.
///
/// # Returns
///
/// The unit, where what it sends at its `world-out` is a value type.
pub fn sends_unit(node: &Node) -> Option<Unit> {
    match carries(node, "world-out") {
        Ok(Carries::Only(kind)) => kind.unit(),
        _ => None,
    }
}

/// Says what unit the values a node takes are in.
///
/// # Arguments
///
/// * `node` - The node.
///
/// # Returns
///
/// The unit, where what it takes at its `world-in`, or failing that at its
/// `moc-in`, is a value type.
pub fn takes_unit(node: &Node) -> Option<Unit> {
    ["world-in", "moc-in"].iter().find_map(|terminal| {
        match carries(node, terminal) {
            Ok(Carries::Only(kind)) => kind.unit(),
            _ => None,
        }
    })
}

/// Checks every node's stated types and every link's two ends.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// Each problem, with the node it is about: a type that does not read or is
/// missing, and a link whose two ends carry different types.
pub fn problems(graph: &Graph) -> Vec<(String, String)> {
    let mut problems: Vec<(String, String)> = Vec::new();
    let mut said = |node: &str, why: String| {
        if !problems.iter().any(|(_, seen)| *seen == why) {
            problems.push((node.to_string(), why));
        }
    };

    // An attribute naming a type is read for every node that may state one,
    // so that a misspelt type is refused where nothing links to it yet.
    for node in &graph.nodes {
        for (attribute, _) in TYPE_ATTRIBUTES {
            if let Err(why) = optional(node, attribute) {
                said(&node.name, why);
            }
        }
        if is_device(node) {
            if let Err(why) = part_format(node) {
                said(&node.name, why);
            }
        }
    }

    for link in &graph.links {
        let origin = link.origin();
        let Some(from) = graph.node(&origin.node) else {
            continue;
        };
        let leaves = match terminals_of(&from.node_type) {
            Some(_) => match terminal_at(
                &from.node_type,
                origin.input.as_deref(),
                false,
            ) {
                Ok(Some(leaves)) => leaves,
                _ => continue,
            },
            None => origin.input.clone().unwrap_or("world-out".to_string()),
        };
        let Some(arrives) = answers(&leaves) else {
            continue;
        };
        // A computation's types are read with the rest of what it is told,
        // and a type it does not state is refused there, once.
        let sends = match carries(from, &leaves) {
            Ok(sends) => sends,
            Err(why) => {
                if from.node_type != "compute" {
                    said(&from.name, why);
                }
                continue;
            }
        };
        for destination in link.to.each() {
            let Destination { node: to, .. } = Destination::parse(destination);
            let Some(reached) = graph.node(&to) else {
                continue;
            };
            let takes = match carries(reached, arrives) {
                Ok(takes) => takes,
                Err(why) => {
                    if reached.node_type != "compute" {
                        said(&reached.name, why);
                    }
                    continue;
                }
            };
            if let (Carries::Only(sent), Carries::Only(taken)) =
                (&sends, &takes)
            {
                if sent != taken {
                    said(
                        &reached.name,
                        format!(
                            "\"{}\" sends {sent} by its {leaves} to \"{to}\", \
                             which takes {taken} at its {arrives}: the two \
                             ends of a link carry one type, so make them the \
                             same",
                            from.name
                        ),
                    );
                }
            }
        }
    }
    problems
}
