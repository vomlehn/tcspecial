"""A Yamcs ground station's view of a configuration that answers to Yamcs:
its mission database and its links, generated from the configuration.

A configuration whose gateway is ``yamcs`` puts a CCSDS primary header on
every packet to and from the ground, and counts an APID for each kind of
packet: see :meth:`tcslibgs.configuration.Configuration.apids`, the Python
spelling of ``graph::telemetry::apids``. Yamcs tells packets apart by those
APIDs, so its mission database must have a container for every one the
vehicle sends down and a command for every one the ground sends up, and its
instance a link on every port they arrive at. All of that is worked out
here from the configuration, so that nothing the configuration counts is
stated a second time by hand:

* :func:`mdb` -- the mission database's wrapper around tcspecial's own XTCE:
  ``yamcs.xtce`` beside this module, which says what does not depend on the
  configuration (the header, the beacon's and the answers' layouts by
  reference, the CONNECT command), with every APID added: the beacon's and
  the answers' restrictions, CONNECT's APID, a container for each source
  drawn into a ``gateway_yamcs`` node and for each payload nothing touches,
  and a command for each ``gateway_yamcs`` node;
* :func:`data_links` and :func:`command_streams` -- the Yamcs instance's
  links and the streams that route commands to them.

A node's data is laid out by no XTCE tcspecial has, so each source's
container holds what follows the header whole, as a binary parameter named
after the node, and each gateway node's command takes its data whole, as a
binary argument. Only the standard library is used.
"""

import re
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

from .configuration import HostPort

#: The XTCE namespace Yamcs 5 reads: XTCE 1.2.
NAMESPACE = "http://www.omg.org/spec/XTCE/20180204"

#: What of the mission database does not depend on the configuration.
TEMPLATE = Path(__file__).resolve().parent / "yamcs.xtce"

#: The template's space system, which every name below is qualified by.
SPACE_SYSTEM = "TcsYamcs"

#: A name XTCE allows: its ``NameType``, no space, period, slash, colon or
#: square bracket.
XTCE_NAME = re.compile(r"[^./:\[\] ]+")

#: The template's containers the configuration's APIDs restrict, by what
#: the APID carries.
RESTRICTED = {"beacon": "BeaconPacket", "commands": "AnswerPacket"}

#: The template's command the command links' APID is assigned to.
CONNECT = "Connect"

#: The binary parameter type a node's data is read as: what follows the
#: header, whole.
BODY_TYPE = "PacketBodyType"

#: The binary argument type a gateway node's command carries its data as,
#: and the argument that says its length in bits.
DATA_TYPE = "DataArg"
DATA = "Data"
DATA_BITS = "DataBits"
DATA_BITS_TYPE = "U32Arg"

#: The base command every command is built on.
BASE_COMMAND = "CcsdsTc"

#: The command stream and link of the command links, which CONNECT goes on.
COMMAND_STREAM = "tc_commands"

#: The stream every packet from the vehicle arrives on.
TM_STREAM = "tm_realtime"

#: How Yamcs reads a packet that carries a CCSDS primary header and no time
#: stamp: stamped with the time it arrived, its sequence count unchecked.
PREPROCESSOR = {
    "packetPreprocessorClassName":
        "org.yamcs.tctm.GenericPacketPreprocessor",
    "packetPreprocessorArgs": {"timestampOffset": -1, "seqCountOffset": -1},
}

#: How Yamcs finishes a command: it fills in the sequence count and the
#: length.
POSTPROCESSOR = {
    "commandPostprocessorClassName": "org.yamcs.tctm.IssCommandPostprocessor",
}


@dataclass(frozen=True)
class Telemetry:
    """A link a source's data reaches the ground on, framed."""

    #: What the mission database calls its parameter: the source's name, or
    #: ``<source>_to_<gateway>`` for a source framed under more than one
    #: APID.
    name: str
    #: The node whose data it carries.
    source: str
    #: The APID its packets carry.
    apid: int
    #: Where it arrives.
    ground: HostPort
    #: ``downlink`` or ``payload_telemetry``, as
    #: :class:`tcslibgs.configuration.ApidUse` says.
    carries: str
    #: The gateway node a downlink goes through, or ``None``.
    gateway: str = None

    @property
    def container(self):
        """The name of its container in the mission database."""
        return f"{self.name}Packet"


@dataclass(frozen=True)
class Uplink:
    """What the ground sends a ``gateway_yamcs`` node."""

    #: The gateway node's name, which its command is called.
    name: str
    #: The APID its commands carry.
    apid: int
    #: Where the node binds aboard, or ``None`` for one that binds nothing,
    #: which the ground has nowhere to send to: its command is in the
    #: mission database, as its APID is counted, but no link or stream
    #: carries it.
    vehicle: HostPort

    @property
    def stream(self):
        """The command stream that routes its command to its link."""
        return f"tc_{self.name}"


def qualified(name):
    """A name in the mission database, as Yamcs knows it.

    Args:
        name: The name in the wrapper's space system.

    Returns:
        ``/TcsYamcs/<name>``.
    """
    return f"/{SPACE_SYSTEM}/{name}"


def local(address):
    """The address to reach a socket at from the ground station's host.

    Args:
        address: The address the socket is bound at.

    Returns:
        The address itself, or this host's for one bound on every interface.
    """
    return "127.0.0.1" if address in ("0.0.0.0", "::", "") else address


def telemetry(config):
    """Every framed link a source's data reaches the ground on.

    Args:
        config: The :class:`tcslibgs.configuration.Configuration`.

    Returns:
        A :class:`Telemetry` for each source drawn into a ``gateway_yamcs``
        node and each payload nothing touches, in the order of
        :meth:`~tcslibgs.configuration.Configuration.apids`.

    Raises:
        ValueError: The configuration does not answer to Yamcs, or an APID
                    or a port does not read, fit, or is used twice.
    """
    uses = [use for use in config.apids()
            if use.carries in ("downlink", "payload_telemetry")]
    framed = {}
    for use in uses:
        framed[use.node] = framed.get(use.node, 0) + 1
    links = []
    for use in uses:
        if use.carries == "downlink":
            ground = config.downlink(use.gateway, use.node)
            name = (use.node if framed[use.node] == 1
                    else f"{use.node}_to_{use.gateway}")
        else:
            ground = config.payload_telemetry_link(use.node)[1]
            name = use.node
        links.append(Telemetry(name, use.node, use.apid, ground,
                               use.carries, use.gateway))
    return links


def uplinks(config):
    """What the ground sends each ``gateway_yamcs`` node.

    Args:
        config: The :class:`tcslibgs.configuration.Configuration`.

    Returns:
        An :class:`Uplink` for each, by name.

    Raises:
        ValueError: As :func:`telemetry`, or a node's address or port does
                    not read, or it states one and not the other.
    """
    links = []
    for use in config.apids():
        if use.carries != "uplink":
            continue
        said = config.node(use.node).attributes
        binds = "address" in said or "port" in said
        links.append(Uplink(use.node, use.apid,
                            config.uplink(use.node) if binds else None))
    return links


def _tag(name):
    """An XTCE element's name, in its namespace.

    Args:
        name: The element's local name.

    Returns:
        The name as ElementTree holds it.
    """
    return f"{{{NAMESPACE}}}{name}"


def _one(root, path, what):
    """The one element at ``path`` in the template.

    Args:
        root: The template's root.
        path: An ElementTree path, its names local to the XTCE namespace.
        what: What it is, for the complaint.

    Returns:
        The element.

    Raises:
        ValueError: There is not exactly one.
    """
    found = root.findall(path, {"x": NAMESPACE})
    if len(found) != 1:
        raise ValueError(f"the Yamcs template has {len(found)} {what}, and "
                         "must have one")
    return found[0]


def _child(parent, kind, **attributes):
    """Adds an XTCE element.

    Args:
        parent:     The element it goes in.
        kind:       Its local name.
        attributes: Its attributes.

    Returns:
        The element.
    """
    return ET.SubElement(parent, _tag(kind), attributes)


def _restrict(base, apid):
    """Restricts a container to packets of one APID.

    Args:
        base: Its ``BaseContainer``.
        apid: The APID.
    """
    criteria = _child(base, "RestrictionCriteria")
    _child(criteria, "Comparison", parameterRef="CcsdsApid", value=str(apid))


def _assign(base, apid):
    """Assigns a command its APID.

    Args:
        base: Its ``BaseMetaCommand``.
        apid: The APID.
    """
    assignments = _child(base, "ArgumentAssignmentList")
    _child(assignments, "ArgumentAssignment", argumentName="CcsdsApid",
           argumentValue=str(apid))


class _Names:
    """The names the mission database holds of each kind, which a new one
    must not repeat.

    Args:
        root: The template's root.
    """

    #: The element kinds whose names share one space, by the space.
    KINDS = {"parameter": ("Parameter",),
             "container": ("SequenceContainer", "CommandContainer"),
             "command": ("MetaCommand",)}

    def __init__(self, root):
        self._taken = {}
        for space, kinds in self.KINDS.items():
            taken = set()
            for kind in kinds:
                for element in root.iter(_tag(kind)):
                    taken.add(element.get("name"))
            self._taken[space] = taken

    def take(self, space, name, node):
        """Takes a name for what the configuration adds.

        Args:
            space: ``parameter``, ``container`` or ``command``.
            name:  The name.
            node:  The node it is named after, for the complaint.

        Raises:
            ValueError: XTCE does not allow it, or it is taken.
        """
        if not XTCE_NAME.fullmatch(name):
            raise ValueError(f'"{node}" gives the Yamcs {space} name '
                             f'"{name}", which XTCE does not allow: a name '
                             "has no space, period, slash, colon or square "
                             "bracket")
        if name in self._taken[space]:
            raise ValueError(f'"{node}" gives the Yamcs {space} name '
                             f'"{name}", which the mission database already '
                             "has: rename the node")
        self._taken[space].add(name)


def mdb(config, template=TEMPLATE):
    """The mission database's wrapper for a configuration: the template,
    with every APID the configuration counts added.

    Args:
        config:   The :class:`tcslibgs.configuration.Configuration`.
        template: The XTCE that does not depend on the configuration.

    Returns:
        The :class:`xml.etree.ElementTree.ElementTree` of the wrapper.

    Raises:
        ValueError: The configuration does not answer to Yamcs; an APID is
                    missing, not one, past eleven bits or used twice; a node
                    gives a name XTCE does not allow or the wrapper already
                    has; or the template lacks what is added to.
        OSError: The template cannot be read.
    """
    ET.register_namespace("", NAMESPACE)
    # The template's comments say why it is as it is, and are kept.
    parser = ET.XMLParser(target=ET.TreeBuilder(insert_comments=True))
    tree = ET.parse(template, parser)
    root = tree.getroot()
    if root.tag != _tag("SpaceSystem") or root.get("name") != SPACE_SYSTEM:
        raise ValueError(f"{template}: not the XTCE 1.2 SpaceSystem "
                         f"{SPACE_SYSTEM}")
    names = _Names(root)
    uses = config.apids()

    # The beacon, the answers and CONNECT, laid out by the template.
    for use in uses:
        if use.carries in RESTRICTED:
            container = RESTRICTED[use.carries]
            base = _one(root, f".//x:SequenceContainer[@name='{container}']"
                              "/x:BaseContainer", f"{container} bases")
            _restrict(base, use.apid)
        if use.carries == "commands":
            base = _one(root, f".//x:MetaCommand[@name='{CONNECT}']"
                              "/x:BaseMetaCommand", f"{CONNECT} bases")
            _assign(base, use.apid)

    # Each source's data, whole, under its APID.
    parameters = _one(root, "./x:TelemetryMetaData/x:ParameterSet",
                      "parameter sets")
    containers = _one(root, "./x:TelemetryMetaData/x:ContainerSet",
                      "container sets")
    for link in telemetry(config):
        names.take("parameter", link.name, link.source)
        names.take("container", link.container, link.source)
        _child(parameters, "Parameter", name=link.name,
               parameterTypeRef=BODY_TYPE)
        container = _child(containers, "SequenceContainer",
                           name=link.container)
        entries = _child(container, "EntryList")
        _child(entries, "ParameterRefEntry", parameterRef=link.name)
        _restrict(_child(container, "BaseContainer",
                         containerRef="CCSDSPacket"), link.apid)

    # A command for each gateway node, carrying its data whole.
    commands = _one(root, "./x:CommandMetaData/x:MetaCommandSet",
                    "command sets")
    for uplink in uplinks(config):
        names.take("command", uplink.name, uplink.name)
        names.take("container", f"{uplink.name}Packet", uplink.name)
        command = _child(commands, "MetaCommand", name=uplink.name)
        _assign(_child(command, "BaseMetaCommand",
                       metaCommandRef=BASE_COMMAND), uplink.apid)
        arguments = _child(command, "ArgumentList")
        _child(arguments, "Argument", name=DATA, argumentTypeRef=DATA_TYPE)
        _child(arguments, "Argument", name=DATA_BITS,
               argumentTypeRef=DATA_BITS_TYPE)
        container = _child(command, "CommandContainer",
                           name=f"{uplink.name}Packet")
        entries = _child(container, "EntryList")
        _child(entries, "ArgumentRefEntry", argumentRef=DATA)
        _child(container, "BaseContainer",
               containerRef=f"{BASE_COMMAND}Container")
    ET.indent(tree, space="  ")
    return tree


def write_mdb(config, path, template=TEMPLATE):
    """Writes the mission database's wrapper for a configuration.

    Args:
        config:   The :class:`tcslibgs.configuration.Configuration`.
        path:     Where to write it.
        template: The XTCE that does not depend on the configuration.

    Raises:
        ValueError: As :func:`mdb`.
        OSError: The template cannot be read, or the wrapper written.
    """
    tree = mdb(config, template)
    said = ("<!-- Written by tcslibgs/yamcs.py from\n"
            f"     {Path(template).resolve()}\n"
            f"     and {Path(config.path).resolve()}:\n"
            "     edit those, not this. -->\n")
    Path(path).write_text('<?xml version="1.0" encoding="UTF-8"?>\n' + said
                          + ET.tostring(tree.getroot(), encoding="unicode")
                          + "\n", encoding="UTF-8")


def data_links(config):
    """The Yamcs instance's links to the vehicle: the command links, the
    beacon, an uplink to each ``gateway_yamcs`` node that binds aboard and a
    link on each port a source's data arrives at.

    Args:
        config: The :class:`tcslibgs.configuration.Configuration`.

    Returns:
        Each link as Yamcs's ``dataLinks`` lists it, a dictionary.

    Raises:
        ValueError: As :func:`telemetry` and :func:`uplinks`, or the
                    command link or the beacon does not read.
    """
    command = config.command_link().recv
    links = [dict({"name": "commands",
                   "class": "org.yamcs.tctm.UdpTcTmDataLink",
                   "host": local(command.address), "port": command.port,
                   "tmStream": TM_STREAM, "tcStream": COMMAND_STREAM},
                  **PREPROCESSOR, **POSTPROCESSOR),
             dict({"name": "beacon", "class": "org.yamcs.tctm.UdpTmDataLink",
                   "stream": TM_STREAM,
                   "port": config.beacon_link().recv.port}, **PREPROCESSOR)]
    for uplink in uplinks(config):
        if uplink.vehicle is None:
            continue
        links.append(dict({"name": f"{uplink.name}-uplink",
                           "class": "org.yamcs.tctm.UdpTcDataLink",
                           "stream": uplink.stream,
                           "host": local(uplink.vehicle.address),
                           "port": uplink.vehicle.port}, **POSTPROCESSOR))
    for link in telemetry(config):
        links.append(dict({"name": f"{link.name}-downlink",
                           "class": "org.yamcs.tctm.UdpTmDataLink",
                           "stream": TM_STREAM, "port": link.ground.port},
                          **PREPROCESSOR))
    return links


def command_streams(config):
    """The command streams that route each command to its link: CONNECT to
    the command links, each gateway node's command to its uplink, where it
    binds aboard.

    Args:
        config: The :class:`tcslibgs.configuration.Configuration`.

    Returns:
        Each stream as Yamcs's ``streamConfig.tc`` lists it, a dictionary.

    Raises:
        ValueError: As :func:`uplinks`.
    """
    streams = [{"name": COMMAND_STREAM, "processor": "realtime",
                "tcPatterns": [qualified(CONNECT)]}]
    for uplink in uplinks(config):
        if uplink.vehicle is None:
            continue
        streams.append({"name": uplink.stream, "processor": "realtime",
                        "tcPatterns": [qualified(uplink.name)]})
    return streams
