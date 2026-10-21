"""What a ground station reads out of a tcspecial configuration.

The Python spelling of what the Rust ground programs ask ``tcslibgs`` for:
where the beacons are (``graph::parameters::beacon_of``), where a ``moc``
node is bound aboard, which port a source's data reaches the ground at
(``graph::order`` and ``graph::telemetry``), whether a datagram came from where
the configuration says (``types::came_from``), and the configuration's digest.

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

import re
import shutil
import subprocess
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass, field
from pathlib import Path

#: The sections whose entries are nodes, in which a node's device id is
#: counted: a node's id is its position among every node the file writes, in
#: the order it writes them, whatever section each is in.
NODE_SECTIONS = ("moc", "vehicle", "payloads")

#: Each gateway node type, and the attribute it says where its data goes
#: with: a radio's ``ground_base``, an F´ gateway's ``bridge_base``. The Python
#: spelling of ``tcslibgs::gateway::GatewayKind::base_attribute``, for the
#: kinds that have a node type.
GATEWAY_BASES = {"moc": "ground_base", "gateway_fprime": "bridge_base"}


@dataclass(frozen=True)
class HostPort:
    """A host and a port, as ``address:port``."""

    address: str
    port: int

    def __str__(self):
        return f"{self.address}:{self.port}"

    @staticmethod
    def of(attribute, stated):
        """``stated`` as a host and a port, or a ``ValueError`` naming
        ``attribute``. Split at the last colon, so that whatever a host is
        called the port is the part after it."""
        said = stated.strip()
        if ":" not in said:
            raise ValueError(
                f'{attribute} "{said}" is not an address and a port: it is '
                "written as address:port, as 0.0.0.0:4000"
            )
        address, port = said.rsplit(":", 1)
        if not address.strip() or not port.strip().isdigit():
            raise ValueError(f'{attribute} "{said}" is not an address and a port')
        number = int(port)
        if not 0 < number < 65536:
            raise ValueError(f'{attribute} "{said}": a port is 1 to 65535')
        return HostPort(address.strip(), number)


@dataclass(frozen=True)
class BeaconLink:
    """Where beacons come from and where they are received."""

    send: HostPort
    recv: HostPort


@dataclass
class Node:
    """One node, as the file writes it."""

    name: str
    node_type: str
    #: Its attributes, by name: every child but its name, its type and its
    #: links.
    attributes: dict = field(default_factory=dict)
    #: The nodes each of its terminals is linked to, by terminal:
    #: ``{"payload-out": ["moc"]}``.
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
        """The node called ``name``, or a ``ValueError`` saying there is none."""
        for node in self.nodes:
            if node.name == name:
                return node
        raise ValueError(f'{self.path}: there is no node called "{name}"')

    def device_id(self, name):
        """A node's device id: its position among the configuration's nodes,
        counting from nought, in the order the file writes them."""
        for at, node in enumerate(self.nodes):
            if node.name == name:
                return at
        raise ValueError(f'{self.path}: there is no node called "{name}"')

    def beacon_link(self):
        """Where tcspecial's beacons go from and to."""
        return BeaconLink(
            send=HostPort.of("beacon.send", self._said("beacon.send")),
            recv=HostPort.of("beacon.recv", self._said("beacon.recv")),
        )

    def beacon_interval(self):
        """How often tcspecial beacons, in nanoseconds: ``beacon.interval``,
        read as :func:`parse_interval` reads it. What the beacon light ages
        against -- see ``tcslib.beacon.Schedule.for_interval``."""
        return parse_interval(self._said("beacon.interval"))

    def moc_nodes(self):
        """The gateway nodes -- the links to what the vehicle answers to -- in
        the order written."""
        return [node for node in self.nodes if node.node_type in GATEWAY_BASES]

    def uplink(self, moc):
        """Where the ground sends to the ``moc`` node called ``moc``: the
        address and port it binds aboard, which is what leaves it by its
        ``moc-out`` links."""
        node = self._moc(moc)
        address = node.attributes.get("address")
        port = node.attributes.get("port")
        if address is None or port is None:
            raise ValueError(
                f'{self.path}: "{moc}" states no address and port, so it binds '
                "nothing aboard and the ground has nowhere to send to it"
            )
        return HostPort.of(f'"{moc}" address:port', f"{address}:{port}")

    def downlink(self, moc, source):
        """Where the ground receives what ``source`` sends to the gateway node
        called ``moc``: that node's ``ground_base`` -- or ``bridge_base``, for an
        F´ gateway -- its port counted on by the source's device id, so that
        each source arrives on a port of its own."""
        node = self._moc(moc)
        attribute = GATEWAY_BASES[node.node_type]
        if moc not in self.node(source).links.get("payload-out", []):
            raise ValueError(
                f'{self.path}: "{source}" sends nothing to "{moc}": it has no '
                "payload-out link to it"
            )
        base = HostPort.of(
            f'"{moc}" {attribute}', node.attributes.get(attribute, "")
        )
        port = base.port + self.device_id(source)
        if port > 65535:
            raise ValueError(
                f'{self.path}: "{moc}" {attribute} {base} plus {source}\'s device '
                f"id is port {port}, and a port is at most 65535"
            )
        return HostPort(base.address, port)

    def _moc(self, name):
        node = self.node(name)
        if node.node_type not in GATEWAY_BASES:
            raise ValueError(
                f'{self.path}: "{name}" is a {node.node_type} node, not a gateway node'
            )
        return node

    def _said(self, attribute):
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
    """A length of time as a configuration writes one -- a number, a space and
    a unit, as ``500 ms`` or ``1.5 s`` -- in nanoseconds, or a ``ValueError``.

    The Python spelling of ``tcslibgs::graph::parse_interval``: the same units,
    underscores among the digits meaning nothing, a decimal fraction kept to
    the nearest whole microsecond, and nought refused. Its messages are
    shorter; what it takes and refuses is the same.
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
            f'"{text}": "{unit}" is not a unit of time: us, ms, s, min, h, d or w'
        )
    bare = digits.replace("_", "")
    # Digits, at most one point, digits: no sign, no exponent.
    if not re.fullmatch(r"[0-9]*\.?[0-9]*", bare, re.ASCII) or bare in ("", "."):
        raise ValueError(f'"{text}" is not a number of {unit}')
    micros = round(float(bare) * INTERVAL_UNITS[unit])
    if micros == 0:
        raise ValueError(f'"{text}" is no interval at all')
    return micros * 1_000


#: The two spellings, by the flag that asks for each and the extension it is
#: written with: ``format::ConfigFormat`` in ``tcslibgs``.
SPELLINGS = {"yaml": ".yaml", "xml": ".xml"}


def choose(named, stem, default, chosen=None):
    """The configuration to read: ``tcslibgs::format::Spelling::configuration``.

    ``named`` where one was named, and then ``chosen`` -- ``"yaml"`` or
    ``"xml"``, from ``--yaml`` or ``--xml`` -- has to agree with its
    extension, a file named ``.xml`` read as asked for in YAML being a command
    line that cannot both be right. Otherwise ``stem``, the example's path
    without an extension, in the spelling chosen, or in ``default`` where
    neither flag was given. The two flags together are refused before this,
    by the parser that reads them.
    """
    if named is not None:
        named = str(named)
        suffix = Path(named).suffix.lower()
        spelled = "yaml" if suffix in (".yaml", ".yml") else "xml" if suffix == ".xml" else None
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
    """
    path = Path(path)
    suffix = path.suffix.lower()
    if suffix == ".xml":
        return _load_xml(path)
    if suffix in (".yaml", ".yml"):
        return _load_yaml(path)
    raise ValueError(
        f"{path}: a configuration file is named .yaml, .yml or .xml, and this is "
        "none of those"
    )


def _includes(path):
    return ValueError(
        f"{path} includes other files, which this does not read: their nodes' "
        "device ids would be counted wrong"
    )


def _load_xml(path):
    root = ElementTree.parse(path).getroot()
    if root.find("include") is not None:
        raise _includes(path)

    tcspecial = {}
    section = root.find("tcspecial")
    if section is not None:
        for part in section:
            if len(part):
                for leaf in part:
                    tcspecial[f"{part.tag}.{leaf.tag}"] = (leaf.text or "").strip()
            else:
                tcspecial[part.tag] = (part.text or "").strip()

    nodes = []
    for element in root:
        if element.tag not in NODE_SECTIONS:
            continue
        node = Node(
            name=(element.findtext("name") or "").strip(),
            node_type=(element.findtext("type") or "").strip(),
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
    try:
        import yaml
    except ImportError as why:
        raise ValueError(
            f"{path} is YAML, which is read with PyYAML, and this Python has none "
            f"({why}): install it (python3-yaml on Debian and Ubuntu, or pip "
            "install pyyaml), or read the XML spelling"
        ) from why

    with open(path, encoding="utf-8") as text:
        document = yaml.safe_load(text) or {}
    if not isinstance(document, dict):
        raise ValueError(f"{path} is not a configuration: it is not a map of sections")
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
                        node_type=_scalar(entry.get("type", "")))
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
    return "localhost" if address in ("localhost", "127.0.0.1", "::1") else address


def binds_every_interface(address):
    """Whether an address is a wildcard, claiming a port on every interface."""
    return address in ("", "0.0.0.0", "::", "*")


def came_from(stated, sender):
    """Whether a datagram from ``sender``, an ``(address, port)`` pair, came
    from the socket the configuration names as ``stated``.

    The port must match. The address must too, unless the configuration names
    every interface, in which case the address a datagram carries is whichever
    interface the route chose and there is nothing written down to compare it
    with.
    """
    address, port = sender[0], sender[1]
    if port != stated.port:
        return False
    if binds_every_interface(stated.address):
        return True
    return one_host(stated.address) == one_host(address)


def find_built(program):
    """Where one of the repository's programs is: its own build first, then
    the path. ``None`` where it is in neither."""
    root = Path(__file__).resolve().parents[2]
    for built in (f"target/debug/{program}", f"target/release/{program}"):
        if (root / built).is_file():
            return str(root / built)
    return shutil.which(program)


def find_tcsverify():
    """Where tcsverify is: see :func:`find_built`."""
    return find_built("tcsverify")


def digest(path, tcsverify=None):
    """The configuration's digest, as tcsverify works it out: thirty-two hex
    digits, or ``None`` with the reason where it cannot be had.

    ``None`` is not a failure. A station that cannot digest its own
    configuration compares nothing, as the Rust ground programs do, and says
    so rather than claiming every beacon agrees.
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
    return None, f"{program} gave no digest: {done.stdout.strip() or done.stderr.strip()}"
