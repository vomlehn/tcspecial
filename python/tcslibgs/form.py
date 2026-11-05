"""What a node sends, as the configuration says it: a value in a unit, a
decision, or bytes.

The Python spelling of ``tcslibgs::graph::fprime::Form``, with
``tcslibgs::unit``'s table of units and ``tcslibgs::graph::units::unit_of``
behind it. The configuration says one thing of a datum's type, and only of
some nodes: a compute node says, by its function, what it sends.

* A value -- four bytes of a signed count of its unit's steps, most
  significant first (``compute::from_value``) -- is a number in the unit
  the graph gives it: ``to_kelvin`` is in kelvin, ``to_value`` and
  ``process`` state their own, and a ``vote`` is in its inputs' one unit.
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
#: ``compute::DECISION_ON`` and ``DECISION_OFF``.
DECISION_ON = "1"
DECISION_OFF = "0"


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


#: What a computation's function produces: ``compute::produces``.
VALUE_FUNCTIONS = ("to_kelvin", "to_value", "vote", "process")
DECISION_FUNCTIONS = ("thermostat",)


@dataclass(frozen=True)
class Form:
    """What a node sends: ``Datum``, ``Value`` (with its :class:`Unit`),
    ``Decision`` or ``Lines`` (with its lines' offsets), as ``fprime::Form``
    names them."""

    kind: str
    unit: Unit = None
    #: For ``Lines``: the offsets of the lines, lowest first, which is the
    #: order their records come in.
    lines: tuple = ()

    def __str__(self):
        """The form as the Rust's ``Debug`` writes it: ``Datum``,
        ``Value(Kelvin)``, ``Decision``, ``Lines(2)``."""
        if self.kind == "Lines":
            return f"Lines({len(self.lines)})"
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
    if node.node_type == "gpio":
        return _lines_form(node)
    function = _function(config, source)
    if function in DECISION_FUNCTIONS:
        return DECISION
    if function in VALUE_FUNCTIONS:
        return Form("Value", _unit(config, source, []))
    return DATUM


#: The bytes of one line's record: ``gpio::EVENT_BYTES``.
LINE_RECORD_BYTES = 16


def _lines_form(node):
    """The form of a GPIO payload's data: ``Form::of``'s rule for one.

    Its lines' levels where it reads them on an interval and watches them
    for no edge, every datum then holding a record of every line; its bytes
    where it watches for edges, a datum of which may hold the one line that
    changed, or drives them.

    Args:
        node: The node.

    Returns:
        A ``Lines`` :class:`Form`, or :data:`DATUM`.
    """
    said = {name: str(value).strip()
            for name, value in node.attributes.items()}
    levels = (said.get("direction", "input") != "output"
              and said.get("edge") == "none"
              and bool(said.get("state_interval")))
    if not levels:
        return DATUM
    offsets = sorted(int(word, 0) for word in
                     said.get("lines", "").replace(",", " ").split())
    return Form("Lines", lines=tuple(offsets))


def _function(config, name):
    """A node's computation.

    Args:
        config: The configuration.
        name:   The node's name.

    Returns:
        Its ``function``, or ``None`` for a node that is not a compute
        node.
    """
    node = config.node(name)
    if node.node_type != "compute":
        return None
    return node.attributes.get("function", "").strip() or None


def _unit(config, name, asking):
    """The unit a node of values sends them in: ``units::unit_of``.

    Args:
        config: The configuration.
        name:   The node's name.
        asking: The votes whose units are being worked out further up, so
                that a loop of votes ends.

    Returns:
        The :class:`Unit`.

    Raises:
        ValueError: As :func:`form_of`, or the node sends no values.
    """
    function = _function(config, name)
    where = f'{config.path}: "{name}" ({function})'
    if function == "to_kelvin":
        return UNITS[0]
    if function in ("to_value", "process"):
        said = config.node(name).attributes.get("unit")
        if said is None:
            raise ValueError(f"{where}: unit is not set, and a {function} "
                             "says what its values are a number of")
        return unit_of_symbol(said, where)
    if function != "vote" or name in asking:
        raise ValueError(f"{where}: sends no values, and a vote's inputs "
                         "are values")
    units = set()
    for source in _feeding(config, name):
        units.add(_unit(config, source, asking + [name]))
    if len(units) != 1:
        raise ValueError(f"{where}: its inputs' values are in "
                         f"{len(units)} units, and a vote is between "
                         "values of one")
    return units.pop()


def _feeding(config, name):
    """The nodes whose links arrive at a node.

    Args:
        config: The configuration.
        name:   The node fed.

    Returns:
        Each node that has a link to it, once, in the configuration's order.
    """
    found = []
    for node in config.nodes:
        for targets in node.links.values():
            # A destination may name the node's input after a point.
            if (any(target.split(".", 1)[0] == name for target in targets)
                    and node.name not in found):
                found.append(node.name)
    return found
