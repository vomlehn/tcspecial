"""What a node sends the ground: a value in a unit, a decision, a whole
number, or bytes.

The Python spelling of ``tcslib_protocol::graph::fprime::Form``, with
``tcslib_protocol::unit``'s table of units behind it. What each node sends is
not worked out here from what the configuration says of it: it is read from the
configuration's own XTCE, which ``tcs-verify --xtce`` writes beside it from the
configuration and ``tcslib-protocol/xtce/types.xtce`` (see "The configuration's
own XTCE" in ``docs/tcspecial-design.rst``), so the rules are the Rust's alone.

* A value -- four bytes of a signed count of its unit's steps, most
  significant first (``compute::from_value``) -- is a number in its unit:
  a parameter of a value type, ``kelvin`` or ``metre_per_second``.
* A decision -- a ``boolean``, the one byte 1 or 0
  (``compute::from_decision``) -- is on or off.
* A whole number, ``u8`` to ``i64``, most significant byte first: a
  sensor's reading, or what a set of GPIO lines is.
* Every other node's data is bytes, as they came: a string, bytes, or a
  structure.

tcs-moc's plot window and the F´ bridge's dictionary know a node's data by
this, and so does the Yamcs mission database :mod:`tcslib_protocol.yamcs`
generates; ``examples/thermostat-yamcs/tests/agrees.rs`` holds the answers
here to the Rust's for every node that reaches the ground, and this table
of units to the Rust's.
"""

import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path

#: The bytes a value takes on a link: ``compute::VALUE_BYTES``.
VALUE_BYTES = 4

#: A decision to assert and to unassert, as each travels:
#: ``compute::DECISION_ON`` and ``DECISION_OFF``: a ``boolean``, the one
#: byte 1 or 0.
DECISION_ON = 1
DECISION_OFF = 0


@dataclass(frozen=True)
class Unit:
    """A unit a value may be in: one row of ``tcslib_protocol::unit``'s
    table."""

    #: The Rust's name for it, ``Kelvin``, which names it where a symbol
    #: cannot: in an XTCE name, which allows no slash.
    variant: str
    #: How a file writes it: ``K``, ``m/s``.
    symbol: str
    #: What it is called in a sentence: ``kelvin``.
    name: str
    #: How many of the counts a value carries are in one of it.
    per_unit: float


#: Every unit, in the Rust's order.
UNITS = (
    Unit("Kelvin", "K", "kelvin", 1e6),
    Unit("Volt", "V", "volts", 1e6),
    Unit("Ampere", "A", "amperes", 1e6),
    Unit("Watt", "W", "watts", 1e3),
    Unit("Metre", "m", "metres", 1e3),
    Unit("MetrePerSecond", "m/s", "metres a second", 1e6),
    Unit("Newton", "N", "newtons", 1e3),
    Unit("KilogramPerSecond", "kg/s", "kilograms a second", 1e6),
    Unit("Ratio", "ratio", "a ratio", 1e6),
)


def unit_of_symbol(said, what):
    """Reads the unit a file writes: ``Unit::of``.

    Args:
        said: The unit as the file wrote it: ``K``, ``V``, ``m/s``.
        what: What states it, for the complaint.

    Returns:
        The :class:`Unit`.

    Raises:
        ValueError: It is no unit there is.
    """
    for unit in UNITS:
        if unit.symbol == said.strip():
            return unit
    every = ", ".join(unit.symbol for unit in UNITS)
    raise ValueError(f'{what}: "{said}" is not a unit this knows. There are '
                     f"{every}")


def unit_of_type_name(said, what):
    """Reads the value type a configuration names: ``Unit::of_type_name``.

    Args:
        said: The type's name: ``kelvin``, ``metre_per_second``.
        what: What states it, for the complaint.

    Returns:
        The :class:`Unit`.

    Raises:
        ValueError: It is no value type there is.
    """
    for unit in UNITS:
        if type_name(unit) == said.strip():
            return unit
    every = ", ".join(type_name(unit) for unit in UNITS)
    raise ValueError(f'{what}: "{said}" is not a value type. There are '
                     f"{every}")


def type_name(unit):
    """Names a unit's value type as a configuration does: its Rust name in
    lower case with an underscore between its words.

    Args:
        unit: The :class:`Unit`.

    Returns:
        Its value type's name: ``kelvin``, ``metre_per_second``.
    """
    words = []
    for letter in unit.variant:
        if letter.isupper() and words:
            words.append("_")
        words.append(letter.lower())
    return "".join(words)


@dataclass(frozen=True)
class Form:
    """What a node sends: ``Datum``, ``Value`` (with its :class:`Unit`),
    ``Decision`` or ``Whole`` (with its type's name), as ``fprime::Form``
    names them."""

    kind: str
    unit: Unit = None
    #: For ``Whole``: the type's name, ``u8`` to ``i64``.
    whole: str = None

    def __str__(self):
        """The form as the Rust's ``Debug`` writes it: ``Datum``,
        ``Value(Kelvin)``, ``Decision``, ``Whole(U8)``."""
        if self.kind == "Whole":
            return f"Whole({self.whole.upper()})"
        if self.unit is None:
            return self.kind
        return f"{self.kind}({self.unit.variant})"


DATUM = Form("Datum")
DECISION = Form("Decision")


def own_xtce(config):
    """Names a configuration's own XTCE: ``tcs_verify::own_xtce::path_of``.

    Args:
        config: The :class:`tcslib_protocol.configuration.Configuration`.

    Returns:
        The file beside its top file, of its name with the extension
        ``.xtce``.
    """
    return Path(config.path).with_suffix(".xtce")


def type_of(config, source):
    """Finds the type a node sends the ground, as the configuration's own
    XTCE names it.

    Args:
        config: The :class:`tcslib_protocol.configuration.Configuration`.
        source: The node's name.

    Returns:
        The type's name -- ``kelvin``, ``u8``, ``string``, ``bytes``, or a
        structure's -- or ``None`` for a node that does not reach the
        ground, which the document does not name.

    Raises:
        ValueError: The document is not there, does not read, or gives the
                    node a type it does not have.
        OSError: The document cannot be read.
    """
    path = own_xtce(config)
    try:
        root = ET.parse(path).getroot()
    except FileNotFoundError:
        raise ValueError(f"{path}: not there: tcs-verify --xtce "
                         f"{config.path} writes it") from None
    except ET.ParseError as why:
        raise ValueError(f"{path}: {why}") from None
    namespace = root.tag[1:].split("}", 1)[0] if "}" in root.tag else ""
    x = {"x": namespace}
    said = None
    for parameter in root.iterfind("./x:TelemetryMetaData/x:ParameterSet"
                                   "/x:Parameter", x):
        if parameter.get("name") == source:
            said = parameter.get("parameterTypeRef")
    if said is None:
        return None
    if not any(kind.get("name") == said for kind in root.iterfind(
            "./x:TelemetryMetaData/x:ParameterTypeSet/*", x)):
        raise ValueError(f'{path}: "{source}" is of the type "{said}", '
                         "which the document does not have")
    return said


def form_of(config, source):
    """Finds the form of what a node sends the ground, as the
    configuration's own XTCE says it: ``Form::of``, for a node that reaches
    the ground.

    Args:
        config: The :class:`tcslib_protocol.configuration.Configuration`.
        source: The node's name.

    Returns:
        A value's :class:`Form`, with its unit, for a parameter of a value
        type; :data:`DECISION` for a ``boolean``; a ``Whole`` form for a
        whole number -- a sensor's reading, what a set of GPIO lines is;
        :data:`DATUM` for a string, bytes or a structure, and for a node
        that does not reach the ground, which the document does not name.

    Raises:
        ValueError: As :func:`type_of`.
        OSError: As :func:`type_of`.
    """
    said = type_of(config, source)
    if said is None:
        return DATUM
    if said in WHOLE:
        return Form("Whole", whole=said)
    if said == "boolean":
        return DECISION
    if any(type_name(unit) == said for unit in UNITS):
        return Form("Value", unit_of_type_name(said, f'{own_xtce(config)}: '
                                                      f'"{source}"'))
    return DATUM


#: The types sent the ground after a count of their bytes:
#: ``LinkType::is_counted``.
COUNTED = ("string", "bytes")


#: The whole numbers' names: ``link_type::Basic``'s.
WHOLE = ("u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64")
