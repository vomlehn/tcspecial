//! The F´ model of a bridge: what the `TcsBridge` component of an F´
//! deployment calls each thing tcspecial sends it, and by which ID.
//!
//! A vehicle that answers to F´ sends what each node drawn into its
//! `gateway_fprime` node produces to the bridge component, at the node's
//! `bridge_base` plus the source's device id, as a radio's sends to the ground
//! (see [`crate::graph::telemetry`]). The bridge hands each datum to the rest
//! of its deployment as a telemetry channel, and F´ knows a channel by an ID
//! and a name its dictionary gives. Here those are made from the
//! configuration, by the same counting that gives the ports, so that the
//! dictionary and the vehicle cannot disagree about which channel is which:
//!
//! * each source drawn into the node is the channel at `fprime_id_base` plus
//!   its device id, named after the source, and of the type the
//!   configuration says its data are (see [`Form`]);
//! * the bridge's own channels -- what it has heard of the beacon, whether it
//!   is matched, what it has counted -- follow every device id the
//!   configuration has, so that adding a link never moves one;
//! * and the component instance's base ID is `fprime_id_base`, so an ID
//!   within the component is the device id itself.
//!
//! The model is written out as FPP and C++ for the component's build -- see
//! [`Model::fppi`] and [`Model::cpp`] -- by `tcsverify --fprime-model`, and
//! never by hand. What it would get wrong is refused where the file is read,
//! by [`Graph::check`](super::Graph::check) through [`every`]: a source whose
//! name is not one F´ can name a channel by, or one named as a channel of the
//! bridge's own; an ID past 32 bits; and two `gateway_fprime` nodes whose IDs
//! overlap, which would be two channels under one ID in one deployment.
//!
//! This is the one place that says any of it.

use std::fmt::Write as _;

use super::telemetry::{self, Via};
use super::Graph;
use crate::gateway::GatewayKind;
use crate::node::vehicle::compute::node_sends_a_value;

/// One of the bridge's own channels: not a source's, and the same in every
/// model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Own {
    /// What F´ calls it.
    pub name: &'static str,
    /// Its type, as FPP writes it, from the component's own FPP.
    pub fpp_type: &'static str,
    /// What it says, for the FPP annotation the dictionary carries.
    pub about: &'static str,
}

/// The bridge's own channels, in the order their IDs are given.
///
/// The component writes each by name, so one named here and not written
/// there, or the other way about, is a build that fails rather than a
/// channel nobody fills.
pub const OWN_CHANNELS: &[Own] = &[
    Own {
        name: "BeaconStage",
        fpp_type: "Stage",
        about: "How long it has been since the vehicle's last beacon",
    },
    Own {
        name: "RestartFlag",
        fpp_type: "bool",
        about: "Whether the vehicle's last beacon had its restart flag set: \
                it has started since it was last matched",
    },
    Own {
        name: "ListeningMatched",
        fpp_type: "bool",
        about: "Whether the vehicle's beacons carry this configuration's \
                checksum, so that its data is read",
    },
    Own {
        name: "CommandingMatched",
        fpp_type: "bool",
        about: "Whether a CONNECT has been answered with this \
                configuration's checksum, so that commands go",
    },
    Own {
        name: "PacketsReceived",
        fpp_type: "U64",
        about: "Every packet received from the vehicle, data and answers",
    },
    Own {
        name: "PacketsProcessed",
        fpp_type: "U64",
        about: "Packets read and handed on",
    },
    Own {
        name: "PacketsMalformed",
        fpp_type: "U64",
        about: "Packets the configuration's definitions could not read",
    },
    Own {
        name: "PacketsMismatched",
        fpp_type: "U64",
        about: "Packets dropped because the two ends may not have been \
                flying the same configuration",
    },
    Own {
        name: "DataCutShort",
        fpp_type: "U32",
        about: "Data longer than a channel holds, whose first bytes went",
    },
    Own {
        name: "DataNotValues",
        fpp_type: "U32",
        about: "Data from a node that sends values that were not one, and \
                went to no channel",
    },
];

/// What a source's channel holds: what the configuration says the source's
/// data are.
///
/// It says one thing of a datum's type, and only of some nodes: a compute
/// node whose function makes a value says, by that function, that what it
/// sends is a value -- four bytes of a signed count of micro-kelvin, see
/// [`crate::node::vehicle::compute::from_value`] -- which is a temperature.
/// Of every other node it says nothing. A sensor's `format` is written on
/// the conversion that reads it, not on the sensor, and says how that one
/// reader takes the bytes; a decision is the flight software's own text. So
/// a value's channel is a temperature, and every other channel is the bytes
/// as they came.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// The bytes, as a `Datum`: the length and the first of them.
    Datum,
    /// A value, as an `F64` of kelvin, which tcslib-c's `tcs_value_kelvin`
    /// reads.
    Kelvin,
}

impl Form {
    /// Finds the form of what a node sends, as the configuration says it.
    ///
    /// # Arguments
    ///
    /// * `graph`  - The configuration.
    /// * `source` - The node's name.
    ///
    /// # Returns
    ///
    /// [`Form::Kelvin`] for a node that sends values, as
    /// [`node_sends_a_value`] says, and [`Form::Datum`] for every other.
    pub fn of(graph: &Graph, source: &str) -> Form {
        match graph.node(source) {
            Some(node)
                if node_sends_a_value(&node.node_type, &node.attributes) =>
            {
                Form::Kelvin
            }
            _ => Form::Datum,
        }
    }

    /// Gives the channel's type, as FPP writes it.
    ///
    /// # Returns
    ///
    /// `Datum`, the component's own struct, or `F64`.
    pub fn fpp_type(self) -> &'static str {
        match self {
            Form::Datum => "Datum",
            Form::Kelvin => "F64",
        }
    }

    /// Says what the channel holds, for its annotation.
    fn about(self) -> &'static str {
        match self {
            Form::Datum => "its bytes",
            Form::Kelvin => "a value, as a temperature in kelvin",
        }
    }
}

/// One channel of a bridge's model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    /// What F´ calls it: the source's name, or one of [`OWN_CHANNELS`].
    pub name: String,
    /// Its ID within the component: the source's device id, or a number
    /// after every device id for one of the bridge's own.
    pub local: u32,
    /// Its ID in the deployment: `fprime_id_base` plus `local`.
    pub id: u32,
}

/// One source drawn into the node: its channel, and what the channel holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub channel: Channel,
    pub form: Form,
}

/// One node a command may name, and the device id the vehicle knows it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub id: usize,
}

/// The F´ model of the bridge at the far end of one `gateway_fprime` node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// The gateway node, which the bridge opens.
    pub gateway: String,
    /// The node's `fprime_id_base`: the component instance's base ID.
    pub id_base: u32,
    /// A channel for each source drawn into the node, in the order the
    /// links are drawn.
    pub data: Vec<Source>,
    /// The bridge's own channels, in [`OWN_CHANNELS`]' order.
    pub own: Vec<Channel>,
    /// Every node in the configuration, in device-id order, which the
    /// bridge's node commands are given by name.
    pub nodes: Vec<Named>,
}

/// What is wrong with the model a configuration would make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The node it is about.
    pub node: String,
    pub why: String,
}

/// Says whether F´ can call something by this name: an FPP identifier,
/// letters, digits and underscores, not starting with a digit.
///
/// FPP's own keywords are names like any other here, because the model
/// writes every name escaped (see [`fpp_name`]).
///
/// # Arguments
///
/// * `name` - The name.
///
/// # Returns
///
/// Whether it is one.
pub fn is_fprime_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Writes a name as FPP reads it, escaped so that a node called `time` or
/// `port`, which are FPP's keywords, is still a name.
///
/// # Arguments
///
/// * `name` - The name, one [`is_fprime_name`] takes.
///
/// # Returns
///
/// The name with FPP's escape before it; the dictionary carries it without.
fn fpp_name(name: &str) -> String {
    format!("${name}")
}

/// Makes the model of the bridge at the far end of one `gateway_fprime`
/// node.
///
/// # Arguments
///
/// * `graph`   - The configuration.
/// * `gateway` - The `gateway_fprime` node's name.
///
/// # Returns
///
/// The model.
///
/// # Errors
///
/// [`Refused`] when there is no such `gateway_fprime` node, when it states no
/// `fprime_id_base` that reads as a whole number, when its links to the
/// bridge cannot be counted (see [`telemetry::of`]), when a source drawn into
/// it is not named as F´ can name a channel, or is named as one of the
/// bridge's own, or when an ID would pass 32 bits.
pub fn of(graph: &Graph, gateway: &str) -> Result<Model, Refused> {
    let refused = |why: String| Refused {
        node: gateway.to_string(),
        why,
    };
    let node = graph
        .node(gateway)
        .filter(|n| {
            GatewayKind::of_node_type(&n.node_type) == Some(GatewayKind::Fprime)
        })
        .ok_or_else(|| {
            refused(format!(
                "there is no gateway_fprime node called \"{gateway}\": name \
                 the one the bridge is to be"
            ))
        })?;
    let base = node
        .attributes
        .get("fprime_id_base")
        .ok_or_else(|| {
            refused(format!(
                "\"{gateway}\" states no fprime_id_base, which is where the \
                 bridge's F´ IDs begin"
            ))
        })
        .and_then(|said| {
            crate::whole::whole_u32(said.as_str()).map_err(|why| {
                refused(format!("\"{gateway}\": fprime_id_base: {why}"))
            })
        })?;
    let links = telemetry::of(graph).map_err(|r| Refused {
        node: r.node.unwrap_or_else(|| gateway.to_string()),
        why: r.why,
    })?;

    // Every ID within the component, then each counted on from the base:
    // nought past 32 bits is an ID that wraps onto another.
    let at = |name: &str, local: usize| -> Result<Channel, Refused> {
        u32::try_from(local)
            .ok()
            .and_then(|local| {
                base.checked_add(local).map(|id| Channel {
                    name: name.to_string(),
                    local,
                    id,
                })
            })
            .ok_or_else(|| {
                refused(format!(
                    "\"{gateway}\": fprime_id_base is {base:#x}, and \
                     \"{name}\", at {local} past it, would be past the last \
                     F´ ID, {:#x}: lower fprime_id_base",
                    u32::MAX
                ))
            })
    };

    let mut data = Vec::new();
    for link in links
        .iter()
        .filter(|l| l.via == Via::Gateway(gateway.to_string()))
    {
        if !is_fprime_name(&link.source) {
            return Err(Refused {
                node: link.source.clone(),
                why: format!(
                    "\"{}\" is drawn into \"{gateway}\", and the F´ bridge \
                     names a channel after it, which F´ cannot do with that \
                     name: rename the node with letters, digits and \
                     underscores, starting with a letter",
                    link.source
                ),
            });
        }
        if let Some(own) = OWN_CHANNELS.iter().find(|o| o.name == link.source) {
            return Err(Refused {
                node: link.source.clone(),
                why: format!(
                    "\"{}\" is drawn into \"{gateway}\", and the F´ bridge \
                     names a channel after it, but {} is the bridge's own \
                     channel saying {}: rename the node",
                    link.source,
                    own.name,
                    own.about.to_lowercase()
                ),
            });
        }
        data.push(Source {
            channel: at(&link.source, link.id)?,
            form: Form::of(graph, &link.source),
        });
    }
    let own = OWN_CHANNELS
        .iter()
        .enumerate()
        .map(|(k, o)| at(o.name, graph.nodes.len() + k))
        .collect::<Result<Vec<_>, _>>()?;
    let nodes = telemetry::ids(graph)
        .into_iter()
        .map(|(name, id)| Named { name, id })
        .collect::<Vec<_>>();
    let mut nodes = nodes;
    nodes.sort_by_key(|n| n.id);

    Ok(Model {
        gateway: gateway.to_string(),
        id_base: base,
        data,
        own,
        nodes,
    })
}

/// Makes the model of every bridge the configuration has, and refuses two
/// whose IDs overlap.
///
/// A `gateway_fprime` node whose `fprime_id_base` is missing or does not
/// read, or whose links to the ground cannot be counted, is left out:
/// [`crate::graph::reach::reach_of`] and the ground links' own check say what
/// is wrong with those, and say it once.
///
/// # Arguments
///
/// * `graph` - The configuration.
///
/// # Returns
///
/// A model per `gateway_fprime` node, in the order the nodes are written.
///
/// # Errors
///
/// [`Refused`] for what [`of`] refuses, and for two nodes whose IDs -- from
/// `fprime_id_base` to the last of the bridge's own channels -- overlap.
pub fn every(graph: &Graph) -> Result<Vec<Model>, Refused> {
    if telemetry::of(graph).is_err() {
        return Ok(Vec::new());
    }
    let mut models: Vec<Model> = Vec::new();
    for node in graph.nodes.iter().filter(|n| {
        GatewayKind::of_node_type(&n.node_type) == Some(GatewayKind::Fprime)
    }) {
        let reads = node
            .attributes
            .get("fprime_id_base")
            .is_some_and(|b| crate::whole::whole_u32(b.as_str()).is_ok());
        if !reads {
            continue;
        }
        let model = of(graph, &node.name)?;
        if let Some(first) = models.iter().find(|m| {
            m.id_base <= model.last_id() && model.id_base <= m.last_id()
        }) {
            return Err(Refused {
                node: model.gateway.clone(),
                why: format!(
                    "\"{}\" gives its F´ bridge the IDs {:#x} to {:#x}, and \
                     \"{}\" gives its own {:#x} to {:#x}: two channels of one \
                     deployment would share an ID. Move one fprime_id_base \
                     past the other's last",
                    first.gateway,
                    first.id_base,
                    first.last_id(),
                    model.gateway,
                    model.id_base,
                    model.last_id()
                ),
            });
        }
        models.push(model);
    }
    Ok(models)
}

impl Model {
    /// Gives the last ID the model uses, which is its last own channel's.
    ///
    /// # Returns
    ///
    /// The ID.
    pub fn last_id(&self) -> u32 {
        self.own.last().map_or(self.id_base, |c| c.id)
    }

    /// Gives how many bytes the longest node name has, which a node command
    /// carries as its argument.
    ///
    /// # Returns
    ///
    /// The length, at least one: FPP has no string of size nought.
    pub fn node_name_bytes(&self) -> usize {
        self.nodes
            .iter()
            .map(|n| n.name.len())
            .max()
            .unwrap_or(0)
            .max(1)
    }

    /// Writes the model as FPP, which the `TcsBridge` component includes
    /// inside its definition: its constants and every telemetry channel.
    ///
    /// # Arguments
    ///
    /// * `from`   - The configuration it was made from, as named, for the
    ///              comment at the top.
    /// * `digest` - That configuration's digest, which the bridge holds the
    ///              configuration it opens to.
    ///
    /// # Returns
    ///
    /// The FPP.
    pub fn fppi(&self, from: &str, digest: &str) -> String {
        let mut out = String::new();
        let line = |out: &mut String, text: String| {
            out.push_str(&text);
            out.push('\n');
        };
        line(&mut out, generated_by("#", from, digest));
        line(
            &mut out,
            "\n# The configuration this model was made from, which the bridge \
             refuses to run\n# without."
                .to_string(),
        );
        line(
            &mut out,
            format!("constant CONFIGURATION_DIGEST = \"{digest}\""),
        );
        line(&mut out, "\n# The gateway node the bridge is.".to_string());
        line(
            &mut out,
            format!("constant GATEWAY_NODE = {}", fpp_string(&self.gateway)),
        );
        line(
            &mut out,
            "\n# Its fprime_id_base, which the deployment gives the component \
             instance as\n# its base ID."
                .to_string(),
        );
        line(&mut out, format!("constant ID_BASE = {:#x}", self.id_base));
        line(
            &mut out,
            "\n# How long a node's name may be, which a node command carries."
                .to_string(),
        );
        line(
            &mut out,
            format!("constant NODE_NAME_BYTES = {}", self.node_name_bytes()),
        );
        line(
            &mut out,
            format!(
                "\n# What each node drawn into {} produces: the channel at \
                 its device id.",
                self.gateway
            ),
        );
        for Source { channel, form } in &self.data {
            line(
                &mut out,
                format!(
                    "@ What {} produces, device id {}: {}\ntelemetry {}: {} \
                     id {}",
                    channel.name,
                    channel.local,
                    form.about(),
                    fpp_name(&channel.name),
                    form.fpp_type(),
                    channel.local
                ),
            );
        }
        line(
            &mut out,
            "\n# The bridge's own, after every device id.".to_string(),
        );
        for (channel, own) in self.own.iter().zip(OWN_CHANNELS) {
            line(
                &mut out,
                format!(
                    "@ {}\ntelemetry {}: {} id {}",
                    own.about,
                    fpp_name(own.name),
                    own.fpp_type,
                    channel.local
                ),
            );
        }
        out
    }

    /// Writes the model as C++: the two member functions of `TcsBridge`
    /// that differ from one configuration to another, which say which
    /// channel a source's datum is written to, as what, and which device id
    /// a node's name is. A datum is made into a channel's type by the
    /// component's own `asDatum` and `asKelvin`, which are the same in every
    /// model; the model says only which.
    ///
    /// # Arguments
    ///
    /// * `from`   - The configuration it was made from, as named, for the
    ///              comment at the top.
    /// * `digest` - That configuration's digest.
    ///
    /// # Returns
    ///
    /// The C++, which compiles with the component's own header.
    pub fn cpp(&self, from: &str, digest: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{}", generated_by("//", from, digest));
        out.push_str(
            "\n#include <cstring>\n\n#include \"Tcs/TcsBridge/TcsBridge.hpp\"\
             \n\nnamespace Tcs {\n\n",
        );
        out.push_str("bool TcsBridge::writeDatum(const tcs_datum &datum)\n{\n");
        for Source { channel, form } in &self.data {
            let _ = writeln!(
                out,
                "\tif (std::strcmp(datum.source, {}) == 0) {{",
                cpp_string(&channel.name)
            );
            let _ = match form {
                Form::Datum => writeln!(
                    out,
                    "\t\tthis->tlmWrite_{}(this->asDatum(datum));",
                    channel.name
                ),
                Form::Kelvin => writeln!(
                    out,
                    "\t\tF64 kelvin = 0.0;\n\t\t\
                     if (this->asKelvin(datum, kelvin)) {{\n\t\t\t\
                     this->tlmWrite_{}(kelvin);\n\t\t}}",
                    channel.name
                ),
            };
            out.push_str("\t\treturn true;\n\t}\n");
        }
        out.push_str("\treturn false;\n}\n\n");
        out.push_str(
            "bool TcsBridge::nodeId(const char *name, U32 &id)\n{\n\t\
             static const struct {\n\t\tconst char *name;\n\t\tU32 id;\n\t} \
             NODES[] = {\n",
        );
        for node in &self.nodes {
            let _ = writeln!(
                out,
                "\t\t{{ {}, {} }},",
                cpp_string(&node.name),
                node.id
            );
        }
        out.push_str(
            "\t};\n\tfor (const auto &node : NODES) {\n\t\t\
             if (std::strcmp(name, node.name) == 0) {\n\t\t\t\
             id = node.id;\n\t\t\treturn true;\n\t\t}\n\t}\n\t\
             return false;\n}\n\n} // namespace Tcs\n",
        );
        out
    }
}

/// Says, as a comment, that a file was written from a configuration and is
/// not to be edited.
///
/// # Arguments
///
/// * `comment` - What starts a comment in the file's language.
/// * `from`    - The configuration, as named.
/// * `digest`  - Its digest.
///
/// # Returns
///
/// The comment, three lines.
fn generated_by(comment: &str, from: &str, digest: &str) -> String {
    format!(
        "{comment} The F´ model of a TcsBridge, written by tcsverify \
         --fprime-model from\n{comment} {from}, md5 {digest}.\n{comment} Not \
         to be edited: change the configuration and write it again."
    )
}

/// Writes text as an FPP string, its quotes and backslashes escaped.
///
/// # Arguments
///
/// * `text` - The text.
///
/// # Returns
///
/// The string, quoted.
fn fpp_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Writes text as a C++ string literal. Every byte that is not printable
/// ASCII, or that a literal would read as something else, is an octal
/// escape of three digits, which -- unlike a hex escape -- never takes in
/// the character after it.
///
/// # Arguments
///
/// * `text` - The text.
///
/// # Returns
///
/// The literal, quoted.
fn cpp_string(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        if (byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\\' | b'?'))
            || byte == b' '
        {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "\\{byte:03o}");
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The fprime example, as shipped.
    fn example() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("examples/fprime/fprime.ytcs"))
            .expect("the example reads")
    }

    /// The example with its `gateway_fprime` node's `fprime_id_base` set.
    ///
    /// # Arguments
    ///
    /// * `base` - What to set it to.
    ///
    /// # Returns
    ///
    /// The configuration.
    fn based_at(base: &str) -> Graph {
        let mut graph = example();
        graph
            .nodes
            .iter_mut()
            .find(|n| n.name == "fsw")
            .expect("fsw")
            .attributes
            .insert("fprime_id_base".to_string(), crate::Scalar::of(base));
        graph
    }

    /// The thermometer, device id 2, is the channel 0x3002, named after it
    /// and holding its bytes; cabin_temp, device id 3, a conversion, is
    /// 0x3003 and holds a temperature; and the bridge's own follow the last
    /// device id.
    #[test]
    fn a_source_is_its_device_id_past_the_base() {
        let model = of(&example(), "fsw").expect("a model");
        assert_eq!(model.id_base, 0x3000);
        // In device-id order, which is not always the order of the links.
        let mut data = model.data.clone();
        data.sort_by_key(|s| s.channel.local);
        assert_eq!(
            data,
            vec![
                Source {
                    channel: Channel {
                        name: "thermometer".to_string(),
                        local: 2,
                        id: 0x3002,
                    },
                    form: Form::Datum,
                },
                Source {
                    channel: Channel {
                        name: "cabin_temp".to_string(),
                        local: 3,
                        id: 0x3003,
                    },
                    form: Form::Kelvin,
                },
            ]
        );
        assert_eq!(model.own.len(), OWN_CHANNELS.len());
        assert_eq!(model.own[0].local, 4);
        assert_eq!(model.own[0].id, 0x3004);
        assert_eq!(model.last_id(), 0x3003 + OWN_CHANNELS.len() as u32);
        let names: Vec<_> =
            model.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["fsw", "heater", "thermometer", "cabin_temp"]);
        assert_eq!(model.node_name_bytes(), "thermometer".len());
    }

    /// The device id is the one the vehicle counts ports by: the channel
    /// and the port agree for every source.
    #[test]
    fn a_channel_and_its_port_count_alike() {
        let graph = example();
        let model = of(&graph, "fsw").expect("a model");
        for link in telemetry::of(&graph).expect("links") {
            let channel = &model
                .data
                .iter()
                .find(|s| s.channel.name == link.source)
                .expect("a channel for every link")
                .channel;
            assert_eq!(channel.local as usize, link.id);
        }
    }

    /// The FPP names every channel escaped, with its ID, and states the
    /// constants the component and the deployment are built with.
    #[test]
    fn the_fpp_says_every_channel_and_constant() {
        let fpp = of(&example(), "fsw")
            .expect("a model")
            .fppi("examples/fprime/fprime.ytcs", "0123");
        for said in [
            "constant CONFIGURATION_DIGEST = \"0123\"",
            "constant GATEWAY_NODE = \"fsw\"",
            "constant ID_BASE = 0x3000",
            "constant NODE_NAME_BYTES = 11",
            "telemetry $thermometer: Datum id 2",
            "telemetry $cabin_temp: F64 id 3",
            "telemetry $BeaconStage: Stage id 4",
            "telemetry $DataCutShort: U32 id 12",
            "telemetry $DataNotValues: U32 id 13",
        ] {
            assert!(fpp.contains(said), "{said} in:\n{fpp}");
        }
    }

    /// The C++ writes each source's channel by its name, as its form says,
    /// and knows every node's id.
    #[test]
    fn the_cpp_says_every_source_and_node() {
        let cpp = of(&example(), "fsw").expect("a model").cpp("x", "0123");
        assert!(
            cpp.contains("this->tlmWrite_thermometer(this->asDatum(datum));"),
            "{cpp}"
        );
        assert!(
            cpp.contains(
                "if (this->asKelvin(datum, kelvin)) {\n\t\t\t\
                 this->tlmWrite_cabin_temp(kelvin);"
            ),
            "{cpp}"
        );
        assert!(cpp.contains("{ \"heater\", 1 },"), "{cpp}");
        assert!(cpp.contains("{ \"thermometer\", 2 },"), "{cpp}");
    }

    /// A name that is no F´ name is refused, naming the node; a keyword of
    /// FPP's is not, because the model escapes it.
    #[test]
    fn a_source_f_prime_cannot_name_is_refused() {
        assert!(is_fprime_name("thermometer"));
        assert!(is_fprime_name("_t2"));
        assert!(is_fprime_name("time"));
        for bad in ["", "2t", "cabin-temp", "cabin temp", "é"] {
            assert!(!is_fprime_name(bad), "{bad:?}");
        }

        let mut graph = example();
        let rename = |graph: &mut Graph, to: &str| {
            graph
                .nodes
                .iter_mut()
                .find(|n| n.name == "thermometer")
                .expect("thermometer")
                .name = to.to_string();
            for link in &mut graph.links {
                if link.from.starts_with("thermometer") {
                    link.from = link.from.replacen("thermometer", to, 1);
                }
            }
        };
        rename(&mut graph, "cabin-temp");
        let refused = of(&graph, "fsw").expect_err("not an F´ name");
        assert_eq!(refused.node, "cabin-temp");
        assert!(refused.why.contains("letters, digits"), "{}", refused.why);

        let mut graph = example();
        rename(&mut graph, "BeaconStage");
        let refused = of(&graph, "fsw").expect_err("the bridge's own name");
        assert!(refused.why.contains("bridge's own"), "{}", refused.why);
    }

    /// A base so high that the bridge's own channels would pass 32 bits is
    /// refused, saying to lower it.
    #[test]
    fn an_id_past_32_bits_is_refused() {
        let refused =
            of(&based_at("0xfffffff8"), "fsw").expect_err("past 32 bits");
        assert!(
            refused.why.contains("lower fprime_id_base"),
            "{}",
            refused.why
        );
        assert!(of(&based_at("0xfffffff0"), "fsw").is_ok());
    }

    /// Two bridges whose IDs overlap are refused, naming both and their
    /// ranges; apart, they are two models.
    #[test]
    fn two_bridges_whose_ids_overlap_are_refused() {
        let add_second = |base: &str| {
            let mut graph = example();
            let mut second = graph
                .nodes
                .iter()
                .find(|n| n.name == "fsw")
                .expect("fsw")
                .clone();
            second.name = "fsw2".to_string();
            second
                .attributes
                .insert("fprime_id_base".to_string(), crate::Scalar::of(base));
            second
                .attributes
                .insert("port".to_string(), crate::Scalar::of("9601"));
            second.attributes.insert(
                "bridge_base".to_string(),
                crate::Scalar::of("127.0.0.1:9800"),
            );
            graph.nodes.push(second);
            graph
        };
        let refused = every(&add_second("0x3005")).expect_err("overlap");
        assert_eq!(refused.node, "fsw2");
        assert!(refused.why.contains("\"fsw\""), "{}", refused.why);
        assert!(refused.why.contains("0x3005"), "{}", refused.why);

        let models = every(&add_second("0x4000")).expect("apart");
        assert_eq!(models.len(), 2);
    }

    /// A value is a temperature by what the configuration says of the node
    /// that sends it -- a conversion, a vote -- and nothing else is: a
    /// decision, a sensor, a node the configuration lacks.
    #[test]
    fn only_a_node_that_sends_values_has_a_temperature() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let thermostat =
            Graph::load(root.join("examples/thermostat/thermostat.ytcs"))
                .expect("the example reads");
        assert_eq!(Form::of(&thermostat, "k_1"), Form::Kelvin);
        assert_eq!(Form::of(&thermostat, "cabin_temp"), Form::Kelvin);
        assert_eq!(Form::of(&thermostat, "thermostat"), Form::Datum);
        assert_eq!(Form::of(&thermostat, "temp_1"), Form::Datum);
        assert_eq!(Form::of(&thermostat, "nobody"), Form::Datum);
        assert_eq!(Form::Kelvin.fpp_type(), "F64");
        assert_eq!(Form::Datum.fpp_type(), "Datum");
    }

    /// A C++ literal escapes what would end it or be read as something else.
    #[test]
    fn a_cpp_literal_escapes_what_it_must() {
        assert_eq!(cpp_string("heater"), "\"heater\"");
        assert_eq!(cpp_string("a\"b\\c?d"), "\"a\\042b\\134c\\077d\"");
        assert_eq!(cpp_string("é"), "\"\\303\\251\"");
        assert_eq!(fpp_string("a\"b"), "\"a\\\"b\"");
    }
}
