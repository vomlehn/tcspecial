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
* :func:`write_mdb` -- the wrapper as Yamcs reads it, no line of it wider
  than :data:`WIDTH`;
* :func:`data_links` and :func:`command_streams` -- the Yamcs instance's
  links and the streams that route commands to them.

A node whose type has an interface document of its own
(:data:`tcslibgs.interface.NODE_DOCUMENTS`: the ``ping`` node's
``ping.xtce``) is given what that document states. Its source's container
holds the fields of the document's container, and a gateway node whose
data goes only to nodes with that document takes the document's command,
each named as the document names it, its types and parameters copied in
from it. A command cannot be put behind another's header from outside, so
the document is copied rather than referred to, and the copy is held to
what the document says by refusing anything of it the copy would not say
the same (:data:`COPIED`). Every other node's container holds what follows
the header as a parameter named after the node, of the type the
configuration's own XTCE gives it -- the file ``tcsverify --xtce`` writes
beside the configuration -- copied from there as a node's document is: a
value as a number in its unit, read from its four bytes, a decision as a
boolean, read from its one byte, a whole number as an integer as wide as
its type, a string or bytes after the 32-bit count of its bytes it is sent
after, as XTCE encodes it. A structure is binary, whole: all of what
follows the header, whose length the header gives. A source whose document
lays out a string's fields has those fields after the string's count,
:data:`COUNT`. A gateway node's command
takes its data whole, as a binary argument. Only the standard library is
used.
"""

import re
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

from . import interface
from .configuration import HostPort
from .form import COUNTED, DATUM, form_of, own_xtce, type_of

#: The XTCE namespace Yamcs 5 reads: XTCE 1.2.
NAMESPACE = "http://www.omg.org/spec/XTCE/20180204"

#: What of the mission database does not depend on the configuration.
TEMPLATE = Path(__file__).resolve().parent / "yamcs.xtce"

#: The namespaces a node's interface document may be in: XTCE 1.2's, which
#: the wrapper is in, and XTCE 1.3's. What the wrapper copies of one is the
#: same in both.
DOCUMENT_NAMESPACES = (NAMESPACE, "http://www.omg.org/spec/XTCE/20250214")

#: The template's space system, which every name below is qualified by.
SPACE_SYSTEM = "TcsYamcs"

#: The widest line :func:`write_mdb` writes, and the indent of each level,
#: which are the template's.
WIDTH = 80
INDENT = "  "

#: What of a node's interface document is copied into the wrapper, by
#: element: the types of its parameters and arguments, which are given to
#: Yamcs as the document gives them. Anything else in a type -- a reference
#: to another value, a calibrator, an alarm -- is refused, as something the
#: copy would not say the same behind a header.
COPIED = frozenset({
    "StringParameterType", "IntegerParameterType", "BinaryParameterType",
    "FloatParameterType", "BooleanParameterType",
    "StringArgumentType", "IntegerArgumentType", "BinaryArgumentType",
    "StringDataEncoding", "IntegerDataEncoding", "BinaryDataEncoding",
    "SizeInBits", "Fixed", "FixedValue", "Variable", "TerminationChar",
    "LeadingSize", "DynamicValue", "ParameterInstanceRef",
    "LinearAdjustment", "ValidRange", "UnitSet", "Unit", "DefaultCalibrator",
    "PolynomialCalibrator", "Term", "AncillaryDataSet", "AncillaryData"})

#: A name XTCE allows: its ``NameType``, no space, period, slash, colon or
#: square bracket.
XTCE_NAME = re.compile(r"[^./:\[\] ]+")

#: The template's containers the configuration's APIDs restrict, by what
#: the APID carries.
RESTRICTED = {"beacon": "BeaconPacket", "commands": "AnswerPacket"}

#: The template's command the command links' APID is assigned to.
CONNECT = "Connect"

#: The binary parameter type a node's data is read as where it is a
#: structure: what follows the header, whole.
BODY_TYPE = "PacketBodyType"

#: The template's parameter that is the 32-bit count before a string whose
#: fields a node's interface document lays out: a string or bytes is sent
#: after one, where it goes as Yamcs decodes it, by XTCE.
COUNT = "DatumCount"

#: The parameter the configuration's own XTCE counts every ``bytes`` by,
#: as ``types.xtce`` does.
BYTES_LENGTH = "bytes_length"

#: The simulated world's ambient temperature, which nothing on the link
#: carries: a local parameter, which the ground station sets from the
#: simulator file, so that Yamcs has the surroundings beside the vehicle's
#: readings. See :func:`mdb`'s ``world``.
WORLD_AMBIENT = "world_ambient_temp"
WORLD_TEMPERATURE_TYPE = "WorldTemperatureType"

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
    #: The source's interface document, which its data is laid out by, or
    #: ``None`` where it has none.
    document: Path = None
    #: What its data are, where no document lays them out: a
    #: :class:`tcslibgs.form.Form`, read from the configuration's own XTCE.
    form: object = DATUM
    #: The configuration's own XTCE, which gives its parameter's type, or
    #: ``None`` where its document lays it out.
    own: Path = None
    #: Whether it is a string or bytes, which is sent after a 32-bit count
    #: of its bytes: see :data:`COUNT`.
    counted: bool = False

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
    #: The interface document of every node its data goes to, whose command
    #: it takes, or ``None`` where they have none, or not the same one.
    document: Path = None
    #: Its command's name: the document's command, or the node's own name.
    command: str = ""

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
        ValueError: The configuration does not answer to Yamcs, an APID or
                    a port does not read, fit, or is used twice, or its own
                    XTCE is not there or does not read (see
                    :func:`tcslibgs.form.form_of`).
        OSError: Its own XTCE cannot be read.
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
        document = interface.node_document(config.node(use.node).node_type)
        form = DATUM if document else form_of(config, use.node)
        own = None if document else own_xtce(config)
        counted = type_of(config, use.node) in COUNTED
        links.append(Telemetry(name, use.node, use.apid, ground,
                               use.carries, use.gateway, document, form,
                               own, counted))
    return links


def uplinks(config):
    """What the ground sends each ``gateway_yamcs`` node.

    Args:
        config: The :class:`tcslibgs.configuration.Configuration`.

    Returns:
        An :class:`Uplink` for each, by name.

    Raises:
        ValueError: As :func:`telemetry`, or a node's address or port does
                    not read, or it states one and not the other, or the
                    document its data's nodes share does not read.
        OSError: That document cannot be read.
    """
    links = []
    for use in config.apids():
        if use.carries != "uplink":
            continue
        node = config.node(use.node)
        binds = "address" in node.attributes or "port" in node.attributes
        document = _shared_document(config, node)
        command = (Document(document).command()[0] if document
                   else use.node)
        links.append(Uplink(use.node, use.apid,
                            config.uplink(use.node) if binds else None,
                            document, command))
    return links


def _shared_document(config, gateway):
    """The interface document every node a gateway node's data goes to has.

    Args:
        config:  The :class:`tcslibgs.configuration.Configuration`.
        gateway: The gateway node's :class:`~tcslibgs.configuration.Node`.

    Returns:
        The document's path, or ``None`` where the node's data goes nowhere,
        or to a node with no document, or to nodes with different ones:
        data that is not laid out by one document is sent whole.

    Raises:
        ValueError: A node it names does not exist.
    """
    documents = set()
    for target in gateway.links.get("moc-out", []):
        # A destination may name the node's input after a point.
        name = target.split(".", 1)[0]
        documents.add(interface.node_document(config.node(name).node_type))
    if len(documents) != 1:
        return None
    return documents.pop()


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

    #: The element kinds whose names share one space, by the space: each
    #: an element and, where only those in one set count, the set.
    KINDS = {"parameter": (("Parameter", None),),
             "container": (("SequenceContainer", None),
                           ("CommandContainer", None)),
             "command": (("MetaCommand", None),),
             "parameter type": ((None, "ParameterTypeSet"),),
             "argument type": ((None, "ArgumentTypeSet"),)}

    def __init__(self, root):
        self._taken = {}
        for space, kinds in self.KINDS.items():
            taken = set()
            for kind, within in kinds:
                if kind is not None:
                    found = root.iter(_tag(kind))
                else:
                    found = (child for owner in root.iter(_tag(within))
                             for child in owner)
                for element in found:
                    if element.get("name") is not None:
                        taken.add(element.get("name"))
            self._taken[space] = taken

    def take(self, space, name, node, fix="rename the node"):
        """Takes a name for what the configuration adds.

        Args:
            space: ``parameter``, ``container``, ``command``,
                   ``parameter type`` or ``argument type``.
            name:  The name.
            node:  The node, or the document, it is named after, for the
                   complaint.
            fix:   What to do about a name that is taken, for the
                   complaint.

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
                             f"has: {fix}")
        self._taken[space].add(name)


class Document:
    """A node type's interface document, as the wrapper takes from it: its
    one container and its one command, and the types and parameters they
    use.

    Args:
        path: The document.

    Raises:
        ValueError: It is not an XTCE ``SpaceSystem`` of one level, or two
                    of its elements of a kind have one name.
        OSError: It cannot be read.
    """

    #: The sets read, by the element each holds and the set's name.
    SETS = {"parameter type": "TelemetryMetaData/x:ParameterTypeSet",
            "parameter": "TelemetryMetaData/x:ParameterSet",
            "container": "TelemetryMetaData/x:ContainerSet",
            "argument type": "CommandMetaData/x:ArgumentTypeSet",
            "command": "CommandMetaData/x:MetaCommandSet"}

    def __init__(self, path):
        self.path = Path(path)
        root = ET.parse(self.path).getroot()
        namespace = root.tag[1:].split("}", 1)[0] if "}" in root.tag else ""
        if (namespace not in DOCUMENT_NAMESPACES
                or root.tag != f"{{{namespace}}}SpaceSystem"):
            raise ValueError(f"{self.path}: not an XTCE 1.2 or 1.3 "
                             "SpaceSystem, which a node's interface document "
                             "is")
        if root.find(f"{{{namespace}}}SpaceSystem") is not None:
            raise ValueError(f"{self.path}: a SpaceSystem within another, "
                             "which a node's interface document does not "
                             "have")
        self.namespace = namespace
        self.name = root.get("name")
        self.sets = {}
        for what, path_in in self.SETS.items():
            named = {}
            for owner in root.findall(f"./x:{path_in}",
                                      {"x": namespace}):
                for element in owner:
                    if not isinstance(element.tag, str):
                        continue
                    name = element.get("name")
                    if name in named:
                        raise ValueError(f'{self.path}: two of its {what}s '
                                         f'are called "{name}"')
                    named[name] = element
            self.sets[what] = named

    def local(self, element):
        """An element's name, without the document's namespace.

        Args:
            element: The element.

        Returns:
            Its local name.

        Raises:
            ValueError: It is in another namespace.
        """
        prefix = f"{{{self.namespace}}}"
        if not isinstance(element.tag, str) or not (
                element.tag.startswith(prefix)):
            raise ValueError(f"{self.path}: {element.tag} is not of the "
                             "document's own XTCE")
        return element.tag[len(prefix):]

    def find(self, what, name, by):
        """One of the document's elements, by a reference to it.

        Args:
            what: Its set: ``parameter type``, ``parameter``, ``container``
                  or ``argument type``.
            name: The reference, which must be to the document's own.
            by:   What refers to it, for the complaint.

        Returns:
            The element.

        Raises:
            ValueError: It refers outside the document, or to nothing.
        """
        if name is None or name not in self.sets[what]:
            raise ValueError(f"{self.path}: {by} refers to the {what} "
                             f'"{name}", which the document does not have: '
                             "a node's interface document refers only to "
                             "its own")
        return self.sets[what][name]

    def _the_one(self, what):
        """The document's one concrete element of a set.

        Args:
            what: The set: ``container`` or ``command``.

        Returns:
            Its name and the element.

        Raises:
            ValueError: It has none, or more than one.
        """
        concrete = [(name, element)
                    for name, element in self.sets[what].items()
                    if element.get("abstract") != "true"]
        if len(concrete) != 1:
            raise ValueError(f"{self.path}: {len(concrete)} {what}s that are "
                             "not abstract, and a node's interface document "
                             "has one, for what the node gives back and for "
                             "what it takes")
        return concrete[0]

    def container(self):
        """The container of what a node with this document gives back.

        Returns:
            Its name, and the parameters it holds in order, its base
            containers' first.

        Raises:
            ValueError: The document has no one such container, or it is
                        laid out by what the wrapper does not copy: an
                        entry other than a parameter's, one placed where it
                        is not next, or a restriction other than a value
                        its own parameters are always equal to.
        """
        name, element = self._the_one("container")
        chain = []
        seen = set()
        while element is not None:
            where = f'container "{element.get("name")}"'
            if element.get("name") in seen:
                raise ValueError(f"{self.path}: {where} is its own base")
            seen.add(element.get("name"))
            chain.append(element)
            base = None
            for part in element:
                kind = self.local(part)
                if kind == "BaseContainer":
                    base = part
                elif kind != "EntryList":
                    raise ValueError(f"{self.path}: {where} has {kind}, "
                                     "which the Yamcs wrapper does not copy")
            if base is None:
                break
            self._constants(base, where)
            element = self.find("container", base.get("containerRef"), where)
        parameters = []
        for element in reversed(chain):
            where = f'container "{element.get("name")}"'
            for entries in element:
                if self.local(entries) != "EntryList":
                    continue
                for entry in entries:
                    kind = self.local(entry)
                    if kind != "ParameterRefEntry" or len(entry):
                        raise ValueError(f"{self.path}: {where} has a "
                                         f"{kind} the Yamcs wrapper does not "
                                         "copy: it copies parameters, one "
                                         "after another")
                    reference = entry.get("parameterRef")
                    self.find("parameter", reference, where)
                    parameters.append(reference)
        return name, parameters

    def _constants(self, base, where):
        """Refuses a restriction the wrapper does not copy. The wrapper
        leaves out the values a document's container holds its own
        parameters to, which say nothing of where anything is: Yamcs shows
        what arrives, and the ground station checks it.

        Args:
            base:  A ``BaseContainer``.
            where: The container it is in, for the complaint.

        Raises:
            ValueError: It is restricted by anything but parameters being
                        equal to values.
        """
        for criteria in base:
            if self.local(criteria) != "RestrictionCriteria":
                raise ValueError(f"{self.path}: {where}'s base has "
                                 f"{self.local(criteria)}, which the Yamcs "
                                 "wrapper does not copy")
            comparisons = []
            for kind in criteria:
                if self.local(kind) == "Comparison":
                    comparisons.append(kind)
                elif self.local(kind) == "ComparisonList":
                    comparisons.extend(kind)
                else:
                    raise ValueError(f"{self.path}: {where} is restricted "
                                     f"by {self.local(kind)}, which the "
                                     "Yamcs wrapper does not copy")
            for comparison in comparisons:
                if (self.local(comparison) != "Comparison"
                        or comparison.get("comparisonOperator",
                                          "==") != "=="):
                    raise ValueError(f"{self.path}: {where} is restricted "
                                     "by other than a parameter being equal "
                                     "to a value, which the Yamcs wrapper "
                                     "does not copy")
                self.find("parameter", comparison.get("parameterRef"),
                          where)

    def command(self):
        """The command a node with this document takes.

        Returns:
            Its name, its arguments' elements, and the entries of its
            container: its name and the entries, in order.

        Raises:
            ValueError: The document has no one such command, or it is
                        laid out by what the wrapper does not copy: a base
                        command, an argument with more than its name, type
                        and initial value, or an entry other than an
                        argument's or a fixed value, one after another.
        """
        name, element = self._the_one("command")
        where = f'command "{name}"'
        arguments, container = [], None
        for part in element:
            kind = self.local(part)
            if kind == "ArgumentList":
                arguments = list(part)
            elif kind == "CommandContainer":
                container = part
            else:
                raise ValueError(f"{self.path}: {where} has {kind}, which "
                                 "the Yamcs wrapper does not copy")
        for argument in arguments:
            if (self.local(argument) != "Argument" or len(argument)
                    or set(argument.attrib) - {"name", "argumentTypeRef",
                                               "initialValue"}):
                raise ValueError(f"{self.path}: {where} has an argument "
                                 "the Yamcs wrapper does not copy: it "
                                 "copies a name, a type and an initial "
                                 "value")
            self.find("argument type", argument.get("argumentTypeRef"),
                      where)
        if container is None:
            raise ValueError(f"{self.path}: {where} has no container")
        entries = []
        for part in container:
            if self.local(part) != "EntryList":
                raise ValueError(f"{self.path}: {where}'s container has "
                                 f"{self.local(part)}, which the Yamcs "
                                 "wrapper does not copy")
            entries.extend(part)
        names = {argument.get("name") for argument in arguments}
        for entry in entries:
            kind = self.local(entry)
            if (kind not in ("ArgumentRefEntry", "FixedValueEntry")
                    or len(entry)):
                raise ValueError(f"{self.path}: {where}'s container has a "
                                 f"{kind} the Yamcs wrapper does not copy: "
                                 "it copies arguments and fixed values, one "
                                 "after another")
            if (kind == "ArgumentRefEntry"
                    and entry.get("argumentRef") not in names):
                raise ValueError(f"{self.path}: {where}'s container refers "
                                 "to an argument it does not have, "
                                 f'"{entry.get("argumentRef")}"')
        return name, arguments, container.get("name"), entries

    def copy(self, element):
        """An element of the document, and everything in it, in the
        wrapper's namespace.

        Args:
            element: The element: a parameter's or an argument's type, or
                     an entry or an argument of the document's command.

        Returns:
            The copy.

        Raises:
            ValueError: It or what is in it is not what the wrapper copies:
                        see :data:`COPIED`.
        """
        kind = self.local(element)
        copied = ET.Element(_tag(kind), dict(element.attrib))
        copied.text = (element.text or "").strip() or None
        for part in element:
            if self.local(part) not in COPIED:
                raise ValueError(f"{self.path}: {kind} "
                                 f'"{element.get("name", "")}" has '
                                 f"{self.local(part)}, which the Yamcs "
                                 "wrapper does not copy")
            copied.append(self.copy(part))
        return copied


class _Copier:
    """What of each node's interface document is in the wrapper, so that
    each type and parameter is copied once however many nodes use it.

    Args:
        root:  The wrapper's root.
        names: The :class:`_Names` it holds.
    """

    def __init__(self, root, names):
        self._names = names
        self._sets = {
            "parameter type": _one(root, "./x:TelemetryMetaData"
                                         "/x:ParameterTypeSet",
                                   "parameter type sets"),
            "parameter": _one(root, "./x:TelemetryMetaData/x:ParameterSet",
                              "parameter sets"),
            "argument type": _one(root, "./x:CommandMetaData"
                                        "/x:ArgumentTypeSet",
                                  "argument type sets")}
        self._copied = set()

    def parameter(self, document, name):
        """Copies a parameter of a document, and its type, unless they are
        copied already.

        Args:
            document: The :class:`Document`.
            name:     The parameter's name.

        Raises:
            ValueError: As :meth:`Document.copy`, or its name or its type's
                        is one the wrapper has of its own.
        """
        if (document.path, "parameter", name) in self._copied:
            return
        parameter = document.find("parameter", name, document.path.name)
        if len(parameter) or set(parameter.attrib) != {"name",
                                                       "parameterTypeRef"}:
            raise ValueError(f'{document.path}: parameter "{name}" has more '
                             "than a name and a type, which the Yamcs "
                             "wrapper does not copy")
        kind = parameter.get("parameterTypeRef")
        self.type(document, "parameter type", kind)
        self._names.take("parameter", name, document.path.name,
                         "rename it in the document")
        _child(self._sets["parameter"], "Parameter", name=name,
               parameterTypeRef=kind)
        self._copied.add((document.path, "parameter", name))

    def argument_type(self, document, name):
        """Copies a type of a document's arguments, unless it is copied
        already.

        Args:
            document: The :class:`Document`.
            name:     The type's name.

        Raises:
            ValueError: As :meth:`Document.copy`, or its name is one the
                        wrapper has of its own.
        """
        self.type(document, "argument type", name)

    def type(self, document, what, name):
        """Copies a type of a document, unless it is copied already.

        Args:
            document: The :class:`Document`.
            what:     ``parameter type`` or ``argument type``.
            name:     The type's name.

        Raises:
            ValueError: As :meth:`Document.copy`, or its name is one the
                        wrapper has of its own.
        """
        if (document.path, what, name) in self._copied:
            return
        kind = document.find(what, name, document.path.name)
        if document.local(kind) not in COPIED or kind.get("baseType"):
            based = " with a base type" if kind.get("baseType") else ""
            raise ValueError(f'{document.path}: {what} "{name}" is a '
                             f"{document.local(kind)}{based}, which the "
                             "Yamcs wrapper does not copy")
        self._names.take(what, name, document.path.name,
                         "rename it in the document")
        self._sets[what].append(document.copy(kind))
        self._copied.add((document.path, what, name))


def _comment(parent, text):
    """Adds a comment, which says why what follows it is there.

    Args:
        parent: The element it goes in.
        text:   What it says, which fits a line.
    """
    parent.append(ET.Comment(f" {text} "))


def _own_entries(copier, link):
    """The parameters a source's data is read as, where no document lays it
    out, with their types: as the configuration's own XTCE states them,
    copied into the wrapper the first time each is needed, or one of
    :data:`BODY_TYPE` for a structure.

    Args:
        copier: The :class:`_Copier` that copies a document's types and
                parameters.
        link:   The :class:`Telemetry`.

    Returns:
        Each parameter of what follows the header, in order: a parameter
        copied from the document by its name, and the source's own, named
        :attr:`Telemetry.name`, with its type's name.

    Raises:
        ValueError: As :meth:`Document.find`, :meth:`_Copier.type` and
                    :meth:`_Copier.parameter`.
        OSError: The configuration's own XTCE cannot be read.
    """
    own = Document(link.own)
    name = own.find("parameter", link.source,
                    own.path.name).get("parameterTypeRef")
    if link.form.kind == "Datum" and not link.counted:
        return [(link.name, BODY_TYPE)]
    copier.type(own, "parameter type", name)
    entries = []
    if name == "bytes":
        copier.parameter(own, BYTES_LENGTH)
        entries.append((BYTES_LENGTH, None))
    entries.append((link.name, name))
    return entries


def _downlink(root, names, copier, link):
    """Adds the container a source's data reaches the ground in.

    Args:
        root:   The wrapper's root.
        names:  The :class:`_Names` it holds.
        copier: The :class:`_Copier` that copies a document's parameters.
        link:   The :class:`Telemetry`.

    Raises:
        ValueError: As :meth:`_Names.take`, :meth:`Document.container` and
                    :meth:`_Copier.parameter`.
        OSError: The source's document cannot be read.
    """
    containers = _one(root, "./x:TelemetryMetaData/x:ContainerSet",
                      "container sets")
    names.take("container", link.container, link.source)
    if link.document is None:
        held = []
        for parameter, kind in _own_entries(copier, link):
            if kind is not None:
                names.take("parameter", parameter, link.source)
                _child(_one(root, "./x:TelemetryMetaData/x:ParameterSet",
                            "parameter sets"),
                       "Parameter", name=parameter, parameterTypeRef=kind)
            held.append(parameter)
    else:
        document = Document(link.document)
        name, held = document.container()
        for parameter in held:
            copier.parameter(document, parameter)
        if link.counted:
            held = [COUNT] + held
        _comment(containers, f"{link.source}: {name}, from "
                             f"{document.path.name}")
    container = _child(containers, "SequenceContainer", name=link.container)
    entries = _child(container, "EntryList")
    for parameter in held:
        _child(entries, "ParameterRefEntry", parameterRef=parameter)
    _restrict(_child(container, "BaseContainer", containerRef="CCSDSPacket"),
              link.apid)


def _uplink_command(root, names, copier, uplink):
    """Adds the command the ground sends a ``gateway_yamcs`` node.

    Args:
        root:   The wrapper's root.
        names:  The :class:`_Names` it holds.
        copier: The :class:`_Copier` that copies a document's types.
        uplink: The :class:`Uplink`.

    Raises:
        ValueError: As :meth:`_Names.take`, :meth:`Document.command` and
                    :meth:`_Copier.argument_type`, or the document's command
                    has an argument the header's command has.
        OSError: The node's document cannot be read.
    """
    commands = _one(root, "./x:CommandMetaData/x:MetaCommandSet",
                    "command sets")
    if uplink.document is None:
        name, container_name = uplink.name, f"{uplink.name}Packet"
        arguments = [ET.Element(_tag("Argument"),
                                {"name": DATA, "argumentTypeRef": DATA_TYPE}),
                     ET.Element(_tag("Argument"),
                                {"name": DATA_BITS,
                                 "argumentTypeRef": DATA_BITS_TYPE})]
        entries = [ET.Element(_tag("ArgumentRefEntry"),
                              {"argumentRef": DATA})]
        fix = "rename the node"
    else:
        document = Document(uplink.document)
        name, said, container_name, from_document = document.command()
        header = {argument.get("name") for argument in root.iterfind(
            f".//x:MetaCommand[@name='{BASE_COMMAND}']/x:ArgumentList"
            "/x:Argument", {"x": NAMESPACE})}
        arguments = []
        for argument in said:
            if argument.get("name") in header:
                raise ValueError(f'{document.path}: command "{name}" has an '
                                 f'argument "{argument.get("name")}", as the '
                                 "header's command does: rename it in the "
                                 "document")
            copier.argument_type(document, argument.get("argumentTypeRef"))
            arguments.append(document.copy(argument))
        entries = [document.copy(entry) for entry in from_document]
        fix = (f'"{uplink.name}" takes {document.path.name}\'s command, '
               "which another gateway_yamcs node takes too: the mission "
               "database can have it once")
        _comment(commands, f"{uplink.name}: {name}, from "
                           f"{document.path.name}")
    names.take("command", name, uplink.name, fix)
    names.take("container", container_name, uplink.name, fix)
    command = _child(commands, "MetaCommand", name=name)
    _assign(_child(command, "BaseMetaCommand", metaCommandRef=BASE_COMMAND),
            uplink.apid)
    _child(command, "ArgumentList").extend(arguments)
    container = _child(command, "CommandContainer", name=container_name)
    _child(container, "EntryList").extend(entries)
    _child(container, "BaseContainer",
           containerRef=f"{BASE_COMMAND}Container")


def mdb(config, template=TEMPLATE, world=False):
    """The mission database's wrapper for a configuration: the template,
    with every APID the configuration counts added.

    Args:
        config:   The :class:`tcslibgs.configuration.Configuration`.
        template: The XTCE that does not depend on the configuration.
        world:    Whether to add :data:`WORLD_AMBIENT`, for a run whose
                  payloads are simulated in a world with an ambient
                  temperature.

    Returns:
        The :class:`xml.etree.ElementTree.ElementTree` of the wrapper.

    Raises:
        ValueError: The configuration does not answer to Yamcs; an APID is
                    missing, not one, past eleven bits or used twice; a node
                    gives a name XTCE does not allow or the wrapper already
                    has; a node's interface document says what the wrapper
                    does not copy; or the template lacks what is added to.
        OSError: The template, or a node's document, cannot be read.
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
    copier = _Copier(root, names)
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

    # Each source's data under its APID, as its document lays it out or
    # whole; and a command for each gateway node, likewise.
    for link in telemetry(config):
        _downlink(root, names, copier, link)
    for uplink in uplinks(config):
        _uplink_command(root, names, copier, uplink)
    if world:
        _world_ambient(root, names)
    return tree


def _world_ambient(root, names):
    """Adds :data:`WORLD_AMBIENT`: a temperature in kelvin, local, with no
    encoding, because no packet carries it.

    Args:
        root:  The wrapper's root.
        names: The :class:`_Names` it holds.

    Raises:
        ValueError: As :meth:`_Names.take`, where a node has the name.
    """
    fix = (f"rename the node: {WORLD_AMBIENT} is the simulated world's "
           "ambient temperature")
    names.take("parameter type", WORLD_TEMPERATURE_TYPE, WORLD_AMBIENT, fix)
    names.take("parameter", WORLD_AMBIENT, WORLD_AMBIENT, fix)
    types = _one(root, "./x:TelemetryMetaData/x:ParameterTypeSet",
                 "parameter type sets")
    _comment(types, "The simulated world's temperature, set by the ground "
                    "station.")
    kind = _child(types, "FloatParameterType", name=WORLD_TEMPERATURE_TYPE,
                  sizeInBits="64")
    _child(_child(kind, "UnitSet"), "Unit",
           description="kelvin").text = "K"
    parameters = _one(root, "./x:TelemetryMetaData/x:ParameterSet",
                      "parameter sets")
    _comment(parameters, "The simulated world's ambient temperature, from "
                         "the simulator file and not from the vehicle.")
    parameter = _child(parameters, "Parameter", name=WORLD_AMBIENT,
                       parameterTypeRef=WORLD_TEMPERATURE_TYPE)
    _child(parameter, "ParameterProperties", dataSource="local")


def write_mdb(config, path, template=TEMPLATE, world=False):
    """Writes the mission database's wrapper for a configuration, no line
    of it wider than :data:`WIDTH`, but a comment's that the template or a
    file's name makes so.

    Args:
        config:   The :class:`tcslibgs.configuration.Configuration`.
        path:     Where to write it.
        template: The XTCE that does not depend on the configuration.
        world:    As :func:`mdb`.

    Raises:
        ValueError: As :func:`mdb`.
        OSError: The template or a node's document cannot be read, or the
                 wrapper written.
    """
    tree = mdb(config, template, world)
    documents = sorted({link.document.name for link in telemetry(config)
                        if link.document}
                       | {uplink.document.name for uplink in uplinks(config)
                          if uplink.document})
    sources = [Path(template).name] + documents + [Path(config.path).name]
    said = ("<!-- Written by tcslibgs/yamcs.py from "
            + ", ".join(sources[:-1]) + f" and {sources[-1]}: edit those, "
            "not this. -->")
    Path(path).write_text('<?xml version="1.0" encoding="UTF-8"?>\n'
                          + "\n".join(_wrapped(said, "     ")
                                      + serialized(tree.getroot()))
                          + "\n", encoding="UTF-8")


def _wrapped(text, indent):
    """A comment, its words wrapped to :data:`WIDTH`.

    Args:
        text:   The comment, on one line.
        indent: What each line after the first begins with.

    Returns:
        Its lines.
    """
    lines, line = [], ""
    for word in text.split(" "):
        if line and len(line) + 1 + len(word) > WIDTH:
            lines.append(line)
            line = indent + word
        else:
            line = f"{line} {word}" if line else word
    return lines + [line]


def _attribute(name, value):
    """An attribute as XML writes it.

    Args:
        name:  Its name.
        value: Its value.

    Returns:
        ``name="value"``, the value escaped.
    """
    for said, written in (("&", "&amp;"), ("<", "&lt;"), (">", "&gt;"),
                          ('"', "&quot;"), ("\n", "&#10;"), ("\r", "&#13;"),
                          ("\t", "&#9;")):
        value = value.replace(said, written)
    return f'{name}="{value}"'


def serialized(root):
    """An XTCE element as text, as the template is written: each element on
    a line of its own, indented by its depth, and an element whose start tag
    would be wider than :data:`WIDTH` with one attribute on each line, each
    under the first.

    Args:
        root: The element, in the wrapper's namespace, which is written as
              its default namespace.

    Returns:
        Its lines.

    Raises:
        ValueError: An element is in another namespace, has a namespaced
                    attribute, or holds both text and elements.
    """
    lines = []

    def write(element, depth):
        """Writes an element and what is in it.

        Args:
            element: The element.
            depth:   How deep it is.
        """
        pad = INDENT * depth
        if element.tag is ET.Comment:
            lines.append(f"{pad}<!--{element.text}-->")
            return
        prefix = f"{{{NAMESPACE}}}"
        if not isinstance(element.tag, str) or not (
                element.tag.startswith(prefix)):
            raise ValueError(f"{element.tag} is not in the wrapper's XTCE "
                             "namespace")
        name = element.tag[len(prefix):]
        if any(key.startswith("{") for key in element.attrib):
            raise ValueError(f"{name} has an attribute in a namespace, which "
                             "the wrapper does not write")
        attributes = [_attribute(key, value)
                      for key, value in element.attrib.items()]
        if depth == 0:
            attributes.insert(0, _attribute("xmlns", NAMESPACE))
        text = (element.text or "").strip()
        inside = list(element)
        if text and inside:
            raise ValueError(f"{name} holds both text and elements")
        for said, written in (("&", "&amp;"), ("<", "&lt;"), (">", "&gt;")):
            text = text.replace(said, written)
        if text:
            end = f">{text}</{name}>"
        else:
            end = ">" if inside else "/>"
        start = f"{pad}<{name}"
        line = " ".join([start] + attributes) + end
        if len(line) <= WIDTH or len(attributes) < 2:
            lines.append(line)
        else:
            under = " " * (len(start) + 1)
            lines.append(f"{start} {attributes[0]}")
            lines.extend(under + attribute for attribute in attributes[1:-1])
            lines.append(under + attributes[-1] + end)
        if inside:
            for part in inside:
                write(part, depth + 1)
            lines.append(f"{pad}</{name}>")

    write(root, 0)
    return lines


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
                        "tcPatterns": [qualified(uplink.command)]})
    return streams
