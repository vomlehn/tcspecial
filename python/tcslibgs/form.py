"""What a node sends, as the configuration says it: a value in a unit, a
decision, or bytes.

The Python spelling of ``tcslibgs::graph::fprime::Form``, with
``tcslibgs::unit``'s table of units and ``tcslibgs::graph::units::unit_of``
behind it. The configuration says one thing of a datum's type, and only of
some nodes: a compute node says, by its function, what it sends.

* A value -- four bytes of a signed count of its unit's steps, most
  significant first (``compute::from_value``) -- is a number in the unit
  its node states: ``to_kelvin`` is in kelvin, ``to_value`` and
  ``process`` state their ``type-out``, and a ``vote`` what it ``carries``,
  each a value type's name, ``kelvin`` or ``metre_per_second``.
* A decision -- the text ``1`` or ``0`` (``compute::from_decision``) -- is
  on or off: what a ``thermostat`` sends.
* A set of GPIO lines read for their levels on an interval, and watched for
  no edge, sends a sixteen-byte record for each line every time, in the
  order of its offsets, lowest first: its lines' levels.
* Every other node's data is bytes, as they came.

tcsmoc's plot window and the F´ bridge's dictionary know a node's data by
this, and so does the Yamcs mission database :mod:`tcslibgs.yamcs`
generates; ``examples/thermostat-yamcs/tests/agrees.rs`` holds the answers
here to the Rust's for every node, and this table to the Rust's.
"""

from dataclasses import dataclass

#: The bytes a value takes on a link: ``compute::VALUE_BYTES``.
VALUE_BYTES = 4

#: A decision to assert and to unassert, as each travels:
#: ``compute::DECISION_ON`` and ``DECISION_OFF``: a ``boolean``, the one
#: byte 1 or 0.
DECISION_ON = 1
DECISION_OFF = 0


@dataclass(frozen=True)
class Unit:
    """A unit a value may be in: one row of ``tcslibgs::unit``'s table."""

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
    ``Decision``, ``Whole`` (with its type's name) or ``Lines`` (with its
    lines' offsets), as ``fprime::Form`` names them."""

    kind: str
    unit: Unit = None
    #: For ``Lines``: the offsets of the lines, lowest first, which is the
    #: order their records come in.
    lines: tuple = ()
    #: For ``Whole``: the type's name, ``u8`` to ``i64``.
    whole: str = None

    def __str__(self):
        """The form as the Rust's ``Debug`` writes it: ``Datum``,
        ``Value(Kelvin)``, ``Decision``, ``Whole(U8)``, ``Lines(2)``."""
        if self.kind == "Lines":
            return f"Lines({len(self.lines)})"
        if self.kind == "Whole":
            return f"Whole({self.whole.upper()})"
        if self.unit is None:
            return self.kind
        return f"{self.kind}({self.unit.variant})"


DATUM = Form("Datum")
DECISION = Form("Decision")


def form_of(config, source):
    """Finds the form of what a node sends, as the configuration says it:
    ``Form::of``.

    Args:
        config: The :class:`tcslibgs.configuration.Configuration`.
        source: The node's name.

    Returns:
        A value's :class:`Form`, with its unit, for a compute node whose
        function makes values; :data:`DECISION` for one whose function
        makes decisions; a ``Lines`` form for a GPIO payload read for its
        lines' levels, as :func:`_lines_form` says; :data:`DATUM` for every
        other node, and for one that is not there.

    Raises:
        ValueError: A node of values does not say its unit, says one that
                    is not a unit, or is a vote whose inputs' units do not
                    agree: a configuration tcsverify refuses, whose data
                    is not to be read as anything.
    """
    names = {node.name for node in config.nodes}
    if source not in names:
        return DATUM
    node = config.node(source)
    if node.node_type == "gpio_in":
        return _lines_form(node)
    said = _sends(node)
    if said in WHOLE:
        return Form("Whole", whole=said)
    if said == "boolean":
        return DECISION
    if said is None or said in ("bytes", "string") or ":" in said:
        return DATUM
    return Form("Value", unit_of_type_name(said, f'{config.path}: '
                                                  f'"{source}"'))


#: The whole numbers' names: ``link_type::Basic``'s.
WHOLE = ("u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64")

#: The type each of a device's formats lays out:
#: ``compute::Format::basic``.
FORMAT_TYPES = {"u8": "u8", "i8": "i8", "i16_be": "i16", "u16_be": "u16",
                "u24_be": "u32", "i16_le": "i16", "i32_be": "i32",
                "u32_be": "u32"}


def _sends(node):
    """The type a node states it sends at its ``world-out``:
    ``link_types::carries``.

    Args:
        node: The node.

    Returns:
        The type's name as the configuration writes it, or ``None`` for a
        node that takes any type and sends nothing.
    """
    said = {name: str(value).strip()
            for name, value in node.attributes.items()}
    kind = node.node_type
    if kind == "compute":
        function = said.get("function")
        if function == "to_kelvin":
            return "kelvin"
        if function == "vote":
            return said.get("carries")
        return said.get("type-out")
    if kind in ("tee-gateway", "tee-payload"):
        return said.get("carries", "bytes")
    if kind == "ping":
        return "string"
    if kind in ("gpio_out", "gpio_inout"):
        count = len(said.get("lines", "").replace(",", " ").split())
        return next(name for name, most in (("u8", 8), ("u16", 16),
                                            ("u32", 32), ("u64", 64))
                    if count <= most)
    if kind.startswith(("null-", "gateway_")) or kind == "log":
        return None
    if "world-out-type" in said:
        return said["world-out-type"]
    if "format" in said and not kind.startswith("app_"):
        return FORMAT_TYPES.get(said["format"])
    return "bytes"


#: The bytes of one line's record: ``gpio::EVENT_BYTES``.
LINE_RECORD_BYTES = 16


def _lines_form(node):
    """The form of a GPIO payload's data: ``Form::of``'s rule for one.

    Its watched lines' levels where it reads them on an interval and
    watches them for no edge, every datum then holding a record of every
    watched line; its bytes where it watches for edges, a datum of which may
    hold the one line that changed, or where it watches none.

    Args:
        node: The node.

    Returns:
        A ``Lines`` :class:`Form`, or :data:`DATUM`.
    """
    said = {name: str(value).strip()
            for name, value in node.attributes.items()}
    levels = (node.node_type == "gpio_in"
              and said.get("edge") == "none"
              and bool(said.get("state_interval")))
    if not levels:
        return DATUM
    offsets = sorted(int(word, 0) for word in
                     said.get("lines", "").replace(",", " ").split())
    return Form("Lines", lines=tuple(offsets))
