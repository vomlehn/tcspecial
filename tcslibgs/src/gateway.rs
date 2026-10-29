//! What the vehicle answers to: its gateway.
//!
//! Whatever commands tcspecial and takes its telemetry -- the ground over a
//! radio, a cFS flight computer on its software bus, an F´ flight computer
//! through a bridge component. A configuration answers to one kind, and says
//! which as the `tcspecial` section's `gateway`, because the kind decides how
//! every other link in that section is written: commands, payload commands,
//! payload telemetry and the beacon all go the one way. See "Gateways: what the
//! vehicle answers to" in `docs/design.rst`.
//!
//! The radio and F´ are built. cFS is specified, and a configuration naming
//! it is refused where it is read, saying so, rather than accepted and found
//! to have nothing to talk to when it flies.
//!
//! F´ is the radio's own protocol with a component of an F´ deployment at the
//! far end in place of a ground station, so a `gateway_fprime` node is opened,
//! counted and driven as a radio's is. What differs is what the far end is
//! called in the file -- `bridge_base` where a radio says `ground_base` -- and
//! that it states where its F´ channel IDs begin, `fprime_id_base`, which
//! nothing aboard reads.
//!
//! The section the gateway nodes are written in, and the node type the radio
//! is, are still `moc`: the rename to `gateway` changes configuration keys, and
//! is held back to land with the `payload` rename.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A kind of gateway, as `tcspecial.gateway` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayKind {
    /// The ground, through the vehicle's radio, over UDP.
    Radio,
    /// A cFS flight computer's software bus, as a peer of its SBN.
    Cfs,
    /// An F´ flight computer, through a bridge component in its deployment.
    Fprime,
}

impl GatewayKind {
    /// Every kind, in the order the design names them.
    pub const ALL: [GatewayKind; 3] =
        [GatewayKind::Radio, GatewayKind::Cfs, GatewayKind::Fprime];

    /// Names the kind as a configuration writes it.
    ///
    /// # Returns
    ///
    /// `radio`, `cfs` or `fprime`.
    pub fn name(self) -> &'static str {
        match self {
            GatewayKind::Radio => "radio",
            GatewayKind::Cfs => "cfs",
            GatewayKind::Fprime => "fprime",
        }
    }

    /// Whether this build of tcspecial can answer to the kind.
    ///
    /// The one place that says which kinds are built, so that the section's
    /// reader and its tests agree about it, and building one is a line here.
    pub fn is_built(self) -> bool {
        matches!(self, GatewayKind::Radio | GatewayKind::Fprime)
    }

    /// Names the node type of the kind's gateway nodes.
    ///
    /// # Returns
    ///
    /// The node type, or `None` for a kind that has no gateway nodes.
    pub fn node_type(self) -> Option<&'static str> {
        match self {
            GatewayKind::Radio => Some("moc"),
            GatewayKind::Fprime => Some("gateway_fprime"),
            GatewayKind::Cfs => None,
        }
    }

    /// Names the attribute a gateway node of the kind says where its data
    /// goes with: an `address:port`, counted on by each source's device id.
    ///
    /// Named for what is there: the ground for a radio, the bridge
    /// component for F´. One rule counts on from either.
    ///
    /// # Returns
    ///
    /// The attribute's name.
    pub fn base_attribute(self) -> &'static str {
        match self {
            GatewayKind::Radio => "ground_base",
            GatewayKind::Fprime => "bridge_base",
            GatewayKind::Cfs => "tlm_base",
        }
    }

    /// Says what is at the far end, in words a complaint can use.
    ///
    /// # Returns
    ///
    /// The far end, such as "the ground station".
    pub fn far_end(self) -> &'static str {
        match self {
            GatewayKind::Radio => "the ground station",
            GatewayKind::Fprime => "the F´ bridge component",
            GatewayKind::Cfs => "the cFS software bus",
        }
    }

    /// Reads the kind a file named, if it is one this build answers to.
    ///
    /// A kind that is specified and not built is refused by name, rather
    /// than as a word nothing knows: the file is right about what the kind is
    /// called and early about when it can be flown.
    ///
    /// # Arguments
    ///
    /// * `said` - The `gateway` attribute as the file wrote it.
    ///
    /// # Returns
    ///
    /// The kind.
    ///
    /// # Errors
    ///
    /// A message offering the kinds that are built, when `said` is no kind
    /// or a kind this build does not have.
    pub fn of(said: &str) -> Result<GatewayKind, String> {
        let said = said.trim();
        let Some(kind) =
            GatewayKind::ALL.into_iter().find(|k| k.name() == said)
        else {
            return Err(format!(
                "gateway is \"{said}\", which is not a kind of gateway: say {}",
                GatewayKind::built_names()
            ));
        };
        if !kind.is_built() {
            return Err(format!(
                "gateway is {said}, which is specified but not yet built, so \
                 there would be nothing aboard to answer to it: say {} until \
                 it is",
                GatewayKind::built_names()
            ));
        }
        Ok(kind)
    }

    /// The kind a gateway node's type belongs to, or `None` for a type that
    /// is not a gateway's.
    ///
    /// The one statement of which node type is which kind's, read from
    /// [`GatewayKind::node_type`]. The radio's is still spelled `moc`.
    pub fn of_node_type(node_type: &str) -> Option<GatewayKind> {
        GatewayKind::ALL
            .into_iter()
            .find(|kind| kind.node_type() == Some(node_type))
    }

    /// Lists the kinds this build answers to, as a file would write them, for
    /// a complaint to offer.
    ///
    /// # Returns
    ///
    /// Their names, joined with "or".
    fn built_names() -> String {
        GatewayKind::ALL
            .into_iter()
            .filter(|kind| kind.is_built())
            .map(GatewayKind::name)
            .collect::<Vec<_>>()
            .join(" or ")
    }
}

/// Whether a node of this type is a gateway's: the vehicle's end of the link
/// to whatever it answers to.
///
/// What every rule that was about the `moc` node is now about, so that a
/// gateway node of any kind is opened, counted and checked as one.
pub fn is_gateway(node_type: &str) -> bool {
    GatewayKind::of_node_type(node_type).is_some()
}

impl fmt::Display for GatewayKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each kind reads back from the name it writes, where it is built.
    #[test]
    fn a_built_kind_reads_from_its_name() {
        for kind in GatewayKind::ALL.into_iter().filter(|k| k.is_built()) {
            assert_eq!(GatewayKind::of(kind.name()), Ok(kind));
        }
        assert_eq!(GatewayKind::of(" radio "), Ok(GatewayKind::Radio));
    }

    /// A kind that is specified and not built is refused saying so, and
    /// a word that is no kind is refused as one, each offering what to say.
    #[test]
    fn an_unbuilt_kind_and_a_non_kind_are_refused_differently() {
        for kind in GatewayKind::ALL.into_iter().filter(|k| !k.is_built()) {
            let why = GatewayKind::of(kind.name()).expect_err(kind.name());
            assert!(why.contains("not yet built"), "{why}");
            assert!(why.contains("radio"), "{why}");
        }
        let why = GatewayKind::of("ground").expect_err("not a kind");
        assert!(why.contains("not a kind of gateway"), "{why}");
        assert!(why.contains("radio"), "{why}");
    }

    /// Each built kind's node type is its own, and no other type is a
    /// gateway's.
    #[test]
    fn each_built_kind_has_its_node_type() {
        assert_eq!(GatewayKind::of_node_type("moc"), Some(GatewayKind::Radio));
        assert_eq!(
            GatewayKind::of_node_type("gateway_fprime"),
            Some(GatewayKind::Fprime)
        );
        for other in ["log", "compute", "i2c", "tee-moc", "gateway_cfs"] {
            assert_eq!(GatewayKind::of_node_type(other), None, "{other}");
            assert!(!is_gateway(other), "{other}");
        }
    }
}
