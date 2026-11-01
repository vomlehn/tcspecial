//! What the vehicle answers to: its gateway.
//!
//! Whatever commands tcspecial and takes its telemetry -- the ground over a
//! radio, a Yamcs ground station, a cFS flight computer on its software bus,
//! an F´ flight computer through a bridge component. A configuration answers to one kind, and says
//! which as the `tcspecial` section's `gateway`, because the kind decides how
//! every other link in that section is written: commands, payload commands,
//! payload telemetry and the beacon all go the one way. See "Gateways: what the
//! vehicle answers to" in `docs/design.rst`.
//!
//! All four are built. The radio, Yamcs and F´ are reached over UDP sockets,
//! and cFS over its software bus, as a peer of its Software Bus Network: see
//! [`crate::sbn`] for the wire and [`crate::cfs`] for the packets on it.
//! [`GatewayKind::on_bus`] says which a kind is, and the rest of the section
//! is written accordingly.
//!
//! F´ is the radio's own protocol with a component of an F´ deployment at the
//! far end in place of a ground station, so a `gateway_fprime` node is opened,
//! counted and driven as a radio's is. What differs is what the far end is
//! called in the file -- `bridge_base` where a radio says `ground_base` -- and
//! that it states where its F´ channel IDs begin, `fprime_id_base`, which
//! nothing aboard reads.
//!
//! Yamcs is the radio's links again with a ground station at the far end that
//! reads CCSDS Space Packets: a `gateway_yamcs` node is opened as a radio's
//! is, and says `ground_base` as a radio's does, and every packet on every
//! link to and from the ground carries a CCSDS primary header, with the APID
//! the configuration gives that link. See [`crate::ccsds`] for the header,
//! and the node's `apid_base` and the section's `beacon.apid` and
//! `tcs_cmd.apid` for the APIDs.
//!
//! The gateway nodes are written in the `gateway` section, and the radio's
//! node type is `gateway_radio`. Both were `moc` once, and a file that still
//! says so is refused where it is read, told the new word: see
//! [`crate::graph::renamed`].

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
    /// A Yamcs ground station, over UDP, every packet a CCSDS Space Packet.
    Yamcs,
}

impl GatewayKind {
    /// Every kind, in the order the design names them.
    pub const ALL: [GatewayKind; 4] = [
        GatewayKind::Radio,
        GatewayKind::Cfs,
        GatewayKind::Fprime,
        GatewayKind::Yamcs,
    ];

    /// Names the kind as a configuration writes it.
    ///
    /// # Returns
    ///
    /// `radio`, `cfs`, `fprime` or `yamcs`.
    pub fn name(self) -> &'static str {
        match self {
            GatewayKind::Radio => "radio",
            GatewayKind::Cfs => "cfs",
            GatewayKind::Fprime => "fprime",
            GatewayKind::Yamcs => "yamcs",
        }
    }

    /// Whether this build of tcspecial can answer to the kind.
    ///
    /// The one place that says which kinds are built, so that the section's
    /// reader and its tests agree about it, and building one is a line here.
    ///
    /// # Returns
    ///
    /// `true` for a kind this build answers to, `false` for one a
    /// configuration may name but this build cannot fly.
    pub fn is_built(self) -> bool {
        matches!(
            self,
            GatewayKind::Radio
                | GatewayKind::Cfs
                | GatewayKind::Fprime
                | GatewayKind::Yamcs
        )
    }

    /// Whether the kind is reached over a software bus, its links message
    /// IDs, rather than over UDP sockets.
    ///
    /// The one statement of which kinds are which, read wherever the links
    /// are written or opened.
    pub fn on_bus(self) -> bool {
        self == GatewayKind::Cfs
    }

    /// Whether every packet on the kind's links to and from the ground is a
    /// CCSDS Space Packet, its primary header added as it is sent and
    /// checked and taken off as it arrives: see [`crate::ccsds`].
    ///
    /// The one statement of which kinds frame their packets, read wherever
    /// the APIDs are required, refused or used.
    pub fn framed(self) -> bool {
        self == GatewayKind::Yamcs
    }

    /// Names the node type of the kind's gateway nodes.
    ///
    /// # Returns
    ///
    /// The node type, or `None` for a kind that has no gateway nodes.
    pub fn node_type(self) -> Option<&'static str> {
        match self {
            GatewayKind::Radio => Some("gateway_radio"),
            GatewayKind::Fprime => Some("gateway_fprime"),
            GatewayKind::Cfs => Some("gateway_cfs"),
            GatewayKind::Yamcs => Some("gateway_yamcs"),
        }
    }

    /// Names the attribute a gateway node of the kind says where its data
    /// goes with, counted on by each source's device id: an `address:port`
    /// for a kind on sockets, a message ID for one on a bus.
    ///
    /// Named for what is there: the ground for a radio and for Yamcs, the
    /// bridge component for F´, the first telemetry message ID for cFS. One
    /// rule counts on from each.
    ///
    /// # Returns
    ///
    /// The attribute's name.
    pub fn base_attribute(self) -> &'static str {
        match self {
            GatewayKind::Radio | GatewayKind::Yamcs => "ground_base",
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
            GatewayKind::Yamcs => "the Yamcs ground station",
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
    /// [`GatewayKind::node_type`].
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
/// What every rule that was about the radio's node is now about, so that a
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

    /// Each kind is built, and reads back from the name it writes.
    #[test]
    fn every_kind_is_built_and_reads_from_its_name() {
        for kind in GatewayKind::ALL {
            assert!(kind.is_built(), "{kind}");
            assert_eq!(GatewayKind::of(kind.name()), Ok(kind));
        }
        assert_eq!(GatewayKind::of(" radio "), Ok(GatewayKind::Radio));
        assert_eq!(GatewayKind::of("cfs"), Ok(GatewayKind::Cfs));
        assert_eq!(GatewayKind::of("yamcs"), Ok(GatewayKind::Yamcs));
    }

    /// Only cFS is on a bus.
    #[test]
    fn only_cfs_is_on_a_bus() {
        assert!(GatewayKind::Cfs.on_bus());
        assert!(!GatewayKind::Radio.on_bus());
        assert!(!GatewayKind::Fprime.on_bus());
        assert!(!GatewayKind::Yamcs.on_bus());
    }

    /// Only Yamcs frames its packets, and it names its far end as the
    /// radio does.
    #[test]
    fn only_yamcs_is_framed() {
        for kind in GatewayKind::ALL {
            assert_eq!(kind.framed(), kind == GatewayKind::Yamcs, "{kind}");
        }
        assert_eq!(GatewayKind::Yamcs.base_attribute(), "ground_base");
        assert_eq!(GatewayKind::Yamcs.far_end(), "the Yamcs ground station");
    }

    /// A word that is no kind is refused as one, offering what to say.
    #[test]
    fn a_non_kind_is_refused() {
        let why = GatewayKind::of("ground").expect_err("not a kind");
        assert!(why.contains("not a kind of gateway"), "{why}");
        assert!(why.contains("radio"), "{why}");
    }

    /// Each built kind's node type is its own, and no other type is a
    /// gateway's.
    #[test]
    fn each_built_kind_has_its_node_type() {
        assert_eq!(
            GatewayKind::of_node_type("gateway_radio"),
            Some(GatewayKind::Radio)
        );
        assert_eq!(
            GatewayKind::of_node_type("gateway_fprime"),
            Some(GatewayKind::Fprime)
        );
        assert_eq!(
            GatewayKind::of_node_type("gateway_cfs"),
            Some(GatewayKind::Cfs)
        );
        assert_eq!(
            GatewayKind::of_node_type("gateway_yamcs"),
            Some(GatewayKind::Yamcs)
        );
        for other in ["log", "compute", "i2c", "tee-gateway", "gateway_moc"] {
            assert_eq!(GatewayKind::of_node_type(other), None, "{other}");
            assert!(!is_gateway(other), "{other}");
        }
    }
}
