"""What a ground station reads out of a tcspecial configuration.

The Python spelling of what the Rust ground programs ask ``tcslibgs`` for:
where the beacons are (``graph::parameters::beacon_of``), where a ``gateway``
node is bound aboard, which port a source's data reaches the ground at
(``graph::order`` and ``graph::telemetry``), whether a datagram came from where
the configuration says (``types::came_from``), and the configuration's digest.
For a configuration that answers to cFS, whose links are message IDs on a
software bus rather than sockets, it reads the bus instead: its SBN peer and
every message ID, counted as ``graph::telemetry::msgids`` counts them -- see
:meth:`Configuration.bus`.

It reads either spelling, chosen by the file's extension as the Rust's
``format::ConfigFormat`` chooses it. The XML with the standard library alone;
the YAML with PyYAML, which is imported only when a YAML file is read, so that
a station reading XML needs nothing outside the standard library. Both are
read into the same :class:`Configuration`, and the two spellings say the same
thing, which ``make comparespellings`` and tcsverify hold them to;
``examples/ping/tests/agrees.rs`` holds what this reads to what the Rust
reads.

The digest is not worked out here. It is taken over what a configuration says
rather than over its bytes -- see ``tcslibgs::config_digest`` -- and a second
statement of that in Python would be a second digest on the day the first
changes. :func:`digest` asks ``tcsverify --digest``, which is the Rust one.

This reads only what it needs, and checks nothing: tcsverify is what says
whether a configuration is sound, and a ground station should run on one it
has passed.
"""

import os
import re
import shutil
import subprocess
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass, field
from pathlib import Path

from . import interface as _interface
from . import sbn as _sbn

#: The sections whose entries are nodes, in which a node's device id is
#: counted: a node's id is its position among every node the file writes, in
#: the order it writes them, whatever section each is in.
NODE_SECTIONS = ("gateway", "vehicle", "payloads")

#: Each gateway node type, and the attribute it says where its data goes
#: with: a radio's ``ground_base``, an F´ gateway's ``bridge_base``. The Python
#: spelling of ``tcslibgs::gateway::GatewayKind::base_attribute``, for the
#: kinds that have a node type.
GATEWAY_BASES = {
    "gateway_radio": "ground_base",
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


@dataclass(frozen=True)
class HostPort:
    """A host and a port, as ``address:port``."""

    address: str
    port: int

    def __str__(self):
        return f"{self.address}:{self.port}"

    @staticmethod
    def of(attribute, stated):
        """``stated`` as a host and a port.

        Split at the last colon, so that whatever a host is called the port is
        the part after it.

        Args:
            attribute: The attribute it was stated as, for the error message.
            stated:    What the configuration says, as ``address:port``.

        Returns:
            The :class:`HostPort`, its address with surrounding space removed.

        Raises:
            ValueError: ``stated`` has no colon, no address, a port that is
                        not a number, or a port outside 1 to 65535.
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
        if not 0 < number < 65536:
            raise ValueError(f'{attribute} "{said}": a port is 1 to 65535')
        return HostPort(address.strip(), number)


@dataclass(frozen=True)
class BeaconLink:
    """Where beacons come from and where they are received."""

    send: HostPort
    recv: HostPort


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
    #: The section it is written in: ``payloads``, ``gateway`` or
    #: ``vehicle``.
    section: str = ""
    #: Its attributes, by name: every child but its name, its type and its
    #: links.
    attributes: dict = field(default_factory=dict)
    #: The nodes each of its terminals is linked to, by terminal:
    #: ``{"payload-out": ["gateway"]}``.
    links: dict = field(default_factory=dict)


@dataclass
class Configuration:
    """A configuration, as far as a ground station reads one."""

    path: Path
    #: The ``tcspecial`` section, flat: ``beacon.recv`` and the rest.
    tcspecial: dict
    #: Every node, in the order the file writes them.
    nodes: list

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
            Its position among the configuration's nodes, counting from
            nought, in the order the file writes them.

        Raises:
            ValueError: There is no node called ``name``.
        """
        for at, node in enumerate(self.nodes):
            if node.name == name:
                return at
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
                        on a bus, ``source`` has no ``payload-out`` link to
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
        touched = set()
        for node in self.nodes:
            for targets in node.links.values():
                if targets:
                    touched.add(node.name)
                    touched.update(targets)
            for target in node.links.get("payload-out", []):
                kind = self._kind(target)
                if kind in ON_BUS:
                    msgids.add(self.downlink_msgid(target, node.name))
        for at, node in enumerate(self.nodes):
            if node.section == "payloads" and node.name not in touched:
                msgids.add(counted("payload_telemetry.tlm_base",
                                   bus.tlm_base, at))
        return sorted(msgids)

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
            send=HostPort.of("tcs_cmd.send", self._said("tcs_cmd.send")),
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
        the order written."""
        return [node for node in self.nodes if node.node_type in GATEWAY_BASES]

    def uplink(self, gateway):
        """Where the ground sends to the ``gateway`` node called ``gateway``.

        Args:
            gateway: The gateway node's name.

        Returns:
            The :class:`HostPort` it binds aboard, which is what leaves it by
            its ``gateway-out`` links.

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
                        ``source`` has no ``payload-out`` link to it, the base
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
            ValueError: ``source`` is missing, or has no ``payload-out`` link
                        to ``gateway``.
        """
        if gateway not in self.node(source).links.get("payload-out", []):
            raise ValueError(
                f'{self.path}: "{source}" sends nothing to "{gateway}": it has '
                "no payload-out link to it"
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
    """Read the configuration at ``path``, YAML or XML by its extension.

    A file that names others in an ``include`` is refused rather than read in
    part: its nodes' device ids depend on the files it includes, and a station
    that counted only the top file's would receive on the wrong ports and say
    nothing about why.

    Args:
        path: The configuration file, named ``.ytcs``, for YAML, or
              ``.xtcs``, for XML.

    Returns:
        The :class:`Configuration` it says.

    Raises:
        ValueError: The extension is neither of those -- ``.yaml``, ``.yml``
                    and ``.xml``, as configurations were once named,
                    included, with the name to give the file -- the file
                    includes
                    others, it is not a map of sections, or it is YAML and
                    PyYAML is not installed.
        OSError: The file cannot be read.
    """
    path = Path(path)
    suffix = path.suffix.lower()
    if suffix == ".xtcs":
        return _interface_checked(_load_xml(path))
    if suffix == ".ytcs":
        return _interface_checked(_load_yaml(path))
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


def _includes(path):
    """The error for a configuration that includes other files.

    Args:
        path: The configuration file.

    Returns:
        The ``ValueError`` to raise.
    """
    return ValueError(
        f"{path} includes other files, which this does not read: their nodes' "
        "device ids would be counted wrong"
    )


def _load_xml(path):
    """Read a configuration written in XML.

    Args:
        path: The configuration file.

    Returns:
        The :class:`Configuration` it says.

    Raises:
        ValueError: The file includes others.
        xml.etree.ElementTree.ParseError: The file is not well-formed XML.
    """
    root = ElementTree.parse(path).getroot()
    if root.find("include") is not None:
        raise _includes(path)

    tcspecial = {}
    section = root.find("tcspecial")
    if section is not None:
        for part in section:
            if len(part):
                for leaf in part:
                    tcspecial[f"{part.tag}.{leaf.tag}"] = (
                        leaf.text or ""
                    ).strip()
            else:
                tcspecial[part.tag] = (part.text or "").strip()

    nodes = []
    for element in root:
        if element.tag not in NODE_SECTIONS:
            continue
        node = Node(
            name=(element.findtext("name") or "").strip(),
            node_type=(element.findtext("type") or "").strip(),
            section=element.tag,
        )
        for child in element:
            if child.tag in ("name", "type"):
                continue
            targets = child.findall("target")
            if targets:
                node.links.setdefault(child.tag, []).extend(
                    (target.text or "").strip() for target in targets
                )
            else:
                node.attributes[child.tag] = (child.text or "").strip()
        nodes.append(node)

    return Configuration(path=path, tcspecial=tcspecial, nodes=nodes)


def _scalar(value):
    """A YAML scalar as the XML spelling writes it: the text of an element.
    YAML's own true and false are the words, as the XML has them."""
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value).strip()


def _load_yaml(path):
    """Read a configuration written in YAML, with PyYAML.

    Args:
        path: The configuration file.

    Returns:
        The :class:`Configuration` it says.

    Raises:
        ValueError: PyYAML is not installed, the file is not a map of
                    sections, or it includes others.
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
        document = yaml.safe_load(text) or {}
    if not isinstance(document, dict):
        raise ValueError(
            f"{path} is not a configuration: it is not a map of sections"
        )
    if "include" in document:
        raise _includes(path)

    tcspecial = {}
    for part, value in (document.get("tcspecial") or {}).items():
        if isinstance(value, dict):
            for leaf, scalar in value.items():
                tcspecial[f"{part}.{leaf}"] = _scalar(scalar)
        else:
            tcspecial[part] = _scalar(value)

    # The node sections in the order the file writes them, and each one's
    # nodes in order: a node's device id is its place among them all.
    nodes = []
    for section, entries in document.items():
        if section not in NODE_SECTIONS:
            continue
        for entry in entries or []:
            node = Node(name=_scalar(entry.get("name", "")),
                        node_type=_scalar(entry.get("type", "")),
                        section=section)
            for key, value in entry.items():
                if key in ("name", "type"):
                    continue
                if isinstance(value, list):
                    node.links.setdefault(key, []).extend(
                        _scalar(link.get("target", "")) for link in value
                    )
                else:
                    node.attributes[key] = _scalar(value)
            nodes.append(node)

    return Configuration(path=path, tcspecial=tcspecial, nodes=nodes)


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

    The port must match. The address must too, unless the configuration names
    every interface, in which case the address a datagram carries is whichever
    interface the route chose and there is nothing written down to compare it
    with.

    Args:
        stated: The :class:`HostPort` the configuration names.
        sender: The ``(address, port)`` pair the datagram came from.

    Returns:
        ``True`` if it came from there.
    """
    address, port = sender[0], sender[1]
    if port != stated.port:
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
