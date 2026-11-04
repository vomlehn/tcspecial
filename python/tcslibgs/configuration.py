"""What a ground station reads out of a tcspecial configuration.

The Python spelling of what the Rust ground programs ask ``tcslibgs`` for:
where the beacons are (``graph::parameters::beacon_of``), where a ``gateway``
node is bound aboard, which port a source's data reaches the ground at
(``graph::telemetry``), whether a datagram came from where
the configuration says (``types::came_from``), and the configuration's digest.
For a configuration that answers to cFS, whose links are message IDs on a
software bus rather than sockets, it reads the bus instead: its SBN peer and
every message ID, counted as ``graph::telemetry::msgids`` counts them -- see
:meth:`Configuration.bus`.
For a configuration that answers to Yamcs, whose packets carry CCSDS
headers, it reads the APIDs, counted as ``graph::telemetry::apids`` counts
them -- see :meth:`Configuration.framed`.

It reads either spelling, chosen by the file's extension as the Rust's
``format::ConfigFormat`` chooses it. The XML with the standard library alone;
the YAML with PyYAML, which is imported only when a YAML file is read, so that
a station reading XML needs nothing outside the standard library. Both are
read into the same :class:`Configuration`, and the two spellings say the same
thing, which ``make comparespellings`` and tcsverify hold them to;
``examples/ping/tests/agrees.rs`` holds what this reads to what the Rust
reads. A configuration's ``include`` list is followed as the Rust follows it
(``graph::Graph::load``), so that a node's device id is counted among the
same names: see :func:`load`.

The digest is not worked out here. It is taken over what a configuration says
rather than over its bytes -- see ``tcslibgs::config_digest`` -- and a second
statement of that in Python would be a second digest on the day the first
changes. :func:`digest` asks ``tcsverify --digest``, which is the Rust one.

This reads only what it needs, and checks nothing: tcsverify is what says
whether a configuration is sound, and a ground station should run on one it
has passed.
"""

import json
import os
import re
import shutil
import subprocess
import xml.parsers.expat
from dataclasses import dataclass, field
from pathlib import Path

from . import interface as _interface
from . import sbn as _sbn

#: The sections whose entries are nodes, and the word a node of each is
#: written as: the key that names it in YAML, its element in XML. See
#: ``tcslibgs::graph::grouped``.
NODE_SECTIONS = {"gateways": "gateway", "vehicle": "compute",
                 "payloads": "payload"}

#: A node's links, each by the data source it leaves, and each written as the
#: node it reaches and the name it arrives by: ``gateway.readings``.
LINKS = ("world-out", "moc-out")

#: A node's data sinks, each listing the names a line may arrive by.
SINKS = ("world-in", "moc-in")

#: The attributes a node writes as a list: ``tcslibgs::graph::LISTED``.
LISTED = ("terminators",)

#: Where the schemas are: a link to tcsgui's ``share/schema``, the one copy,
#: beside this module so that an installed package has them too, as the
#: XTCE documents are (see :mod:`tcslibgs.interface`).
SCHEMAS = Path(__file__).resolve().parent / "schema"

#: Each gateway node type, and the attribute it says where its data goes
#: with: a radio's and a Yamcs gateway's ``ground_base``, an F´ gateway's
#: ``bridge_base``. The Python spelling of
#: ``tcslibgs::gateway::GatewayKind::base_attribute``, for the kinds that have
#: a node type.
GATEWAY_BASES = {
    "gateway_radio": "ground_base",
    "gateway_yamcs": "ground_base",
    "gateway_fprime": "bridge_base",
    "gateway_cfs": "tlm_base",
}

#: The gateway node types whose far end is a software bus, and whose base is
#: a message ID rather than an address: ``GatewayKind::on_bus``.
ON_BUS = ("gateway_cfs",)


def whole(attribute, said, most=0xFFFFFFFF):
    """A whole number as a configuration writes one: ``tcslibgs::whole``'s
    grammar, decimal or ``0x`` hexadecimal, underscores among the digits
    meaning nothing.

    Args:
        attribute: The attribute it was stated as, for the error message.
        said:      What the configuration says.
        most:      The largest value the field holds.

    Returns:
        Its value.

    Raises:
        ValueError: It is not a whole number, or is larger than ``most``.
    """
    text = said.strip()
    digits, radix = text, 10
    if text[:2] in ("0x", "0X"):
        digits, radix = text[2:], 16
    bare = digits.replace("_", "")
    allowed = "0123456789abcdefABCDEF"[: 22 if radix == 16 else 10]
    if not bare or any(c not in allowed for c in bare):
        raise ValueError(f'{attribute} is "{said}", which is not a whole '
                         "number")
    value = int(bare, radix)
    if value > most:
        raise ValueError(f"{attribute} is {value}, and the most it holds is "
                         f"{most}")
    return value


@dataclass(frozen=True)
class Sbn:
    """The ``sbn`` section: the peer every link of the vehicle is carried
    over. ``tcslibgs::Sbn``."""

    release: _sbn.Release
    #: ``SBN_IDENT``, the 48 bytes every subscription opens with.
    ident: bytes
    spacecraft_id: int
    #: The vehicle's processor ID.
    processor_id: int
    #: Where the vehicle binds, and where the far end is.
    local: "HostPort"
    peer: "HostPort"
    peer_processor_id: int
    max_message: int

    def ground_processor_id(self):
        """The processor ID a ground station joins a stand-in for the far
        end as: see :func:`tcslibgs.sbn.ground_processor_id`."""
        return _sbn.ground_processor_id(self.processor_id,
                                        self.peer_processor_id)


@dataclass(frozen=True)
class Bus:
    """A vehicle's links over a cFS software bus: its SBN peer, and the
    message IDs the ``tcspecial`` section states. ``tcslibgs::Bus``."""

    sbn: Sbn
    first: int
    last: int
    cmd_msgid: int
    response_msgid: int
    cmd_base: int
    response_base: int
    tlm_base: int
    beacon_msgid: int


def counted(attribute, base, device_id):
    """A message ID counted on from a base by a node's device id:
    ``graph::telemetry::counted``.

    Args:
        attribute: The base's attribute, for the error message.
        base:      The base.
        device_id: The node's device id.

    Returns:
        The message ID.

    Raises:
        ValueError: The sum is more than a message ID's sixteen bits.
    """
    msgid = base + device_id
    if msgid > 0xFFFF:
        raise ValueError(
            f"{attribute} {base:#06x} plus {device_id} is more than a message "
            "ID's sixteen bits"
        )
    return msgid


#: The largest APID, eleven bits: ``tcslibgs::ccsds::MAX_APID``.
MAX_APID = 0x7FF


def apid(attribute, said):
    """Reads an APID a configuration states: ``tcslibgs::types::apid_of``.

    Args:
        attribute: Its name, for the complaint.
        said:      The value as the file writes it, decimal or 0x hex, or
                   ``None`` where the file does not state it.

    Returns:
        The APID.

    Raises:
        ValueError: It is missing, not a number, or past eleven bits.
    """
    if said is None or not said.strip():
        raise ValueError(f"{attribute} is not stated, and a configuration "
                         "whose gateway is yamcs must state it: the APID, 0 "
                         f"to {MAX_APID:#x}")
    return whole(attribute, said, MAX_APID)


def counted_apid(attribute, base, device_id):
    """An APID counted on from a base by a node's device id:
    ``graph::telemetry::counted_apid``.

    Args:
        attribute: The base's attribute, for the error message.
        base:      The base.
        device_id: The node's device id.

    Returns:
        The APID.

    Raises:
        ValueError: The sum is past an APID's eleven bits.
    """
    counted = base + device_id
    if counted > MAX_APID:
        raise ValueError(
            f"{attribute} {base:#x} plus the device id {device_id} is past an "
            f"APID's eleven bits, {MAX_APID:#x}: lower the base, or give the "
            "node a name that sorts earlier"
        )
    return counted


@dataclass(frozen=True)
class HostPort:
    """A host and a port, as ``address:port``."""

    address: str
    port: int

    def __str__(self):
        return f"{self.address}:{self.port}"

    @staticmethod
    def of(attribute, stated, any_port=False):
        """``stated`` as a host and a port.

        Split at the last colon, so that whatever a host is called the port is
        the part after it.

        Args:
            attribute: The attribute it was stated as, for the error message.
            stated:    What the configuration says, as ``address:port``.
            any_port:  Whether port 0, any port from the host, is allowed: for
                       where the ground sends from, which the vehicle never
                       binds. ``tcslibgs::types::sender_of``.

        Returns:
            The :class:`HostPort`, its address with surrounding space removed.

        Raises:
            ValueError: ``stated`` has no colon, no address, a port that is
                        not a number, or a port outside 1 to 65535 (0 to
                        65535 where ``any_port``).
        """
        said = stated.strip()
        if ":" not in said:
            raise ValueError(
                f'{attribute} "{said}" is not an address and a port: it is '
                "written as address:port, as 0.0.0.0:4000"
            )
        address, port = said.rsplit(":", 1)
        if not address.strip() or not port.strip().isdigit():
            raise ValueError(
                f'{attribute} "{said}" is not an address and a port'
            )
        number = int(port)
        if number == 0 and any_port:
            return HostPort(address.strip(), number)
        if not 0 < number < 65536:
            raise ValueError(f'{attribute} "{said}": a port is 1 to 65535')
        return HostPort(address.strip(), number)


@dataclass(frozen=True)
class BeaconLink:
    """Where beacons come from and where they are received."""

    send: HostPort
    recv: HostPort


@dataclass(frozen=True)
class ApidUse:
    """One APID a configuration that answers to Yamcs uses, and what it
    carries: ``graph::telemetry::ApidUse``."""

    apid: int
    #: What it carries: ``beacon``, ``commands`` (both command links and
    #: their answers), ``uplink`` (what the ground sends a gateway_yamcs
    #: node), ``downlink`` (a source's data through one) or
    #: ``payload_telemetry`` (a payload nothing touches).
    carries: str
    #: What it is for, as a complaint names it.
    what: str
    #: The node whose data it carries -- the gateway node for an uplink, the
    #: source for a downlink or a payload's telemetry -- or ``None``.
    node: str = None
    #: The gateway node a downlink goes through, or ``None``.
    gateway: str = None

    def may_share(self, other):
        """Whether two uses may be one APID: only a gateway node's commands
        and what reaches the ground through that same node, which their
        packet type tells apart. ``ApidUse::may_share``.

        Args:
            other: The other use.

        Returns:
            Whether the two may share an APID.
        """
        pair = {self.carries, other.carries}
        if pair != {"uplink", "downlink"}:
            return False
        uplink, downlink = ((self, other) if self.carries == "uplink"
                            else (other, self))
        return uplink.node == downlink.gateway


@dataclass(frozen=True)
class CommandLink:
    """The spacecraft command link: where the ground sends from, ``send``,
    and where the vehicle takes commands, ``recv``."""

    send: HostPort
    recv: HostPort


@dataclass
class Node:
    """One node, as the file writes it."""

    name: str
    node_type: str
    #: The section it is written in: ``gateways``, ``vehicle`` or
    #: ``payloads``.
    section: str = ""
    #: Its attributes, by name: every child but its name, its type and its
    #: links.
    attributes: dict = field(default_factory=dict)
    #: The nodes each of its terminals is linked to, by terminal:
    #: ``{"world-out": ["gateway"]}``.
    links: dict = field(default_factory=dict)


@dataclass
class Configuration:
    """A configuration, as far as a ground station reads one."""

    path: Path
    #: The ``tcspecial`` section, flat: ``beacon.recv`` and the rest.
    tcspecial: dict
    #: Every node, by name: the order a file writes them in means nothing,
    #: and a node's device id is its place among the names.
    nodes: list
    #: The command bindings, each the gateway node a command arrives on
    #: and the ``node.attribute`` it sets: ``(from, sets)``.
    commands: list = field(default_factory=list)
    #: The other files the configuration is made of, by the names its
    #: ``include`` list gives them: see :func:`include_path`.
    included: list = field(default_factory=list)

    def node(self, name):
        """The node called ``name``.

        Args:
            name: The node's name.

        Returns:
            The :class:`Node`.

        Raises:
            ValueError: There is no node called ``name``.
        """
        for node in self.nodes:
            if node.name == name:
                return node
        raise ValueError(f'{self.path}: there is no node called "{name}"')

    def device_id(self, name):
        """A node's device id.

        Args:
            name: The node's name.

        Returns:
            Its position among the configuration's nodes sorted by name,
            counting from nought: ``graph::telemetry::ids``. The order the
            file writes them in changes no id.

        Raises:
            ValueError: There is no node called ``name``.
        """
        names = sorted(node.name for node in self.nodes)
        if name in names:
            return names.index(name)
        raise ValueError(f'{self.path}: there is no node called "{name}"')

    def answers_over_bus(self):
        """Whether the vehicle answers over a software bus, as one that
        answers to cFS does: ``tcspecial.gateway`` is ``cfs``."""
        return self.tcspecial.get("gateway", "").strip() == "cfs"

    def bus(self):
        """The software bus a vehicle that answers to cFS is on.

        Returns:
            The :class:`Bus`, or ``None`` for a configuration whose links are
            sockets.

        Raises:
            ValueError: An attribute of it is missing or does not read.
        """
        if not self.answers_over_bus():
            return None

        def number(attribute, most=0xFFFFFFFF):
            return whole(attribute, self._said(attribute), most)

        def msgid(attribute):
            return number(attribute, 0xFFFF)

        sbn = Sbn(
            release=_sbn.release_of(self._said("sbn.release")),
            ident=_sbn.ident_of(self._said("sbn.ident")),
            spacecraft_id=number("sbn.spacecraft_id"),
            processor_id=number("sbn.processor_id"),
            local=HostPort.of("sbn.local", self._said("sbn.local")),
            peer=HostPort.of("sbn.peer", self._said("sbn.peer")),
            peer_processor_id=number("sbn.peer_processor_id"),
            max_message=number("sbn.max_message"),
        )
        return Bus(
            sbn=sbn,
            first=msgid("msgid_range.first"),
            last=msgid("msgid_range.last"),
            cmd_msgid=msgid("tcs_cmd.cmd_msgid"),
            response_msgid=msgid("tcs_cmd.response_msgid"),
            cmd_base=msgid("payload_command.cmd_base"),
            response_base=msgid("payload_command.response_base"),
            tlm_base=msgid("payload_telemetry.tlm_base"),
            beacon_msgid=msgid("beacon.tlm_msgid"),
        )

    def node_command_msgid(self, name):
        """The message ID commands about a node are published under:
        ``payload_command.cmd_base`` plus its device id.

        Args:
            name: The node's name.

        Returns:
            The message ID.

        Raises:
            ValueError: The configuration does not answer over a bus, there
                        is no such node, or the sum does not fit.
        """
        bus = self._bus()
        return counted("payload_command.cmd_base", bus.cmd_base,
                       self.device_id(name))

    def node_answer_msgid(self, name):
        """The message ID the answers about a node are published under:
        ``payload_command.response_base`` plus its device id.

        Args:
            name: The node's name.

        Returns:
            The message ID.

        Raises:
            ValueError: As :meth:`node_command_msgid`.
        """
        bus = self._bus()
        return counted("payload_command.response_base", bus.response_base,
                       self.device_id(name))

    def downlink_msgid(self, gateway, source):
        """The message ID what ``source`` sends to the ``gateway_cfs`` node
        called ``gateway`` is published under: its ``tlm_base`` plus the
        source's device id.

        Args:
            gateway: The gateway node's name.
            source:  The name of the node whose data is received.

        Returns:
            The message ID.

        Raises:
            ValueError: Either node is missing, ``gateway`` is not a gateway
                        on a bus, ``source`` has no ``world-out`` link to
                        it, or the base does not read or the sum not fit.
        """
        node = self._gateway(gateway)
        if node.node_type not in ON_BUS:
            raise ValueError(
                f'{self.path}: "{gateway}" is a {node.node_type} node, whose '
                "data arrives at a socket: see downlink"
            )
        self._sends_to(gateway, source)
        base = whole(f'"{gateway}" tlm_base',
                     node.attributes.get("tlm_base", ""), 0xFFFF)
        return counted(f'"{gateway}" tlm_base', base, self.device_id(source))

    def telemetry_msgids(self):
        """Every message ID the vehicle publishes on the bus, which a ground
        station subscribes to: the spacecraft's answers, the beacon, each
        node's answers, what each source drawn into a ``gateway_cfs`` node
        sends, and the telemetry of each payload nothing touches, over
        ``payload_telemetry.tlm_base`` -- the telemetry half of
        ``graph::telemetry::msgids``.

        Returns:
            The message IDs, sorted, each once.

        Raises:
            ValueError: The configuration does not answer over a bus, or a
                        message ID does not read or fit.
        """
        bus = self._bus()
        msgids = {bus.response_msgid, bus.beacon_msgid}
        for name in (node.name for node in self.nodes):
            msgids.add(self.node_answer_msgid(name))
        for node in self.nodes:
            for target in node.links.get("world-out", []):
                kind = self._kind(target)
                if kind in ON_BUS:
                    msgids.add(self.downlink_msgid(target, node.name))
        for name in self.untouched_payloads():
            msgids.add(counted("payload_telemetry.tlm_base", bus.tlm_base,
                               self.device_id(name)))
        return sorted(msgids)

    def framed(self):
        """Whether every packet to and from the ground carries a CCSDS
        primary header, as one that answers to Yamcs does:
        ``tcspecial.gateway`` is ``yamcs``. ``GatewayKind::framed``."""
        return self.tcspecial.get("gateway", "").strip() == "yamcs"

    def beacon_apid(self):
        """The APID the beacon carries as CCSDS telemetry: ``beacon.apid``.

        Returns:
            The APID.

        Raises:
            ValueError: The configuration does not answer to Yamcs, or the
                        APID is missing or not one.
        """
        self._framed()
        return apid("beacon.apid", self.tcspecial.get("beacon.apid"))

    def command_apid(self):
        """The APID commands on both command links carry as telecommands,
        and their answers as telemetry: ``tcs_cmd.apid``.

        Returns:
            The APID.

        Raises:
            ValueError: As :meth:`beacon_apid`.
        """
        self._framed()
        return apid("tcs_cmd.apid", self.tcspecial.get("tcs_cmd.apid"))

    def uplink_apid(self, gateway):
        """The APID what the ground sends the ``gateway_yamcs`` node called
        ``gateway`` carries as telecommands: its ``apid_base``.
        ``graph::reach::apid_base_of``.

        Args:
            gateway: The gateway node's name.

        Returns:
            The APID.

        Raises:
            ValueError: The configuration does not answer to Yamcs, there is
                        no such node, it is not a ``gateway_yamcs`` node, or
                        its ``apid_base`` is missing or not an APID.
        """
        self._framed()
        node = self._gateway(gateway)
        if node.node_type != "gateway_yamcs":
            raise ValueError(
                f'{self.path}: "{gateway}" is a {node.node_type} node, and '
                "only a gateway_yamcs node states an apid_base"
            )
        return apid(f'"{gateway}" apid_base',
                    node.attributes.get("apid_base"))

    def downlink_apid(self, gateway, source):
        """The APID what ``source`` sends to the ``gateway_yamcs`` node
        called ``gateway`` carries as telemetry: its ``apid_base`` plus the
        source's device id, as :meth:`downlink`'s port is counted.

        Args:
            gateway: The gateway node's name.
            source:  The name of the node whose data is received.

        Returns:
            The APID.

        Raises:
            ValueError: As :meth:`uplink_apid`, ``source`` has no
                        ``world-out`` link to ``gateway``, or the sum is
                        past eleven bits.
        """
        base = self.uplink_apid(gateway)
        self._sends_to(gateway, source)
        return counted_apid(f'"{gateway}" apid_base', base,
                            self.device_id(source))

    def untouched_payloads(self):
        """The payloads nothing in the drawing touches -- no link to or from
        them, no command binding naming them -- each of which reports over
        the section's ``payload_telemetry``: ``graph::telemetry::of``.

        Returns:
            Their names, in order.
        """
        touched = self._touched()
        return [node.name for node in self.nodes
                if node.section == "payloads" and node.name not in touched]

    def payload_telemetry_link(self, payload):
        """Where the telemetry of ``payload``, which nothing touches, leaves
        the vehicle and arrives at the ground: ``payload_telemetry``'s
        ``source_base`` and ``dest_base``, each counted on by its device id.

        Args:
            payload: The payload's name.

        Returns:
            The vehicle's :class:`HostPort` and the ground's.

        Raises:
            ValueError: The payload is not one nothing touches, the
                        configuration answers over a bus, or a base does not
                        read or its port counted on is past 65535.
        """
        self._not_on_a_bus("a payload's telemetry")
        device_id = self._untouched(payload)
        ends = []
        for attribute in ("payload_telemetry.source_base",
                          "payload_telemetry.dest_base"):
            base = HostPort.of(attribute, self._said(attribute))
            port = base.port + device_id
            if port > 65535:
                raise ValueError(
                    f"{self.path}: {attribute} {base} plus {payload}'s "
                    f"device id is port {port}, and a port is at most 65535"
                )
            ends.append(HostPort(base.address, port))
        return tuple(ends)

    def payload_telemetry_apid(self, payload):
        """The APID the telemetry of ``payload``, which nothing touches,
        carries as CCSDS telemetry: ``payload_telemetry.apid_base`` plus its
        device id, as its port is counted.

        Args:
            payload: The payload's name.

        Returns:
            The APID.

        Raises:
            ValueError: The configuration does not answer to Yamcs, the
                        payload is not one nothing touches, or the base is
                        missing, not an APID, or the sum past eleven bits.
        """
        self._framed()
        device_id = self._untouched(payload)
        base = apid("payload_telemetry.apid_base",
                    self.tcspecial.get("payload_telemetry.apid_base"))
        return counted_apid("payload_telemetry.apid_base", base, device_id)

    def apids(self):
        """Every APID the configuration uses, each checked: an APID, and
        used once. ``graph::telemetry::apids``.

        The beacon's and the command links', each ``gateway_yamcs`` node's
        ``apid_base``, every source drawn into one at that base plus its
        device id, and every payload nothing touches at
        ``payload_telemetry.apid_base`` plus its device id. Two uses may
        share an APID only as :meth:`ApidUse.may_share` allows.

        Returns:
            The :class:`ApidUse` of each, in that order: the gateway nodes
            and their sources, and the payloads, by name.

        Raises:
            ValueError: The configuration does not answer to Yamcs, an APID
                        is missing or not one, a sum is past eleven bits, or
                        two uses share an APID, naming both.
        """
        used = [ApidUse(self.beacon_apid(), "beacon", "beacon.apid"),
                ApidUse(self.command_apid(), "commands", "tcs_cmd.apid")]
        gateways = [node for node in self.nodes
                    if node.node_type == "gateway_yamcs"]
        for gateway in gateways:
            used.append(ApidUse(self.uplink_apid(gateway.name), "uplink",
                                f'the apid_base of "{gateway.name}"',
                                node=gateway.name))
        for gateway in gateways:
            for node in self.nodes:
                if gateway.name not in node.links.get("world-out", []):
                    continue
                used.append(ApidUse(
                    self.downlink_apid(gateway.name, node.name), "downlink",
                    f'the APID of what "{node.name}" sends to '
                    f'"{gateway.name}", its apid_base plus the device id '
                    f"{self.device_id(node.name)}",
                    node=node.name, gateway=gateway.name))
        for name in self.untouched_payloads():
            used.append(ApidUse(
                self.payload_telemetry_apid(name), "payload_telemetry",
                f'the APID of "{name}"\'s telemetry, '
                "payload_telemetry.apid_base plus its device id "
                f"{self.device_id(name)}",
                node=name))
        for at, one in enumerate(used):
            for before in used[:at]:
                if before.apid == one.apid and not before.may_share(one):
                    raise ValueError(
                        f"{self.path}: {before.what} and {one.what} are both "
                        f"APID {one.apid:#x}: Yamcs sorts packets by APID, so "
                        "each would be read as the other")
        return used

    def _framed(self):
        """Refuses a question about APIDs of a configuration whose packets
        carry none.

        Raises:
            ValueError: The configuration does not answer to Yamcs.
        """
        if not self.framed():
            raise ValueError(
                f"{self.path}: this configuration's gateway is "
                f"{self.tcspecial.get('gateway')!r}, not yamcs, so its "
                "packets carry no CCSDS header and it states no APIDs"
            )

    def _untouched(self, payload):
        """The device id of a payload nothing touches.

        Args:
            payload: The payload's name.

        Returns:
            Its device id.

        Raises:
            ValueError: There is no such node, or it is not a payload
                        nothing touches.
        """
        if payload not in self.untouched_payloads():
            self.node(payload)
            raise ValueError(
                f'{self.path}: "{payload}" is not a payload nothing in the '
                "drawing touches, so it does not report over "
                "payload_telemetry: it goes where the drawing says"
            )
        return self.device_id(payload)

    def _touched(self):
        """Every node the drawing touches: an end of a link, or of a command
        binding. ``graph::telemetry``'s ``touched``.

        Returns:
            Their names.
        """
        touched = set()
        for node in self.nodes:
            for targets in node.links.values():
                if targets:
                    touched.add(node.name)
                    touched.update(targets)
        for came, sets in self.commands:
            touched.add(came)
            touched.add(sets.split(".", 1)[0])
        return touched

    def _kind(self, name):
        """The node type of the node called ``name``.

        Args:
            name: The node's name.

        Returns:
            Its node type, or ``None`` where there is no such node.
        """
        for node in self.nodes:
            if node.name == name:
                return node.node_type
        return None

    def _bus(self):
        """The bus, refusing a configuration that answers over sockets.

        Returns:
            The :class:`Bus`.

        Raises:
            ValueError: The configuration does not answer over a bus.
        """
        bus = self.bus()
        if bus is None:
            raise ValueError(
                f"{self.path}: this configuration answers over sockets, not a "
                "software bus: it states no message IDs"
            )
        return bus

    def _not_on_a_bus(self, what):
        """Refuses a question about sockets of a configuration on a bus.

        Args:
            what: What was asked for, as a clause.

        Raises:
            ValueError: The configuration answers over a bus, saying where
                        to look instead.
        """
        if self.answers_over_bus():
            raise ValueError(
                f"{self.path}: this configuration answers to cFS, so {what} "
                "is a message ID on its software bus and not a socket: read "
                "it from bus(), through tcslib.bus"
            )

    def beacon_link(self):
        """Where tcspecial's beacons go from and to.

        Returns:
            The :class:`BeaconLink` of ``beacon.send`` and ``beacon.recv``.

        Raises:
            ValueError: Either is missing or is not an address and a port,
                        or the configuration answers over a bus.
        """
        self._not_on_a_bus("its beacon")
        return BeaconLink(
            send=HostPort.of("beacon.send", self._said("beacon.send")),
            recv=HostPort.of("beacon.recv", self._said("beacon.recv")),
        )

    def command_link(self):
        """The spacecraft command link: ``tcs_cmd.send`` and
        ``tcs_cmd.recv``.

        Returns:
            The :class:`CommandLink`.

        Raises:
            ValueError: Either is missing or is not an address and a port,
                        or the configuration answers over a bus.
        """
        self._not_on_a_bus("its command link")
        return CommandLink(
            send=HostPort.of("tcs_cmd.send", self._said("tcs_cmd.send"),
                             any_port=True),
            recv=HostPort.of("tcs_cmd.recv", self._said("tcs_cmd.recv")),
        )

    def beacon_interval(self):
        """How often tcspecial beacons, in nanoseconds:
        ``beacon.default_interval``, read as :func:`parse_interval` reads it.
        What the beacon indicator ages against -- see
        ``tcslib.beacon.Schedule.for_interval``."""
        return parse_interval(self._said("beacon.default_interval"))

    def gateway_nodes(self):
        """The gateway nodes -- the links to what the vehicle answers to -- in
        the order of their names."""
        return [node for node in self.nodes if node.node_type in GATEWAY_BASES]

    def uplink(self, gateway):
        """Where the ground sends to the ``gateway`` node called ``gateway``.

        Args:
            gateway: The gateway node's name.

        Returns:
            The :class:`HostPort` it binds aboard, which is what leaves it by
            its ``moc-out`` links.

        Raises:
            ValueError: There is no such gateway node, or it states no address
                        and port, or they are not a valid address and port.
        """
        node = self._gateway(gateway)
        address = node.attributes.get("address")
        port = node.attributes.get("port")
        if address is None or port is None:
            raise ValueError(
                f'{self.path}: "{gateway}" states no address and port, so it '
                "binds nothing aboard and the ground has nowhere to send to it"
            )
        return HostPort.of(f'"{gateway}" address:port', f"{address}:{port}")

    def downlink(self, gateway, source):
        """Where the ground receives what ``source`` sends to the gateway node
        called ``gateway``.

        Args:
            gateway: The gateway node's name.
            source:  The name of the node whose data is received.

        Returns:
            The :class:`HostPort`: the gateway's ``ground_base`` -- or
            ``bridge_base``, for an F´ gateway -- its port counted on by the
            source's device id, so that each source arrives on a port of its
            own.

        Raises:
            ValueError: Either node is missing, ``gateway`` is not a gateway,
                        ``source`` has no ``world-out`` link to it, the base
                        is not an address and a port, or the port counted on
                        is past 65535.
        """
        node = self._gateway(gateway)
        attribute = GATEWAY_BASES[node.node_type]
        if node.node_type in ON_BUS:
            raise ValueError(
                f'{self.path}: "{gateway}" is a {node.node_type} node, whose '
                "data is published on a software bus: see downlink_msgid"
            )
        self._sends_to(gateway, source)
        base = HostPort.of(
            f'"{gateway}" {attribute}', node.attributes.get(attribute, "")
        )
        port = base.port + self.device_id(source)
        if port > 65535:
            raise ValueError(
                f'{self.path}: "{gateway}" {attribute} {base} plus {source}\'s '
                f"device id is port {port}, and a port is at most 65535"
            )
        return HostPort(base.address, port)

    def _sends_to(self, gateway, source):
        """Refuses a source that sends nothing to ``gateway``.

        Args:
            gateway: The gateway node's name.
            source:  The source's name.

        Raises:
            ValueError: ``source`` is missing, or has no ``world-out`` link
                        to ``gateway``.
        """
        if gateway not in self.node(source).links.get("world-out", []):
            raise ValueError(
                f'{self.path}: "{source}" sends nothing to "{gateway}": it has '
                "no world-out link to it"
            )

    def _gateway(self, name):
        """The gateway node called ``name``.

        Args:
            name: The node's name.

        Returns:
            The :class:`Node`.

        Raises:
            ValueError: There is no such node, or it is not a gateway node.
        """
        node = self.node(name)
        if node.node_type not in GATEWAY_BASES:
            raise ValueError(
                f'{self.path}: "{name}" is a {node.node_type} node, not a '
                "gateway node"
            )
        return node

    def _said(self, attribute):
        """What the ``tcspecial`` section says of ``attribute``.

        Args:
            attribute: The attribute's flat name, as ``beacon.recv``.

        Returns:
            Its value, as text.

        Raises:
            ValueError: The section does not state it.
        """
        value = self.tcspecial.get(attribute)
        if value is None:
            raise ValueError(f"{self.path}: tcspecial states no {attribute}")
        return value


#: The units of a length of time, in microseconds: ``graph::parse_interval``'s.
INTERVAL_UNITS = {
    "us": 1,
    "ms": 1_000,
    "s": 1_000_000,
    "min": 60 * 1_000_000,
    "h": 3_600 * 1_000_000,
    "d": 24 * 3_600 * 1_000_000,
    "w": 7 * 24 * 3_600 * 1_000_000,
}


def parse_interval(text):
    """A length of time as a configuration writes one, in nanoseconds.

    The Python spelling of ``tcslibgs::graph::parse_interval``: the same units,
    underscores among the digits meaning nothing, a decimal fraction kept to
    the nearest whole microsecond, and nought refused. Its messages are
    shorter; what it takes and refuses is the same.

    Args:
        text: The length of time: a number, a space and a unit, as ``500 ms``
              or ``1.5 s``.

    Returns:
        The length of time in nanoseconds, a whole number of microseconds.

    Raises:
        ValueError: ``text`` is not a number and a unit, the unit is not one
                    of time, or the length is nought.
    """
    said = text.strip()
    parts = said.split()
    if len(parts) != 2:
        raise ValueError(
            f'"{text}" is a number and a unit with a space between them, and '
            "nothing else: 500 ms, 1.5 s, 15 min"
        )
    digits, unit = parts
    if unit not in INTERVAL_UNITS:
        raise ValueError(
            f'"{text}": "{unit}" is not a unit of time: us, ms, s, min, h, d '
            "or w"
        )
    bare = digits.replace("_", "")
    # Digits, at most one point, digits: no sign, no exponent.
    if not re.fullmatch(r"[0-9]*\.?[0-9]*", bare, re.ASCII) or bare in (
        "",
        ".",
    ):
        raise ValueError(f'"{text}" is not a number of {unit}')
    micros = round(float(bare) * INTERVAL_UNITS[unit])
    if micros == 0:
        raise ValueError(f'"{text}" is no interval at all')
    return micros * 1_000


#: The two spellings, by the flag that asks for each and the extension it is
#: written with: ``format::ConfigFormat`` in ``tcslibgs``.
SPELLINGS = {"yaml": ".ytcs", "xml": ".xtcs"}

#: The extensions every configuration once had, by the one each is named
#: with now: a configuration still named so is refused, and told the name to
#: give it.
OLD_EXTENSIONS = {".yaml": ".ytcs", ".yml": ".ytcs", ".xml": ".xtcs"}


def choose(named, stem, default, chosen=None):
    """The configuration to read: ``tcslibgs::format::Spelling::configuration``.

    ``named`` where one was named, and then ``chosen`` -- ``"yaml"`` or
    ``"xml"``, from ``--yaml`` or ``--xml`` -- has to agree with its
    extension, a file named ``.xtcs`` read as asked for in YAML being a command
    line that cannot both be right. Otherwise ``stem``, the example's path
    without an extension, in the spelling chosen, or in ``default`` where
    neither flag was given. The two flags together are refused before this,
    by the parser that reads them.

    Args:
        named:   The configuration named on the command line, or ``None``.
        stem:    The example's own configuration's path, without an
                 extension.
        default: The spelling, ``"yaml"`` or ``"xml"``, to read where no flag
                 chose one.
        chosen:  The spelling a flag chose, or ``None``.

    Returns:
        The path of the configuration to read, as text.

    Raises:
        ValueError: ``named``'s extension says one spelling and ``chosen``
                    the other.
    """
    if named is not None:
        named = str(named)
        suffix = Path(named).suffix.lower()
        spelled = {".ytcs": "yaml", ".xtcs": "xml"}.get(suffix)
        if chosen is not None and spelled is not None and spelled != chosen:
            raise ValueError(
                f"{named} is {spelled.upper()} and --{chosen} asks for "
                f"{chosen.upper()}: name the other file, or leave the flag off"
            )
        return named
    return f"{stem}{SPELLINGS[chosen or default]}"


def load(path):
    """Read the configuration at ``path``, YAML or XML by its extension,
    with every file its ``include`` list names.

    The included files are read as ``Graph::load`` reads them, by the same
    rules: each in the top file's spelling, at the path :func:`include_path`
    gives its name, none with a ``tcspecial`` section or an ``include`` list
    of its own, none named twice, and no node named in two of them. Their
    nodes are counted with the top file's, because a node's device id is its
    place among the names of the whole configuration: a station that counted
    only the top file's would receive on the wrong ports.

    Args:
        path: The configuration file, named ``.ytcs``, for YAML, or
              ``.xtcs``, for XML.

    Returns:
        The :class:`Configuration` it says.

    Raises:
        ValueError: The extension is neither of those -- ``.yaml``, ``.yml``
                    and ``.xml``, as configurations were once named,
                    included, with the name to give the file -- an
                    include is refused, a file is not a map of sections, or
                    it is YAML and PyYAML is not installed.
        OSError: A file cannot be read.
    """
    path = Path(path)
    suffix = path.suffix.lower()
    if suffix in (".xtcs", ".ytcs"):
        return _interface_checked(_with_includes(path))
    if suffix in OLD_EXTENSIONS:
        new = OLD_EXTENSIONS[suffix]
        spelling = "YAML" if new == ".ytcs" else "XML"
        raise ValueError(
            f"{path}: a configuration in {spelling} is named {new} now, not "
            f"{suffix}: rename it {path.with_suffix(new)}"
        )
    raise ValueError(
        f"{path}: a configuration file is named .ytcs, for YAML, or .xtcs, "
        "for XML, and this is neither"
    )


def _interface_checked(config):
    """The configuration, once its ``tcspecial`` section's ``interface`` is
    found to be this tree's: see :func:`tcslibgs.interface.of_configuration`.
    A configuration with no section states no links, and names no interface.

    Args:
        config: The configuration read.

    Returns:
        The configuration.

    Raises:
        ValueError: Its interface is not stated, or is not this tree's.
    """
    if config.tcspecial:
        try:
            _interface.of_configuration(config.tcspecial.get("interface"))
        except ValueError as why:
            raise ValueError(f"{config.path}: {why}") from None
    return config


def include_path(top, name):
    """Where an included file is: ``graph::include_path``.

    A name is a relative path without an extension, its parts separated by
    ``/``, each a name or ``..``, read from the directory of the file that
    names it and given that file's extension: ``zone``, or
    ``../thermostat-common/thermostat``. An empty name, an absolute one, one
    with a part that is empty or ``.``, one beginning with ``~`` and one with
    a ``$`` in it are refused, as the Rust refuses them: nothing is expanded.

    Args:
        top:  The file that names it.
        name: The name it is included by.

    Returns:
        The included file's path.

    Raises:
        ValueError: The name is of a shape refused above.
    """
    why = None
    if name == "":
        why = "is empty"
    elif name.startswith("/"):
        why = "is an absolute path"
    elif any(part in ("", ".") for part in name.split("/")):
        why = 'has a part that is empty or "."'
    elif name.startswith("~"):
        why = "begins with ~, which is not expanded"
    elif "$" in name:
        why = "has a $ in it, and no variable is expanded"
    if why is not None:
        raise ValueError(
            f'{top}: the include "{name}" {why}: an include is relative to '
            "this file's directory, naming a file by its path from there "
            "without an extension, such as zone or ../common/zone")
    return top.parent / f"{name}{top.suffix}"


def _load_one(path):
    """Read one file of a configuration, in the spelling its extension says.

    Args:
        path: The file.

    Returns:
        A pair: the :class:`Configuration` it says alone, and whether it has
        a ``tcspecial`` section.
    """
    if path.suffix.lower() == ".xtcs":
        return _load_xml(path)
    return _load_yaml(path)


def _with_includes(path):
    """Read a configuration and every file it includes, as one.

    Args:
        path: The top file.

    Returns:
        The :class:`Configuration`, every file's nodes and command bindings
        in it.

    Raises:
        ValueError: A file is refused, an include name is refused, a file is
                    included twice or is the top file, an included file has
                    a ``tcspecial`` section or includes of its own, or two
                    files have a node of one name.
        OSError: A file cannot be read.
    """
    config, _ = _load_one(path)
    homes = {node.name: path for node in config.nodes}
    seen = [path.resolve(strict=True)]
    for name in config.included:
        included = include_path(path, name)
        part, has_section = _load_one(included)
        real = included.resolve(strict=True)
        if real in seen:
            raise ValueError(f"{path}: {included} is included twice, or is "
                             "this file")
        seen.append(real)
        if has_section:
            raise ValueError(
                f"{included}: an included file may not have a tcspecial "
                "section -- a configuration has one command interpreter, and "
                "the file that was asked for says what it is")
        if part.included:
            raise ValueError(
                f"{included}: an included file may not include others")
        for node in part.nodes:
            if node.name in homes:
                raise ValueError(
                    f'{included}: there is a node called "{node.name}" here '
                    f"and in {homes[node.name]}: a node's name is the one "
                    "thing every other part of a configuration knows it by, "
                    "so no two nodes share one")
            homes[node.name] = included
        config.nodes = sorted(config.nodes + part.nodes,
                              key=lambda n: n.name)
        config.commands.extend(part.commands)
    return config


def node_types(section):
    """The node types a section may hold, as a file spells them: the
    schema's list, which tcsgui's schema tests hold to
    ``tcslibgs::graph::types_in``.

    Args:
        section: The section, as a file names it: ``gateways``, ``vehicle``
                 or ``payloads``.

    Returns:
        The types.
    """
    with open(SCHEMAS / "tcspecial.schema.json", encoding="utf-8") as text:
        schema = json.load(text)
    return [key[len("type "):]
            for key in schema["$defs"][f"{section}Type"]["enum"]]


def _section_of(node_type):
    """The section a node type belongs in, if any.

    Args:
        node_type: The type.

    Returns:
        The section's name, or ``None`` for a type of none.
    """
    for section in NODE_SECTIONS:
        if node_type in node_types(section):
            return section
    return None


def _type_refused(section, named):
    """What is wrong with a type a section groups its nodes by, if anything:
    ``graph::grouped::type_of_section``.

    Args:
        section: The section.
        named:   The type, as the file wrote it.

    Returns:
        The complaint, or ``None`` for one of the section's types.
    """
    if not named:
        return f"a type in {section} names no type: write the type after " \
               "the word"
    if any(c.isspace() for c in named):
        return f'"{named}" is not a type: a type is one word, with no space ' \
               "in it"
    if named in node_types(section):
        return None
    belongs = _section_of(named)
    if belongs is not None:
        return f"{named} is a type of the {belongs} section, and is written " \
               f"under {belongs} rather than {section}"
    return f"{named} is not a type of node the {section} section holds, " \
           f"which are {', '.join(node_types(section))}"


#: How a configuration in the old shape is rewritten in the new.
CONVERTER = "tools/convert-format.py rewrites a file in the old shape into " \
            "this one"


class _Reader:
    """What one file is read into, with the complaints it may raise."""

    def __init__(self, path):
        """Begin reading.

        Args:
            path: The file, for the complaints.
        """
        self.path = path
        self.nodes = []
        self.commands = []
        self.included = []

    def refuse(self, line, why):
        """The error for something refused.

        Args:
            line: The line it is on.
            why:  What is wrong.

        Returns:
            The ``ValueError`` to raise.
        """
        return ValueError(f"{self.path}: at line {line}, {why}")

    def add(self, node, line):
        """Take a node in, refusing a name already taken.

        Args:
            node: The node.
            line: The line it is on.

        Raises:
            ValueError: A node of that name has been read already.
        """
        if any(other.name == node.name for other in self.nodes):
            raise self.refuse(line,
                              f'there is a node called "{node.name}" '
                              "twice in this file: a node's name is the one "
                              "thing every other part of a configuration knows "
                              "it by, so no two nodes share one")
        self.nodes.append(node)

    def done(self, tcspecial, has_section):
        """The file read.

        Args:
            tcspecial:   The ``tcspecial`` section, flat.
            has_section: Whether the file has a ``tcspecial`` section.

        Returns:
            A pair: the :class:`Configuration` the file says alone, its nodes
            by name, and ``has_section``.
        """
        return (Configuration(path=self.path, tcspecial=tcspecial,
                              nodes=sorted(self.nodes, key=lambda n: n.name),
                              commands=list(self.commands),
                              included=list(self.included)),
                has_section)


class _Element:
    """One XML element, and the line it begins on."""

    def __init__(self, tag, attributes, line):
        """Begin one.

        Args:
            tag:        Its name.
            attributes: Its attributes.
            line:       The line its start tag is on.
        """
        self.tag = tag
        self.attributes = attributes
        self.line = line
        self.children = []
        self.text = ""

    def find(self, tag):
        """Its first child of a name.

        Args:
            tag: The name.

        Returns:
            The child, or ``None``.
        """
        return next((c for c in self.children if c.tag == tag), None)


def _xml_tree(path):
    """Read an XML file into elements that know their lines.

    Args:
        path: The file.

    Returns:
        The root element.

    Raises:
        xml.parsers.expat.ExpatError: The file is not well-formed XML.
    """
    parser = xml.parsers.expat.ParserCreate()
    stack = []
    root = []

    def start(tag, attributes):
        """Open an element.

        Args:
            tag:        Its name.
            attributes: Its attributes.
        """
        element = _Element(tag, attributes, parser.CurrentLineNumber)
        if stack:
            stack[-1].children.append(element)
        else:
            root.append(element)
        stack.append(element)

    def end(tag):
        """Close an element.

        Args:
            tag: Its name, which is not needed.
        """
        stack.pop()

    def text(said):
        """Add text to the element it is in.

        Args:
            said: The text.
        """
        if stack:
            stack[-1].text += said

    parser.StartElementHandler = start
    parser.EndElementHandler = end
    parser.CharacterDataHandler = text
    with open(path, "rb") as data:
        parser.ParseFile(data)
    return root[0]


def _xml_old(reader, root):
    """Refuse an XML file in the old shape, at the line of its first node:
    ``graph::reshaped``.

    Args:
        reader: The reader, for the complaint.
        root:   The root element.

    Raises:
        ValueError: The file is in the old shape.
    """
    old = {"gateway": ("gateways", "gateway"),
           "vehicle": ("vehicle", "compute"),
           "payloads": ("payloads", "payload")}
    for element in root.children:
        if element.tag == "tcsgui":
            for child in element.children:
                if child.tag == "nodes":
                    stale = child.find("name") is not None
                elif child.tag == "set-aside":
                    stale = (child.find("concern") is not None
                             or any("name" not in n.attributes
                                    for n in child.children))
                else:
                    stale = False
                if stale:
                    raise reader.refuse(child.line,
                                        f"<{child.tag}> in the tcsgui section "
                                        "is written as it was before the "
                                        "editor's section named nodes as the "
                                        "sections do; " + CONVERTER)
            continue
        if element.tag not in old:
            continue
        stale = (element.tag == "gateway"
                 or any(c.tag == "name" or c.tag == "type"
                        and "name" not in c.attributes
                        for c in element.children))
        if not stale:
            continue
        section, word = old[element.tag]
        name = element.find("name")
        name = name.text.strip() if name is not None else "..."
        kind = element.find("type")
        kind = kind.text.strip() if kind is not None else "..."
        raise reader.refuse(
            element.line,
            f'the node "{name}" is written as a node was before nodes were '
            f'grouped by type, as a <{element.tag}> element with <name> and '
            f'<type> in it. Write it under its type now: <{section}><type '
            f'name="{kind}"><{word} name="{name}">...</{word}></type>'
            f'</{section}>; ' + CONVERTER)


def _xml_node(reader, section, node_type, element):
    """Read one XML node element.

    Args:
        reader:    The reader, for the complaints.
        section:   The section it is in.
        node_type: The type it is written under.
        element:   The element.

    Returns:
        The :class:`Node`.

    Raises:
        ValueError: The node is refused: see :func:`_load_xml`.
    """
    word = NODE_SECTIONS[section]
    for attribute in element.attributes:
        if attribute == "type":
            raise reader.refuse(element.line,
                                "a node says a type of its own: a node's type "
                                "is the type it is written under")
        if attribute != "name":
            raise reader.refuse(element.line,
                                f"<{word}> has a name attribute and no other, "
                                f"and this one has {attribute}")
    if "name" not in element.attributes:
        raise reader.refuse(element.line,
                            f'a {node_type} node has no name: write it as '
                            f'<{word} name="...">')
    name = element.attributes["name"].strip()
    if not name:
        raise reader.refuse(element.line,
                            f"a {node_type} node has an empty name")
    node = Node(name=name, node_type=node_type, section=section)
    seen = set()
    for child in element.children:
        if child.tag in ("name", "type", "editor"):
            raise reader.refuse(child.line,
                                f'"{name}" says <{child.tag}>, which a node '
                                'does not: its name is its name attribute, '
                                'its type the type it is written under')
        if child.tag in seen and child.tag not in LINKS + SINKS + LISTED:
            raise reader.refuse(child.line,
                                f'"{name}" states <{child.tag}> twice')
        seen.add(child.tag)
        if child.tag in LINKS:
            said = _destination(reader, child.line, child.tag,
                                child.find("target") is not None,
                                child.text.strip())
            node.links.setdefault(child.tag, []).append(said)
        elif child.tag in SINKS:
            name = _sink(reader, child.line, child.tag, child.text.strip())
            had = node.attributes.get(child.tag)
            node.attributes[child.tag] = (name if had is None
                                          else f"{had}, {name}")
        elif child.tag in LISTED:
            had = node.attributes.get(child.tag)
            one = child.text.strip()
            node.attributes[child.tag] = one if had is None else f"{had}, {one}"
        else:
            node.attributes[child.tag] = child.text.strip()
    return node


def _layout_entry(reader, line, what, said, seen):
    """Refuse an entry of the editor's section that does not name a node and
    say the words its list takes: ``graph::grouped::layout`` and
    ``set_aside``.

    Args:
        reader: The reader, for the complaint.
        line:   The line it is on.
        what:   The list: ``nodes`` or ``set-aside``.
        said:   What the entry says, by word, its node as ``node``.
        seen:   The entries read so far, which this one is added to.

    Raises:
        ValueError: It names no node, leaves out or adds a word, gives a
                    position that is not a whole number, or says again what
                    one before it said.
    """
    words = ("x", "y") if what == "nodes" else ("concern",)
    if not str(said.get("node", "")).strip():
        raise reader.refuse(line, f"an entry of {what} names no node")
    for word in said:
        if word != "node" and word not in words:
            raise reader.refuse(
                line, f"an entry of {what} says {word}, which it does not")
    for word in words:
        if word not in said:
            raise reader.refuse(line,
                                f"the entry of {what} for \"{said['node']}\" "
                                f"says no {word}")
    if what == "nodes":
        for axis in words:
            try:
                int(str(said[axis]).strip())
            except ValueError:
                raise reader.refuse(line,
                                    f"\"{said['node']}\" is placed at {axis} "
                                    f"\"{said[axis]}\", which is not a whole "
                                    "number") from None
    key = said["node"] if what == "nodes" else tuple(sorted(said.items()))
    if key in seen:
        raise reader.refuse(line, f"{what} names \"{said['node']}\" twice")
    seen.add(key)


def _sink(reader, line, sink, said):
    """Read one name a data sink lists, as ``tcslibgs::graph`` reads it.

    Args:
        reader: The reader, for the complaint.
        line:   The line it is on.
        sink:   The sink.
        said:   The name.

    Returns:
        The name.

    Raises:
        ValueError: It is the flag a sink once was, or not one word.
    """
    if said in ("true", "false"):
        raise reader.refuse(line,
                            f'{sink}: "{said}" is a name for where a line may '
                            'arrive here, not whether one does: list the '
                            f'names, as `{sink}: [readings]`')
    if not said or any(c in said for c in ".,  "):
        raise reader.refuse(line, f'{sink}: "{said}" is not a name')
    return said


def _destination(reader, line, source, entry, said):
    """Read where one line goes, as ``tcslibgs::graph`` reads it, and keep
    the node it reaches.

    The name after the dot is the node's own business: what a ground
    station reads here is which node each line reaches.

    Args:
        reader: The reader, for the complaint.
        line:   The line it is on.
        source: The data source it leaves by.
        entry:  Whether it was written as an entry with a ``target``.
        said:   The destination, ``node.name``.

    Returns:
        The node.

    Raises:
        ValueError: It is an entry, or names no name.
    """
    if entry:
        raise reader.refuse(line,
                            f"{source}: a link is written as the node it "
                            "reaches and the name it arrives by, "
                            "`zone_temp.in0`, and not as an entry with "
                            "`target`")
    node, dot, name = said.partition(".")
    if not dot or not node or not name:
        raise reader.refuse(line,
                            f'{source}: "{said}" names no name to arrive by: '
                            f"write it as `{said}.<name>`")
    return node


def _load_xml(path):
    """Read a configuration written in XML.

    Refused, as ``tcslibgs::graph::grouped`` refuses them: a file in the
    old shape, a type that is not one of its section's, a type written twice
    in a section or with no nodes in it, a node with a type of its own, with
    no name or an empty one, or that says a word twice, and two nodes of one
    name.

    Args:
        path: The configuration file.

    Returns:
        A pair: the :class:`Configuration` the file says alone, its includes
        named but not read, and whether it has a ``tcspecial`` section.

    Raises:
        ValueError: The file is refused.
        xml.parsers.expat.ExpatError: The file is not well-formed XML.
    """
    root = _xml_tree(path)
    reader = _Reader(path)
    for element in root.children:
        if element.tag == "include":
            reader.included.append(element.text.strip())
    _xml_old(reader, root)

    tcspecial = {}
    section = root.find("tcspecial")
    if section is not None:
        for part in section.children:
            if part.children:
                for leaf in part.children:
                    tcspecial[f"{part.tag}.{leaf.tag}"] = leaf.text.strip()
            else:
                tcspecial[part.tag] = part.text.strip()

    seen = set()
    for element in root.children:
        if element.tag not in NODE_SECTIONS:
            continue
        section = element.tag
        if section in seen:
            raise reader.refuse(element.line, f"<{section}> is written twice")
        seen.add(section)
        word = NODE_SECTIONS[section]
        typed = set()
        if element.attributes or element.text.strip():
            raise reader.refuse(element.line,
                                f'<{section}> holds <type name="..."> elements '
                                'and nothing else')
        for group in element.children:
            if group.tag != "type" or set(group.attributes) - {"name"}:
                raise reader.refuse(group.line,
                                    f'<{section}> holds <type name="..."> '
                                    'elements, each with a name and no other '
                                    'attribute')
            if "name" not in group.attributes:
                raise reader.refuse(group.line,
                                    f"a <type> in <{section}> has no name")
            node_type = group.attributes["name"]
            why = _type_refused(section, node_type)
            if why is not None:
                raise reader.refuse(group.line, why)
            if node_type in typed:
                raise reader.refuse(group.line,
                                    f"type {node_type} is written twice in "
                                    f"{section}: put its nodes under one")
            typed.add(node_type)
            if not group.children:
                raise reader.refuse(group.line,
                                    f"type {node_type} in <{section}> has no "
                                    "nodes in it")
            for child in group.children:
                if child.tag != word:
                    raise reader.refuse(child.line,
                                        f"a <type> in <{section}> holds "
                                        f"<{word}> elements, and this one has "
                                        f"<{child.tag}>")
                reader.add(_xml_node(reader, section, node_type, child),
                           child.line)

    for element in root.children:
        if element.tag != "commands":
            continue
        came = element.find("from")
        sets = element.find("sets")
        if came is None or sets is None:
            raise reader.refuse(element.line,
                                "a command binding says the gateway node it "
                                "arrives on, <from>, and what it sets, <sets>")
        reader.commands.append((came.text.strip(), sets.text.strip()))

    layout = root.find("tcsgui")
    for part in layout.children if layout is not None else []:
        if part.tag not in ("nodes", "set-aside"):
            raise reader.refuse(part.line,
                                f"the tcsgui section has no <{part.tag}>")
        seen = set()
        for entry in part.children:
            if entry.tag != "node":
                raise reader.refuse(entry.line,
                                    f'<{part.tag}> holds <node name="..."> '
                                    'elements')
            said = dict(entry.attributes)
            if "name" in said:
                said["node"] = said.pop("name")
            _layout_entry(reader, entry.line, part.tag, said, seen)

    return reader.done(tcspecial, root.find("tcspecial") is not None)


def _scalar(value):
    """A YAML scalar as the XML spelling writes it: the text of an element.
    YAML's own true and false are the words, as the XML has them."""
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value).strip()


def _yaml_mapping(reader, node, what):
    """The entries of a YAML mapping, refusing a key said twice.

    Args:
        reader: The reader, for the complaints.
        node:   The mapping, as composed.
        what:   What it is, for the complaint.

    Returns:
        Each key, as text, with its value node and the line of the key.

    Raises:
        ValueError: It is not a mapping, or says a key twice.
    """
    import yaml

    if not isinstance(node, yaml.MappingNode):
        raise reader.refuse(node.start_mark.line + 1,
                            f"{what} is not a mapping")
    entries = []
    seen = set()
    for key, value in node.value:
        said = str(key.value)
        line = key.start_mark.line + 1
        if said in seen:
            raise reader.refuse(line, f"{what} states {said} twice")
        seen.add(said)
        entries.append((said, value, line))
    return entries


def _yaml_list(reader, node, what):
    """The items of a YAML list; none for a value left empty.

    Args:
        reader: The reader, for the complaints.
        node:   The list, as composed.
        what:   What it is, for the complaint.

    Returns:
        The items, as composed.

    Raises:
        ValueError: It is neither a list nor empty.
    """
    import yaml

    if isinstance(node, yaml.ScalarNode) and node.tag.endswith(":null"):
        return []
    if not isinstance(node, yaml.SequenceNode):
        raise reader.refuse(node.start_mark.line + 1, f"{what} is not a list")
    return node.value


def _yaml_old(reader, root):
    """Refuse a YAML file in the old shape, at the line of its first node:
    ``graph::reshaped``.

    Args:
        reader: The reader, for the complaint.
        root:   The file's mapping, as composed.

    Raises:
        ValueError: The file is in the old shape.
    """
    import yaml

    old = {"gateway": ("gateways", "gateway"),
           "vehicle": ("vehicle", "compute"),
           "payloads": ("payloads", "payload")}
    for key_node, value in root.value:
        key = str(key_node.value)
        items = value.value if isinstance(value, yaml.SequenceNode) else []
        if key == "tcsgui" and isinstance(value, yaml.MappingNode):
            for part, placed in value.value:
                if (part.value != "nodes"
                        or not isinstance(placed, yaml.SequenceNode)):
                    continue
                for entry in placed.value:
                    if (isinstance(entry, yaml.MappingNode)
                            and any(k.value == "name" for k, _ in entry.value)):
                        raise reader.refuse(entry.start_mark.line + 1,
                                            "a position is written as it was "
                                            "before the editor's section named "
                                            "nodes as the sections do: write "
                                            "it `- { node: <name>, x: ..., y: "
                                            "... }`; " + CONVERTER)
            continue
        if key not in old:
            continue
        for entry in items:
            if not isinstance(entry, yaml.MappingNode):
                continue
            said = {str(k.value): v for k, v in entry.value}
            if "name" in said or "type" in said:
                section, word = old[key]
                name = getattr(said.get("name"), "value", "...")
                kind = getattr(said.get("type"), "value", "...")
                raise reader.refuse(entry.start_mark.line + 1,
                                    f'the node "{name}" is written as a node '
                                    'was before nodes were grouped by type, '
                                    'with name: and type: of its own. Write '
                                    f'it under its type now: {section}: - type '
                                    f'{kind}: - {word}: {name}; ' + CONVERTER)
        if key == "gateway":
            raise reader.refuse(key_node.start_mark.line + 1,
                                "the gateway section is gateways now, its "
                                "nodes grouped by type; " + CONVERTER)


def _yaml_node(reader, section, node_type, item, construct):
    """Read one YAML node.

    Args:
        reader:    The reader, for the complaints.
        section:   The section it is in.
        node_type: The type it is written under.
        item:      The node's mapping, as composed.
        construct: Turns a composed value into a Python one.

    Returns:
        The :class:`Node`.

    Raises:
        ValueError: The node is refused: see :func:`_load_yaml`.
    """
    word = NODE_SECTIONS[section]
    line = item.start_mark.line + 1
    name = None
    attributes = {}
    links = {}
    for key, value, at in _yaml_mapping(reader, item, f"a {node_type} node"):
        if key == word:
            name = _scalar(construct(value))
            if not name:
                raise reader.refuse(at, f"a {node_type} node has an empty name")
        elif key in NODE_SECTIONS.values():
            raise reader.refuse(at,
                                f"a node of the {section} section is written "
                                f"`- {word}: <name>`, and this one is written "
                                f"`{key}:`")
        elif key in ("name", "type", "editor"):
            raise reader.refuse(at,
                                f"a node says {key}:, which a node does not: "
                                f"it is named as `- {word}: <name>`, and its "
                                "type is the type it is written under")
        elif key in LINKS:
            said = construct(value)
            for one in said if isinstance(said, list) else [said]:
                entry = isinstance(one, dict)
                links.setdefault(key, []).append(_destination(
                    reader, at, key, entry, "" if entry else _scalar(one)))
        elif key in SINKS:
            said = construct(value)
            names = [_sink(reader, at, key, _scalar(one))
                     for one in (said if isinstance(said, list) else [said])]
            attributes[key] = ", ".join(names)
        elif key in LISTED:
            said = construct(value)
            said = said if isinstance(said, list) else [said]
            attributes[key] = ", ".join(_scalar(one) for one in said)
        else:
            attributes[key] = _scalar(construct(value))
    if name is None:
        raise reader.refuse(line,
                            f"a {node_type} node has no name: write it as `- "
                            f"{word}: <name>`")
    return Node(name=name, node_type=node_type, section=section,
                attributes=attributes, links=links)


def _load_yaml(path):
    """Read a configuration written in YAML, with PyYAML.

    Refused, as ``tcslibgs::graph::grouped`` refuses them: a file in the
    old shape, a type key that is not the word ``type``, one space and one of
    its section's types, a type written twice in a section or with no nodes,
    a node with a type of its own, with no name or an empty one, or that
    says a key twice, and two nodes of one name.

    Args:
        path: The configuration file.

    Returns:
        A pair: the :class:`Configuration` the file says alone, its includes
        named but not read, and whether it has a ``tcspecial`` section.

    Raises:
        ValueError: PyYAML is not installed, the file is not a map of
                    sections, or it is refused.
        yaml.YAMLError: The file is not well-formed YAML.
    """
    try:
        import yaml
    except ImportError as why:
        raise ValueError(
            f"{path} is YAML, which is read with PyYAML, and this Python has "
            f"none ({why}): install it (python3-yaml on Debian and Ubuntu, or "
            "pip install pyyaml), or read the XML spelling"
        ) from why

    with open(path, encoding="utf-8") as text:
        root = yaml.compose(text)
    reader = _Reader(path)
    if root is None:
        return reader.done({}, False)
    if not isinstance(root, yaml.MappingNode):
        raise ValueError(f"{path} is not a configuration: it is not a map of "
                         "sections")
    constructor = yaml.constructor.SafeConstructor()

    def construct(value):
        """A composed value, as a Python one.

        Args:
            value: The value.

        Returns:
            What it says.
        """
        return constructor.construct_object(value, deep=True)

    sections = _yaml_mapping(reader, root, "the file")
    for key, value, at in sections:
        if key != "include":
            continue
        for name in _yaml_list(reader, value, "include"):
            said = construct(name)
            if not isinstance(said, str):
                raise reader.refuse(name.start_mark.line + 1,
                                    "an include is a file's name")
            reader.included.append(said)
    _yaml_old(reader, root)

    tcspecial = {}
    for key, value, _ in sections:
        if key != "tcspecial":
            continue
        for part, said in (construct(value) or {}).items():
            if isinstance(said, dict):
                for leaf, scalar in said.items():
                    tcspecial[f"{part}.{leaf}"] = _scalar(scalar)
            else:
                tcspecial[part] = _scalar(said)

    for section, value, _ in sections:
        if section not in NODE_SECTIONS:
            continue
        typed = set()
        for entry in _yaml_list(reader, value, section):
            groups = _yaml_mapping(reader, entry, f"an entry of {section}")
            if len(groups) != 1:
                raise reader.refuse(entry.start_mark.line + 1,
                                    f"an entry of {section} names one type, "
                                    "and its nodes are the list under it")
            key, nodes, at = groups[0]
            if not key.startswith("type "):
                raise reader.refuse(at,
                                    f'"{key}" is not a type: each entry of '
                                    f'{section} is the word type, a space and '
                                    'the type its nodes are')
            node_type = key[len("type "):]
            if node_type[:1].isspace():
                raise reader.refuse(
                    at, f'"{key}" has more than one space after type')
            why = _type_refused(section, node_type)
            if why is not None:
                raise reader.refuse(at, why)
            if node_type in typed:
                raise reader.refuse(at,
                                    f"type {node_type} is written twice in "
                                    f"{section}: put its nodes under one")
            typed.add(node_type)
            items = _yaml_list(reader, nodes, f"type {node_type}")
            if not items:
                raise reader.refuse(
                    at, f"type {node_type} in {section} has no nodes under it")
            for item in items:
                reader.add(_yaml_node(reader, section, node_type, item,
                                      construct), item.start_mark.line + 1)

    for key, value, at in sections:
        if key != "commands":
            continue
        for binding in construct(value) or []:
            if (not isinstance(binding, dict) or "from" not in binding
                    or "sets" not in binding):
                raise reader.refuse(at,
                                    "a command binding says the gateway node "
                                    "it arrives on, from, and what it sets, "
                                    "sets")
            reader.commands.append((_scalar(binding["from"]).strip(),
                                    _scalar(binding["sets"]).strip()))

    for key, value, at in sections:
        if key != "tcsgui":
            continue
        for part, entries, line in _yaml_mapping(reader, value, "tcsgui"):
            if part not in ("nodes", "set-aside"):
                raise reader.refuse(line, f"the tcsgui section has no {part}")
            seen = set()
            for entry in _yaml_list(reader, entries, part):
                said = {
                    key: construct(value)
                    for key, value, _ in _yaml_mapping(reader, entry,
                                                       f"an entry of {part}")}
                _layout_entry(reader, entry.start_mark.line + 1, part, said,
                              seen)

    has_section = any(key == "tcspecial" for key, _, _ in sections)
    return reader.done(tcspecial, has_section)


def one_host(address):
    """The names that mean this host, as one name: ``localhost``,
    ``127.0.0.1`` and ``::1`` are one host everywhere in these rules."""
    return (
        "localhost" if address in ("localhost", "127.0.0.1", "::1") else address
    )


def binds_every_interface(address):
    """Whether an address is a wildcard, claiming a port on every interface."""
    return address in ("", "0.0.0.0", "::", "*")


def came_from(stated, sender):
    """Whether a datagram from ``sender`` came from the socket the
    configuration names as ``stated``.

    The port must match, unless the configuration names port 0: a ground
    that cannot send from a port of its own choosing -- Yamcs sends each
    command from whatever port the system gives it -- is named with port 0,
    and then any port from that host is it. The address must match too,
    unless the configuration names every interface, in which case the address
    a datagram carries is whichever interface the route chose and there is
    nothing written down to compare it with.

    Args:
        stated: The :class:`HostPort` the configuration names.
        sender: The ``(address, port)`` pair the datagram came from.

    Returns:
        ``True`` if it came from there.
    """
    address, port = sender[0], sender[1]
    if stated.port != 0 and port != stated.port:
        return False
    if binds_every_interface(stated.address):
        return True
    return one_host(stated.address) == one_host(address)


def find_built(program):
    """Where one of the repository's programs is: its own build first, then
    the path.

    Its own build is where cargo put it: the directory ``CARGO_TARGET_DIR``
    names, where that is set -- a relative one read from the repository's
    root, which is where cargo is run -- and ``target`` otherwise. A build
    sent elsewhere and looked for in ``target`` is a program that is not
    there, or worse, an older one that is.

    Args:
        program: The program's file name, as ``tcsverify``.

    Returns:
        Its path, or ``None`` where it is in neither.
    """
    root = Path(__file__).resolve().parents[2]
    targets = [root / "target"]
    if os.environ.get("CARGO_TARGET_DIR"):
        targets.insert(0, root / os.environ["CARGO_TARGET_DIR"])
    for target in targets:
        for profile in ("debug", "release"):
            built = target / profile / program
            if built.is_file():
                return str(built)
    return shutil.which(program)


def find_tcsverify():
    """Where tcsverify is: see :func:`find_built`."""
    return find_built("tcsverify")


def digest(path, tcsverify=None):
    """The configuration's digest, as tcsverify works it out.

    ``None`` is not a failure. A station that cannot digest its own
    configuration compares nothing, as the Rust ground programs do, and says
    so rather than claiming every beacon agrees.

    Args:
        path:      The configuration file.
        tcsverify: The tcsverify program to run, or ``None`` to find one with
                   :func:`find_tcsverify`.

    Returns:
        A pair: the digest, as thirty-two hex digits, and ``None``; or
        ``None`` and the reason it cannot be had.
    """
    program = tcsverify or find_tcsverify()
    if program is None:
        return None, "tcsverify is not built or on the PATH"
    try:
        done = subprocess.run(
            [program, "--digest", "--error-limit=0", str(path)],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except OSError as why:
        return None, f"{program} cannot be run: {why}"
    for line in done.stdout.splitlines():
        if ": md5 " in line:
            return line.rsplit(": md5 ", 1)[1].strip(), None
    return None, (
        f"{program} gave no digest: "
        f"{done.stdout.strip() or done.stderr.strip()}"
    )
