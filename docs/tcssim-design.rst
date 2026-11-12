=============
tcssim Design
=============

.. contents:: Contents
   :depth: 3
   :local:

How tcssim, the payload simulator, works and why: the simulator
configuration it reads, and the program. How a simulated payload is told
from a real one, and how tcspecial waits for tcssim, are in Testing, in
`the tcspecial design <tcspecial-design.html#testing>`__.

Simulator Configuration Files
=============================
The tcspecial configuration describes payloads. Simulating one takes more than
that description holds, and the extra is not payload configuration: how often a
payload produces a packet, and how it divides a packet into segments, are
choices about a simulation. They are stated in a separate file, which only
tcssim acts on. A shipped pair is ``examples/minimal/minimal.ytcs`` and
``minimal-sim.yaml`` beside it; the parser is ``tcslibgs::sim_config``.

Requirement
    The parser lives with the other three configuration languages rather than
    inside tcssim. Both GUIs read this file now -- one to simulate payloads
    and one to show what a payload set says about them -- and a configuration
    language read by two programs cannot live inside one of them.

The format is chosen from the extension exactly as every other configuration
file's is, so a simulator configuration may be written in YAML or XML.

File Structure
--------------
A simulator configuration file has a description of its own, a ``tcssim``
section, which is required, and two sections that are optional.

tcssim
    What the simulator itself is configured with. Today that is one address:

    .. code-block:: yaml

       tcssim:
         ready: "127.0.0.1:4800"

    ``ready`` is the UDP address of the readiness socket, where the simulator
    says whether it has made its simulated devices; see `Waiting for the
    Simulator <tcspecial-design.html#waiting-for-the-simulator>`__. It is required, and every shipped simulator file states one.

simulated payload groups
    Named groups of settings, each named by ``group``. A group carries what
    several simulated payloads have in common, and may say whether they are
    simulated, with ``simulate``.

simulated payloads
    The simulated payloads themselves, grouped by type as a configuration's
    nodes are. Each names the data handler it simulates, by ``payload``,
    may name a group, and may state settings of its own. Each
    has ``simulate``, from itself or from its group but not both: see
    `Saying Which Payloads Are Simulated <tcspecial-design.html#saying-which-payloads-are-simulated>`__.

.. code-block:: yaml

   simulated_payload_groups:
     - group: zone_temps
       packet_interval: 100 ms
       simulate: true

   simulated_payloads:
     - type network:
         - payload: temp_1
           group: zone_temps
           protocol: udp

In XML a group is ``<group name="zone_temps">`` in one
``<simulated_payload_groups>``, and a payload ``<payload name="temp_1">`` in
a ``<type name="network">`` in one ``<simulated_payloads>``.

Requirement
    A simulated payload's type is said once, by the entry it is written
    under: ``- type network:`` in YAML, ``<type name="network">`` in XML. A
    payload that says a type of its own is refused, and so is a group that
    says one, a group's payloads each having the type they are written
    under. A type is written once in a file, with at least one payload
    under it, and a word that is no type of payload is refused, listing the
    types.

Requirement
    An entry is named by its word -- ``payload`` for a simulated payload,
    ``group`` for a group, the ``name`` attribute of its element in XML --
    and one named by ``name:`` is refused, saying how one is named.

Requirement
    Nothing in a simulator file is in an order that means anything: not the
    types, not the payloads of one, not the groups, and not the settings of
    an entry. Each list is held in name order, whatever order the file
    wrote it in, and tcssim makes and shows its simulated payloads in the
    configuration's order, which is by name.

Requirement
    A simulator file in the shape before this one, each payload an entry of
    its own with ``name:`` and ``type:`` and each group named by ``name:``,
    is refused at the line of its first entry, shown that entry in the shape
    it has now and told that ``tools/convert-format.py`` rewrites the file,
    keeping its comments.

**Settings**

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| simulate            | bool   | Whether the payload is simulated: ``true``  |
|                     |        | for tcssim to simulate it, ``false``        |
|                     |        | for the real device. Required, from the     |
|                     |        | payload or its group and not both           |
+---------------------+--------+---------------------------------------------+
| packet_interval     | length | How long between packets, as a length of    |
|                     | of     | time -- ``1 s``, ``500 ms`` -- or           |
|                     | time   | ``continuous`` for as fast as the payload   |
|                     |        | can be driven                               |
+---------------------+--------+---------------------------------------------+
| segment_interval    | length | How long between the segments of one        |
|                     | of     | packet, ``continuous`` included. Defaults   |
|                     | time   | to the packet interval                      |
+---------------------+--------+---------------------------------------------+
| segment_size        | number | Bytes in one segment. Defaults to the whole |
|                     |        | packet, whose size the payload configuration|
|                     |        | gives                                       |
+---------------------+--------+---------------------------------------------+

**What it simulates.** Neither of these is a setting of the simulation.
The payload configuration decides both, and they are stated here again only so
that the two files can be checked against each other.

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| type                | string | The type of payload this simulates, in the  |
|                     |        | payload configuration's own word:           |
|                     |        | ``network``, ``device-file``, ``serial``,   |
|                     |        | ``i2c``, ``spi``, ``can``, ``iio``,         |
|                     |        | ``rpmsg``, ``gpio_in``, ``gpio_out``,       |
|                     |        | ``gpio_inout`` or ``hdlc``. Not a           |
|                     |        | setting of the entry: the type it is        |
|                     |        | written under                               |
+---------------------+--------+---------------------------------------------+
| protocol            | string | Which transport, for a network payload.     |
|                     |        | Refused for every other type                |
+---------------------+--------+---------------------------------------------+

**The payload's own end of the link**, which nothing else states:

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| payload_address     | string | The socket the payload answers from: a host |
|                     |        | for a UDP payload, a path for a Unix        |
|                     |        | datagram one. Optional, and only for those  |
+---------------------+--------+---------------------------------------------+
| payload_port        | number | The port it answers from. Optional, and     |
|                     |        | refused for a Unix datagram payload         |
+---------------------+--------+---------------------------------------------+

**Fault injection settings**, each defaulting to no fault at all:

+------------------------+--------+------------------------------------------+
| Attribute              | Type   | Meaning                                  |
+========================+========+==========================================+
| drop_percent           | number | Packets in a hundred that are never sent |
+------------------------+--------+------------------------------------------+
| corrupt_percent        | number | Packets in a hundred with one byte       |
|                        |        | altered                                  |
+------------------------+--------+------------------------------------------+
| truncate_percent       | number | Packets in a hundred sent short of their |
|                        |        | size                                     |
+------------------------+--------+------------------------------------------+
| jitter                 | length | How late a packet may be, as a length    |
|                        | of     | of time, chosen afresh for each one      |
|                        | time   |                                          |
+------------------------+--------+------------------------------------------+
| silent_after           | number | Packets after which the payload sends no |
|                        |        | more, the link still open. 0 is never    |
+------------------------+--------+------------------------------------------+
| close_after            | number | Packets after which the payload hangs    |
|                        |        | up. 0 is never                           |
+------------------------+--------+------------------------------------------+
| ignore_trigger_percent | number | Requests in a hundred a triggered        |
|                        |        | payload leaves unanswered                |
+------------------------+--------+------------------------------------------+

Requirement
    A simulated payload takes each setting it does not state from the group it
    names.

Requirement
    A simulated periodic payload has a packet interval, from itself or from
    its group. One with none is an error rather than a payload driven at a
    rate nobody chose.

Requirement
    A simulated triggered payload has no packet interval, and stating one is
    an error. It sends when tcspecial asks and at no rate of its own, so an
    interval here would govern nothing -- and a file that gave one has not
    understood which type of payload it is simulating. The segment settings
    still apply: how a payload divides an answer is its own business whichever
    way it was prompted.

Requirement
    A simulated triggered payload sends one packet for each request it is
    sent, and nothing otherwise. A payload that answered on a clock as well
    would be two types at once, and no file could say which rate was which.

Requirement
    The simulator refuses a triggered payload it cannot hear a request on.
    Tcspecial opens a device and reads it, so there is nothing for the
    simulator to be written to; and an I2C master's own read is the trigger,
    which a second master on the bus cannot see. A payload that answered
    nothing would look exactly like a handler whose polling had stopped
    working.

What a Simulated Payload Simulates
----------------------------------
The two files are joined by handler name and by nothing else, and a name is
exactly what survives a payload set copied from another one. Two sets that
differ only in how their payload is reached -- one a datagram socket, one a
stream -- therefore have simulator files that each parse perfectly well
against the other's payloads, and the run that follows looks like a
payload that never sends. So each simulated payload says what it simulates,
and the statement is compared rather than used.

Requirement
    A simulated payload states the type of payload it simulates, by the type
    it is written under, and a network payload the transport as well.
    Neither is taken from this file: the payload configuration decides both,
    and a payload that stated neither could not be checked against anything.

Requirement
    The two files cannot disagree about a payload's type or its transport. A
    disagreement is an error naming the payload and what each file says of it,
    rather than one file overriding the other: both describe the one payload,
    so a disagreement means one of them is not the file that was meant.

Requirement
    A transport is stated by a network payload and refused for every other
    type. A line, a bus or a peripheral is not reached by a transport, so one
    stated for it would govern nothing.

Requirement
    The type and the transport are written in the same words the payload
    configuration uses, and the error that refuses a word that is neither
    lists the words that would have worked. The spellings are one table read
    from both files, so the two cannot come to disagree about what a type is
    called.

Requirement
    Where the payload is is not stated here. The simulator takes it from the
    payload configuration, which decides it, so this file stating it again
    would be a second place to keep in step and nothing gained:
    ``payload_interface`` written here -- or ``address`` and ``port``, which
    that file spelled it before -- is refused with the file it belongs in, and
    with the pair this file does have.

What this file may say about an address is the other end of the same link: the
socket the simulated payload answers from.

Requirement
    A simulated payload may be given the socket it binds for itself, as
    ``payload_address`` and ``payload_port``. Nothing in the link needs it --
    a handler learns where its payload is from the first packet it gets -- but
    something outside the simulation may: a rule on a firewall, or a capture
    being read afterwards. Stated by neither file otherwise, so there is
    nothing for this to disagree with.

Requirement
    Only a datagram payload has an address of its own to bind. A stream
    payload is the end that waits, so it binds the address the payload
    configuration gives it, and a second address stated here would be two
    answers to where it is; a device, a line, a bus or a peripheral has no
    address at all.

Requirement
    A Unix datagram payload answers from a path, so it takes a
    ``payload_address`` and no ``payload_port``.

Requirement
    Neither end may be the other. A payload told to bind its handler's own
    socket is refused here rather than left to fail at the bind, which says
    nothing about which file was wrong: two sockets cannot be one, and the two
    ends of a link are two sockets.

Requirement
    A file that states neither gets what every simulator file asked for before
    this existed: any interface and a port the system chooses, or for a Unix
    datagram payload a path beside the handler's.

Requirement
    A group may carry the transport, like every other setting a payload does
    not state, and never the type, which is the one a payload is written
    under. A group whose payloads are not all reached the same way leaves the
    transport with the payloads: the thermostat's sensors share a rate and a
    terminal, and two of the three are on a different kind of bus from the
    third.

The Physics Model
-----------------
Every other simulated payload makes its bytes up. A buffer filled from a
random number generator exercises a link and nothing at all of a control loop:
three sensors sending noise never agree, so the vote over them produces
nothing anybody can check, and a thermostat downstream of it cannot be watched
holding a band.

A physics model is what makes them agree. It is the vehicle's physical parts --
a zone, the heaters that warm it -- each simulated alone and joined to the
others, and the simulated payloads are joined to it: the sensors report the
zone's temperature, each in its own encoding, and the heaters' simulated
payloads switch the heaters -- so what the vehicle reads is a consequence of
what the vehicle drove, and the whole loop can be watched closing.

Requirement
    The model is in the simulator file, under ``world: physics:``, beside the
    simulated payloads joined to it, so that one file says the whole of what the
    simulator simulates and a simulated payload's ``connects`` names a terminal
    the same file says is there. The world also states ``ambient_temp``, the one
    temperature every thermal mass's surroundings are about, required where
    there is a zone. The world is YAML only: a simulator file written in XML has
    none, and one with a ``world`` or a ``physics`` element is refused.

Requirement
    The shapes a model had before are refused, each told where it goes: a
    simulator file naming a model in a file of its own,
    ``physics: zone-physics.yaml``; such a file given as a simulator file;
    a ``physics`` section at the top of a simulator file; a zone stating
    its own ``environment``, which is the world's ``ambient_temp`` now; and
    a simulator file that still declares masses itself.

Requirement
    The model is the components it is made of, grouped by type as a
    configuration's nodes are, and a list of connections between them, as a
    configuration has links: ``components`` and ``connections``, under
    ``physics``. Each entry of ``components`` is ``type`` and a type,
    ``- type heater:``, holding the components of that type, each
    named by ``component``. The types are ``heater``, ``thermal_mass``,
    ``sun``, ``solar_array``, ``load``, ``bus``, ``battery``, ``engine`` and
    ``body``, and a type of any other is refused; a ``thermal_zone``, which
    is what a ``thermal_mass`` was called before, is told the name it has now.
    A type is written once, with a component under it. Each type's settings,
    terminals and rules are stated once, in ``tcslibgs::node::physics``; its
    arithmetic is the simulator's, in ``tcssim::physics``.

Requirement
    A ``thermal_mass`` has two heaters of its own, ``heater_1`` and
    ``heater_2``: inputs taking a decision, each adding its stated
    ``heater_1_power`` or ``heater_2_power`` to the mass's heat while it is
    on. A heater's switch -- a ``gpio_out`` or a ``gpio_inout`` -- may then be
    joined straight to the zone it warms, with no heater component between
    them, where nothing about the heater needs modelling but its power. A
    zone whose ``heater_N`` is joined and which states no power for it is
    refused, told what to write: a heater that switched and warmed nothing
    would be a model that looked right. Two, and fixed, because a
    component's terminals are its type's, and two heaters on one zone is what
    a vehicle that must not lose its only way to warm it carries.

Requirement
    Nothing in a model is in an order that means anything: not the types,
    the components of one, or the connections. The components are held in
    name order and the connections in the order of the names of the
    terminals they join, so that what tcssim does with a model is the same
    however it is written: an input that sums is given its outputs in that
    order, and of two events due at one moment, the one whose component's
    name sorts first happens first.

Requirement
    A model in the shape before this one, each component with a ``name`` and
    a ``type`` of its own, is refused at the line of its first component,
    shown it in the shape it has now and told that
    ``tools/convert-format.py`` rewrites the file.

Requirement
    Each type is one simple part, with its own words for what goes in and
    what comes out, rather than a whole assembly: a heater is switched
    ``on``, gives out ``heat``, draws ``current``, and says whether it is
    ``heating`` -- what a line sensing it reads, which is what it is doing
    rather than what it was told; a zone takes
    ``heat`` and has a ``temperature``. A vehicle is made by joining parts,
    so a part is written once and a different vehicle is different
    connections.

Requirement
    Each type's terminals are fixed. A terminal is an input or an output,
    and carries a value in one unit or a decision: a heater's ``heat`` is in
    watts and its ``on`` a decision, a zone's ``temperature`` in kelvin. A
    connection goes from an output to an input of the same kind, written
    ``from: heater_1.heat`` and ``to: zone.heat``, and one joining two
    outputs, two inputs or two kinds is refused, naming both.

Requirement
    An input takes one output, or the sum of every output joined to it where
    its type says so: a zone's ``heat`` is the sum of its heaters'. A second
    output on an input that takes one, or a connection made twice, is
    refused. An input that must be joined -- a heater's ``on`` -- and is
    not, is refused: it is a part nothing can switch.

Requirement
    Every figure is written with its unit -- ``"283.15 K"``, ``"60 W"``,
    ``"20 J/K"`` -- and one without it, or in another unit, is refused,
    saying what to write. A bare ``60`` could be watts or kelvin a second,
    which differ by the zone's heat capacity. A fraction, which has no unit,
    is written bare.

Requirement
    A heater states its power at its nominal voltage and that voltage. A
    thermal mass sits in surroundings about the world's ``ambient_temp``,
    and states its time constant -- how long it takes to lose a given
    fraction of the difference between itself and its surroundings -- and
    its heat capacity, which turns a heater's watts into kelvin a second.

Requirement
    A simulated payload is joined to one terminal by ``connects``, written
    as a connection's end is: ``connects: zone.temperature``. Joined to an
    output, it reports it; joined to an input, it drives it. A terminal the
    model does not have is refused, naming what it has.

Requirement
    A simulated payload reporting a value sends it in the encoding of the
    conversion its readings go to. One reporting a decision sends the text ``1``
    or ``0``, again every packet interval and at once when it changes: it waits
    on the world for the change, and the world advances itself to each event it
    foresees at that event's moment on the wall clock, so the vehicle hears of a
    touchdown when it happens and not at the simulated payload's next turn.

Requirement
    A simulated payload driving a decision takes ``1`` or ``0``, as a thermostat
    sends it. One driving a value takes four bytes of its unit's steps, as every
    value on a link is, and what reaches it must be values in that unit: a node
    sending another is refused, naming both units, since an engine reading
    newtons as a ratio would be at full throttle. The settings that joined a
    payload to a thermal mass, ``reads``, ``heats``, ``output``, ``input`` and
    ``rate``, are refused, told what says it now.

Requirement
    An input is driven by one simulated payload, and not by a simulated payload
    and a connection. Two would each switch it over the other, and which was in
    force would depend on which the vehicle drove last.

Requirement
    A component nothing is joined to -- no connection and no simulated payload
    -- is refused. It would be a state nothing reads and nothing changes, and
    the likeliest cause is a misspelled ``connects``.

Requirement
    ``tcsverify`` reports every problem with a component at its line in
    the simulator file, a problem with the connections at the world's
    ``connections`` line, and one with the ambient temperature at its
    ``ambient_temp`` line.

**How the world advances**

The simulator makes one world of the model, which every simulated payload joined
to it shares, and nothing ticks.

Requirement
    The world is brought up to a moment only when something needs it there: a
    simulated payload reading an output, or driving an input. A world nobody
    asks about costs nothing.

Requirement
    Simulated time tracks the wall clock, a second for a second, because the
    vehicle at the other end of every link runs on the wall clock. It starts
    when the run begins -- the first time a running simulated payload asks the
    world anything -- and not when the world is made: the simulated payloads
    start stopped, and a world running on with nobody joined to it would have a
    lander on the ground before its radar was enabled. The mapping has a rate,
    so that a world can run faster than the wall clock where nothing at the
    other end needs it to keep pace; a run with tcspecial keeps it at one.

Requirement
    Between two moments every component is advanced with its inputs held, by
    its type's closed form where there is one, which is exact. The world is
    advanced whenever it is asked, so its intervals are whatever length the
    traffic makes them, and a method whose error grew with the interval
    would make the simulated values depend on how often somebody looked.

Requirement
    Where a component's behaviour changes inside an interval -- a battery
    reaching full, a body reaching the ground -- it foresees the moment, as
    its next event. The events wait in a queue, soonest first, one for each
    component, set again whenever the component's inputs change; the world
    advances to each in its order, lets it happen and goes on, so nothing is
    stepped across. The queue's room is the number of components, fixed
    when the world is made, and nothing is allocated once it runs.

Requirement
    What is at each input is worked out from every output as it stands at
    the start of an interval, and given again until it settles, so that a
    heater switched on is a zone warmed from that moment and not from the
    next one.

Requirement
    A component that foresees its next event at the moment it is already at,
    time after time, is stopped after ten thousand and the log says so: the
    world falls behind rather than every simulated payload waiting on it for
    ever.

**A thermal mass and its heaters**

A heater's heat goes as the square of its supply voltage and its current as
the voltage, ``power * (1 + v)^2`` and ``power / voltage * (1 + v)`` with
``v`` the supply's departure from nominal, nought while it is off. A zone's
arithmetic is Newton's cooling with that heat held,

.. code-block:: text

   dT/dt = -(T - environment) / time_constant + heat / capacity

which over an interval has a closed form, and the zone is advanced by that
rather than integrated:

.. code-block:: text

   T(t + dt) = rest + (T - rest) * exp(-dt / time_constant)
   rest      = environment + heat / capacity * time_constant

``rest`` is where the zone would settle with that heat on for ever, and it is
the figure that says whether the heaters can win at all: with a fifty second
constant, twenty joules a kelvin and a hundred and twenty watts, it is three
hundred kelvin above the environment, which is the zone's surroundings: the
world's ``ambient_temp``, swinging and wandering as the zone says.

Requirement
    A zone starts at the world's ``ambient_temp``, and a heater off, unless the model
    states ``start``, and then there. Coming up cold is the case a
    thermostat exists for, and its first climb is worth watching; a model
    that would rather show the thermostat holding its band from the first
    reading states a start in the band, as the thermostat example's does.
    The start is one fixed figure and never drawn at random, so that every
    run of a model starts alike and two runs can be compared.

**A power bus**

A sun lights a solar array, the array supplies a bus, loads draw on it, and
what is left over charges a battery, or, in shadow, what the array cannot give
the battery makes up.

Requirement
    A sun gives out ``light``, a ratio: one in sunlight and nought in
    shadow. Its orbit's ``period`` and the fraction of it in shadow,
    ``eclipse``, are stated, and where round it it starts. The light changes
    all at once, so each change is an event, foreseen from the orbit.

Requirement
    A solar array takes ``light`` and gives out its full-sun ``current``
    times it, in amperes. The bus of a spacecraft without a regulator sits
    where its battery holds it, on the flat of the array's curve, where its
    current is very nearly its short-circuit figure.

Requirement
    A load gives out the ``current`` it draws, nought while its decision
    ``shed`` is asserted. A load nothing sheds is never shed.

Requirement
    A bus takes the sum of the sources joined to its ``supply`` and of the
    loads joined to its ``demand``, and gives out what is left over as its
    ``current``. It has no settings: a bus is its connections.

Requirement
    A battery takes what the bus has left over, ``offered``, and gives out
    its ``voltage``, the ``current`` that actually flows into it, and its
    ``charge``, the fraction of its ``capacity`` it holds. Its charge moves
    in a straight line with the current, and its voltage runs straight from
    ``empty`` to ``full`` with its charge, plus what its ``resistance``
    drops. A straight line reaches full or empty at a moment it can foresee,
    which is its next event, so it is never stepped past either.

Requirement
    A full battery takes no current: what would charge it is shunted, as a
    spacecraft's regulator does. An empty one is held at empty, and what is
    drawn is still drawn, its voltage below ``empty`` showing it: a cell past
    what it is rated for is outside what the model says.

**An engine and a body**

Requirement
    An engine takes its ``throttle``, a ratio held within nought and one,
    and gives out its ``force``, its full ``thrust`` times the throttle, and
    the ``flow`` of fuel that burns, the force over its specific impulse
    ``isp`` times standard gravity, in kilograms a second. Its ``fuel``
    falls in a straight line with the throttle held, and reaches nought at a
    moment it foresees, its next event; from then on it gives out nothing.

Requirement
    A body takes the sum of the forces joined to its ``force`` and of the
    flows joined to its ``flow``, and gives out its ``altitude``, its
    ``velocity``, up more than nought, and ``contact``, a decision asserted
    on the ground. It falls by the rocket equation with ``gravity``, whose
    closed form with the force and the flow held is

    .. code-block:: text

       v(t) = v - (F / q) ln(1 - q t / m) - g t
       x(t) = x + v t - g t^2 / 2 + (F m / q^2) h(q t / m)
       h(u) = (1 - u) ln(1 - u) + u

    with ``h`` taken by its series where ``u`` is small, where its two terms
    would cancel; with no flow it is the constant acceleration's.

Requirement
    Where a body reaches the ground has no closed form, and is found ahead
    of time, as its next event, by stepping that never passes it: the
    acceleration it has is the least it will have while the force and the
    flow are held, so the earliest it could reach the ground is where it
    would with that acceleration, and each step goes there. The steps close
    on the ground from above until within a micrometre. There it is set
    down, altitude and velocity nought and ``contact`` asserted, and stays
    until a force beating its weight lifts it.

**What the bytes are**

A sensor has to send bytes the vehicle will read back as what it reports, and
the format, the scale and the offset are not the sensor's own: they are on the
conversion, ``to_kelvin`` or ``to_value``, its readings go to.

Requirement
    The encoding is taken from that node and not stated here. A scale written
    in both files is a scale that can disagree with itself, which would show
    up as a simulated temperature the vehicle reads back as a different one,
    with both files looking right.

Requirement
    A sensor whose readings reach no conversion is refused, and so is one
    whose readings reach two, and one whose conversion is in another unit
    than the terminal it reports. The first has no encoding to borrow, the
    second has no single answer, and the third would have a zone's
    temperature read back as volts.

Requirement
    A value the format cannot hold is clamped and not wrapped. A wrapped
    count is a sensor reporting the other end of its range, which is a fault
    the vehicle has no way to tell from a reading.

**What a heater's simulated payload is**

Requirement
    A simulated payload that drives sends nothing. Being driven is the whole of
    what it is for, so a packet of its own would be bytes nothing reads.

Requirement
    Its packet interval is how often it looks for what the vehicle decided.
    It is the one thing pacing still means for a payload that sends nothing,
    and it is part of how promptly the loop responds: the vehicle cannot see
    a band crossed until its next sample, and the heaters cannot hear about
    it until they next look, so the overshoot is their heat over the sum of
    the two.

Requirement
    It acts on the last thing the vehicle said and not on the next thing in the
    queue. A decision is not a reading -- it says what the device should be now
    -- so a vehicle deciding faster than the simulated payload looks must not
    leave it acting on a decision that is older every pass.

Requirement
    It must be a kind of payload the simulator can be driven through: a
    socket, or discrete lines on ``gpio-sim``, whose simulated payload
    watches the lines the vehicle drives.

**Physically Realistic Noise**

A simulated sensor that reports the zone exactly agrees with the others to
the count, and a vote over sensors that always agree is never exercised.
Real readings go wrong in two places, and a loop treats them differently:
what goes wrong at a sensor is its own, and a vote can outvote it; what goes
wrong in the zone every sensor sees, and the heaters must work against it.

Requirement
    A sensor may state the noise on each reading, the accuracy its fixed
    calibration error is drawn within, a slow drift and its time, and its
    own lag, each as a datasheet gives it and in the unit of the terminal it
    reports, which is checked. The calibration error is drawn once a run,
    normally distributed with a third of the accuracy as its standard
    deviation and clamped to the accuracy, which is a bound most parts sit
    well inside.

Requirement
    A sensor may state how far the place it is on sits from what its
    terminal says, as its ``offset``: a body warmed in one place is not one
    temperature throughout.

Requirement
    A heater may state how its supply voltage varies, and how slowly.

Requirement
    A zone may state how its surroundings swing about the world's
    ``ambient_temp`` as a sine wave, and over what period, and how they
    wander at random, and how slowly.

Requirement
    Everything that wanders is an Ornstein-Uhlenbeck process stepped by its
    exact transition, so that its statistics do not depend on how often it
    is stepped, and starts where a settled one would be. The swing is in
    the zone's equation, whose closed form gains the swing's steady
    response, so it is advanced as exactly as the rest. A wander is held for
    an interval, which is exact for intervals much shorter than its time
    constant.

Requirement
    Each component and each sensor draws from a generator of its own, seeded
    from the run's seed and its name, so that a run with the same seed
    varies the same way whatever order the payloads' threads ask in. The
    seed is the ``tcssim`` section's; without one tcssim chooses it and
    says which.

Requirement
    Every one of these is off unless stated, so a file that states none
    simulates the zone and its sensors exactly.

**A Simulated Thermostat**

The loop closes through tcspecial when the heaters are joined: the sensors
report the zone, the vehicle decides, the heaters warm it. To watch the zone and
the heaters with no tcspecial running, tcssim can simulate the vehicle's
thermostat too. It is ``tcssim::node::vehicle::thermostat``, the one vehicle
node the simulator has: everything else a vehicle computes runs on a workstation
as it runs aboard.

Requirement
    The simulator file's ``tcssim`` section may name a thermostat node of
    the configuration. tcssim then simulates it, taking its ``setpoint``
    and ``half_band`` from the configuration, which states them once.

Requirement
    Its sensors are the simulated payloads reporting the world whose data
    reaches the node, and its heaters the simulated payloads driving it that
    data from the node reaches. A node that is not a thermostat node, or
    that no such sensor reaches, or that reaches no such heater, is refused.

Requirement
    It reads what each sensor's simulated payload sends, as it leaves: a dropped
    reading never arrives, and a truncated or corrupted one arrives as the
    vehicle would see it. The bytes are read as the conversion the sensor
    feeds reads them, ``Format::counts`` in tcslibgs being the one reading
    both use, and a reading that conversion would refuse is ignored.

Requirement
    It decides on the median of each sensor's latest reading, as the example
    votes them, and asserts the decision to every heater on every reading,
    as the thermostat node does aboard.

Requirement
    It is never named with tcspecial running, whose own thermostat switches
    the same heaters.

Fault Injection
---------------
A payload that always works exercises only the path where everything works.
The settings above have a simulated payload misbehave in the ways a real one
does, so that a handler's own counting, framing, timeouts and reconnection can
be made to happen on purpose rather than waited for.

Requirement
    Fault injection settings appear in the simulator configuration file only.
    A tcspecial configuration file describes the payloads a mission flies, and
    nothing flying is configured to drop a tenth of its packets; a fault
    stated there would also be read by tcspecial, which must be told what the
    payload is meant to do rather than what the simulator is doing to it.

Requirement
    A fault setting is inherited from a group exactly as every other setting
    is. A group of flaky payloads is as much a thing several payloads share as
    a rate is.

Requirement
    A percentage is nought to a hundred. A larger number is an error naming
    the setting and the value, because it is a misunderstanding of the unit
    rather than a request for a severer fault.

Requirement
    A dropped packet is not counted as sent. The simulator's counters and the
    handler's are what tell a dropped packet from a lost one, and counters
    that agreed about a packet that never existed could not.

Requirement
    A corrupted byte is never the byte it was, and a truncated packet is
    always shorter than its size and never empty. A fault that left the data
    as it found it would have a file asking for a tenth of its packets
    corrupted quietly get fewer. A packet of one byte is the exception to
    truncation: there is no length both shorter than one and not empty, so it
    is sent whole, an empty packet being the dropping fault instead.

Requirement
    Jitter is waited out before a packet is sent and inside its interval,
    never added to it. A payload told to send every second still sends every
    second, with the packet late within that second. Late and never early:
    early would be the simulator sending sooner than its own configuration
    asked for.

Requirement
    A payload that has gone silent keeps its link open. That is what makes it
    worth simulating separately from one that closed: a wedged instrument
    looks to the handler exactly like a healthy link carrying nothing.

Requirement
    A payload that hangs up listens again. The handler has to reconnect, which
    is the path being exercised, and a payload gone for the rest of the run
    would exercise it once at most.

Requirement
    The simulator refuses a fault the payload could not have. Only a stream
    has a connection to close, so ``close_after`` is refused for every other
    type; only a triggered payload is asked for anything, so
    ``ignore_trigger_percent`` is refused for a periodic one. Either would
    otherwise sit in the file looking as though it governed something.

Requirement
    A payload starting with any fault set says so once, in a line of log. A
    run whose payload was dropping a tenth of its packets should not have to
    be guessed at from the results afterwards.

Matching
--------
The two files are joined by name, not by position: the order of one file has no
bearing on the other, and a data handler's id is not a row number.

Requirement
    A simulated payload names a data handler of the tcspecial configuration
    file. A name matching no data handler is an error, because a line that
    matched nothing would otherwise be indistinguishable from a misspelled
    name.

Requirement
    Every data handler of the tcspecial configuration file has a simulated
    payload naming it. A handler nothing names is an error, because the
    simulator would otherwise start with a payload that never produces
    anything.

Requirement
    A name is defined once in a file. Two groups or two simulated payloads
    sharing a name is an error rather than one silently shadowing the other,
    and as in the payload configuration the rule reaches no further than the
    file: a simulator file names the payloads of the set it belongs to, and
    two sets may name their payloads alike.

Requirement
    A word that is not a setting is an error, in a payload, in a group, and in
    the file's own sections. A misspelled setting that was ignored would have
    the simulator do something other than what the file asked for and say
    nothing: a payload asked through a misspelling to drop a tenth of its
    packets drops none, and looks exactly like one that was never asked. A
    group is where such a word hides best, being read once and applying to
    every payload that names it.

Requirement
    A group is named by a simulated payload. A group no payload names is an
    error rather than a group with no effect, for the same reason a payload
    configuration's unnamed group is: it is also what a misspelled group name
    looks like. As there, the error is reported from the payload's end.

Example
-------
This is the shipped ``examples/thermostat/thermostat-sim.yaml``, a link to
``examples/thermostat-common/thermostat-sim.yaml``, which thermostat-yamcs's
simulator file is a link to too. It simulates the three sensors and the
two heaters' switches of ``thermostat.ytcs``, joined to its world: one zone
with two heaters of its own.

The three sensors are read together and so share a group, which carries
their rate and the terminal they report; the zone is what makes three
sensors agree about a temperature, each from a place of its own on it. Each
heater's switch drives the zone's ``heater_1`` or ``heater_2``, which adds
that heater's power to the zone's heat while the vehicle has it on. Every one
of them is reached over UDP at its ``sim_address``, where tcssim answers, so
no dummy device is needed. Shortened:

.. code-block:: yaml

   description: Simulator settings for the payloads of thermostat.ytcs and
     thermostat-yamcs.ytcs

   world:
     ambient_temp: "288.15 K"
     physics:
       components:
         - type thermal_mass:
             - component: zone
               capacity: "20 J/K"
               heater_1_power: "4 W"
               heater_2_power: "4 W"
               time_constant: "50 s"

   simulated_payload_groups:
     - group: zone_temps
       connects: zone.temperature
       packet_interval: 1 s
       simulate: true

   simulated_payloads:
     - type gpio_inout:
         - payload: gpio-out-tmass1
           connects: zone.heater_1
           simulate: true

         - payload: gpio-out-tmass2
           connects: zone.heater_2
           simulate: true

     - type i2c_tmass:
         - payload: temp_1
           group: zone_temps

         - payload: temp_2
           group: zone_temps
           offset: +0.2 K

     - type spi_tmass:
         - payload: temp_3
           group: zone_temps
           offset: -0.3 K

The tcssim program
==================
Tcssim is a GUI simulating the payloads. It reads two files. The payload
definition comes from the configuration, named by the ``SIM_PAYLOAD_CONFIG_PATH``
environment variable when that is set, and says which payloads exist, how each
is reached, and how big its packets are. What simulating them takes comes from
the simulator file beside it, named by ``PAYLOAD_SIM_YAML``; see `Simulator Configuration
Files`_.

Requirement
    A tcspecial configuration file describes payloads and says nothing about
    simulating them. How often a payload produces a packet, and how a packet is
    divided into segments, are properties of a simulation rather than of a
    payload, and are stated in the simulator configuration file.

The two are joined by name: a simulated payload names the data handler it stands
in for. Neither file is read in the order of the other.

Requirement
    Every simulated payload names a data handler of the tcspecial
    configuration. A name that matches none is reported rather than simulated
    around, because a misspelled name is otherwise indistinguishable from a
    payload deliberately left out.

Requirement
    A data handler the simulator file does not name is not simulated, and
    whether a run may have one is for the simulation mode to say: under
    ``all`` every payload must be a simulated or dummy device, and under
    ``none`` tcssim does
    not run. See `Simulation Modes <tcspecial-design.html#simulation-modes>`__. Every handler was once required to be
    named, which left no way to write a configuration that is part hardware
    and part simulation.

Which Types Can Be Simulated
----------------------------
A simulated payload has to be whatever is at the far end of its handler's
payload endpoint, and the types of endpoint are not equally easy to be. The
``can``, ``iio``, ``rpmsg`` and GPIO types are covered where each of them
is: a CAN bus of peers, a sensor's buffer, a channel to another core and a set
of discrete lines.

Requirement
    A synchronous line is the one type nothing can simulate at its own
    endpoint. Every other type has a dummy device in the kernel to be --
    ``vcan``, ``i2c-stub``, ``gpio-sim``, ``iio_dummy`` -- or a
    pseudo-terminal to be simulated on, and generic HDLC has nothing: there
    is no virtual HDLC interface, so such a payload needs a card. One whose
    node gives a ``sim_address`` is simulated over UDP there (see `Saying
    Which Payloads Are Simulated <tcspecial-design.html#saying-which-payloads-are-simulated>`__); one that gives none is refused where
    the simulator is asked to simulate it, naming the interface and what a
    card would be, rather than becoming a payload that looks simulated and
    is not.

Requirement
    A simulated payload of a network endpoint is a socket of the configured
    protocol, for all four of them. A stream payload listens; a datagram
    payload sends to where the handler waits.

The four divide in two, and which end binds divides with them. A TCP or
Unix-stream payload listens at the configured address and the handler connects
to it. A UDP or Unix-datagram payload that is read and not asked sends first,
so the handler binds that address and the payload reaches out to it; one that
is asked or driven is spoken to first, so it binds the address and the
handler reaches out.

Requirement
    A Unix-domain socket's address is a path, which outlives the process that
    bound it, so what is already at that path is looked at before binding. A
    socket nothing answers was left by a run that was killed rather than
    stopped and is replaced; a socket something answers belongs to something
    running and is left alone, with what is there reported; anything that is
    not a socket is left alone too.

Requirement
    A simulated Unix-datagram payload binds a path of its own, beside the
    handler's, so that the handler can answer it. A datagram socket with no
    path of its own cannot be sent to, where a UDP one gets an address for
    nothing from its port. The path is derived from the handler's rather than
    configured, since nothing need name it: the handler learns where its
    payload is from the first datagram, as it does over UDP.

Requirement
    A simulated payload of a device endpoint produces data and counts it.
    Tcspecial opens and reads the device itself, so there is nothing for the
    simulator to write to and the pacing is the whole of the behaviour.

Requirement
    A simulated payload of a serial endpoint is a pseudo-terminal. Its slave
    is a device file with a line discipline behind it, so the handler opens it,
    sets it to the rate and framing its configuration gives, and reads and
    writes it as it would a real port; the simulator holds the master.

A pty is a line in every respect but the one it cannot be: the kernel accepts
a data rate and ignores it, so bytes cross at memory speed whatever the
configuration says. A simulated line therefore exercises the handler's reading,
its writing and the terms it sets, and says nothing about whether the rate is
one the cable would carry.

Requirement
    The path the handler was told to open is made a symbolic link to the pty's
    slave for as long as the simulated payload runs, the slave's own name being
    the kernel's to choose. The link is removed when the payload stops.

Requirement
    A path that is anything other than a link the simulator itself made is left
    alone, and what is there is reported. A simulator that unlinked
    ``/dev/ttyS0`` to simulate it would be worse than one that could not,
    and a tcspecial configuration meant for simulation can name a path the
    simulator may create.

Requirement
    A simulated payload of an I2C endpoint is a chip of the kernel's
    ``i2c-stub``. That adapter's chips are a bank of registers in memory and
    every master on the bus reads and writes the same bank, so the bank is the
    payload: the simulator writes packets into it and the handler reads them
    out.

Nothing in user space can answer an address on a bus, which is why this is the
one type of simulated payload that is not the far end of anything. It has two
consequences worth stating.

The first is that a bus is a register and not a stream. Bytes do not queue: a
handler reads whatever is in the register now, so it may read one packet twice
or miss one altogether, depending on how its reading falls against the packet
interval. That is also true of a real sensor read over a bus, and is why a
payload link over a bus is a different thing from one over a line.

The second is that the simulator does not create the bus. ``i2c-stub`` is a
kernel module, loading it needs root, and the addresses its chips answer at are
fixed when it is loaded.

Requirement
    The simulator finds a stub bus by name among the kernel's adapters, since
    their numbers are assigned as they are found and a tcspecial configuration
    cannot know which number the stub will have. With none loaded, or with no
    chip at the configured address, what is reported is the command that would
    put it there.

Requirement
    A handler asks the adapter behind its bus what transfers it can do, and
    uses raw I2C where that is offered and SMBus block transfers to a single
    register where it is not. ``i2c-stub`` has no raw transfer at all -- a
    plain read of it fails -- so a handler that assumed raw transfers could
    talk to real hardware and never to a simulated bus.

A block transfer carries at most thirty-two bytes, so a segment of a packet on
a bus is capped at that: the packet size is a property of the payload and the
thirty-two is a property of SMBus, and the smaller of the two wins without the
file being refused.

Requirement
    A simulated payload of a SPI endpoint is a pseudo-terminal, as a serial
    one is, and carries a peripheral's bytes and none of its clocking.

This is the weakest of the simulated devices and the one place the simulator is
openly not the thing it simulates. Nothing emulates a SPI peripheral: a
peripheral is what a controller clocks, and nothing in user space can be
clocked. What a pty offers is the bytes -- a packet, in its segments, at its
pace -- and no clock, no mode, no chip select and no word width.

Requirement
    A handler takes a terminal at the end of a SPI path as a simulated
    peripheral: it carries bytes on it, says once in the log that it is doing
    so, and does not apply the configured clocking to it, there being nothing
    to apply it to. Anything else that is not a spidev node is refused.

The narrowness is the point. A terminal is what a simulated device is, and a
handler
that accepted any node it could open would open a wrongly configured path
confidently -- which is the fault the types were separated to prevent.

What such a link tests is a handler's reading and writing of a peripheral and
the pacing of a payload that produces data in blocks. What it cannot test is
whether the peripheral would stand being clocked that way, which is a question
for the peripheral.

So every type of endpoint, and every protocol of the network type, can be
simulated. The simulated devices are not all of one quality, and that is the
thing to keep in view rather than the coverage: a socket is a socket and a pty
is a line in all but its rate, while a bus is a register bank that every master
shares and a SPI peripheral is a pty with no clock behind it. Each type's own
file says which it is.

Requirement
    A handler opens a payload device with ``O_NOCTTY``. A device that is a
    terminal -- a real serial port, or a pty simulating one -- otherwise
    becomes the controlling terminal of the process that opened it, and then a
    read from a background process group stops the handler with ``SIGTTIN``
    and a hangup kills it with ``SIGHUP``, for reasons that have nothing to do
    with the payload.

The window is built from the two files. Nothing in the GUI names a payload or
fixes how many there are, so a payload added to or removed from the files adds
or removes a panel.

Each payload occupies a portion of the window, displaying its name, configuration,
and statistics. It also displays when a packet last went each way.

Requirement
    A panel shows the time of the last packet sent and the time of the last
    packet received, and the head of each. A count says how much has moved
    since the payload started, and only a time says whether any of it is
    moving now: a payload stopped ten minutes ago and one sending every
    second look identical by their counts a moment after either is looked at.
    The bytes are what say the traffic is the traffic that was expected
    rather than merely traffic.

Requirement
    The time is taken when the packet moves, not when the window next looks.
    A time stamped by the refresh would creep forward on a payload that had
    stopped sending, which is the fault tcsmoc's samples avoid for the same
    reason.

Requirement
    A payload counts a packet and times it in one operation, so a count
    cannot move without the time moving with it.

Requirement
    A direction nothing has gone in shows ``--:--:--``, as tcsmoc's panels do.
    A payload that has sent nothing has no time to show, which is a different
    thing from one that sent something at midnight.

Requirement
    A panel shows a line for a direction only where that direction can carry
    something. A simulated payload that sends of its own accord is never
    spoken to, so tcssim shows it no received line; a handler whose payload
    sends of its own accord is never asked anything, so tcsmoc shows it no
    sent line. Either would otherwise read the same from the first packet of
    a run to the last, which is a row of a panel spent saying that nothing
    can happen there.

Requirement
    The line stays and the items on it go. Every panel is then the same shape,
    and the rows below do not move from one panel to the next -- a grid of
    ragged panels is harder to read across than a grid with a blank line in
    it.

Requirement
    Every panel of a row is the same width, whatever type of payload it is
    for.

The items go by their text going empty rather than by each of them being
conditional, and that is the whole of why. A layout whose children come and go
is measured differently from one whose children are always there: with the
three texts of tcsmoc's sent line behind an ``if``, the two payloads of set 2
came out 196 and 458 pixels wide -- the triggered one taking every pixel of
slack in the row and the other sitting at its minimum, which says nothing
about either payload and reads as a window that has gone wrong. Empty text
draws nothing, so the line still shows no label and no reading. A test
measures the widths of a mixed row and of a row of each type alone.

Requirement
    A payload keeps the same sample of a transfer that a data handler keeps:
    the time, the head of the data, and the whole length. The two programs
    show the one transfer from opposite ends of a link, so a payload keeping
    something else would have the two panels disagree about what moved.

Requirement
    Both programs show a time and a sample the same way: the time of day, in
    UTC, with the date and the fraction of a second dropped, and the bytes as
    hex pairs with an ellipsis where the transfer was longer than the sample.
    Tcsmoc's panels and tcssim's are read side by side while a link is being
    watched, so one of them showing local time would make a transfer look an
    hour old, and a byte written differently would look like a difference in
    the data. The formatting is one function both call.

The packet size and interval can be changed, as can the segment size and
interval. Each starts at what the files gave it: the packet size from the
tcspecial configuration, the rest from the simulator file.

Requirement
    A segment is a piece of a packet, so a segment larger than the packet is
    refused. Equal is the ordinary case -- a packet sent in one segment, which
    is what a simulator file stating no segment size asks for -- so only a
    segment strictly larger than the packet is wrong.

Requirement
    A pair of sizes that cannot be sent is said at once and the button that
    would start the payload is taken away until they can be. Either alone
    would be worse: a message with the button still there invites a press that
    does something nobody asked for, and a button that had quietly stopped
    working would leave someone pressing it and wondering.

Requirement
    What is said names both sizes and both ways out of it. The two spin boxes
    are side by side and either can be the one that was meant to change, so a
    message naming only one of them answers half the question.

Requirement
    The message is taken back by the edit that makes the sizes sendable
    again. An open popup still saying the sizes cannot be sent, after they
    can, is the wrong answer left on the screen.

Requirement
    The two sizes are checked when the files are read, not only when a spin box
    is touched. A simulator file may state a segment larger than the packet the
    tcspecial configuration gives, and a panel that said nothing until it was
    edited would have the payload refuse to start for no stated reason.

Requirement
    Only starting is refused. A payload already sending when its sizes stop
    agreeing offers ``Disable`` as ever, and a press of it stops the
    payload. The button was taken away whatever the payload was doing, so a
    payload running when its sizes went wrong could not be stopped until
    they were put right, and went on sending them meanwhile -- the one
    thing the refusal is there to prevent.

Requirement
    A payload that ``--sending`` or its simulator file's ``enable`` would
    start is not started while its files give sizes that cannot be sent.
    It is started as though its ``Enable`` had been pressed, and that press
    would be refused; tcssim says so as it starts, and the panel says why.

Requirement
    The window decides neither. It asks for the two sizes to be judged and
    obeys the answer -- showing what it says and disabling the button that
    starts the payload while it says anything -- so there is one rule
    rather than one in each place that acts on it. Rust holds a press to
    the same answer, so an ``Enable`` that reached it while the sizes cannot
    be sent starts nothing.

Requirement
    The popup that says the sizes cannot be sent is as tall as what it
    says, and hangs under the panel's buttons, so that it hides neither the
    two sizes nor whether the payload can be started. It was a fixed size
    that the sentence ran out of, hung over the spin box just stepped.

Each panel has two buttons: one that starts and stops the payload, and one
that shows the whole of what the two files said about it.

Requirement
    The button that starts and stops a payload is labelled with what pressing
    it will do. A payload that is sending offers ``Disable``, which stops it;
    a payload that is not offers ``Enable``, which starts it. There were two
    buttons, Start and Stop, and either was pressable whatever the payload was
    doing: a Stop on a payload that was not sending did nothing and read as
    though it had.

Requirement
    One button here where the MOC's panels have two, because a press in this
    window is the act itself: it starts or stops a payload in this same
    program, so it cannot be lost and there is never a reason to make it
    twice. The status the label is built from is this program's own knowledge
    of its own payload, so the press it offers is the press that is wanted.
    The MOC presses across a link to a spacecraft, where a command may not
    arrive and may have to be sent again; see the MOC's panels for why that
    takes two buttons.

Requirement
    Both windows label those two presses the same way. ``Enable`` and
    ``Disable`` in the MOC's panels and in the simulator's, so that one state
    is said one way in the two windows an operator reads side by side. The
    simulator said ``Send`` and ``Silent`` -- the payload's own words, the
    payload being the thing that sends -- which read well in one window alone
    and left the reader of both translating.

Requirement
    Which way a press goes is decided by the status the label was made from.
    The window builds the label and the program decides what to do, so the two
    are checked against each other: a button that offered one thing and did
    the other would be worse than either button alone.

Requirement
    A payload starts disabled unless its simulator file's ``enable`` is
    ``true``, its own or its group's, a payload's own overriding the group's:
    ``enable: true`` in YAML and ``<enable value="true"/>`` in XML. Then it
    starts as though its ``Enable`` had been pressed. Disabled is the
    default, so that a simulator never starts sending what nobody asked for,
    and a run that wants a payload sending from the start says which in the
    file it is run with rather than in a flag for every payload at once.
    ``--sending`` still starts every payload. ``enable`` is the simulator's
    alone: tcspecial never reads the simulator file, and a configuration
    that flies says nothing of what a simulator does.

Requirement
    A payload that failed to start offers ``Enable`` again. It is not
    sending, which is what its status says, and the useful thing to offer is
    the start that failed rather than a stop of something that never began.

Requirement
    For a part the vehicle drives -- a heater -- the same button is the
    part's power. Disabled, it is unpowered, or has failed, and stays off
    whatever the vehicle or a thermostat tcssim simulates decides: it
    gives the world nothing. What it is sent meanwhile is kept, the latest
    replacing the one before, and ``Enable`` applies that at once, because
    the vehicle sends a decision only when it changes and a heater waiting
    for the next one would sit cold through a whole band. One button and not
    a second control: a heater sends nothing, so its button had nothing else
    to mean. Nothing aboard changes, and the ground sees a disabled heater
    only through the temperature, which is the point of being able to
    disable one.

Requirement
    A heater's panel says what it is doing in words -- ``heating``,
    ``off``, or ``disabled, so off`` -- on the line where another payload's
    says what it last sent, a heater sending nothing. Words, so that the
    state is never told by a colour alone.

Requirement
    The words follow the name of the heater the panel switches, so that no
    two panels say the same thing of different heaters. A switch joined to
    a ``heater`` component's ``on`` names the component, ``heater_1:
    heating``; one joined to a thermal mass's own ``heater_1`` or
    ``heater_2`` names the mass and the input, ``zone.heater_2: off``. The
    thermostat example's two switches both named the zone they are joined
    to, and said ``on`` rather than ``heating``, so that its two heaters'
    panels read ``zone: on`` alike.

Requirement
    A row of text in a panel is the height of its text, which is the window's
    type. Rows told a height of their own, fourteen and eighteen pixels in
    type of twelve points, cut off every descender and underscore --
    ``temp_1`` read ``temp 1`` -- and left the packet count half under the
    next row of panels.

Requirement
    Each panel has a button that shows every parameter read from the two
    configuration files for that payload, in both programs. What a panel has
    room for is a name, a size and what has moved; a payload set says a good
    deal more than that -- a trigger, a line rate, a slave address, a clock
    mode, a fault -- and until this there was nowhere to see it but the files
    themselves, which is no use for telling what the program actually read.

Requirement
    That button is labelled ``Configuration`` in both programs and sits to the
    right of the button that starts and stops the payload. The press a panel
    is opened to make comes first, and one word for one button means an
    operator reading the two windows side by side is not told that a ``Config``
    and a ``Configuration`` are the same thing. The MOC's was abbreviated while
    its panel was narrower than the word; the panel is wider now, which is
    what that cost.

Requirement
    Both halves are shown and each is said to come from the file it came from.
    One is flight configuration, which tcspecial serves and the ground
    controls; the other is how a simulated payload behaves and reaches no
    spacecraft at
    all. A list that mixed them would have someone reading a drop percentage
    as something a payload does.

Requirement
    Nothing shown is a measurement. What a payload has done is on the panel
    itself, and a list that mixed the two would leave no way to tell a
    configured value from an observed one.

Requirement
    What it shows scrolls, so the foot of a list longer than the popup --
    where the faults are -- can be reached.

Requirement
    Only the attributes of the payload's own type are shown, and only the
    faults that were asked for. A device has no protocol and a bus no port;
    six faults of nought say less about a payload than the one that was set.

Requirement
    A value the files did not state is shown as what the program will use
    instead, said to be that rather than passed off as a statement. A handler
    with no OC address says it cannot be started, since that is what the
    absence means.

Requirement
    Both programs describe a payload with one function. They show the same
    payload set from opposite ends of a link, so two descriptions would be two
    things to keep in step and a difference between them would read as a
    difference in the configuration.

Requirement
    Tcsmoc reads the simulator configuration to show it, and does not require
    it. The MOC controls payloads and simulates nothing, so a simulator file
    that is missing, unreadable, or written for another tcspecial configuration
    costs it that half of what a panel can show and nothing else; it says so
    once on the way past, and the panel says so where the parameters are shown.

The panels are laid out in a grid whose shape follows from how many there are.
A row is filled before another is started under it, three panels across, which
is the rule tcsmoc follows and for the same reasons given there.

It replaced one that kept the window wider than tall and chose the shape
nearest square among those that were. The claim here used to be that the two
rules came to the same thing for the counts the shipped sets have. They did
not: five payloads went into two columns and three rows, because a 680x620
window is squarer than a 1062x476 one, so the set went down the window instead
of across it. Squareness was never the thing wanted either -- it was a proxy
for a window that fits a screen, and a column count says that directly.

How many columns there are is worked out in the window rather than handed over
by Rust, from the width the window has at the time, so dragging a window wider
reflows the grid into the room that appeared. Rust opens the window wide enough
for the columns the payload count calls for, and because it opens it at exactly
that many panels' width plus the chrome, the window arrives at the same count.

The five sources of the thermostat example give a row of three and a row of two in a
1206x764 window. The size is the one ``window_size`` gives, which a test reads
out of this document so that the number here cannot quietly stop being the one
the program opens at.

See Also
========

`The TCSpecial design documents <design.html>`__
    What each of TCSpecial's design documents covers.

`The tcspecial design <tcspecial-design.html>`__
    The flight software and what the programs share: the node architecture,
    the link, fault resiliency and the requirements.

`The tcssim manual <tcssim.html>`__
    Using tcssim and writing its configuration.

