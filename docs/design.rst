================
TCSpecial Design
================
.. contents:: Table of Contents
   :depth: 4
   :local:

Introduction
============

.. note::
   TCSpecial has been designed for spacecraft and ground communication with high
   latency communication links. It is just as applicable for other environments,
   such as submersibles, drones, etc. Simply translate "spacecraft" to your
   device type.

What there is, in the order a reader meets it:

``tcspecial``
    The flight software. It serves the payloads its configuration file
    describes, takes commands from the ground, and sends beacons.

``tcslibgs``
    What both ends share: the configuration file formats and the rules about
    them, the command and telemetry messages, and the digest and version each
    end says it read. Every rule in this document is stated there once.

``tcslib``
    What a control application on the ground is built on. A mission control
    system drives a client rather than speaking the protocol itself.

``tcsmoc``
    A simulated operations center, for testing: a window of panels, one per
    payload, that commands them and shows what they have moved.

``tcssim``
    Simulated payloads, for testing: it stands in for the far end of every
    link a payload set describes.

``tcsverify``
    Reads a payload set and says what is wrong with it, running none of it.
    See `tcsverify`_.

The ranked quality attributes
-----------------------------

Four attributes, in order. The order is the point: where two designs satisfy
the requirements differently, the earlier attribute decides, and an argument
about such a choice should cite this list rather than taste.

1. **Reliability.** The vehicle is not reachable for repair. A mechanism that
   can fail in a new way loses to a rigid one that cannot, and a fault found
   when a configuration is loaded beats the same fault found in flight.
2. **Ease of use.** An operator under time pressure must be able to read what
   the system does and say what it will do next. Clarity of the configuration
   and of what the programs report counts as a requirement, not a courtesy.
3. **Maintainability.** The software outlives the people who wrote it. One
   concern per module, a test per claim, and a written reason for anything that
   is not obvious.
4. **Flexibility and extensibility.** New interfaces, new computations and new
   vehicle arrangements must be reachable without disturbing what already
   flies -- but never at the cost of the three above.

Terminology
===========

These words are used in one sense each, here and in the code. Several of them
had been used in two, which is why they are written down: "device" in
particular had been both a type of endpoint and a protocol beside TCP and UDP.

endpoint
    One end of the path a data handler's data travels. A handler has two: the
    OC endpoint, where the ground's data arrives and leaves, and the payload
    endpoint, where the payload's does. An endpoint is of exactly one type, and
    the type decides what is needed to open it and what terms it is operated
    on.

endpoint type
    What an endpoint is made of, and so what locates it and what else must be
    said about it. Five are supported -- ``network``, ``device-file``,
    ``serial``, ``i2c`` and ``spi`` -- and five more are experimental:
    ``can``, ``iio``, ``rpmsg``, ``gpio`` and ``hdlc``. A type is not a protocol -- a network endpoint
    carries the protocol it runs over, and no other type is addressed that
    way.

device
    An endpoint that is a device file, opened and read as it comes. Not the
    general word for hardware: a serial line, an I2C device and a SPI
    peripheral are all reached through device files and are none of them a
    device endpoint, because each has terms of its own that a plain device has
    not. A device *file* is the thing in the filesystem; a device *endpoint* is
    one type among the nine.

bus
    A device that several devices are reached through, each by an address on
    it. I2C is the one in use. The bus is named by a device file and the device
    on it by an address, which is why an endpoint on a bus carries both.

payload
    What is at the far end of a payload endpoint: the instrument, recorder or
    radio whose data a handler carries. Also what tcssim stands in for, and
    what the payload configuration describes.

data handler
    The pair of conduits between one payload endpoint and one OC endpoint,
    named by a ``name``, which is what every command addresses it by.
    "Handler" alone always means this.

conduit
    One direction of one data handler: a thread that reads one endpoint and
    writes the other. A handler has two, and they share each endpoint between
    them.

OC
    The Operations Center: the ground. The OC endpoint of every handler is a
    UDP address, and the OC's own address is learnt from what it sends rather
    than configured.

link
    A configured pairing of two endpoints that data passes between. Used of
    the whole path, where "endpoint" is used of one end of it.

tcspecial configuration file
    The file tcspecial is configured by: the command interpreter's own
    settings in its ``tcspecial`` section, and the payload configuration
    section beside them. No name of its own -- any name ending ``.yaml`` or
    ``.xml`` -- the shipped examples being ``tcspecial1.yaml`` and the rest.

payload configuration section
    The part of that file describing payloads: which data handlers there are,
    how tcspecial reaches each one, and how large its packets are. A section
    rather than a file, because what tcspecial serves and what tcspecial is
    are one configuration.

simulator configuration file
    The file describing how a simulated payload behaves: how often it produces
    a packet and how it divides one into segments. Properties of a simulation
    rather than of a payload, which is why they are not in the payload
    configuration.

The Node Architecture
=====================

What this supersedes
--------------------

TCSpecial began as a set of data handlers, each one a fixed pair: a payload
endpoint at one end, an OC endpoint at the other, and a conduit in each
direction between them. ``dh.rs`` says so in its own fields -- one
``ground_to_payload`` and one ``payload_to_ground`` -- and that is the whole of
the topology the software could express. It served while every payload's data
went to the ground and nowhere else.

It cannot express a control loop. Three sensors whose readings are averaged and
compared against a setpoint, with the comparison driving heaters, needs a
reading to reach two places at once, needs three readings to arrive at one
place, needs something aboard that remembers a setpoint, and needs a set of
discrete lines to be driven rather than read. None of those is a type of
endpoint; all of them are shapes of connection.

So the fixed pair is replaced by a graph the configuration describes. A data
handler is no longer a thing: what it did -- joining a device to the ground in
both directions -- is two links, and the software that moved its bytes is the
machinery that moves bytes along any link.

The four concepts
-----------------

Deliberately four, and no more. Each is a thing an engineer already thinks
about, which is the ease-of-use argument; each is checkable before anything
runs, which is the reliability one.

node
    Anything the configuration names. A node has a name unique across the
    configuration, a type, and the attributes its type requires. Three families
    of type:

    *devices* -- ``i2c``, ``spi``, ``serial``, ``hdlc``, ``can``, ``gpio``,
    ``iio``, ``rpmsg``, ``network``, ``device-file`` -- which are the interfaces
    real
    hardware arrives on. Each is what used to be called a payload endpoint, and
    the attributes are unchanged: a bus and an address, a chip and a set of
    lines, a path.

    *MOC links* -- ``moc`` -- a network link carrying telemetry to the ground
    and commands back. This is what the OC endpoint was.

    *compute elements* -- ``compute`` -- software that transforms what arrives.

    *sinks* -- ``log`` -- somewhere data is kept rather than passed on. See
    `The logging node`_.

group
    A set of devices read on one interval, so that readings which will be
    compared with one another are taken as close to together as the hardware
    allows. A device in no group is read on its own interval, as every payload
    was.

    The interval belongs to the group and not to its members. A member may not
    state one: two sensors in a group with intervals of their own would be a
    configuration whose own description disagrees with itself, and it is
    refused.

link
    A directed connection from one node to one or more destinations. One
    destination is the ordinary case. Several is how a reading reaches the
    ground and an on-board computation at once -- the arrangement usually drawn
    as a "T" -- and it is a property of the link rather than a node of its own,
    because a node that only copied its input would be a type to implement,
    test, configure and get wrong for no gain.

    Where a destination takes more than one input, the link names which: a mean
    over three sensors is three links into three named inputs, and a
    configuration that leaves one unbound is refused rather than averaging two
    numbers and a zero.

command binding
    A statement that a named command from the MOC sets a named attribute of a
    node. A setpoint arrives this way. The binding is in the configuration, so
    what the ground is able to change in flight is written down and checkable,
    rather than being whatever the compute element happens to accept.

What a device node says
-----------------------

A node of an interface type says where it is, what the link is set up as, and
how it is read. The second of those is unchanged from version 1 -- a data rate,
a bus address, a clock mode, a set of line numbers -- and is deliberately so:
those rules are the ones that have been tested against real buses, and a second
set of them for a second schema would be two statements of every rule drifting
apart one flight at a time. A version 2 node's attributes are handed to the
same per-type rules, which refuse in the same words.

What locates a node depends on its type:

.. code-block:: text

   network, moc    address, port, and for network the protocol
   i2c             path (the bus) and address (the device on it)
   can             interface and can_id
   everything else path

And every interface type may say, beyond the terms of its own link:

.. code-block:: text

   group           which group reads it, which the schedule reads
   read_size       how many bytes one read asks for
   trigger         what to write to make it answer
   max_length      where one read of a line ends: the three attributes a
   read_timeout    version 1 file nests under "stream". A node's attributes
   terminators     are flat, so they are written beside the line's terms

Requirement
    An attribute nothing reads is refused, by name, for the type it is written
    on. A term belonging to another type is refused by that type's own rules;
    a term belonging to no type at all would otherwise vanish, and
    ``max_speeed`` ignored is a peripheral clocked at whatever the last
    program left the bus at, which looks exactly like a configured one.

Requirement
    An attribute that locates another type is refused. A port on a node
    reached through a device file is a file that looks configured and is not.

Requirement
    ``read_size`` bounds a read, and the buffer offered bounds it too: the
    smaller wins. A bus master clocks exactly as many bytes as it asks for, so
    a sensor that answers in two bytes read sixty-four at a time is a sensor
    clocked sixty-two bytes past the end of what it has to say.

Requirement
    A node with a ``trigger`` is a node that is asked, and there is no
    separate word for the mode. A trigger and nothing else is what made a
    version 1 payload triggered, so a second word could only disagree with it.

Requirement
    A port of ``0`` is refused. It is the one number a socket would accept and
    a configuration cannot mean -- it asks the kernel for whichever port is
    free, so a configuration stating it describes a link nothing could be told
    where to find.

Requirement
    A ``moc`` node needs no protocol and a ``network`` node does. A ground link
    is a datagram link, as the one tcspecial has always had is; a device
    reached over a socket could be any of the four, and guessing which is how a
    stream payload comes to be addressed as datagrams.

The logging node
----------------

Keyword ``type: log`` records what reaches it, built on ``tcslog`` -- the same library
the telemetry log already uses, so a log node is a generalisation of something
the software does rather than a new mechanism to trust. Two attributes:

.. code-block:: text

   directory      where the log's segment files are written
   format         fixed: <n> | variable_simple | variable_ts_rc

It is the first node with no fixed number of inputs. Any number of sources may
link into one log, which is the point of it: a flight's housekeeping is one log
holding what several devices produced, and a reader of that log wants the
records interleaved as they happened rather than a file per sensor. A node with
named inputs, like a mean over three sensors, is the other case and is checked
the other way.

What "any number" does not mean is any *type*. A source is accepted only if its
records fit the format the log was opened with, and the three formats accept
different things:

``fixed: <n>``
    Every record holds exactly ``n`` bytes and no data header is written, which
    is the most compact log and the one whose length cannot change for the life
    of the log. A source is compatible only if every record it produces is
    exactly ``n`` bytes -- a device read in fixed-size packets, a computation
    with a fixed-width result. A source whose records vary is refused, naming
    the source, the log and the two lengths.

``variable_simple``
    Records of nought to ``tcslog::RecSize::MAX`` bytes, each with a four-byte
    length header. Any source whose largest record fits is compatible. Suited
    to data that already carries its own timestamp.

``variable_ts_rc``
    As ``variable_simple``, and the header also carries the time the record was
    written and its position in the session. The format to choose when the
    question asked of the log later will be *when*, which for a control loop it
    usually is.

The compatibility is settled when the configuration is loaded, not when a
record is written. A log that would have to refuse a record in flight is a log
that loses telemetry at the moment telemetry is worth most, so the arrangement
is refused on the ground instead: either every source fits the format or the
configuration does not load.

A log with no source links into it records nothing, and is named at load for the
same reason a node with no outbound link is -- a configuration that cannot do
anything is more likely a mistake than an intention.

Requirement
    A fixed-length log takes only sources whose record length the
    configuration states, which means a device with a ``read_size``. A
    computation's result is a number written out as text and its width depends
    on the number, so nothing in the file could say otherwise -- and a source
    whose length is not knowable is refused with that reason rather than with a
    number it does not have. The two variable formats take any source.

Requirement
    A log is never read, and no link may leave one. It is where data stops; a
    link out of a log would be a log read back into the vehicle that wrote it.

Requirement
    Segment files are named for the node: ``log-<node>-`` and the sequence
    ``tcslog`` gives them. Two logs may write into one directory -- a
    housekeeping log and a log of one noisy instrument, say -- and a reader of
    that directory has to be able to tell which files are which.

Requirement
    A log node may say ``segment_bytes`` and need not. The default is the
    telemetry log's, because a log node whose segment size differed from it for
    no stated reason would be two answers to one question; a segment smaller
    than one record is refused, being a log that would fill a file per record
    and then fail on the one that would not fit.

Requirement
    The directory must already exist. A log node is not the thing to be
    creating directories on a vehicle: a mistyped path created would become a
    log nobody is looking in, where a mistyped path refused is a configuration
    somebody fixes.

Requirement
    A record is not flushed as it is written. A log flushed per arrival is a
    write system call per reading, and the records are in the kernel's hands
    either way; what is flushed is a pass, and anything the log should be on
    the far side of.

Requirement
    A write that fails is reported and counted, not swallowed. The fabric
    attempts each destination of a link independently, so a log that cannot be
    written does not stop the ground getting its telemetry -- which is exactly
    why the failure must be visible: a silent one would be a log that stopped
    recording with everything else still working.

.. code-block:: yaml

   nodes:
     - { name: housekeeping, type: log, directory: /var/log/tcspecial,
         format: variable_ts_rc }

   links:
     - { from: k_fore, to: [moc, mean_temp.in0, housekeeping] }
     - { from: k_aft, to: [moc, mean_temp.in1, housekeeping] }
     - { from: k_bay, to: [moc, mean_temp.in2, housekeeping] }
     - { from: thermostat, to: [heaters, housekeeping] }

That is the thermostat's three converted readings and what the comparison
decided, in one log, in the order they happened -- which is what reconstructing
the loop's behaviour afterwards requires, and what a log per sensor could not
give.

Compute elements
----------------

Two of them, and the difference is where the code lives.

A **built-in** names a function compiled into tcspecial: ``to_kelvin``,
``mean``, ``hysteresis`` and the rest of a catalogue that grows as flights need
it. These are the ones to prefer. They are written in Rust, tested with the
rest of the software, cannot fail to start, and cost nothing to call.

An **external process** names a program. tcspecial starts it, writes what
arrives on its standard input and reads results from its standard output. It
buys a computation that can be written in anything and changed without
rebuilding the flight software, which is why it is here; it costs a process to
supervise, and that is a new way to fail. Reliability outranks the flexibility,
so the arrangement is deliberately strict:

* the program is started when the configuration is loaded, not at the first
  datum, so a program that cannot be executed is a configuration fault found on
  the ground.
* every result has a deadline. A program that has not answered within its
  ``timeout`` is treated as having failed, not waited for.
* a failed or exited program does not silently swallow data. Its link is
  **refused**: what would have gone into it is dropped, counted, and reported
  in telemetry, so the ground sees a computation stop rather than seeing
  plausible numbers that are stale.
* restarting is bounded and declared. ``restart: never`` is the default for a
  node in a control path, because a heater driven by a computation that
  restarts under load is worse than a heater driven by nothing.
* a built-in and an external process are interchangeable in the graph, so a
  computation can be proved as a program and later moved into the catalogue
  without the configuration changing shape.

A node that runs a program names it, says how long it may take, and says
whether it may be restarted:

.. code-block:: yaml

   - { name: attitude, type: compute, function: process,
       program: /opt/flight/attitude, timeout: 500ms, restart: never }

What the program sees is a line per arrival on its standard input and a line
per answer on its standard output:

.. code-block:: text

   in:  293.15          an arrival on a link that names no input
   in:  in1=293.15      an arrival on a link that names one
   out: 291.0           a result, which is passed on
   out:                 an empty line: nothing to send yet

Requirement
    The program is handed lines and answers in lines. A value on a link is
    decimal text, and a line is what every language reads without a library --
    which is the point of this type of node existing at all.

Requirement
    An empty line is the program saying "nothing to send yet", and is the same
    answer a built-in gives by handing back nothing. A computation over several
    inputs has to be able to say it: an external mean over three sensors
    answers on the third arrival and not before, and a program that answered
    something on every line would have to invent a value twice per pass.

Requirement
    A program that misses its deadline is put down, not left running. It still
    holds the datum it was given, so a second datum handed to it would be
    answered with the first one's result -- a control loop reading last pass's
    number as this pass's.

Requirement
    A datum that went into a program which then failed is refused, including
    when a restart allowance paid for the program to be started again. It went
    into a computation that died without answering, and saying otherwise would
    be inventing a result.

Requirement
    A restart is taken at once, in the pass that spent it, rather than at the
    next datum. A vehicle is better off learning that the program will not
    start again while something is still watching the pass that killed it.

Requirement
    A program is killed and waited for when its node goes. A child that is
    killed and not waited for is a zombie, and a vehicle that restarted a
    computation a few times would collect one per restart; a vehicle that
    stopped and left one running would leave the next start to find a program
    already holding whatever the last one held.

Requirement
    Standard error is left alone. Whatever the program complains about arrives
    wherever tcspecial's own complaints do, which is where somebody reading a
    vehicle's log is already looking.

Requirement
    A child inherits nothing but its two pipes and the standard error it
    shares. Everything this software opens, it opens close-on-exec, so a
    computation starts without the command socket, without the chip another
    node holds, and without the log's segment file -- none of which it has
    any business with, and one of which, a chip's line request, is given to a
    single owner at a time. Sweeping the descriptors closed in the child by
    hand was tried and taken out again: the descriptor the standard library
    reports a failed exec on is one such a sweep closes, and with it closed a
    program that cannot be run looks like a program that started.

Requirement
    A blocking receive that a signal interrupts is waited on again, not
    treated as a failure. This matters because of the node above: a computation
    exiting is enough to interrupt a receive elsewhere in the process, and the
    two command loops used to return on it -- which would have stopped the link
    a spacecraft is commanded over because a program on it had finished.

Running a graph
---------------

Four pieces, and the first three open nothing. That is on purpose: they are the
arithmetic of the graph, they can be tested against a configuration with no
vehicle present, and a fault in any of them would otherwise look like a fault
in a device.

routing
    The links, turned into a table once at startup: for each node, where its
    output goes, and which link fills each of its named inputs. A link is
    written the way a person wants to write one -- from a node to one or more
    destinations -- and that is not the shape running software wants, which is
    "this node has produced bytes, where do they go". Answering that by walking
    every link would be a scan per datum.

schedule
    When each group is next read. A group's clock runs from the time a read was
    *due* and not from the time it happened, so a pass that ran late does not
    push the next one later -- a reading every three seconds stays every three
    seconds rather than drifting by the cost of the work.

fabric
    What holds the nodes and moves the bytes, using the other two. Delivery is
    attempted per destination and the failures are counted rather than raised:
    a log that cannot be written must not stop the ground getting its
    telemetry, and a heater that will not take a command must not stop the next
    reading being taken. What must never happen is a failure nobody hears
    about, so every one is counted and named.

ports
    The piece that does open something: an endpoint, presented as a node the
    fabric can move bytes to and from. One type for every interface rather than
    one per interface -- every endpoint already answers the same two traits, and
    a port per type would be nine copies of one delegation with nine places to
    drift apart. A computation is a port of its own, holding what a function
    produced until the fabric takes it.

Requirement
    A delivery that produces a result carries on from there, in the same pass.
    Three sensors read on one interval must become three temperatures, an
    average, a decision and a command to the heaters before the pass is over;
    a fabric that stopped at the first delivery would leave a result sitting in
    a computation until something else happened to read it, and a control loop
    whose steps happen on different passes is a control loop whose delay
    depends on the shape of the graph.

Requirement
    A node is asked whether it has a result rather than simply read after a
    delivery. The two are different things: a sensor has a reading when its
    group is due and a delivery does not change that, while a computation has
    one *because* something arrived. Reading every destination instead would
    cost a device read per delivery, and would take a command off a MOC link
    and pass it on as though it were a result.

Requirement
    Delivery is breadth first. The three destinations of a "T" are all reached
    before any of their own destinations are, so the ground has a reading
    before an on-board computation over it begins -- the order an operator
    watching telemetry expects, and the order that loses least when a later
    step fails.

Requirement
    A result is passed on at most as far as there are nodes, and a cascade that
    goes further is stopped and reported as a cycle. A path through a graph
    with no cycle visits each node at most once. The links are checked for
    cycles before any of this runs, so this is the backstop for a check that
    was wrong rather than the rule itself -- and what it must not do is loop: a
    fabric going round a cycle is a vehicle that has stopped reading its
    sensors, with no telemetry to say why.

Requirement
    A computation that produces nothing produces no deliveries, and that is not
    a failure. A function waiting for the rest of its inputs and one that
    decided nothing needs to change give the same answer, and a relay
    commanded to the state it is already in every three seconds is a relay
    nobody could tell had stopped answering.

Requirement
    A result too large for the buffer it is asked for is reported and kept, not
    cut to fit. Half of a number is a different number, and keeping it leaves
    it there for a caller with room.

Requirement
    What a compute node may say is checked by the same code in both places,
    and it lives in the shared library rather than in the flight software.
    ``tcsverify`` reads a configuration on a desk and cannot check what it
    cannot see: these rules used to sit beside the arithmetic, inside
    tcspecial, so a thermostat with no band or a program with no deadline
    passed the ground check in silence and was refused when the vehicle
    started. That is the wrong end of the trip. The arithmetic stays where it
    was; what moved is the reading of attributes.

Requirement
    A command from the ground has to get past two gates, in this order. The
    configuration must have a binding saying that this attribute of this node
    may be set -- an attribute no binding mentions cannot be set, which is what
    makes what the ground can change in flight something written down and
    checkable rather than whatever a computation happens to accept. The node
    must then accept the value, which is where a setpoint that is not a number,
    or a band of nought, is refused.

Requirement
    A device accepts no settings. A bus address or a set of line numbers is
    what the configuration said, and a vehicle whose hardware could be
    re-described from the ground in flight would have no checked configuration
    at all. What the ground changes is a parameter of a computation.

Requirement
    A refused command changes nothing. The value is read and checked before
    anything is written, so a thermostat whose new setpoint was rejected goes
    on running against the one it had rather than against half of a number.

Requirement
    A command does not act on its own. Setting a setpoint does not recompute
    the decision; the next reading does, one interval away at most. Acting
    immediately would switch the heaters on the strength of a number rather
    than of a temperature.

Requirement
    One pass is: every group that is due, read in full and in order; then the
    nodes in no group, polled; then the logs flushed. A group is read in full
    before anything else happens because that is what a group is for, and each
    reading is delivered as it is taken -- so a converted reading reaches the
    ground, the mean and the log before the next sensor is read, and the
    decision reaches the heaters inside the pass that read the sensors.

Requirement
    A pass asks a descriptor whether there is anything before reading it, with
    no waiting at all, and a node with nothing to say is a read of no bytes.
    Some endpoints are opened blocking -- a device file is opened as a file --
    so a read of one with nothing in it does not return, and a pass that
    blocked there would never end: the group that was due unfinished, every
    other group starved, and a stop unnoticed, a stop being noticed between
    passes.

Requirement
    A node is polled only if it is in no group, its output goes somewhere, and
    it produces by the clock rather than by arrival. Each of those rules out a
    read whose result would be discarded. The one that matters is a MOC link:
    what arrives on one is a command, which sets an attribute rather than
    travelling a link, so polling it would be taking commands off the socket
    and throwing them away. A ground link that does feed the graph says so with
    a link out of it, and is polled like anything else.

Requirement
    A vehicle starts only if every node opens, and the complaint lists every
    reason rather than the first. The configuration was checked on the ground,
    so a failure here is the hardware. Carrying on without one node sounds
    kinder and is not: a thermostat whose third sensor did not open would hold
    its last decision for ever, because a mean over three inputs waits for
    three, and nothing in the telemetry would say that is why. An operator who
    wants to fly without a sensor changes the configuration, which is
    checkable, visible in the diagram and recorded in the beacon's digest.

Requirement
    The checks are run again before anything is opened, by the same code the
    ground tool runs. A configuration that reached a vehicle unchecked is the
    one case the ground tool cannot help with, and a bus opened on the strength
    of a file nobody checked is worse than a vehicle that will not start.

Requirement
    A stop takes effect between passes. A signal sets a flag and returns; the
    loop notices within one pause, finishes the pass it is making and flushes
    its logs. Stopping inside a pass would leave a log part way through a
    record and a heater in a state nothing decided. Both an interrupt and a
    termination are answered, those being the two ways a vehicle is stopped: a
    person at a console and a service manager.

Requirement
    A pass that delivered everything reports nothing. A vehicle reading three
    sensors every three seconds would otherwise fill its log with the news that
    it is working, and the refusals -- which are what somebody is looking for
    -- would be the hardest lines to find in it.

Requirement
    Which version a configuration is decides which of the two programs runs,
    and the file says which. No flag beside it: a flag would be a second
    statement of the same fact, and the two could disagree. ``tcsverify``
    already decides the same way on the same evidence.

Requirement
    A command binding counts as a connection at both of its ends. A MOC link
    exists to carry commands as much as telemetry, and a computation whose
    setpoint is the only thing the ground sets is reached by its binding and by
    nothing else -- so a configuration judged by its links alone would call
    both of them unconnected and refuse something correct.

Requirement
    A node is opened for the direction the configuration uses it in, and the
    other direction is refused. The links say which: a node whose output goes
    somewhere is read, a node things are delivered to is written, and a node
    that is both is both. The refusal names which way the configuration has it,
    because the fault is in the file and not in the attempt.

Requirement
    A MOC link that a command binding arrives on is read as well as written,
    though no link leaves it. A command sets an attribute rather than
    travelling a link, so the ``commands`` section is the only statement that
    anything arrives from the ground -- and a ground link judged by its links
    alone would be opened as something only written, with a setpoint that never
    arrives and nothing saying so.

Requirement
    A node no link touches is refused. A device opened and never read holds its
    lines or claims its bus address and says nothing; the usual cause is a node
    whose links were deleted and whose declaration was left behind. The
    configuration checks say so on the ground, and the port says so again
    aboard.

Requirement
    A datum is delivered whole or refused. A write that moved some of the bytes
    is reported, naming how many moved, and a write that moved none of them is
    a failure and not a success of zero bytes. A reading cut in half is not a
    smaller reading, it is a different number, and the ground cannot tell a
    truncated record from a short one.

Requirement
    Nothing to read is not a failure. A read of no bytes is the ordinary answer
    from a device that is not due, which on most passes is most devices. A port
    that reported those would bury the real faults among them.

Requirement
    Both halves of an endpoint are kept whichever direction the graph uses.
    They are not independent -- a UDP writer learns where the far end is from
    what its reader received -- and a device that is only read today is one a
    command binding reaches tomorrow, which should be a configuration change
    and not a code change.

Requirement
    A link that names an input of a device is refused rather than delivered
    anyway. Only a computation has named inputs; a device has one way in and
    does not name it. Delivering it would make a file that says something it
    does not mean work by accident.

Requirement
    A device that must be asked before it answers is asked after it is read,
    not before. Asked and read in the same breath, a device has had no time to
    answer, so what comes back is either nothing or the answer to the previous
    question arriving by luck -- and a control loop cannot tell those apart. A
    pass takes what the last question produced and then asks again, which costs
    the first pass after a start and makes every later reading one of a known
    age: one interval.

Requirement
    A question that could not be asked is reported even when the reading
    succeeded. The reading in hand is the last one that node will produce, so a
    pass that took one and could not ask for the next has not succeeded.

Configuration
-------------

One logical configuration, which may be written as several files.

The top file may carry an ``include`` list naming others; their contents are
merged into it. Nothing is discovered by scanning a directory: what is read is
what a file names, so the set of files that make up a configuration is itself
written down. The digest the beacon carries is taken over the whole set, so the
ground can say which configuration is flying rather than which top file is.

A small vehicle needs no split and should not have one. Where a split earns its
place, the useful seam is who owns a file and how often it changes, not what
type of data is in it:

.. code-block:: text

   platform.yaml   what hardware exists and how it is reached -- bus,
                   address, chip, lines, paths. Changes when the vehicle does.
   graph.yaml      nodes, groups, links, command bindings. Changes when the
                   control strategy does.
   tuning.yaml     setpoints, intervals, thresholds: the values the MOC may
                   also change in flight.

This is version 2 of the payload configuration, and it does not read version 1.
A clean break rather than two parsers: the two schemas describe different
things, and a reader that accepted both would be two sets of rules to keep
correct for as long as the software lives. The shipped sets are converted with
it.

What is checked before anything runs
------------------------------------

The reliability argument for the whole design is that a graph is checkable, and
``tcsverify`` performs these checks on the ground against the same code that
performs them again aboard. One command, on one file:

.. code-block:: console

   $ tcsverify tests/manual/thermostat.yaml
   $ make verify TCSGUI_FILE=tests/manual/thermostat.yaml

The editor reads what it writes. A configuration opened in tcsgui comes up on
the canvas -- nodes where their ``editor`` positions put them, links between
them, groups and command bindings held and written back -- and the comments
come with it: a comment is written as a comment rather than as a field, so the
editor reads the run of comment lines above an entry as that entry's. What a
round trip does not keep is prose attached to nothing: a heading at the top of
a file, or a paragraph after the last entry, belongs to no node and is not
written back.

The canvas is divided in two. The right-hand quarter is the *payload working
pane* and holds everything the vehicle carries -- a device on a bus, a line it
drives, a link to something aboard -- and the rest of the canvas is the
*vehicle working pane* and holds the rest of the graph: the computations, the
log, the ground link, a page of its own. There is a selection pane for each,
one above the other, and which one a shape was pressed in is what decides
where it lands. A node stays in its pane: a drag that would carry a payload
across the divider stops at it.

What that buys is a drawing that can be compared with last mission's. The
vehicle's own graph is the part that persists; what it is carrying is the part
that changes between flights, and a diagram that puts the changing part in one
column shows at a glance what is different. Links cross the divider freely --
that is what a payload is for -- and only positions are confined.

The divider is drawn, and it can be moved: it carries a handle halfway down it,
and dragging that handle moves the line and resizes both panes. Neither pane
may be squeezed below a twentieth of the working pane, so neither can be
dragged shut -- a pane with nothing visible in it is a pane whose nodes cannot
be reached, and the drag that does it is one press away. The divider also stops
against a node rather than passing over it: against the right edge of the
rightmost vehicle node and the left edge of the leftmost payload. The
alternative was to let it go where it liked and shunt the nodes it passed,
which is a gesture that quietly rearranges a diagram -- and one that would have
to carry every node it moved through the history to be undoable. A line that
stops where it would orphan something says what it is doing while it does it. A
node already on the wrong side holds nothing, so one such node cannot freeze
the control.

Where the divider sits is kept in ``$HOME/.tcsgui/preferences.xml``, as the
percentage of the working pane the vehicle working pane takes -- the vehicle
working pane being always the left-hand one, so a percentage and that sentence
are the whole of it. The file is read when the editor starts and is what places
the divider; it is written whenever the line is moved. The directory is made
automatically, and a preferences file that is missing, unreadable or
hand-edited into nonsense leaves the default three quarters in place and says
so on the error output: an editor that refused to start because it could not
remember a pane width would be an editor traded for a preference. The file was
called ``config.xml`` in one build and is still read under that name when the
current one is absent, so the rename forgets nothing; the old file is left for
its owner to delete.

That file and not the configuration, which is the distinction worth stating. A
node's position is in the configuration because it is part of the diagram --
two people reading one configuration should see one drawing. How the canvas is
divided is not part of the diagram: it is a working preference of the person at
the keyboard, and writing it into the configuration would make resizing a pane
a change to a file that flies. A percentage rather than pixels for the same
reason: a percentage is the thing the person chose, where a pixel count is how
this build happens to lay a canvas out.

Within a session the divider is one position in the document's own
coordinates, shared by every window, and not three quarters of whatever window
it is drawn in. Which pane a node is in says what it is allowed to be, so a
divider that moved with the window would make a payload into a vehicle node by
being looked at on a smaller screen. For the same reason opening a file moves
nothing: a node the file puts on the wrong side stays where the file put it
until somebody drags it, and then it comes home. An editor that silently
rewrote positions would be an editor that changed a file by being pointed at
it.

Every node is drawn the same width. Nodes of four widths in a column have no
edge to line up on, and a diagram built from them looks approximate however
carefully each node was placed; height is still per type, because it is one of
the channels that tells two types apart.

Every node type has its own colour, and the fourteen were chosen for one thing
before anything else: that a reader with any of the three dichromacies can tell
them apart. They are the output of a search over sRGB that maximised the
smallest perceived difference between any two of them, measured in CIE
L\*a\*b\* under normal vision and under simulated protanopia, deuteranopia and
tritanopia at once. The worst pair of the fourteen is seventeen CIE units apart
under every one of the four, where two or three is "the same colour in two
printings" and ten is "clearly two colours". Each also has a contrast against
white of at least three to one, so that a one-pixel border of it is legible on
the canvas. What aesthetics the first priority left room for went on keeping
the chroma moderate and spreading the lightness over five levels -- which is
useful as well as calmer, lightness being the one channel every kind of colour
blindness keeps, the total ones included. The simulation and the check are in
the editor's own tests, run against the colours the window is actually given,
because a fourteenth colour added by hand would be a near-copy of one of the
other thirteen under one of the dichromacies and nobody with normal vision
would see it.

A link is drawn as an orthogonal route rather than a straight line -- out of
the source, across, into the target, corners rounded -- and it goes around
whatever is in the way. A line that runs through a node says there is a
connection where there is none, which is worse than a line that is hard to
follow: a reader tracing it arrives at the wrong node and has no way of
knowing. So a route is chosen rather than computed. A handful of shapes are
laid out -- straight through, a vertical run down the middle or beside each
node in the way, and a run along a clear row above or below everything -- each
is checked against every node on the page, and the one that crosses fewest,
turns least and runs shortest is drawn, in that order of priority. Crossings
first because a line through a node is a lie about the graph; turns before
length because two orthogonal routes between one pair of points are usually
the same length, and of two equal routes the one with fewer corners is the one
that can be followed. A line keeps six pixels off any node it only passes,
because a line along a node's edge reads as a line that touches it. Where
nothing misses everything -- a target walled in on every side -- the best
available is drawn anyway: a link with no line is a connection that does not
appear in the drawing at all.

Colour remains the redundant channel for all that. Nothing is told apart by
colour alone: every type differs from every other in at least two of outline,
border width, shading, height and the way its label is set, and a test fails if
that stops being true. Red is reserved for things that are wrong.

A configuration that does not exist yet opens with the ground station already
in it, at the upper left. Every graph has
one -- a vehicle whose telemetry reaches nobody is a vehicle nobody is
operating -- so placing it is work the editor can do rather than work it can
wait for. It is placed through the history like any other edit, so it can be
taken back by somebody drawing a page of a larger configuration.

``TCSGUI_FILE`` is the same variable the editor takes, so checking what was
just edited is the same name in both commands: ``make runguiy`` and ``make
runguix`` open it as YAML and as XML, one target per spelling as the
``runmocy`` and ``runmocx`` pair is, and ``make verify`` checks the file the
variable names.

It prints one line per problem -- the file, the line to look at, and what is
wrong -- and says nothing at all about a configuration it accepts. Which
version a file is decides which checks are run: the two describe different
things, so a version 1 file goes to the payload-set checks and a version 2 one
to these. A configuration made of several files has each problem reported in
the file it is in, which is not the file that was named on the command line.

These are the checks:

* every name a link mentions is a node that exists, and no two nodes share a
  name.
* every input of every destination is bound exactly once. None missing, none
  twice. A node that takes any number of them -- a log -- is exempt from the
  count and not from the rest: it must have at least one source, and every
  source must suit its format.
* no cycle, unless a node on it declares itself a loop and states what breaks
  the cycle -- a control loop is a cycle on purpose, and an accidental one is a
  configuration that would run forever on its own output.
* direction agrees with the type. A set of discrete lines driven as outputs is
  not also read; a sensor buffer is not written; a node with no outbound link
  produces data nobody reads and is named as such.
* every device in a group is readable on the group's interval, and no member
  states an interval of its own.
* every command binding names an attribute its node will accept, and a
  attribute no binding mentions cannot be set from the ground.
* every compute node names a computation that exists and says what that
  computation cannot run without -- a conversion's format, scale and offset, a
  thermostat's setpoint and band, a program's deadline -- and nothing else. An
  attribute belonging to another computation, or to none, is refused by name.
* every mean is over the inputs the links bind, and says which they are. A
  mean over three sensors whose third link was deleted has two inputs, each
  bound exactly once, and the declaration is what makes that a fault instead
  of a mean over two.
* every external process has a timeout. Whether it can be *executed* is the
  vehicle's answer and not this tool's -- the program is on the vehicle -- so
  that one is found when the configuration is loaded aboard.
* every source of a log produces records the log's format can hold, which for
  ``fixed: <n>`` means a stated ``read_size`` of exactly ``n`` from every one
  of them. A source whose record length the configuration does not state --
  every computation -- can feed a variable-length log and not a fixed one.

Example: a cabin thermostat
---------------------------

Three temperature sensors, two on an I\ :superscript:`2`\ C bus and one on a
SPI link, each producing a different format. All three are read together every
three seconds. Each reading is converted to kelvin, and each converted reading
goes to two places: down to the MOC, and into a mean. The mean goes to the MOC
as well, and into a comparison against a setpoint the ground sets. More than
two degrees above it, three heater lines are unasserted; more than two below,
they are asserted.

.. code-block:: yaml

   version: "2.0"
   description: Cabin thermostat

   groups:
     - name: cabin_temps
       interval: 3s

   nodes:
     - { name: temp_fore, type: i2c, group: cabin_temps,
         path: /dev/i2c-1, address: 0x48, read_size: 2 }
     - { name: temp_aft, type: i2c, group: cabin_temps,
         path: /dev/i2c-1, address: 0x49, read_size: 2 }
     - { name: temp_bay, type: spi, group: cabin_temps,
         path: /dev/spidev0.0, max_speed: 1000000, spi_mode: 0, read_size: 3 }

     - { name: k_fore, type: compute, function: to_kelvin,
         params: { format: i16_be, scale: 0.0625, offset: 273.15 } }
     - { name: k_aft, type: compute, function: to_kelvin,
         params: { format: i16_be, scale: 0.0625, offset: 273.15 } }
     - { name: k_bay, type: compute, function: to_kelvin,
         params: { format: u24_be, scale: 0.0078, offset: 255.37 } }

     - { name: mean_temp, type: compute, function: mean, inputs: [in0, in1, in2] }
     - { name: thermostat, type: compute, function: hysteresis,
         params: { setpoint: 293.15, above: 2.0, below: 2.0 } }

     - { name: moc, type: moc, address: 10.0.0.1, port: 5000 }
     - { name: heaters, type: gpio, direction: output,
         path: /dev/gpiochip0, lines: [3, 4, 5] }

   links:
     - { from: temp_fore, to: k_fore }
     - { from: temp_aft, to: k_aft }
     - { from: temp_bay, to: k_bay }

     - { from: k_fore, to: [moc, mean_temp.in0] }
     - { from: k_aft, to: [moc, mean_temp.in1] }
     - { from: k_bay, to: [moc, mean_temp.in2] }

     - { from: mean_temp, to: [moc, thermostat] }
     - { from: thermostat, to: heaters }

   commands:
     - { from: moc, sets: thermostat.setpoint }

Several things in that file are refusals waiting to happen, which is the point.
Drop one of the three links into ``mean_temp`` and the configuration is refused
for an unbound input rather than averaging over a reading that never arrives.
Give ``temp_aft`` an interval of its own and it is refused for disagreeing with
its group. Remove ``direction: output`` from ``heaters`` and it is refused for
being written to as an input. None of those is discovered in flight.

What the cycle costs is also visible in the file rather than in the code. One
group interval governs the whole loop: the sensors are read, the conversions
run, the mean is taken and the comparison made once every three seconds,
because that is what ``cabin_temps`` says and everything downstream of it is
driven by arrival rather than by a clock of its own.

Features
========

* Centralized control

  * Statistics
    
    * Bytes transferred and I/O operations attempted and completed

    * Maintained on global and per payload basis

  * Memory allocation complete during initialization

* TCSpecial code is comprised of:
  
  * Running on spacecraft

    * tcspecial: process running on spacecraft

* Testing software is:

  * tcsmoc: Simple interface using tcslib for visualizing tcspecial operation and doing testing. 

  * tcssim: Simulated payloads

* There are two libraries and two configuration files -- the tcspecial
  configuration, which holds the payload configuration section, and the
  simulator configuration beside it:

    * tcslib: ground software library providing simple integration with mission control software

    * tcslibgs: software library containing the commands, the telemetry, and
      the configuration languages -- what tcspecial and tcslib share, and what
      the two GUIs share with them: every configuration file format is parsed
      here, including the simulator's, and a time or a sample of a transfer is
      formatted here so that both windows show it the same way

    * tcspecial1.yaml: Configuration information for the payloads tcspecial
      serves, tcsmoc controls, and tcssim simulates

    * tcspecial1sim.yaml: What simulating those payloads takes, which is not part
      of describing them

      * Network connections support Stream and datagram
    
      * Network and device
  
        * Support for <n> interfaces on Linux
  
        * Network devices are specified by node and server, like getaddrinfo(), along
          with the other getaddrinfo() hints.
  
        * Other devices are specified by a pathname.
  
        * Streams, both network streams, and devices, have a message length and
          a wait interval.

      * There are as many data handlers as are defined in this file.


* Radio interfaces

  * UDP standard

* Straight-forward extensibility for custom payload and uplink/downlink interfaces

* Written in Rust:

    * Provides a high degree of portability

    * Take advantage of Rust memory ownership features to eliminate many errors

    * Leverage the many features Rust has to produce clean, well documented code, with many modern features to make it easy to write code.


High-Level View of TCSpecial
----------------------------
Tcspecial fits into the ground and space portions of the command and telemetry
systems as follows:
FIXME: tweak diagram as necessary):

**High-Level View of TCSpecial**

.. code-block:: text

          GROUND              :                     SPACE
   ===========================:===================================================
                              :
   +=======================+  :    +=========================+     +=================+
   ||  Ground Ops         ||  :    ||  Flight Software      ||     ||  Payload Bay  ||
   +=======================+  :    +=========================+     +=================+
   |  +---------+          |  :    |    +=================+  |     |                 |
   |  | Mission |          |  :  ----+->| tcspecial      |  |     |                 |
   |  | Control |          |  : |  | |  | (tcslibgs)      |  |     |                 |
   |  | S/W     |          |  : |  | |  +-----------------+  |     |                 |
   |  +---------+          |  : |  | |  | Command         |  |     |                 |
   |       ^               |  : |  | +->| Interpreter     |  |     |                 |
   |       |               |  : |  | |  +-----------------+  |     |  +-----------+  |
   |       v               |  : |  | |  | Data            |<-------|->| Payload 0 |  |
   |  +-----------------+  |  : |  | +->| Handler 0       |  |     |  +-----------+  |
   |  | tcslib          |<------   | |  +-----------------+  |     |  +-----------+  |
   |  | (tcslibgs)      |  |  :    | |  | Data            |<--------->| Payload 1 |  |
   |  +-----------------+  |  :    | |  | Handler 1       |  |     |  +-----------+  |
   |  | tcspecial1.yaml |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    | tcspecial1.yaml |  |     |                 |
                              :    |    +-----------------+  |     |                 |
                              :    +-------------------------+     +-----------------+

On the ground, the tcslib library is used by the mission control software (such as YAMCS
or MCT) to issue commands and receive telemetry. These are transmitted over
what is shown as a single communications link, though these could be using
multiple frequencies or channels.

Commands sent through tcslib go to the tcspecial process, directed as
appropriate to either the command interpreter or the various data handlers.
This is shown as a multiplexed link, and over IP it is one: the command
interpreter takes commands about the spacecraft on one port and commands about
a payload on another -- see `Where the Addresses Are`_ -- and each data
handler exchanges payload data with the ground on an address of its own. On a
space link the same division is by virtual channel.

Telemetry from the command interpreter and the
data handlers could also be transmitted in various ways. Each data handler
will generally communicate to a single payload, though payloads may use
multiple data handlers for various communication links. Data handlers all run
in the same address space, so any need for data handlers to communicate with
each other is straight forward to implement.

The tcslibgs library contains definitions shared between the ground portion of the
software, tcslib, and the space portion, tcspecial.

Commands and Telemetry
======================
Both command and telemetry messages are subject to loss between the sender
and the receiver. Thus, commands must be idempotent, generating the same
resulting state and the same telemetry response

The formats of command and telemetry messages are as given in the document:

   `CCSDS 732.1-B-3 Unified Space Data Link Protocol <https://ccsds.org/Pubs/732x1b3e1.pdf>`_ (Blue Book, June 2024)

Commands and Response Telemetry
-------------------------------
Commands cause generation of one or more telemetry responses. Every response
contains a success failure and may contain additional parameter values

Every response carries a header before whatever else it says: the sequence
number of the command it answers, which type of response it is, the status,
and the time the spacecraft made it. The tables below list what each type
adds to that.

Requirement
    A timestamp is milliseconds since the UNIX epoch: one number and one
    unit, wherever a time is carried -- a response's header, a handler's
    samples and statistics, a beacon. Two ends comparing times, or a log read
    long afterwards, then have nothing to agree about beyond the epoch.

It was a pair, seconds and nanoseconds within the second, which is two fields
to carry, two to serialize and two to get wrong: arithmetic had to borrow
across the second, and nothing ever read the nanoseconds. Milliseconds are
finer than anything here measures, and a u64 of them is half a billion years.


PING
^^^^
Verify that TLSpecial is able to process commands.

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| PING_CM        | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+----------+-----------+--------------------------------------+
| PING_TM        | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | None, beyond the header every response carries              |
+----------------+-------------------------------------------------------------+

RESTART_ARM
^^^^^^^^^^^
Enable a restart of TCSpecial for the next TBD interval.

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| RESTART_ARM_CM | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | arm_key    | ArmKey    | Value that must match the RESTART  |
|                |            |           | command key                        |
+----------------+------------+-----------+------------------------------------+
| RESTART_ARM_TM | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+

RESTART
^^^^^^^
Restart TCSpecial if the time is within TBD interval from the last RESTART_ARM
command and arm_key matches the value of arm_key from the last RESTART_ARM
command

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| RESTART_CM     | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | arm_key    | ArmKey    | Value that must match the          |
|                |            |           | RESTART_ARM command key            |
+----------------+------------+-----------+------------------------------------+
| RESTART_TM     | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+

**The rest of these need work done, but do list the applicatable commands**

START_DH
^^^^^^^^
Start a data handler. 

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| START_DH_CM    | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | type       | DHType    | Which type the handler is, which   |
|                |            |           | must be the type its configuration |
|                |            |           | gives it                           |
|                +------------+-----------+------------------------------------+
|                | name       | DHName    | Which handler to start, as the     |
|                |            |           | configuration names it. If the DH  |
|                |            |           | is a network interface             |
|                |            |           | this is server:port optionally     |
|                |            |           | followed by a colon and a          |
|                |            |           | protocol. If this is a device      |
|                |            |           | DH, this is the path to the        |
|                |            |           | device.                            |
+----------------+------------+-----------+------------------------------------+
| START_DH_TM    | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+

STOP_DH
^^^^^^^
Stop stop a data handler. To make this idempotent, if the command has previously
been executed sucessfully, supplying a DH that has been stopped must also
produce a successful value. This, in turn, means a handler's name must not be
reused.

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| STOP_DH_CM     | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | name       | DHName    | Data handler to stop               |
+----------------+------------+-----------+------------------------------------+
| STOP_DH_TM     | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+

QUERY_DH
^^^^^^^^
Return what the indicated data handler is doing and what it has moved.

Requirement
    A QUERY_DH answer says what state the handler is in, and says nothing
    where there is no such handler. Nothing else on the link says it: the
    ground had only its own commands to go by, so a panel showed what the
    last START_DH or STOP_DH implied and went on showing it -- through a
    handler stopped on the spacecraft, and through a MOC attached to a
    tcspecial serving another payload set, whose every command is answered
    NotFound for payloads the operator can see on the screen. A state for a
    handler that does not exist would have to be invented, and a ground
    station reading one would show it, so a NotFound carries none.

Requirement
    A running data handler reports what has moved so far. Its conduits count
    as data passes and a handler reads those counts where they stand, rather
    than receiving them when a conduit finishes, so a handler that is working
    does not report an idle one.

Requirement
    What a handler reports does not go backwards. Stopping a conduit folds its
    counts into the handler's own, so the same numbers are reported before and
    after.

The statistics are:

* Spacecraft time

* Number of bytes received

* Number of read operations completed successfully

* Number of read operations that returned an error

* Number of bytes sent

* Number of write operations completed successfull
  
* Number of write operations that returned an error

* Number of triggers sent to a triggered payload, and the number that would
  not go -- counted apart from the writes above and from each other, because a
  payload that is answering, one that has gone away, and a handler that has
  stopped asking are three states that one count could not tell apart

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| QUERY_DH_CM    | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | name       | DHName    | Data handler to ask about          |
+----------------+------------+-----------+------------------------------------+
| QUERY_DH_TM    | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | name       | DHName    | Data handler this answers for      |
|                +------------+-----------+------------------------------------+
|                | state      | DHState?  | Created, Active or Stopped, and    |
|                |            |           | absent for a handler this          |
|                |            |           | tcspecial has not got              |
|                +------------+-----------+------------------------------------+
|                | Statistics                                                  |
+----------------+------------+-----------+------------------------------------+

QUERY_DH_SAMPLE
^^^^^^^^^^^^^^^
Return what the indicated data handler last sent and last received. Each is
the time the data moved and the first few bytes of it.

This is a separate command rather than more of QUERY_DH so that the statistics
every poll asks for stay the size they are. A sample is wanted only while
someone is looking at a data handler's panel, and carrying payload bytes in
routine telemetry would spend downlink on data nobody reads.

Requirement
    A command tcspecial does not implement is refused. CONFIG_DH is the one
    such command: nothing configures a data handler, and the command carries
    nothing to configure one with, so it answers ``InvalidCommand``. Answering
    success would tell the ground a handler had been reconfigured when none
    had, and there is no way to tell that from a success that was earned.

Requirement
    A sample carries the time the data moved, not the time it was asked about.
    A panel showing the latter would read as activity whenever it was looked
    at.

Requirement
    A sample carries at most ``DH_SAMPLE_BYTES`` bytes, and the length of the
    whole transfer. Keeping the length is what lets a display say a packet was
    longer than what it shows; without it a transfer of exactly that many
    bytes and one of a thousand look alike.

Requirement
    A direction that has carried nothing has no time. It is distinct from one
    that carried a transfer of no bytes, which has a time and no data.

Recording a sample is best effort: the conduit moving payload data does not
wait for a reader of the samples to finish. A lost sample costs one stale line
on a display, where a blocked conduit costs data.

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| QUERY_DH_      | Name       | Type      | Description                        |
| SAMPLE_CM      +------------+-----------+------------------------------------+
|                | name       | DHName    | Data handler to ask about          |
+----------------+------------+-----------+------------------------------------+
| QUERY_DH_      | Parameters                                                  |
| SAMPLE_TM      +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | name       | DHName    | Data handler this answers for      |
|                +------------+-----------+------------------------------------+
|                | sent       | DHSample  | Time and head of the last data     |
|                |            |           | written towards its destination    |
|                +------------+-----------+------------------------------------+
|                | received   | DHSample  | Time and head of the last data     |
|                |            |           | read from its source               |
+----------------+------------+-----------+------------------------------------+

Configure
^^^^^^^^^
Configure various TCSpecial values

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| CONFIG_CM      | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | beacon_int | BeaconTime| Interval at which BEACON telemetry |
|                |            |           | is sent                            |
+----------------+------------+-----------+------------------------------------+
| CONFIG_TM      | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+

Configure Data Handler
^^^^^^^^^^^^^^^^^^^^^^
Configure various TCSpecial data handler values

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| CONFIG_CM      | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                |            |           |                                    |
+----------------+------------+-----------+------------------------------------+
| CONFIG_TM      | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+


Asynchronous Telemetry
----------------------
Beacon

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
+----------------+------------+-----------+------------------------------------+
| BEACON         | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | sequence   | 4 bytes   | Which beacon this is: the number   |
|                |            |           | of beacons this process has sent,  |
|                |            |           | from one                           |
|                +------------+-----------+------------------------------------+
|                | version    | 3 bytes   | Major, minor and patch version of  |
|                |            |           | the tcspecial build sending it,    |
|                |            |           | from its ``Cargo.toml``            |
|                +------------+-----------+------------------------------------+
|                | config     | 3 bytes   | The version the payload            |
|                | version    |           | configuration file states, in the  |
|                |            |           | same three parts                   |
|                +------------+-----------+------------------------------------+
|                | digest     | 16 bytes  | MD5 of the configuration file this |
|                |            |           | tcspecial read                     |
+----------------+------------+-----------+------------------------------------+

Requirement
    A beacon's time is the one in its header, which every response carries.
    It had a second one of its own, which was the same time said twice and
    two things to keep in step.

Requirement
    A beacon carries the version and the digest a CONNECT is answered with,
    and the version the payload set states as well. A beacon arrives whether
    or not anything has connected, so a ground station that is only listening
    can see which software is flying and which payload set it is serving
    rather than having to ask first.

Requirement
    The payload set's version is the three decimal parts its ``version``
    states: ``1.0`` is 1.0.0, the part it leaves off being nought. A version
    that cannot be read that way is refused where the file is read, rather
    than announced as 0.0.0 -- which is a version some other set might really
    have. ``tcsverify`` reports it from the line that states it.

Requirement
    The MOC shows what the last beacon said as one line, each of the three
    labelled: ``v0.1.0 config v1.0.0 md5:`` and the digest's sixteen
    bytes in hex with nothing between them. The answer to a ``CONNECT`` is
    written the same way; see `What Each End Read`_.

Requirement
    Both are settled when the process starts, not read when a beacon goes
    out. The digest is of the file this process read, and a file edited since
    would otherwise have the beacons claiming a configuration nothing is
    serving.

Requirement
    Every telemetry response carries the time the spacecraft made it, in its
    header. Not the time the command was received, and not the time of
    whatever the answer is about: a QUERY_DH_SAMPLE carries the times its
    samples moved, and this is the time it was asked. The two are worth
    telling apart -- a sample from an hour ago in an answer made a second ago
    is a handler that has gone quiet, and without a time on the answer there
    is nothing to read that against.

Two of them used to carry a time of their own, PING and the beacon, and the
other eight carried none; the header carries it for all of them now, and a
header is built one way so that a type of telemetry added later cannot be the
ninth.

Requirement
    Every beacon carries its own number, counted by the sender from one. A
    beacon answers nothing, so the number in its header is not a command's as
    every other telemetry's is: it says which beacon this is.

A beacon says only that the spacecraft is alive, and without a number a
ground station cannot tell a beacon it missed from one the spacecraft never
sent -- which are a network to look at and a spacecraft to look at. The
indicator says how long ago the last beacon was; the number says how many
there should have been in between.

Requirement
    The count is the sender's, so it starts again at one when tcspecial does.
    A number that does not go forward is therefore a restart rather than a
    gap, and the MOC says which it was: *n beacons did not arrive*, or *the
    beacons start again at #1, so tcspecial has restarted*.

TCSpecial
=========

Configuration Files
-------------------
Requirement
    Every configuration file may be written in YAML or XML: the command
    interpreter's own settings, the payload configuration beside them, and a
    simulator configuration alike. Both formats describe the same settings and
    are read into the same Rust values, so which one a file uses is a matter of
    local preference and tooling and never of capability.

Requirement
    The format is taken from the file's extension and from nothing else.
    ``.yaml`` and ``.yml`` are YAML, ``.xml`` is XML, and a file whose
    extension is neither is refused, naming the file and what a configuration
    file is called.

Refused rather than assumed. JSON was read as well and was what an
extensionless or unfamiliar name was taken to be, so a misspelled extension
was not an error but a file parsed as the wrong language -- a ``.yam`` file
"describing no payloads" rather than a file whose name was wrong. Nothing was
written in JSON that was not also written in YAML.

The structs the files parse into are the single description of the data, in
``tcslibgs``; a format is a spelling of it. Adding a third would be a variant
of ``ConfigFormat`` and no second set of structs. Every shipped payload set
exists in both formats, so each parser is exercised by a file someone runs
rather than only by a fixture, and a test loads both of every set and
compares the payloads they produce.

**How the two correspond**

A section is a key in YAML and an element in XML; a value is a key in YAML and
a child element in XML.

Requirement
    In XML every value is a child element and never an attribute. The two
    formats then say the same thing in the same shape, and one reader of a
    configuration file learns one convention.

Requirement
    A sequence in XML is repeated sibling elements named for the field, and
    the repeats are contiguous. There is no wrapper element around a list:
    two payloads are two ``<payloads>`` elements, which is how a sequence
    reaches serde from quick-xml.

Requirement
    An optional value nothing states is an omitted element, not an empty one.
    An empty element is a value of no characters, which is a different thing
    from an absent one.

A value's own syntax is the same in both -- a number may be decimal or
hexadecimal, a flag is ``true`` or ``false``, a timeout carries its unit; see
`Value Syntax`_. The YAML of a shipped set is the one to read, because it
carries the comments.

(The command link carries JSON between the ground and the spacecraft. That is
not configuration and is no part of this.)

tcspecial
---------
Tcspecial reads two configuration files. Its own is
``tcspecial/src/tcspecial.yaml``, named by ``TCSPECIAL_CONFIG_PATH`` when that
is set, and holds the address the command interpreter listens on, where
beacons are sent, the beacon interval, and where the telemetry log goes. The other describes the payloads
and is named on the command line; see `The Payload Configuration`_.

Both choose their parser from their extension, as every configuration file in
the project does, so either may be written in YAML or XML; see `Configuration
Files`_. The shipped one is YAML because that is what the rest of the
project's configuration is written in.

This is the shipped ``tcspecial/src/tcspecial.yaml``:

.. code-block:: yaml

   address: "0.0.0.0"
   port: 4000
   protocol: udp
   beacon_interval_ms: 5000
   beacon_address: "0.0.0.0:5550"
   log_dir: telemetry
   log_segment_bytes: 65536

The address and port are where the command interpreter binds, which is where
the ground sends commands, and ``beacon_address`` is where beacons are sent.
Every address tcspecial uses is in this file and nowhere else: see
`Where the Addresses Are`_. The quotes on the addresses are deliberate:
unquoted, ``0.0.0.0`` is a string in YAML only because it happens not to be a
number, which is a thin reason to rely on. ``log_dir`` holds the telemetry log's
segment files and must already exist; logging is off when it is absent.
``log_segment_bytes`` is the payload bytes in one segment file, the header
being added on top of that.

Tcspecial hass a command interpreter (CI) running on the spacecraft where the
payloads are located. CI has one or more threads to handle OC communications, i.e.
data exchanged with tcslib over a bi-directional communication link. It
defaults to using datagram communication to the tcslib, though this is usually actually
a link to the spacecraft radio. The radio may use a different protocol.

Tcspecial also has threads associated with each data handler (DH). Each DH
communicates to payloads via a bi-direction channel. A key feature of tcspecial
is that
each DH may use a different protocol to communicate with its payload. This includes
not only the core communication protocols supported by the operating system, such
as stream or datagram protocols, or serial or parallel interfaces,
but also stackable DH protocols that can be
employed to build custom protocol stacks.

Command Interpreter (CI)
^^^^^^^^^^^^^^^^^^^^^^^^
The payload system
software has a command interpreter with two threads. The threads manage
commands from the OC and status messages to the OC. I/O is done
with datagrams. Status messages are queued with a fixed-length queue.

Ground/Space Link
"^^^^^^^^^^^^^^^^^
The connection between the OC and the CI is usually implemented with a UDP/IP
datagram since it is generally the ground/space link, for which TCP/IP
is unsuitable beyond MEO. However, TCP/IP may be suitable if the link is
indirect, that is, to the radio, or for LEO and MEO orbits.

The CI has a Mutex<BTreeMap<<DH>>> which holds all of the allocated DHs. The
use of a Mutex allows status of all DHs to be determined atomically.

Initialization
"^^^^^^^^^^^^^^
When the CI starts up, it will allocate all resources, including threads
and communication links. It then enters the main loop.

Main Loop
"^^^^^^^^^
The main CI loop simply reads and processes command from the OC, along with
periodic sending Beacon Telemetry. The Exit command causes the CI to exit.

Shut Down
"^^^^^^^^^
During CI shutdown, all DHs are also shut down.

Data Handlers (DHs)
^^^^^^^^^^^^^^^^^^^
The usual lifetime of a DH starts with creation by CI, followed by start up
of the threads used and allocation of any other resources. It then waits for
activation. 

After activation, it enters a loop conduiting data between OC
and a payload. During I/O, it may receive notification from CI that something
command needs to be done. This could be something like transmitting statistics
or deactivating the DH.

After the DH is deactivated, all resources are freed and the threads used are
exiting.

Initialization
"^^^^^^^^^^^^^^
When the CI Allocate command is given, a DH is sets up all require resources
and then waits for a DH Activate command.

Main Loop
"^^^^^^^^^
Data handlers (DHs) conduit data between the OC and
a payload. Data is transmitted between the OC and a DH is done with a
UDP/IP link. The data exchange between a DH and a payload may be done
with stream and datagram methods. 

A DH has four functions used to send and receive data:

* oc_read(): receives data from the OC. No protocol conversion done.
* oc_write(): sends data to the OC. No protocol conversion done.
* payload_read(): receives data from a payload. Protocol conversion possible.
* payload_write(): sends data to a payload. Protocol conversion possible.

A communication path between the OC and a DH uses UDP/IP. A path between
a DH and a payload uses one of multiple different communication protocols.

Requirement
    Exactly one end of a payload link waits at the address the configuration
    gives, and which end that is follows from which end can speak first.

Both ends binding the same address is what they did, and an address can be
bound once, so whichever started second got "address already in use" and no
data moved through a network handler at all. A device handler was the only one
that ever worked, because two opens of a device are fine where two binds of a
socket are not.

Requirement
    For a stream payload the payload waits and the handler connects to it. The
    payload is what exists at that address, and a stream has to be accepted
    before anything can pass either way, so the connection itself tells each
    end where the other is.

Requirement
    For a datagram payload the handler waits and the payload sends to it. A
    datagram connect sends nothing, so a payload waiting at that address is
    never spoken to; and a payload produces data because it is running rather
    than because it was asked, so it must be able to send without having
    heard anything. The handler learns where its payload is from the first
    datagram that arrives, as it does for the OC.

A payload that waits to be spoken to before it sends is a payload that never
sends over a datagram link. Both ends sat there with nothing moving: the
handler waiting for data from a payload that was waiting to be asked.

Requirement
    A handler whose stream payload refuses the connection keeps trying.
    Nothing listening yet is the ordinary case rather than a fault: a
    simulated payload is a program someone has to start, and a handler started
    first would otherwise fail for a reason that fixes itself a moment later.
    A datagram handler needs none of this, having nothing to be refused by.

The delay doubles from ``ENDPOINT_DELAY_INIT``, never exceeds
``ENDPOINT_DELAY_MAX``, and the whole attempt is bounded by
``ENDPOINT_CONNECT_BUDGET``.

Requirement
    The waiting is bounded, and bounded well below the ground's command
    timeout. START_DH is answered on the command interpreter's own thread, so a
    handler that waited longer would hold up every other command and leave the
    ground with no answer rather than a refusal.

Requirement
    Only a refused connection is retried. An address that cannot be resolved,
    or a network that cannot be reached, does not become right by being asked
    again, and repeating those would turn a clear fault into a slow one.

Requirement
    Everything a program says about a payload names it as the payload
    configuration file names it. Not a row of a window, not an index, and not
    only the device or address it is reached at: the name is what every other
    part of the system calls it -- a panel, a log line, the list a panel's
    Configuration button shows -- so a message that used anything else left
    the reader to work out which payload of four was being talked about.

So every endpoint constructor takes the name as its first argument, and the
two that open a handler's pair of them -- ``bind_endpoint_pair`` for the OC
side and ``connect_endpoint_pair`` for the payload side -- take it and pass it
down. The endpoint layer otherwise knows nothing but a path or an address, and
a path is not a name: several handlers may wait on one bus, a stand-in pty is
whichever one the system handed out that run, and a refused connection says
only which address refused it. A handler that cannot open its line now says
which handler could not and what it was reaching for, in that order.

Requirement
    A handler that has been stopped starts again. Both windows offer one
    button that stops and starts a payload, so a run is a thing to be begun
    and ended more than once -- and a handler that could be started only once
    answered ``START_DH`` with a failure for the rest of the process, in the
    words of a state machine rather than of anything anyone had written.

Requirement
    A handler's command pipes belong to a run rather than to the handler.
    They are made when it starts and closed when it stops, and a start that
    fails closes them too: they used to be made once, and stopping closed
    them, so even a state machine that allowed a restart would have failed on
    the pipe that was no longer there. Four descriptors a run would otherwise
    leak quietly and work perfectly well until a spacecraft that had been up
    for months ran out.

Requirement
    A handler that is already running is the one state a start refuses, and it
    says so in those words. The command interpreter answers such a ``START_DH``
    with ``Success`` without reaching the handler -- what makes the command
    idempotent is the state and not the presence -- so this is the backstop
    rather than the path.

Requirement
    A START_DH naming a type that is not the handler's type is refused with
    ``InvalidParameter``, and starts nothing. The ground and the spacecraft
    read the same configuration file, so a mismatch is not a handler to be
    started differently: it is the two ends disagreeing about what that file
    says, or a command made by hand. Starting it anyway would open the
    endpoint the configuration describes while the ground believed it had
    started something else.

The type a START_DH carries is worked out from the endpoint by one mapping,
``EndpointConfig::kind``, which both ends use: the ground to say what it is
asking for and the spacecraft to check what was asked. Two copies of it would
be two chances for the ground to ask for something the spacecraft refuses. The
check is made before the handler's state is looked at, so a mis-named START_DH
is refused whether or not the handler happens to be running.

Requirement
    An endpoint is of one type: ``network``, ``device-file``, ``serial``,
    ``i2c`` or ``spi``, or one of the experimental ``can``, ``iio``,
    ``rpmsg``, ``gpio`` and ``hdlc``. The type decides what locates the endpoint and what terms it is
    opened on, and a handler carries both: a serial line's framing, the address
    of a device on a bus, and a SPI peripheral's clock are as much a part of
    where the payload is as the name of the file.

A serial line, an I2C device and a SPI peripheral are all reached through
device files, and all three used to become device endpoints on the way to
being started -- which opened the right file and then talked to it on whatever
terms it had been left on: a line at the wrong rate, a peripheral in the wrong
mode. An I2C endpoint could not be started at all, there being nowhere in a
handler's configuration to put the address of a device on a bus. Each is its
own type now, and tcspecial has an endpoint implementation for each.

Requirement
    A data handler binds the UDP address its configuration gives as
    ``oc_address`` and ``oc_port``. That is where the OC sends to, and it is
    distinct from the address of the payload link. A handler with a stream
    payload therefore binds one of its two addresses and connects the other;
    one with a datagram payload binds both, and learns where each far end is
    from what arrives.

Requirement
    A data handler sends to the OC at the address the OC last sent from. No
    configuration states where the OC is, and a datagram's sender is the only
    statement of it; this is how the command interpreter already answers the
    ground.

A consequence worth stating: a handler cannot send to the OC before the OC has
sent to it. Until then its payload-to-ground writes fail and the data is
dropped, which its statistics record as failed writes rather than as writes of
no bytes. A handler whose payload only produces telemetry therefore needs the
OC to speak first, even if only once. Configuring the OC's address instead
would remove that, at the cost of stating in every tcspecial configuration
where the ground is.

A handler's stream payload socket knows where to send from the moment it is
connected, so a handler can speak to such a payload before the payload has
said anything. Neither its datagram payload socket nor its OC socket does;
both learn as above, so until each far end has spoken once the writes that way
fail and the data is dropped.

The two conduits of a handler share one socket on each side rather than
opening one each, because a socket cannot be opened twice; the conduit reading a
socket is what learns where the far end is, and the conduit writing it holds a
duplicate of the same socket, so it sends where the reader learnt.
For example:

.. code-block:: text

         ______     ___________________     _________
        |      |   |                   |   |         |
        |      |-->| OC        OC      |-->|         |
        |      |   | read      write   |   |         |
        |      |   |                   |   |         |
        |  OC  |   |        DH1        |   | Payload |
        |      |   |                   |   |         |
        |      |<--| Payload   Payload |<--|         |
        |      |   | write     read    |   |         |
        |______|   |___________________|   |_________|

**Insert description of how we push DH Helpers on a DH to modify data as it flows from one side to the other**

Before the read and write interfaces to a DH are called,
a select(), epoll(), or an equivalent multiple file descriptor wait for
I/O ready operation is called. In addition to the corresponding read or
write file descriptor, a pipe file descriptor is supplied to the wait for
I/O read operation. A byte is written to the pipe file descriptor to
wake up the DH so that it can read a command from the CI. Thus, each
wait for I/O ready operation has a pipe file descriptor and a read or write
file descriptor to either the OC or payoad.

NOTE: The mio crate might be suitable for the wait for I/O ready operation.

Each static DH is assign static address information. This can be a path to a
device or a network address.

Payload data can be sent by the payload as datagrams or as a stream. 

When data is being read as a stream, there are two options:

:Send any data: All pending data on the file descriptor will be read when there is at least one byte pending.

:Wait for full: Wait for the buffer to fill up. A timer is set so that, if the buffer does not fill up, all data in the input buffer is sent

EndPoints
^^^^^^^^^
Endpoints handle the low level I/O and each one is associated with a thread.

The EndpointWaitable trait defines wait_for_event() as a function that waits for
an event to occur on a Read or Write trait, similar to the Linux poll()
system call.

Derived from the EndpointWaitable trait is the trait EndpointReadable,
which defines
read() for reading, and EndpointWritable, which defines write() for
writing. Each is used after EndpointWaitable::wait_for_event() indicates I/O
of the particular type is possible on a corresponding Read or Write
trait.

Requirement
    Each Endpoint has an I/O file descriptor

Requirement
    Each Endpoint has a command file descriptor

In order for tcspecial to notify a given endpoint that it has some action
to perform, it passes it a command file descriptor, which is a pipe
interface, during endpoint initialization.
When it wants an endpoint to perform some action, it writes a byte to that
file descriptor.
By waiting on the I/O file descriptor for a read or write, and the command file
descriptor, for a read, the Endpoint
will know that it has a command waiting or I/O waiting, or conceivably both,
by using select(), epoll(), or a similar call.

Requirement
    When an Endpoint needs to perform I/O, it first uses a select()-like interface
    to wait for the the I/O file descriptor to become ready for a read or write,
    depending on the operation, and a read of the command file descriptor.

Requirement
    When the select()-like operation completes, the Endpoint will first check the
    command file descriptor

Requirement
    If the command file descriptor has data ready, the Endpoint will read one byte
    from that file descriptor and call the command interpreter to handle the command.

Requirement
    Regardless whether the command interpreter command handle is successful or not,
    the Endpoint will return the the select()-like call.

Requirement
    If the select()-like call indicates an operation can be performed on the I/O
    file descriptor and one cannot be performed on the command file descriptor, it
    will issue the appropriate I/O with the NO_DELAY option.

Requirement
   When the I/O file descriptor operation succeeds, the Endpoint will return a status
   of Ok(n) where n is a u32 indicate the number of bytes transferred.

Requirement
   If the I/O file descriptor operation fails, it will be repeated after a delay
   up to a specific number of times

Requirement
   The initial value of the delay is a configurable named EndpointDelayInit

Requirement
   Each time the I/O file descriptor fails, the delay is set to twice its previous
   value up to a configurable named EndpointDelayMax.

Requirement
   If the next value of the delay reaches EndpointDelayMax, the Endpoint will
   exit with an appropriate Err() value.

Stream and Datagram Endpoints
"^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Stream data flows along a communication link as one or more bytes at a time with
generally irregular timing and without any error checking. 
Datagrams differ in that all data in a datagram is either transferred in a complete
block with no errors, or it is discarded.

Datagram Endpoints
..................
A consequence of datagrams being transferred in a single block is that select()-like
calls indicate that the entire datagram is present or none is. There is no need to
read a piece at a time. It is possible to read the amount of data available and allocate
a bigger buffer but tcspecial does not support this capability.

Stream Endpoints
................
Streams do not have embedded markers to indicate data boundaries, so select()-like
calls indicate only that one or more bytes are available. Transferring one byte
at a time will incur a significant amount of overhead, so tcspecial can delay for
some time to collect more bytes. This configurable is known as StreamEPDelay.

When the select()-like operation indicates at least one byte is available,
Stream Endpoints with values of StreamEPDelay, the Stream Endpoint will pause
for StreamEPDelay and only afterwards perform a non-blocking read of up to the
buffer size to get the data.

Network and Device Endpoints
"^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Endpoints may correspond to a variety of file descriptor types. In a broad
sense, these are network-related and device-related. While device-related
file descriptors are generally stream devices, network-related file
descriptors may behave like streams or datagrams.

Network Endpoints
.................

The table below lists network protocols supported on Linux-based systems. The
protocols below are generally available, see the man page for socket(2) and
other, associated, documentation. The protocol family, socket type, and protocol
information is provided to the Rust Socket::new() interface as follows:

.. code-block:: rust

   use socket2::{Domain, Type, Protocol, Socket};

   pub fn new(domain: Domain, type_: Type, protocol: Option<Protocol>) -> io::Result<Socket>


In the following table, all socket types except for SOCK_STREAM have datagram
semantics. SOCK_STREAM types have stream semantics.

FIXME: What are the semantics of those items marked TBD?

**Linux Networking Families and Types**

+--------------+----------------+-----------+-----------+
| domain       | type           | protocol  | Stream or |
|              |                |           | Datagram  |
+==============+================+===========+===========+
| AF_UNIX      | SOCK_STREAM    | 0         | stream    |
| or           +----------------+-----------+-----------+
| AF_LOCAL     | SOCK_DGRAM     | 0         | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_SEQPACKET | 0         | datagram  |
+--------------+----------------+-----------+-----------+
| AF_INET      | SOCK_STREAM    | 0         | stream    |
|              +----------------+-----------+-----------+
|              | SOCK_DGRAM     | 0         | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_RAW       | yes       | datagram  |
+--------------+----------------+-----------+-----------+
| AF_AX25      | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_IPX       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_APPLETALK | SOCK_DGRAM     | yes       | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_RAW       | yes       | datagram  |
+--------------+----------------+-----------+-----------+
| AF_X25       | SOCK_SEQPACKET | 0         | datagram  |
+--------------+----------------+-----------+-----------+
| AF_INET6     | SOCK_STREAM    | yes       | stream    |
|              +----------------+-----------+-----------+
|              | SOCK_DGRAM     | yes       | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_RAW       | yes       | datagram  |
+--------------+----------------+-----------+-----------+
| AF_DECnet    | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_KEY       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_NETLINK   | SOCK_DGRAM     | yes       | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_RAW       | yes       | datagram  |
+--------------+----------------+-----------+-----------+
| AF_PACKET    | SOCK_DGRAM     | yes       | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_RAW       | yes       | datagram  |
+--------------+----------------+-----------+-----------+
| AF_RDS       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_PPPOX     | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_LLC       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_IB        | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_MPLS      | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_CAN       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_TIPC      | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_BLUETOOTH | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_ALG       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+
| AF_VSOCK     | SOCK_DGRAM     | yes       | datagram  |
|              +----------------+-----------+-----------+
|              | SOCK_RAW       | yes       | datagram  |
+--------------+----------------+-----------+-----------+
| AF_XDP       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+

.. note::
   The AF_KCM domain is not suported:

**Unsupported Domains**

+--------------+----------------+-----------+-----------+
| domain       | type           | protocol  | Stream or |
|              |                |           | Datagram  |
+==============+================+===========+===========+
| AF_KCM       | TBD            | TBD       | TBD       |
+--------------+----------------+-----------+-----------+


Protocol support may require configuring the Linux kernel
to include protocol drivers. Of course, the hardware supporting the protocol
must also be present. For more information on each of the address families, consult
the Linux man page address_families(7).

Requirement
   Tcspecial and tclib must translate the operating system dependent values, such as
   the address families, types, and protocols, to canonical values, and back again
   to avoid building in operating system dependent code. All values transmitted
   must use the canonical values.

Endpoints Opened Through Device Files
"^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
Most of the endpoint types are opened through an entry in ``/dev``. Which type
an entry is depends on what is at the end of it, and not on the entry's name:
the name says where to look, and the type says what else must be said about it
and which implementation opens it. The two that are not device files at all
are ``network``, which is a socket at an address, and the experimental
``can``, which is a socket on an interface.

**Endpoints Opened Through Device Files**

+----------------+-------------------------------+--------------------------+
| Name           | What is there                 | Endpoint type            |
+================+===============================+==========================+
| /dev/ttyS0     | Serial port, RS-232 or RS-422 | ``serial``: the line's   |
|                |                               | rate and framing are     |
|                |                               | configured with it       |
+----------------+-------------------------------+--------------------------+
| /dev/ttyUSB0   | USB serial adapter, to RS-232 | ``serial``, as above     |
|                | or RS-422                     |                          |
+----------------+-------------------------------+--------------------------+
| /dev/i2c-2     | I2C bus, with several devices | ``i2c``: the endpoint    |
|                | addressed on it               | carries the address too  |
+----------------+-------------------------------+--------------------------+
| /dev/spidev0.1 | SPI peripheral, bus and chip  | ``spi``: the clock, mode |
|                | select named together         | and word are configured  |
|                |                               | with it                  |
+----------------+-------------------------------+--------------------------+
| /dev/urandom   | A character device read as it | ``device-file``: only    |
|                | comes                         | the path is needed       |
+----------------+-------------------------------+--------------------------+

A ``device-file`` endpoint is the last of these and only that: a file opened
and read as it comes. The other three are device files too, and were device
endpoints until each was given a type of its own, which is what let their terms
reach the handler that opens them.


Conduits
^^^^^^^^
Conduits contain two Endpoints, an EndpointReadable and an EndpointWritable. Data
flows in just one direction, from the EndpointReadable to the EndpointWriteable.
A Conduit is implemented as a thread that simply looks between the Read and
EndpointWriteables, handling commands from the Command Interpreter as necessary.
The directions are denoted "Ground to Payload" and "Payload to Ground"

Data Handlers
^^^^^^^^^^^^^
Data Handlers package two Conduits, one in one direction and one in the
other. File descriptors are shared between the EndpointReadable of one direction
and the EndpointWriteable of the other direction, and the EndpointWriteable of
the first direction and the EndpointReadable of the other direction, as show below:

**Visualization of a Data Handler**

.. code-block:: text

         ______     ___________________     _________
        |      |   |                   |   |         |
        |      |-->| OC        OC      |-->|         |
        |      |   | read      write   |   |         |
        |      |   |                   |   |         |
        |  OC  |   |        DH1        |   | Payload |
        |      |   |                   |   |         |
        |      |<--| Payload   Payload |<--|         |
        |      |   | write     read    |   |         |
        |______|   |___________________|   |_________|


Resource Allocation
^^^^^^^^^^^^^^^^^^^
In an ideal world, resources would be allocated at link time (really, process
load time). From a practical standpoint, however, the constraint of
pre-execution allocation is not possible to meet for verious reasons. For
example, configuration-dependent code may require runtime allocations. The
constraint TCSpecial is intend to obey is to have all allocations done before
entering the main loop.

This approach allows TCSpecial resources to be allocated statically, but
the underlying operating system may use dynamic resource allocation. These
may fail, so the CI and DH code must be prepared to handle failures in
in the operating system and retry at intervals if the various protocols do
not already support this.

The software running on the spacecraft is TCSpecial. This has a command
interpreter and some number of data handlers (DHs). The CI talks to ground
software and to the DHs. The DHs talk to the CI and to the payloads.

DH Links
^^^^^^^^
Supported DH protocols include networking protocols and device interfaces,
such as RS-422, as well as stackable protocols. These links can be divided
into stream and datagram types. Stream data has no delimiters, no error
detection and correction codes, etc. There may be gaps between bytes and
sequences of bytes that can be used to identify groups of data.

Datagrams are groups of data with a count or other mechanism to identify the
start and end locations. Under normal situations, datagrams are limited to a specific length, but the DH interfaces optionally allow examining incoming
packets to determine whether they exceed the previously expected length and
reallocating a larger buffer to be able to read the whole packet with
truncation. This is only effective up to some size, however, the kernel
itself will have limitations on the packet size it can read.

Provided Stacked DHs
==≈=≈===============
TCSpecial offers several several stacked DHs that can be used as-is and as examples.

Supported payload protocols are (or will be):

**Currently supported payload protocols**

+-----------------------+----------+----------+--------------------+
| Name                  | Stream/  | Async/   | Count(n bytes)[#]_ |
|                       | Datagram | Cmd/Resp | Term(n bytes)[#]_) |
+=======================+==========+==========+====================+
| UDP/IP Telemetry      | Datagram | Async    | None               |
+-----------------------+----------+----------+--------------------+
| UDP/IP Cmd/Resp       | Datagram | Cmd/Resp | None               |
+-----------------------+----------+----------+--------------------+
| TCP/IP Raw            | Stream   | Async    | None [#]_          |
+-----------------------+----------+----------+--------------------+
| TCP/IP Raw Cmd/Resp   | Stream   | Async    | None [#]_          |
+-----------------------+----------+----------+--------------------+
| TCP/IP Term           | Stream   | Async    | Term(1-2 bytes)    |
+-----------------------+----------+----------+--------------------+
| TCP/IP Count          | Stream   | Async    | Count(1-2 bytes)   |
+-----------------------+----------+----------+--------------------+
| TCP/IP Term Cmd/Resp  | Stream   | Async    | Term(1-2 bytes)    |
+-----------------------+----------+----------+--------------------+
| TCP/IP Count Cmd/Resp | Stream   | Async    | Count(1-2 bytes)   |
+-----------------------+----------+----------+--------------------+
| Device Raw            | Stream   | Async    | None [#]_          |
+-----------------------+----------+----------+--------------------+
| Device Raw Cmd/Resp   | Stream   | Async    | None [#]_          |
+-----------------------+----------+----------+--------------------+
| Device Term           | Stream   | Async    | Term(1-2 bytes)    |
+-----------------------+----------+----------+--------------------+
| Device Count          | Stream   | Async    | Count(1-2 bytes)   |
+-----------------------+----------+----------+--------------------+
| Device Term Cmd/Resp  | Stream   | Async    | Term(1-2 bytes)    |
+-----------------------+----------+----------+--------------------+
| Device Count Cmd/Resp | Stream   | Async    | Count(1-2 bytes)   |
+-----------------------+----------+----------+--------------------+

.. [#] Counts are the first n bytes of the data.

.. [#] Terminators are the last n bytes of the data.

.. [#] The data rate must be limited to that which can be handled
   since there is no data throttling.

.. [#] Data volume must be limited to that which can be handled as a
   response

Packetizing U32 Streaming Data
"^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
The payload is sending big-endian u32 data items every three seconds. The DH accumulates up to four of these. If the bytes have a time interval > between 100 mS, that data item is discarded. The resulting packet has a leading byte with a bit set for each valid data item, followed by valid data item values.

Byte Swapping Values
"^^^^^^^^^^^^^^^^^^^
This DH is composited with the DH that packetizes streaming data. It converts the big-endian u32s to little-endian u32s.

Packetizing Arbitrary Streaming Data With Constant Overhead Byte Stuffing
‐-----‐------------------------------------------------------------------
Read 254  * 4 bytes from the payload, write to a buffer with COBS, stick a u16 header which is a byte count and send to the OC.

tcslib
------
The TCSpecial library has a set of operations for global control and status
and a set of per-payload interface operations.  It is used for building control applications using mission control
software such as YAMCS or MCT.

Those operations are ``tcslib::client::TcsClient``. A control application
drives one of those rather than speaking the command and telemetry protocol
itself: each operation sends its command, waits for the answering telemetry,
and hands back what that telemetry carried. The global operations are PING,
RESTART_ARM, RESTART and CONFIG; the per-handler ones are START_DH, STOP_DH,
QUERY_DH and QUERY_DH_SAMPLE.

Requirement
    An operation that is not answered within the client's timeout is an error.
    A control application on a link that has gone quiet is told so rather than
    left waiting.

The transport is a ``Connection``, so the same client serves a UDP socket to a
radio and a TCP socket to a test harness without the operations or their
callers changing. Tcsmoc is built on this client, which is why it is here
rather than in tcsmoc: a second copy of it would be a second thing to keep
correct, and the one in tcsmoc was the only one that existed.

tcslibgs
--------
The TCSpecial ground/space library contains definitions used by both
tcslib and tcspecial.

tcspecial1.yaml
---------------
This is a YAML file that defines the actual payloads. It is considered part
of TCSpecial as it must be supplied, but is also used by the
test software. All three programs read it: tcspecial serves the payloads it
describes, tcsmoc controls them, and tcssim simulates them, so a handler
changed in it changes for all three at once. Its format is given under
`The Payload Configuration`_.

The ``1`` names a payload set rather than a version, leaving room for further
sets alongside it. The data handlers of this one have the following
configurations:

+------+---------+-------------------------+-------------+
| DH # | Type    | Configuration           | Packet Size |
+======+======+============================+=============+
| 0    | Network | TCP/IP | localhost:5000 | 12 bytes    |
+------+---------+--------+----------------+-------------+
| 1    | Network | UDP/IP | localhost:5001 | 11 bytes    |
+------+---------+--------+----------------+-------------+
| 2    | Device  | n/a    | /dev/urandom   | 1 byte      |
+------+---------+--------+----------------+-------------+
| 3    | Network | UDP/IP | localhost:5003 | 15 bytes    |
+------+---------+--------+----------------+-------------+

There is no packet interval here. How often a payload produces a packet is a
property of a simulation rather than of the payload, so it is stated in a
simulator configuration file; see `Simulator Configuration Files`_.

What Each End Read
------------------
Requirement
    A link begins with each end saying what it read. The ground sends a
    ``CONNECT`` carrying its software version and a digest of its
    configuration, and the spacecraft answers with its own, and with the
    version the payload set it is serving states.

Requirement
    Each version is three bytes -- major, minor and patch, each in binary --
    and the digest is an MD5.

Requirement
    The answer is shown as one line, each of the three labelled:
    ``v0.1.0 config v1.0.0 md5:`` and the digest in hex. The same three
    facts a beacon carries, written the same way by the same function, because
    the two are read against each other.

Requirement
    The ground compares and the spacecraft says. Tcsmoc is the end that knows
    which file it read and can name it, so it reports the difference; tcspecial
    answers with what it read and logs a disagreement it can see.

Requirement
    Each end says its own version, the version of the set it read, and the
    digest, before it makes the socket the link runs over. A socket that
    cannot be made, or a far end that is not answering, takes the exchange
    with it -- and that is exactly when those facts are wanted. In tcspecial
    this also puts an address already in use under the configuration it was
    going to serve rather than instead of it.

Requirement
    The two ends write that line the same way, naming the file each read:

    .. code-block:: text

       v0.1.0, configuration tests/manual/tcspecial2.yaml config v1.0.0 md5: 8d908f54a3d72d0704213113b92d958b

    Two ends reading one payload set therefore print the same line twice,
    which is what makes a difference in any part of it worth reading. The
    MOC's line used to be written differently -- ``Version 0.1.0``, a bare
    ``md5``, and no set version at all -- so the two could only be compared
    by the one fact they spelled alike.

Nothing else in the protocol makes the two ends prove they are talking about
the same payload set, and when they were not, the only sign was a command
answered ``NotFound`` for a payload the operator could see on the screen: a
tcspecial left running from an earlier set, which tcsmoc attaches to rather
than replacing. The two ends also say what they read on their own consoles as
they start, so the question can be answered without a link at all.

Requirement
    The digest is of what a file says, not of its bytes. Two ends reading
    different spellings of one payload set have read the same set and must
    agree, so the two formats a set is written in digest alike, and so do
    two files differing only in comments, indentation, the order of their
    sections or the order of one payload's attributes.

Requirement
    The order of the payloads does change the digest. Ids will be assigned in
    the order the payloads appear, so that order is configuration rather than
    spelling.

Requirement
    Each payload is given a sequence number as the file is read: the first is
    0. A file may not state it -- the number is what this program read, not
    something a file gets to assert.

Requirement
    The digest takes the payloads in order of their sequence numbers. Two ends
    that read the same file then agree about which came first whatever either
    has since done with its own list.

The numbering rests on one assumption: that a parser hands entries to the code
above it in the order the file gave them. Both formats here do, and a format
that did not would be unusable for a language whose payload order means
something.

The payloads are numbered as the list is deserialized, which is the one place
every format arrives through, so nothing can hold a configuration whose
payloads do not know which ones they are. A file stating a number of its own
is refused by name.

Carried by the data rather than left to the order a list happens to keep. The
order used to live only in that, so anything sorting the payloads -- for a
lookup, for a display -- would have changed what the configuration said about
which payload came first, and the only thing in the way was that nothing did
it yet. The two halves of the check are then independent: walking by sequence
number is what makes a list in some other order digest the same, and hashing
the numbers themselves is what makes a *file* in some other order digest
differently.

A sequence number and not a line number. A line is not something these parsers
hand to the code that builds a document, and it would be a different number in
each of the two formats one set is written in, where the sequence number is
the same in both -- which is what lets them digest alike.

How it is reached: a file is parsed into the document it describes and that
document is written back out as a tree of values, which is where the format
stops showing through -- a number is a number rather than the text that spelled
it, a list is a list rather than XML's repeated sibling elements, and an
attribute nobody stated is absent rather than missing. The tree is then written
to bytes with every map's keys in order and every list in the order it was
given, and those bytes are what the MD5 is of.

A map's keys are put in order twice over, deliberately. The document is a set
of Rust structs, and a struct writes its fields in the order it declares them
however the file gave them, so the order a file used is already gone; sorting
is what keeps the rule true of anything that is a map rather than a struct.
What a file's own order does reach the digest through is the sequence number
each payload was given as it was read, which is the one ordering that means
something.

The Payload Configuration
=========================
The payload configuration section of a tcspecial configuration file describes
the payloads themselves: which data handlers there are, how tcspecial reaches
each one, and how big its packets are. What it deliberately does not describe
is how a simulated payload behaves; see `Simulator Configuration Files`_ for
that. The parser is ``tcslibgs::config``.

Four sets are shipped, each with a simulator configuration beside it named
for it -- ``tcspecial1sim.yaml`` for ``tcspecial1.yaml``, and so on -- and a set
added by that convention needs no change anywhere to be run. Each is named
below by its YAML, and each exists in both formats; see the requirement
at the end of this section.

``tcspecial1.yaml``
    Four handlers of three types, which is what exercises the panels and the
    grid. It is also what every program falls back to when nothing names a
    file.

``tcspecial2.yaml``
    Two handlers, one of each type of payload: one that sends of its own
    accord and one that answers a request. The Makefile defaults to this set.

``tcspecial3.yaml``
    One handler, reached over a stream, which is what a single link is
    debugged against. It differs from set 2's first handler only in the
    transport, which is what the type and transport stated in a simulator file
    are there to tell apart.

``tcspecial4.yaml``
    One handler of every supported type. It began as the set that exercises a
    line's own terms -- a data rate, a framing, a rule for where a read ends --
    and the simulator's pty stand-in, and it was written in a second language,
    there being no words in a tcspecial configuration for a line; `Link Terms`_
    are those words, and the set is written here like every other.

    The other four types were added beside the line so that one set reaches
    all five: a socket, a device file, a bus and a clocked link. One payload
    each, no more -- a set is for running, and what covers a type thoroughly
    is its tests.

A set is a pair of files: the tcspecial configuration and the simulator file
beside it, each read by whichever programs need it.

The format is chosen from the extension exactly as every other configuration
file's is, so a tcspecial configuration may be written in YAML or XML.

Requirement
    Every shipped set is written in both formats, and the two describe the
    same data handlers. ``tcspecial2.yaml`` and ``tcspecial2.xml`` are one set
    written two ways.

The formats are two spellings of one configuration, which nothing in the
code enforces: each is parsed by its own parser, and a set transcribed by hand
can differ in a port or lose an attribute without either file becoming invalid.
So a test loads both of every set and compares the handlers they produce.
Shipping both also means each parser is exercised by a file someone runs
rather than only by a fixture -- ``make runmocx`` runs the XML of a set --
which is what turned up the one bug this found:
the check that a file describes payloads at all asked for the
``payloads`` section with a single field, and a sequence in XML is repeated
sibling elements, so every XML configuration of more than one payload was
refused as a duplicate field.

The YAML of a set is the one to read: it carries the comments. The simulator
files are YAML alone, nothing having asked for them in both spellings.

File Structure
--------------
A tcspecial configuration file has a version, a description, and three
sections, two of them optional. They are given below in the order the files
carry them.

``tcspecial``
    What tcspecial itself is configured with for this payload set: the same
    section a ``tcspecial.yaml`` holds, with the same attributes. Optional.
    Where the set's beacons go is read from here -- ``beacon_address`` and
    ``beacon_interface``; see `Where the Addresses Are`_ -- and the rest is
    carried and checked and not read, tcspecial being placed by the file
    ``TCSPECIAL_CONFIG_PATH`` names. Which includes the two command ports: a
    set does not decide where the interpreter serving it takes commands. The section is here so
    that one file can describe a whole payload set, the command interpreter
    included.

``payload_groups``
    Named groups of attributes. A group carries what several payloads have
    in common. Optional: a file whose payloads share nothing, or that prefers
    to spell every one of them out, has no groups.

``payloads``
    The payloads themselves. Each has an id and a name, may name a group,
    and states whatever attributes it does not take from that group.

Requirement
    ``tcspecial`` is the first of the sections, in every tcspecial
    configuration. It is about the set as a whole where the other two are about
    the payloads in it, and a reader opening a file to find out what it
    describes should not have to pass a list of payloads to find where its
    command interpreter is.

Requirement
    A ``tcspecial`` section a file states is checked when the file is loaded,
    whether or not anything reads the attribute in question. A section stating
    a protocol that is not a protocol is refused where the file is read,
    rather than left for whichever program first tries to be placed by it.

The section was ``ci_config``, and named for the command interpreter rather
than for the program whose configuration it is. No shipped file carried one,
so the old name is not accepted: a file using it is refused the way any other
unknown section is.

Requirement
    The sections are ``payloads`` and ``payload_groups``. They were
    ``data_handlers`` and ``data_handler_groups``, and a file still naming
    either is refused with the names they have now rather than read as a file
    describing no payloads -- which is what it would otherwise look like,
    since a section this does not recognise is a section it does not see.

**Attributes**

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| type                | string | ``network`` or ``device-file``              |
+---------------------+--------+---------------------------------------------+
| protocol            | string | ``tcp``, ``udp``, ``unix_stream``, or       |
|                     |        | ``unix_dgram``. Network handlers only       |
+---------------------+--------+---------------------------------------------+
| address             | string | Host a network handler connects to          |
+---------------------+--------+---------------------------------------------+
| port                | number | Port a network handler connects to          |
+---------------------+--------+---------------------------------------------+
| path                | string | Device a device handler opens               |
+---------------------+--------+---------------------------------------------+
| packet_size         | number | Bytes in one packet                         |
+---------------------+--------+---------------------------------------------+
| oc_address          | string | Host the handler exchanges payload data     |
|                     |        | with the OC on. Always UDP                  |
+---------------------+--------+---------------------------------------------+
| oc_port             | number | Port it does so on                          |
+---------------------+--------+---------------------------------------------+
| mode                | string | ``periodic`` or ``triggered``. Absent is    |
|                     |        | periodic                                    |
+---------------------+--------+---------------------------------------------+
| trigger             | bytes  | What to send to make the payload answer:    |
|                     | or     | bytes, as unquoted hexadecimal after        |
|                     | string | ``0x``, or text, as a quoted C string.      |
|                     |        | Triggered only                              |
+---------------------+--------+---------------------------------------------+
| packet_interval_ms  | number | How often the trigger is sent. Triggered    |
|                     |        | only: a periodic payload's rate is the      |
|                     |        | simulator configuration's                   |
+---------------------+--------+---------------------------------------------+

A group states any of these; a data handler states any of these plus its
``name`` and the ``group`` it takes attributes from.

The OC address is where a handler is reached from the ground, and is a
different thing from the address at which it reaches its payload. It is
commonly shared by every handler of a group, which is why a group may carry
it, while the port is what tells one handler of a group from another.

Requirement
    A data handler states both an OC address and an OC port, or neither. Half
    of an address reaches nothing, and nothing in the file says which half was
    meant.

Requirement
    The payload configuration need not give any handler an OC address. Only
    starting a handler needs one, so a file describing payloads is complete
    without it and a handler without one is refused at the point it would be
    started.

How a Payload Is Made to Send
-----------------------------
Payloads are generally one of two types, and which type a payload is decides
where the timing of it is written down.

Requirement
    A payload is periodic or triggered, and says which. A file that says
    neither describes a periodic payload, which is both the commoner type and
    what every tcspecial configuration written before there was a mode
    describes.

Requirement
    A periodic payload sends of its own accord and tcspecial reads what
    arrives. It states no trigger and no interval in the payload
    configuration: there is nothing to send it, and how fast a simulated one
    produces data is a property of the simulation, stated in the simulator
    configuration file.

Requirement
    A triggered payload answers a request. It states the trigger to send and
    the interval it is sent at, both of which are flight behaviour: tcspecial
    does the sending, and what to send comes from the payload's interface
    document. Its simulator configuration states no interval at all, packet
    or segment, and takes none from the group it names: it sends when it is
    asked, so a rate there would govern nothing and the segments of one answer
    go as fast as they can.

Requirement
    That holds for an interval a triggered payload inherits as much as one it
    states. A group's settings reach a payload exactly as its own do, so a
    payload that could inherit a rate it may not state would be governed by a
    group it joined for the settings it may -- and the file would read as
    though the rule had been kept.

Requirement
    The interval is ``packet_interval_ms`` in whichever file states it. One
    name, in the file the payload's type puts it in: the rate a triggered
    payload is asked at is the rate its packets come back at, so calling it
    something else in one file would be two names for one thing. It was
    ``trigger_interval_ms`` in the payload configuration section, and a file
    still using that name is told the name it has now rather than told the word
    is unknown.

The two sets of attributes are exclusive in both directions, and a file giving
the wrong one is reported rather than run: either guess about what was meant
would operate the payload in a way nobody asked for. The exclusion is also
structural in the Rust the file parses into -- a periodic handler has nowhere
to put a trigger -- so it cannot be lost downstream of the check.

Requirement
    A datagram payload cannot be triggered. Its handler learns where to send
    from the payload's first packet, a datagram's sender being the only
    statement of where it came from, so there is nowhere to send a trigger
    until the payload has spoken -- which a triggered payload does not do. A
    file asking for it is refused rather than left to fail as silence.

A trigger is bytes rather than text: a payload's interface document asks for
what it asks for, and that is as likely to be ``55 AA`` as ``"READ\r"``.

Requirement
    A trigger is written as a list of hexadecimal values separated by spaces,
    or as a string in C's notation. One or two digits is one byte -- ``5`` and
    ``05`` are the same byte, a datasheet writing either -- and three or four
    digits is two bytes, most significant first: the order it is written in,
    and the order every other multi-byte quantity in this project uses. A
    value of more digits than that is refused, which of its digits pair with
    which not being something to decide for a file.

Requirement
    There is no ``0x`` on a value. A trigger is a list of byte values and
    nothing else, so a prefix saying "this is hexadecimal" would be saying it
    once per value for a notation with no alternative. A file written for the
    older spelling, which had one, is told so by name rather than being
    refused for the ``x``: an error about a digit says nothing about what to
    write instead.

Requirement
    Which notation a value is in follows from its first character and nothing
    else: a quote opens a C string, and anything else is a list of values.
    ``52 45`` is a plausible pair of bytes and a plausible pair of digits, so
    something has to say which, and a file that meant one and got the other
    would send something nobody asked for.

Requirement
    The quotes of a C string are part of the value rather than part of the way
    the file is written, since they are the mark that says which notation it
    is. So a YAML file writes them inside its own -- ``'"READ\r"'`` -- and an
    XML one as ``<trigger>&quot;READ\r&quot;</trigger>``. A value whose quotes
    belong only to the format, such as ``"55 AA"``, arrives as ``55 AA`` and
    is read as the two bytes it is. A string that opens and does not close is
    refused rather than taken for a list of values.

Requirement
    A C string's escapes are C's own -- ``\\`` ``\"`` ``\'`` ``\n`` ``\r``
    ``\t`` ``\0`` ``\a`` ``\b`` ``\f`` ``\v`` and ``\xNN`` -- and an escape
    C does not have is refused rather than passed through. A file that wrote
    ``\q`` meant something, and a backslash and a q is unlikely to be it. This
    is also what lets an XML configuration state a trigger that a YAML one
    states with the format's own escapes.

Requirement
    A trigger is one byte or more. A payload that answers requests has to be
    asked something, and nought bytes asks nothing.

Requirement
    A trigger's bytes go out as the file wrote them, and are shown as a file
    could have written them: hexadecimal, with the text beside it where the
    bytes are text.

Requirement
    Tcspecial sends a trigger on the conduit that writes the payload, and
    sends the first one as soon as the handler starts. A payload that answers
    requests has nothing to say until it has been asked, so waiting out an
    interval first would be a handler that begins by doing nothing.

Requirement
    Triggers are counted separately from the data a handler carries. A
    handler's received and sent are the ground's data in each direction, so
    the payload side of each conduit is deliberately left out of both -- those
    are the same bytes seen twice -- and a trigger is a write with no read
    behind it. Without a count of its own it would appear in no statistic, and
    a payload that had stopped answering would look exactly like a handler
    that had stopped asking.

Requirement
    A handler goes on asking its payload however the asking goes. A trigger
    that will not go is the payload's silence and not the handler's: the
    payload may come back, and a handler is stopped by a STOP_DH rather than
    by a payload that has gone away. It used to give up three different ways
    -- the write failing, the payload's end of a stream reading as end of
    file, and the wait on the endpoint reporting POLLERR or POLLHUP -- and in
    two of them it spun through the interval it was no longer asking in.

Requirement
    Triggers that failed are counted apart from triggers that were taken, and
    apart from the writes that carry data to the ground. A payload that is
    answering, one that has gone away, and a handler that has given up asking
    are three states, and one count of attempts could not tell them apart. A
    failed write to the ground is data the ground did not get; a failed
    trigger is a payload that would not take a request.

Requirement
    A conduit with nothing to read and nothing to wait on pauses before going
    round again, and spends the pause watching its command pipe rather than
    asleep. An endpoint in error or at its end is ready the instant it is
    polled and yields nothing every time, so a conduit that only went round
    held a core down for as long as its handler ran; and one that slept the
    pause out would answer a STOP_DH only when it woke. A conduit that asks
    pauses until its next trigger is due, so the asking keeps its interval.

The format has no packet interval. How fast a payload produces packets is a
property of a simulation rather than of the payload, and belongs to a
simulator configuration file.

Requirement
    A tcspecial configuration file states no packet interval, and one stated
    here is an error naming the file it belongs in. How fast a payload sends is
    a property of the simulation, so a tcspecial configuration that states an
    interval is describing a simulation. It was ignored for a while, which was
    the same silence the mode rules exist to prevent: a file that states an
    interval has said something about the payload's timing, and reading it as
    though it had not is reading a different file than the one that was
    written. An interval a payload takes from its group is refused as one it
    states itself is.

Requirement
    A ``name`` belongs to a data handler and never to a group. It is what
    tells one handler of a group from another, and what every command about a
    handler addresses it by.

Requirement
    A data handler has no number. It was addressed by a ``dh_id`` the file
    stated and found by its name, which meant a file could state two
    identities for one payload and keep them consistent by hand -- and a
    command naming an id the ground had and the spacecraft had not was
    reported against a number nobody reading the file would recognise. The
    name was already required, already unique and already what the panels and
    the simulator configuration use, so it is the identity.

Requirement
    A data handler takes each attribute it does not state from the group it
    names, and overrides any the group does state. A group therefore holds what
    its handlers share without keeping one of them from differing.

Requirement
    A data handler has a type and a packet size, from itself or from its group.
    A handler with no packet size is an error rather than a handler whose
    packets are zero bytes long.

Requirement
    A data handler names a group the file defines. A name matching no group is
    an error, because the handler would otherwise be left with none of the
    attributes the group was to supply, which is a more confusing failure than
    the misspelling that really caused it.

Requirement
    A group name is defined once in a file. Two groups sharing a name is an
    error rather than one silently shadowing the other.

Requirement
    A payload name is defined once in a file. It picks out one payload, and a
    repeat parses cleanly and then loses one: tcspecial keeps its handlers by
    name, so a repeated name has one silently replace the other, and a
    simulator configuration is joined to the payloads by name, so it would
    drive two payloads from one entry.

Requirement
    Each of these names is unique within its file and nowhere wider. Two
    payload sets are two descriptions of what a mission flies, and a set that
    had to rename its payloads because another set in the same directory had
    used the names would be renaming them for no reason: a set is read on its
    own, with the simulator file beside it, and nothing compares the names in
    one file with the names in another. The shipped sets rely on it: set 3 is
    set 2's first payload over a stream and carries the same name,
    ``auto-send``.

Requirement
    An attribute of another type of payload is an error. A device file payload
    stating a protocol, an address or a port, or a network payload stating a
    path, has written something nothing will read -- and a payload that looks
    configured and is not is worse than one that is refused. A link's own
    terms are refused the same way, by the rules for its type: see
    `Link Terms`_.

Requirement
    The check is made after a group has been laid under a payload, because an
    attribute inherited from a group reaches the payload exactly as one it
    states itself does. A payload of one type joining a group written for
    another is the case this catches.

Requirement
    A word that is not an attribute is an error. A misspelled attribute is
    otherwise ignored, which is silence about something the file plainly
    meant: a payload whose ``oc_address`` is misspelled has nowhere to send
    its data and nothing says so. ``packet_interval_ms`` is the one word this
    language still knows without accepting: it is named so that a file stating
    one is told which file it belongs in rather than told the word is unknown.

Requirement
    No two data handlers want the same thing. Two at one address cannot both
    be started; two on one device file, one serial line or one SPI peripheral
    is worse, each taking part of the one stream and reporting it as though it
    were the whole. A handler's own address and the address it reaches the OC
    on are compared together, since a port is a port whatever claims it.

Requirement
    Hosts that certainly mean this host -- ``localhost``, ``127.0.0.1`` and
    ``::1`` -- are one host, so naming a port's host differently does not make
    it a different port. Two names for some other host are left as written:
    resolving them is a question for the network rather than for a file.

Requirement
    A Unix socket collides by its path. It is named by a file rather than by a
    port, so its port means nothing here, and a device handler opening that
    same path is the same collision from the other side. Every path is one
    type of claim for that reason, whichever type of handler opens it.

Requirement
    A bus is shared and a place on it is not. Several devices on one
    I\ :superscript:`2`\ C bus is what a bus is for, so a claim is on the bus
    and the address together rather than on the bus device.

Requirement
    The rule is applied in one place, over the handlers a file produced rather
    than over the words that produced them. The hazard belongs to the
    handlers: two of them opening one device file collide whatever either was
    called in the file.

Requirement
    A group is named by a data handler. A group no handler names is an error
    rather than a section with no effect, because that is also what a group
    whose name a handler misspelled looks like.

A misspelled group name leaves the group unnamed and the name undefined at
once. The handler's end is where the misspelling actually is, so that is the
error reported.

Link Terms
----------
Three types of payload are on a link of their own -- a serial line, a device
on an I2C bus, a peripheral on a SPI bus -- and each has terms that say how
that link is driven: a data rate, an address on a bus, the shape of a clock.
A payload states them beside everything else it states, and may take any of
them from its group as it takes the rest.

The rules are in ``tcslibgs::endpoint_config`` and the per-type modules beside
it, and are asked of a payload by ``types::link_params_of``.

Requirement
    A term that belongs to another type of link is an error rather than
    something ignored, so that a misspelled or misplaced term is reported
    where it was written. ``pec`` on a line, ``cs_active`` on a bus, and
    ``datarate`` on a peripheral are each refused.

Requirement
    A payload documents and takes only keywords whose values it can set
    through the interface it uses. A value a file states and nothing applies
    is worse than no keyword at all: the file looks configured, a reader
    believes what it says, and the equipment is doing something else. Four
    words went for that reason -- ``bus_speed``, ``bitrate``, ``channels`` and
    ``sample_rate``. Each said something true about the equipment and none of
    them could be set by a program that opens the interface: a bus clock
    belongs to the controller the platform configures, a CAN bitrate to the
    interface as it is brought up, and a sensor's channels and rate to the
    ``sysfs`` entries whoever brought the sensor up wrote. A file that states
    one is refused by name and told why, as a file writing
    ``trigger_interval_ms`` is told the name that has now.

    The one place this rule is not yet kept is a synchronous serial line,
    whose ``clock_type``, ``encoding`` and ``loopback`` are recorded and not
    applied -- the kernel drives such a line through a network interface of
    its own rather than through the terms of a device file, so a handler that
    opens a path cannot set them. Either those terms go the way of the four
    above, or they are applied through that interface; see the requirement
    below, which records what is so today.

Requirement
    A term of a link is refused on the two types that are not on one: a
    network address and a plain device file. Every other type refuses what
    belongs to another type as its own rules are asked, and these two have no
    rules to ask -- so a term written on one of them was read and dropped. A
    device file payload could state a data rate, or a sensor's channels, and be
    read as a device file payload that had said nothing of the sort: a
    misspelling that configured nothing and reported nothing. A network
    payload's ``protocol`` is a term of its own type and is allowed.

Requirement
    Such a term is refused whether the payload stated it or took it from its
    group, and the error says which. The two are fixed differently: a line is
    deleted from the payload, or a payload is in a group that was not written
    for it.

Requirement
    A term a payload does not state is taken from the group it names, exactly
    as every other attribute of a payload is. A term reaches the link the same
    way whichever of the two stated it, so both are checked the same way.

Requirement
    The SPI mode is written ``spi_mode``. A payload already has a ``mode`` --
    whether it sends of its own accord or on a trigger -- and one word cannot
    mean both.

What Is Experimental
^^^^^^^^^^^^^^^^^^^^
Some of what follows is **experimental and not yet supported**: RS-485 on a
serial line, and the ``can``, ``iio``, ``rpmsg``, ``gpio`` and ``hdlc`` types
of payload. Each is written, parses, reaches a handler and has tests of its
rules; none has been run against the hardware it describes, and no shipped
payload set uses one. ``hdlc`` is the furthest out of them: every other has
something in the kernel to be exercised against, and a synchronous line has
nothing but a card.

The requirements for them are written the way every other requirement here is,
because that is what they are for: a statement of what the configuration will
be, settled before there is equipment to try it against. They may change,
where the five supported types -- ``network``, ``device-file``, ``serial``,
``i2c``
and ``spi`` -- will not.

GPIO Payloads: Discrete Lines
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
**Experimental, not yet supported.**

Not every payload is a stream of data. A deployment switch, an arming line, a
thermostat's output: each is one wire that is high or low, and what the ground
wants to know is when it changed. Linux offers those through a chip's
character device, where a program asks for a set of lines with edge detection
and then reads events.

The rules are in ``tcslibgs::endpoint_config_gpio``, the endpoint in
``tcspecial::endpoint_gpio`` and the stand-in in ``tcssim::payload_gpio``.

Requirement
    A GPIO payload states a ``path`` and the ``lines`` it watches, and may
    state the ``edge``, the ``bias`` and a ``debounce_us``. Offsets are
    separated by commas or spaces and written in decimal or hexadecimal,
    because a file written for a board copies them out of a schematic in
    whichever form the schematic used.

Requirement
    A payload names at least one line, names none of them twice, and names no
    more than the kernel's limit of 64 on one request. A payload watching no
    lines has nothing to report, and one watching a line twice would report
    every change of it twice.

Requirement
    This is the one type whose payload data is made rather than carried, so
    its shape is settled here: each event becomes a record of sixteen bytes
    -- eight of time, four of line offset, four of value. The time is
    milliseconds since the UNIX epoch, which is what every other time in this
    project is, so a line's change and a telemetry response can be read
    against each other; the kernel's own timestamp is nanoseconds on a
    monotonic clock, which says nothing to a ground station.

Requirement
    A ``packet_size`` that is not a whole number of records is refused. A
    packet that was part of a record would be a reading nobody could use.

Requirement
    A chip is shared and a line is not. Several payloads commonly watch lines
    of one chip, as several devices sit on one bus, and the kernel gives a
    line to one request at a time -- so two payloads naming one line are two
    handlers of which the second could not start.

Requirement
    A set of lines states its direction, and the default is ``input``. A set
    of discretes is more often something read -- a deployment switch, an
    arming line -- than something driven, and a line requested as an input is
    one the kernel will not write, so the direction decides the request and
    cannot be deduced from anything else the file says.

Requirement
    ``edge``, ``bias`` and ``debounce_us`` are refused on an output rather
    than ignored. They are about watching a line, an output has no edges to
    report and nothing to debounce, and a file that states one of them for a
    driven line is saying something it does not mean -- commonly a set of
    lines whose direction was changed and whose other terms were left as they
    were. The refusal names the term that was missed.

Requirement
    A GPIO payload cannot be triggered, and only one of a read and a write is
    open to it. An input refuses a write -- there is nothing to write a
    request to, and driving one would be commanding a payload rather than
    reading it, as the IIO endpoint says of its own. An output refuses a read:
    a driven line reports nothing, so reading one would be reporting what this
    node itself asked for, and a configuration that said ``output`` where it
    meant ``input`` is told so rather than left waiting for events that never
    come. Each refusal names the direction that would allow what was tried.

Requirement
    Driven lines are driven together, to the one state, by a value on a link
    -- the text ``1`` or ``0``, and nothing else. There is nothing in the
    configuration to say which line of a set is which, so a pattern would
    have no meaning; and three heater circuits switched one at a time would be
    three thermal states of a vehicle on the way to the one asked for. A datum
    that is neither ``1`` nor ``0`` is refused and not guessed at, because what
    is on the other end of an output is a heater or a valve.

Requirement
    The bits and the mask of a set-values call are indexed by a line's place
    in the request and not by its offset on the chip, which is how the v2
    interface counts. Every line of the request appears in the mask whichever
    way they are going: a mask of zero is a call that says nothing.

Requirement
    The request names the handler as its consumer. A line held by something
    unnamed is the hardest type of conflict to find, and ``gpioinfo`` shows
    the name beside the line.

Requirement
    The line request's descriptor is read without waiting. A read gathers
    every event that is waiting, so a blocking descriptor would have it wait
    for the next one after collecting the last -- a conduit stopped until a
    line happened to change again, holding the events it had already
    gathered.

Requirement
    The kernel's structures are declared in the endpoint and their sizes
    asserted against the kernel's own. ``libc`` has no GPIO character-device
    definitions, and a structure of the wrong size is an ioctl that reads past
    what it was handed -- which nothing else in a test would notice.

Requirement
    The simulator moves lines through ``gpio-sim``, which it finds rather than
    makes. Nothing in user space can drive a wire, so what stands in is a chip
    whose lines exist in memory with a file per line saying what it is pulled
    to; loading the module and making a chip need root, so a payload whose
    chip is not there says how to make one -- as the I2C stand-in does.

Requirement
    A simulated chip is found by the node it became and not by its number. A
    ``gpio-sim`` chip's node is numbered as chips appear, exactly as an
    ``i2c-stub`` adapter's bus is, so the number a payload set names is not an
    identity.

Requirement
    The packet and segment sizes and the faults mean nothing to a simulated
    set of discretes, and it says so rather than appearing to obey them. There
    are no bytes to size, drop, corrupt or truncate: what it produces is an
    edge on each of its lines every packet interval.

rpmsg Payloads: Another Core
^^^^^^^^^^^^^^^^^^^^^^^^^^^^
**Experimental, not yet supported.**

A flight computer is commonly one package with cores of more than one type in
it, and the payload is then software on another core rather than anything on a
wire. Linux talks to it over rpmsg: the remote announces a named channel,
Linux makes a character device for it, and the two ends exchange whole
messages through that device.

The rules are in ``tcslibgs::endpoint_config_rpmsg``, the endpoint in
``tcspecial::endpoint_rpmsg`` and the stand-in in ``tcssim::payload_rpmsg``.

Requirement
    An rpmsg payload states a ``path`` and a ``service``, and the service is
    required. A node's number is not an identity: it is assigned as channels
    announce themselves, so the node a file names can be another core's
    channel after a change in boot order -- the hazard an I2C stub adapter
    has with its bus number.

Requirement
    The handler holds the node it opened against that service, where the
    kernel will say. ``/sys/class/rpmsg/<node>/name`` is the name the remote
    announced; a node that is not the channel the file asked for is refused
    rather than exchanged with, because the alternative is a handler trading
    telemetry with the wrong software.

Requirement
    Where the kernel publishes no name, nothing is refused. A stand-in has no
    ``sysfs`` entry and no node number to move, and the check exists to catch
    a number that moved.

Requirement
    A ``packet_size`` larger than one message is refused where the file is
    read. rpmsg messages go in buffers whose size the transport fixes -- 496
    bytes after the header on the usual virtio transport -- and rpmsg does not
    fragment, so the remote would receive the first part of a packet with no
    way of knowing the rest was coming. A platform with other buffers states
    ``max_message_bytes``.

Requirement
    An rpmsg payload has no stream section: a read hands over one whole
    message and never part of one.

Requirement
    An rpmsg payload may be triggered. The channel carries both directions, so
    the handler writes a request and the remote answers -- which is what an
    IIO payload cannot do and a CAN payload can.

Requirement
    The stand-in is a pseudo-terminal, which carries the bytes both ways and
    keeps no message boundaries. A payload that writes faster than the handler
    reads will therefore have two messages arrive as one read, where the real
    driver would hand over one per read; said here because it is the one way
    the stand-in differs from the thing.

IIO Payloads: a Sensor's Buffer
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
**Experimental, not yet supported.**

The kernel's Industrial I/O subsystem is what a sensor looks like on Linux. A
program that wants a stream of samples enables the channels it cares about and
a buffer, both through ``sysfs``, and then reads ``/dev/iio:deviceN``, which
hands over packed sample sets as they become ready.

That is a device file a handler reads, which is nearly a ``device-file``
payload.
What makes it a type of its own is that it is read-only. The rules are in
``tcslibgs::node::iio``, the endpoint in ``tcspecial::node::iio`` and the
stand-in in ``tcssim::node::iio``.

Requirement
    An IIO payload is located by a path and states nothing else. It took two
    terms once -- a number of ``channels`` and a ``sample_rate`` -- and both
    were recorded and never applied: channels and buffers are enabled through
    ``sysfs`` by whoever brought the sensor up, and a handler that wrote there
    would be reconfiguring a sensor other programs may be reading. A payload
    documents and takes only what it can set, so a file that states either is
    refused and told which word and why.

Requirement
    An IIO payload has no stream section. A buffer read hands over whole
    sample sets and never part of one, so there is nothing for a rule about
    where a read ends to say.

Requirement
    An IIO payload cannot be triggered, and the refusal is in the payload
    configuration rather than in the simulator. It is not that a stand-in
    cannot hear a request: a sample buffer is a stream out of the sensor, so
    there is nothing on that descriptor to write and nowhere for a trigger to
    go. A datagram payload is refused in the same place for the same type of
    reason.

Requirement
    The endpoint refuses a write rather than attempting one, and says what an
    IIO payload is. The write would fail anyway -- the device is opened for
    reading -- but a driver's error says nothing about why, where this says
    that a sensor's settings are in sysfs and not on this descriptor.

Requirement
    The stand-in is a pseudo-terminal, as a serial line's is, and stands in
    for the data and not the ``sysfs`` around it. The kernel's ``iio_dummy``
    module would be a sensor and is the wrong shape for this: it generates
    values of its own, so a simulator file would decide nothing about what the
    payload produced. What is missing from the stand-in is missing from the
    handler too, which touches none of those attributes.

CAN Payloads: a Bus of Peers
^^^^^^^^^^^^^^^^^^^^^^^^^^^^
**Experimental, not yet supported.**

A CAN controller is a network interface rather than a device file, so a CAN
payload is located by an interface and by the identifier its frames carry.
Both ends of the link are sockets on that interface, which is what makes this
the one experimental type whose stand-in is as faithful as a socket: a bus
carries every member's frames to every other member, so the simulator joins
the bus rather than standing in for something on it.

The rules are in ``tcslibgs::endpoint_config_can``, the endpoint in
``tcspecial::endpoint_can`` and the stand-in in ``tcssim::payload_can``.

Requirement
    A CAN payload states an ``interface`` and a ``can_id``, and neither is
    optional. The interface is where the bus is; the identifier is what tells
    one payload on a shared bus from another, as an address tells one device
    on an I\ :superscript:`2`\ C bus from another.

Requirement
    A CAN payload states no ``path``, ``address``, ``port`` or ``protocol``,
    and an ``interface`` or a ``can_id`` on any other type is refused. A
    controller is not a device file and not a host and port, and a line that
    named an interface would be a line written for a bus it is not on.

Requirement
    Two payloads may share an interface and may not share an identifier. A
    bus is shared by design -- the reason an I\ :superscript:`2`\ C payload
    claims its address rather than its bus -- and two handlers filtering on
    one identifier would each read the other's frames and count them as its
    own.

Requirement
    A ``packet_size`` a frame cannot carry is refused where the file is read.
    A classic frame carries eight bytes and a CAN FD frame sixty-four, both
    exactly, and CAN does not fragment: a payload whose packets are larger
    would be a handler that could never send one, discovered a frame at a
    time at run time rather than when the file was read.

Requirement
    The identifier's format follows from its value. The base range is eleven
    bits and the extended range twenty-nine, and the two do not overlap, so a
    file stating both a value and a format would have two answers to one
    question. An identifier above the extended range is refused.

Requirement
    A handler's socket is filtered to its payload's identifier. A bus carries
    everybody's traffic, and a handler that read the lot would count another
    payload's frames as its own -- which is the one thing a bus makes easy to
    get wrong.

Requirement
    A CAN payload states no ``bitrate``. It is set where the interface is
    brought up, by the platform or by ``ip link``, and a handler that opens a
    socket on it cannot change it -- so a file stating one would be stating a
    value nothing applies. What a socket *can* set it does state: the frame
    format and the loopback.

Requirement
    A CAN payload may be triggered. Both ends are peers on the bus, so a
    simulated payload really does hear the handler ask -- which a payload on
    an I\ :superscript:`2`\ C bus cannot, the request there being the
    master's own read. The trigger goes out with the payload's own identifier,
    and the socket does not receive its own frames, so a handler never reads
    back the trigger it just sent.

Requirement
    The simulator finds the bus and does not make one. Creating a virtual bus
    and bringing it up need root, so a simulated payload whose interface is
    not there says so and names the two commands that would make one -- as the
    I\ :superscript:`2`\ C stand-in does for its kernel module.

Serial Payloads: a Line
^^^^^^^^^^^^^^^^^^^^^^^
A start-stop line: each byte delimited by its own start and stop bits, which is
what ``termios`` drives. A line whose bits are carried on a clock is not one of
these -- Linux drives such a line as a network interface rather than as a
device file -- and is an `HDLC Payloads: a Synchronous Line`_ instead.

Requirement
    This type is start-stop framing and nothing else. It used to be both,
    reached by saying ``asynchronous: true`` or ``false``, and almost nothing
    about the two is the same: one is located by a path and the other by an
    interface, one is a stream and the other frames, one is set with termios on
    a file descriptor and the other with an ioctl on a socket. Two types with
    one word between them was the arrangement that let a file state a clock
    nothing applied.

Requirement
    ``stop_bits`` is required. A stop bit is what start-stop framing uses in
    place of a clock, so a line that states none is not a line of this type.

Requirement
    A clocked line's terms are refused by name on one of these: ``standard``,
    ``clock_type``, ``clock_rate``, ``encoding``, ``frame_check`` and
    ``loopback`` belong to the ``hdlc`` type, and a start-stop line shares no
    clock.

Requirement
    ``parity`` is what termios sets per character -- ``none``, ``even``,
    ``odd``, ``mark`` or ``space`` -- and a frame check is not one of them. A
    file writing a CRC here is refused with the five a character can have.

Requirement
    A line is read back after it is set, and a parity the port did not take is
    an error. A pseudo-terminal -- which is what a simulated line is -- rejects
    even parity outright and quietly drops the rest, so a handler that did not
    look would be checking nothing while its configuration said it was
    checking every character. The message names the parity and says the port
    is what will not have it.


RS-485: a Two-Wire Bus
^^^^^^^^^^^^^^^^^^^^^^
**Experimental, not yet supported.**

A start-stop line may be half of an RS-485 bus rather than one end of a
point-to-point link: one pair of wires, several transmitters, and each of them
obliged to stop driving the pair the instant it has finished. The turnaround
is a few bit times -- a write returns before the last bit has left the shift
register -- so Linux does it in the UART driver, told through ``TIOCSRS485``.

RS-485 is not a type of link. It is the electrical layer under a start-stop
line: the same termios terms, the same framing, the same device file. So these
are terms of the ``serial`` type, in ``tcslibgs::endpoint_config_rs485``, and a
payload on a four-wire line states none of them and is unchanged.

Requirement
    A line states ``rs485: true`` to be a bus, and says nothing to be a plain
    line. Absent is what every payload set written before this says, and what
    an RS-422 line says.

Requirement
    A line that states ``rs485: true`` states ``rs485_rts_on_send``. Which
    level of RTS drives the transceiver is a property of the board rather than
    of the protocol -- the signal commonly reaches the enable pin through an
    inverter -- so a line given a default would have an even chance of
    transmitting exactly when it should be listening.

Requirement
    The turnaround delays default to nought and are refused above a second. A
    turnaround is measured in bit times, so a file asking for seconds of it
    has mistaken the unit, and a line that held the bus that long would look
    like one that had failed.

Requirement
    A turnaround stated without ``rs485: true`` is refused rather than kept
    against the day it is turned on. A file that set one and left the mode off
    has described a bus whose driver is never released, and reading it as a
    plain line would be reading a different file from the one that was
    written.

Requirement
    None of it applies to an ``hdlc`` payload, and there is no word for it
    there. The kernel's RS-485 support is part of its start-stop drivers,
    where a synchronous line is driven through generic HDLC and a network
    interface instead.

Requirement
    The mode is applied and read back, and a line that did not take it is
    refused rather than started. A driver that does not support the mode may
    take the call and clear the flag, and a line left in point-to-point mode
    on a two-wire bus transmits into every other transmitter on it --
    including the payload this handler is trying to read.

Requirement
    A simulated line cannot be an RS-485 bus and says so. A pseudo-terminal
    has no transceiver and no RTS to drive, so it refuses the mode, and a set
    meant to be run against tcssim states no ``rs485`` -- as it states
    ``parity: none`` for the same reason.

**A line's terms**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| datarate      | Yes      | Line rate in bits per second                     |
+---------------+----------+--------------------------------------------------+
| stop_bits     | Yes      | Stop bits after each byte: ``1``, ``1.5``,       |
|               |          | ``2``                                            |
+---------------+----------+--------------------------------------------------+
| parity        | No       | ``none``, ``even``, ``odd``, ``mark`` or         |
|               |          | ``space``, defaulting to ``none``                |
+---------------+----------+--------------------------------------------------+
| byte_length   | Yes      | Data bits in each byte, from 5 to 8              |
+---------------+----------+--------------------------------------------------+
| stream        | Yes      | Stream payload protocol attributes, below        |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet. Shared by every type of     |
|               |          | payload, and listed with the rest above          |
+---------------+----------+--------------------------------------------------+

Requirement
    A serial payload has a stream section, because a serial port is a stream and
    so there is always a rule deciding where one read ends.

HDLC Payloads: a Synchronous Line
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
**Experimental, not yet supported, and the one type with nothing to be run
against.**

A synchronous serial line: bits carried on a clock, framed as HDLC. Linux calls
the mechanism **generic HDLC**, and what it presents is a network interface --
``hdlc0`` -- not a device file. The terms of the line are set with one ioctl on
it, ``SIOCWANDEV``, which carries whichever of the structures in
``linux/hdlc/ioctl.h`` its ``type`` field names; the data is frames on that
interface. The rules are in ``tcslibgs::node::hdlc``, the endpoint in
``tcspecial::node::hdlc``, and there is no stand-in at all.

Requirement
    An hdlc payload is located by an ``interface`` and nothing else. The kernel
    drives such a line as a network interface, so there is no file to open; a
    payload stating a ``path`` is refused and told which word to use.

Requirement
    One interface is one payload. A raw HDLC frame carries no address, so there
    is nothing to tell two payloads on one line apart -- which is what makes
    this a claim on the whole interface, where a CAN payload's claim is on an
    identifier and the bus is shared by design.

Requirement
    The terms are the kernel's own two structures, and nothing beyond them.
    ``standard``, ``clock_type``, ``clock_rate`` and ``loopback`` are
    ``sync_serial_settings`` -- the port's wiring -- and ``encoding`` and
    ``frame_check`` are ``raw_hdlc_proto``, which is what the bits mean once
    the port is clocked. The values are the kernel's, word for word.

Requirement
    ``standard`` and ``clock_type`` are required. A line carries no
    negotiation, so both ends must be told the same thing, and a line read with
    the wrong one of either delivers noise rather than nothing. The encoding
    and the frame check default to the kernel's own plain values -- ``nrz`` and
    ``none`` -- a short cable commonly wanting neither clock recovery nor a
    CRC.

Requirement
    ``t1`` and ``e1`` are not standards this takes. They are telco framings
    rather than a port's wiring, and the kernel takes a different structure for
    them -- ``te1_settings``, with a map of which timeslots the link owns --
    which nothing here states. A file asking for one is told that rather than
    being given a port set up as something else.

Requirement
    ``clock_rate`` belongs to whoever generates the clock: required with
    ``internal`` or ``tx_internal``, where this end drives it, and refused with
    ``external`` or ``tx_from_rx``, where the far end does. A rate this end
    cannot set is a number it has no business stating, which is the rule the
    whole type was rewritten under.

Requirement
    The frame check is called ``frame_check`` and not ``parity``. The kernel's
    field is named ``parity`` for both types of line, and on a synchronous one
    it is a CRC over a frame rather than a bit per character: one word for the
    two was what made a start-stop line's parity and a frame check look like
    one term.

Requirement
    Both halves are read back after they are set, and a line that reports
    something else is refused. ``IF_GET_IFACE`` and ``IF_GET_PROTO`` exist for
    exactly this: a card may take the call and keep less than it was given, and
    a line whose frame check is not the one the configuration asked for is a
    line checking nothing while the file says it checks every frame. It is the
    rule a start-stop line already follows with its parity.

Requirement
    The port's wiring is set before the framing. The framing is attached to a
    port that is already clocked, which is the order ``sethdlc`` uses and the
    order the drivers expect.

Requirement
    The frames move over an ``AF_PACKET`` socket of type ``SOCK_DGRAM`` bound
    to the interface. ``SOCK_DGRAM`` rather than ``SOCK_RAW`` because a raw
    HDLC frame has no link header worth seeing: a read hands over one frame's
    payload and a write puts one payload into one frame, as on a CAN bus.

Requirement
    An hdlc payload has no stream section. A frame is its own length, so there
    is nothing for a rule about where a read ends to say.

Requirement
    An hdlc payload may be triggered. Both ends are peers on the line, so a
    far end really does hear the handler ask -- as on a CAN bus, and unlike on
    an I\ :superscript:`2`\ C bus where the request is the master's own read.

Requirement
    Bringing the interface up is not this software's business, as it is not for
    a CAN controller: ``ip link set dev hdlc0 up``, from whatever starts the
    vehicle. What this does is set the line's terms and move its frames.

**A line's terms**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| interface     | Yes      | The interface the card presents: ``hdlc0``       |
+---------------+----------+--------------------------------------------------+
| standard      | Yes      | How the port is wired: ``v35``, ``v24``,         |
|               |          | ``x21`` or ``x21d``                              |
+---------------+----------+--------------------------------------------------+
| clock_type    | Yes      | Whose clock the bits are on: ``external``,       |
|               |          | ``internal``, ``tx_internal``, ``tx_from_rx``    |
+---------------+----------+--------------------------------------------------+
| clock_rate    | If ours  | Bits per second, where this end generates the    |
|               |          | clock. Refused where the far end does            |
+---------------+----------+--------------------------------------------------+
| encoding      | No       | How the bits are carried: ``nrz`` (the default), |
|               |          | ``nrzi``, ``fm_mark``, ``fm_space``,             |
|               |          | ``manchester``                                   |
+---------------+----------+--------------------------------------------------+
| frame_check   | No       | ``none`` (the default), ``crc16_pr0``,           |
|               |          | ``crc16_pr1``, ``crc16_pr0_ccitt``,              |
|               |          | ``crc16_pr1_ccitt``, ``crc32_pr0_ccitt`` or      |
|               |          | ``crc32_pr1_ccitt``                              |
+---------------+----------+--------------------------------------------------+
| loopback      | No       | ``true`` to put the port in loopback, for        |
|               |          | proving a card with no line attached. Default    |
|               |          | ``false``                                        |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet. Shared by every type of     |
|               |          | payload, and listed with the rest above          |
+---------------+----------+--------------------------------------------------+

I2C Payloads: a Bus
^^^^^^^^^^^^^^^^^^^
A serial port is configured by describing how a byte is framed on the wire.
I2C needs none of that, because the protocol fixes it: every byte is eight
data bits, most significant bit first, followed by an acknowledge bit. What a
payload does carry is how the master addresses the device and how far it will go
to complete a transfer.

**A bus's terms**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| ten_bit       | No       | Address the device with 10 address bits rather   |
|               |          | than 7. Default ``false``                        |
+---------------+----------+--------------------------------------------------+
| pec           | No       | Append an SMBus packet error check, a CRC-8, to  |
|               |          | each transfer. Default ``false``                 |
+---------------+----------+--------------------------------------------------+
| retries       | No       | Times a transfer is retried after a lost         |
|               |          | arbitration or an unexpected NAK. Default ``0``  |
+---------------+----------+--------------------------------------------------+
| timeout       | No       | How long a transfer waits before it fails        |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet. Shared by every type of     |
|               |          | payload, and listed with the rest above          |
+---------------+----------+--------------------------------------------------+

Requirement
    An I2C payload specifies no byte length, parity, or stop bits, because the
    protocol fixes the framing of a byte and leaves nothing to configure.

Requirement
    An I2C payload has no stream section, because the master clocks exactly as
    many bytes as it asks for and a transfer is therefore already bounded.

Requirement
    With 7-bit addressing, an address names a device only in the range
    ``0x08`` to ``0x77``. The specification keeps ``0x00`` to ``0x07`` for the
    general call, the CBUS address, and the high-speed master code, and
    ``0x78`` to ``0x7F`` for the 10-bit addressing prefix and for future use.
    A payload given one of those is reported as an error rather than
    accepted as a device that will never answer.

Requirement
    With 10-bit addressing there is no reserved block, and the whole range
    ``0x000`` to ``0x3FF`` is available. A 10-bit transfer is introduced by
    the ``0x78`` prefix and carries its address in the bytes that follow, so
    the reservations that apply to a 7-bit address do not apply to it.

Requirement
    A timeout takes effect rounded up to a multiple of 10 ms, that being the
    resolution the bus driver keeps it in. The value the file gave is kept as
    it was written, so that what a reader of the file sees and what the bus
    was asked for can both be reported.

Requirement
    An I2C payload states no ``bus_speed``. The bus clock belongs to the
    controller, which the platform configures from the device tree or from
    ACPI, and a program holding an endpoint open cannot change it -- so a file
    stating one would be stating a value nothing applies. It was a term once,
    recorded and never applied, and is refused by name now.

SPI Payloads: a Peripheral
^^^^^^^^^^^^^^^^^^^^^^^^^^
SPI is clocked and full duplex, so, as with I2C, there is no parity and there
are no stop bits; a transfer is delimited by the chip select rather than by
framing bits. What a payload carries is the shape of the clock and of a word.

**A peripheral's terms**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| max_speed     | Yes      | Greatest clock rate the peripheral accepts, in   |
|               |          | Hz                                               |
+---------------+----------+--------------------------------------------------+
| spi_mode      | Yes      | Clock polarity and phase: ``0``, ``1``, ``2``,   |
|               |          | ``3``                                            |
+---------------+----------+--------------------------------------------------+
| bits_per_word | No       | Bits in each word. Default ``8``                 |
+---------------+----------+--------------------------------------------------+
| bit_order     | No       | Bit sent first: ``msb`` or ``lsb``. Default      |
|               |          | ``msb``                                          |
+---------------+----------+--------------------------------------------------+
| cs_active     | No       | Chip select asserted ``low`` or ``high``.        |
|               |          | Default ``low``                                  |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet. Shared by every type of     |
|               |          | payload, and listed with the rest above          |
+---------------+----------+--------------------------------------------------+

Requirement
    A SPI payload specifies ``spi_mode``, because the controller has no default that
    is right for every peripheral and a mismatched mode fails the way a
    mismatched data rate fails on a serial line.

Requirement
    A SPI payload has no stream section, because the master clocks exactly as
    many words as it asks for and a transfer is therefore already bounded.

Requirement
    The clock rate is an upper bound. A controller divides its own clock down
    and so runs at the greatest rate it can produce that does not exceed the
    rate the payload gives.

Where a Read of a Line Ends
^^^^^^^^^^^^^^^^^^^^^^^^^^^
A stream delivers bytes with no frame boundaries of its own, so a payload
on one needs to be told where one read ends. Three attributes say so.

**Stream attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| max_length    | Yes      | Longest payload one read may deliver, in bytes   |
+---------------+----------+--------------------------------------------------+
| timeout       | See      | How long a read waits, or ``none``               |
|               | below    |                                                  |
+---------------+----------+--------------------------------------------------+
| terminators   | See      | One or more byte values, each of which ends a    |
|               | below    | read                                             |
+---------------+----------+--------------------------------------------------+

Requirement
    A stream section specifies the maximum data length.

Requirement
    A stream section specifies the timeout, the terminator list, or both.

Requirement
    Specifying both a timeout and a terminator list is valid. A read then ends
    on whichever condition occurs first.

Requirement
    A terminator list contains at least one byte value. An empty list is an
    error, because it states a terminator list without naming a terminator.

Requirement
    To read a fixed number of bytes at a time, with no timeout and no
    terminator, a stream section specifies a timeout of ``none``.

The last requirement deserves a word, because it is the one case where the
format asks for something to be stated that could have been left implicit.
A stream section giving only a maximum length would be ambiguous: it reads
either as "hand me this many bytes at a time", which is a real and useful
intent, or as a file whose author forgot to say when a read should end. These
have different consequences on a link that goes quiet, so the format does not
guess. Writing ``timeout: none`` says the first explicitly, and a section with
neither attribute is reported as an error rather than being taken for it.

Value Syntax
^^^^^^^^^^^^
XML carries every attribute value as text, while YAML distinguishes
numbers from strings. Both are accepted everywhere a value is expected, so the
same value may be written ``8`` or ``"8"`` without changing its meaning.

**Value syntax**

+---------------+--------------------------------------------------------------+
| Type          | Syntax                                                       |
+===============+==============================================================+
| Whole number  | Decimal, or hexadecimal with an ``0x`` prefix                |
+---------------+--------------------------------------------------------------+
| Byte value    | Decimal or ``0x`` hexadecimal, from 0 to 255                 |
+---------------+--------------------------------------------------------------+
| Timeout       | A whole number with a unit of ``us``, ``ms``, or ``s``, or   |
|               | the word ``none``                                            |
+---------------+--------------------------------------------------------------+
| Byte list     | A sequence, or one string of values separated by commas or   |
|               | whitespace                                                   |
+---------------+--------------------------------------------------------------+
| Flag          | ``true`` or ``false``                                        |
+---------------+--------------------------------------------------------------+
| Bus address   | Decimal or ``0x`` hexadecimal, within the width the          |
|               | payload's addressing allows, and not one the bus reserves    |
+---------------+--------------------------------------------------------------+

Requirement
    A timeout carries an explicit unit. A bare number is an error, so that a
    file cannot silently mean milliseconds where seconds were intended.

Requirement
    A timeout of zero is an error. A read that is not to wait is written
    ``none``.

Requirement
    A hexadecimal value may be written as a number or as a string: ``0x48`` or
    ``"0x48"``. Both formats accept either spelling and both mean the same
    value.

Attribute names may be spelled with either underscores or hyphens, so
``max_length`` and ``max-length`` are the same attribute.
Example
-------
This is the shipped tcspecial1.yaml. ``udp-steady`` and ``udp-fast`` are
reached the same way, over a UDP socket on this host, so what they share is a
group and what tells them apart stays with each of them. ``tcp-stream`` and
``random-device`` share nothing with anything and so state every attribute for
themselves. Each payload is named for what it is: a set whose payloads were
called DH0 to DH3 said which handler each was and nothing else, and the name
is what a panel in the MOC shows.

.. code-block:: yaml

   version: "1.0"
   description: TCSpecial payload set 1

   payload_groups:
     - name: udp_localhost
       type: network
       protocol: udp
       address: localhost

   payloads:
     - name: tcp-stream
       oc_address: 127.0.0.1
       oc_port: 6000
       type: network
       protocol: tcp
       address: localhost
       port: 5000
       packet_size: 12

     - name: udp-steady
       oc_address: 127.0.0.1
       oc_port: 6001
       group: udp_localhost
       port: 5001
       packet_size: 11

     - name: random-device
       oc_address: 127.0.0.1
       oc_port: 6002
       type: device-file
       path: /dev/urandom
       packet_size: 1

     - name: udp-fast
       oc_address: 127.0.0.1
       oc_port: 6003
       group: udp_localhost
       port: 5003
       packet_size: 15

Every handler here shares an OC address and has a port of its own. The OC
address could have been a group attribute for that reason; it is written out
per handler so that the file reads as the four handlers it describes rather
than as three attributes and an exception.

Simulator Configuration Files
=============================
The tcspecial configuration describes payloads. Simulating one takes more than
that description holds, and the extra is not payload configuration: how often a
payload produces a packet, and how it divides a packet into segments, are
choices about a simulation. They are stated in a separate file, which only
tcssim acts on. The shipped pair is tcspecial1.yaml and tcspecial1sim.yaml; the
parser is ``tcslibgs::sim_config``.

Requirement
    The parser lives with the other three configuration languages rather than
    inside tcssim. Both GUIs read this file now -- one to simulate payloads
    and one to show what a payload set says about them -- and a configuration
    language read by two programs cannot live inside one of them.

The format is chosen from the extension exactly as every other configuration
file's is, so a simulator configuration may be written in YAML or XML.

File Structure
--------------
A simulator configuration file has two sections, both optional, and a version
and description of its own.

simulated payload groups
    Named groups of settings. A group carries what several simulated payloads
    have in common.

simulated payloads
    The simulated payloads themselves. Each names the data handler it stands in
    for, may name a group, and may state settings of its own.

**Settings**

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| packet_interval_ms  | number | Milliseconds between packets. 0 is as fast  |
|                     |        | as the payload can be driven                |
+---------------------+--------+---------------------------------------------+
| segment_interval_ms | number | Milliseconds between the segments of one    |
|                     |        | packet. Defaults to the packet interval     |
+---------------------+--------+---------------------------------------------+
| segment_size        | number | Bytes in one segment. Defaults to the whole |
|                     |        | packet, whose size the payload configuration|
|                     |        | gives                                       |
+---------------------+--------+---------------------------------------------+

**What it stands in for.** Neither of these is a setting of the simulation.
The payload configuration decides both, and they are stated here again only so
that the two files can be checked against each other.

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| type                | string | The type of payload this stands in for, in  |
|                     |        | the payload configuration's own word:       |
|                     |        | ``network``, ``device-file``, ``serial``,   |
|                     |        | ``i2c``, or ``spi``                         |
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
| jitter_ms              | number | Milliseconds a packet may be late,       |
|                        |        | chosen afresh for each one               |
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

What a Simulated Payload Stands In For
--------------------------------------
The two files are joined by handler name and by nothing else, and a name is
exactly what survives a payload set copied from another one. Two sets that
differ only in how their payload is reached -- one a datagram socket, one a
stream -- therefore have simulator files that each parse perfectly well
against the other's payloads, and the run that follows looks like a
payload that never sends. So each simulated payload says what it stands in
for, and the statement is compared rather than used.

Requirement
    A simulated payload states the type of payload it stands in for, and a
    network payload the transport as well. Neither is taken from this file:
    the payload configuration decides both, and a payload that stated neither
    could not be checked against anything.

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
    would be a second place to keep in step and nothing gained: ``address``
    and ``port`` written here are refused with the file they belong in, and
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
    A group may carry the type and the transport, like every other setting a
    payload does not state. A group whose payloads are not all reached the
    same way leaves them with the payloads: the shipped ``tcspecial1sim.yaml``
    has two payloads sharing a rate that do not share a transport.

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
This is the shipped tcspecial1sim.yaml, which simulates the four data handlers of
tcspecial1.yaml. Two of them are driven alike and so share a group; the last
states its own rate instead.

.. code-block:: yaml

   version: "1.0"
   description: Simulator settings for the payloads of tcspecial1.yaml

   simulated_payload_groups:
     - name: steady_1hz
       packet_interval_ms: 1000

     - name: continuous
       packet_interval_ms: 0

   simulated_payloads:
     - name: tcp-stream
       type: network
       protocol: tcp
       group: steady_1hz

     - name: udp-steady
       type: network
       protocol: udp
       group: steady_1hz

     - name: random-device
       type: device-file
       group: continuous

     - name: udp-fast
       type: network
       protocol: udp
       packet_interval_ms: 500

Testing
=======
There are two components of testing software. Tcssim is a GUI used to simulate
payloads and tcsmoc is used to simulate the MOC. Both allow user interaction
to change parameters and see what result the changes produce. A third program,
tcsverify, runs none of it: it reads the files the other three read and says
what is wrong with them.

**High-Level View of Test Configuration**

.. code-block:: text

          GROUND              :                     SPACE
   ===========================:===================================================
                              :
   +=======================+  :    +=========================+     +=================+
   ||  Simulated MOC      ||  :    ||  Flight Software      ||     ||  Simulated    ||
   ||                     ||  :    ||                       ||     || Payloads      ||
   +=======================+  :    +=========================+     +=================+
   |  +---------+          |  :    |    +=================+  |     |                 |
   |  | tcsmoc  |          |  :  ----+->| tcspecial      |  |     |                 |
   |  | Control |          |  : |  | |  | (tcslibgs)      |  |     |                 |
   |  | S/W     |          |  : |  | |  +-----------------+  |     |                 |
   |  +---------+          |  : |  | |  | Command         |  |     |                 |
   |       ^               |  : |  | +->| Interpreter     |  |     |                 |
   |       |               |  : |  | |  +-----------------+  |     |  +-----------+  |
   |       v               |  : |  | |  | Data            |<-------|->| Payload 0 |  |
   |  +-----------------+  |  : |  | +->| Handler 0       |  |     |  +-----------+  |
   |  | tcslib          |<------   | |  +-----------------+  |     |  +-----------+  |
   |  | (tcslibgs)      |  |  :    | |  | Data            |<--------->| Payload 1 |  |
   |  +-----------------+  |  :    | |  | Handler 1       |  |     |  +-----------+  |
   |  | tcspecial1.yaml |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    | tcspecial1.yaml |  |     |                 |
                              :    |    +-----------------+  |     |                 |
                              :    +-------------------------+     +-----------------+

tcssim
------
Tcssim is a GUI simulating the payloads. It reads two files. The payload
definition comes from tcspecial1.yaml, named by the ``SIM_PAYLOAD_CONFIG_PATH``
environment variable when that is set, and says which payloads exist, how each
is reached, and how big its packets are. What simulating them takes comes from
tcspecial1sim.yaml, named by ``PAYLOAD_SIM_YAML``; see `Simulator Configuration
Files`_.

Requirement
    A tcspecial configuration file describes payloads and says nothing about
    simulating them. How often a payload produces a packet, and how a packet is
    divided into segments, are properties of a simulation rather than of a
    payload, and are stated in the simulator configuration file.

The two are joined by name: a simulated payload names the data handler it stands
in for. Neither file is read in the order of the other, and neither may be
silently short.

Requirement
    Every data handler of the tcspecial configuration has a simulated payload
    naming it, and every simulated payload names a data handler of the
    tcspecial configuration. Either type of mismatch is reported rather than
    simulated around, because a misspelled name is otherwise indistinguishable
    from a payload deliberately left out.

Which Types Can Be Simulated
^^^^^^^^^^^^^^^^^^^^^^^^^^^^
A simulated payload has to be whatever is at the far end of its handler's
payload endpoint, and the types of endpoint are not equally easy to be. The
experimental types are covered where each of them is: a CAN bus of peers, a
sensor's buffer, a channel to another core and a set of discrete lines.

Requirement
    A synchronous line is the one type nothing stands in for, and the
    simulator says so. Every other type has something in the kernel to be --
    ``vcan``, ``i2c-stub``, ``gpio-sim``, ``iio_dummy``, or a pseudo-terminal
    -- and generic HDLC has nothing: there is no virtual HDLC interface, so
    such a payload needs a card. A set with one in it is refused where the
    simulator is asked to stand in for it, naming the interface and what a
    card would be, rather than becoming a payload that looks simulated and is
    not.

Requirement
    A simulated payload of a network endpoint is a socket of the configured
    protocol, for all four of them. A stream payload listens; a datagram
    payload sends to where the handler waits.

The four divide in two, and which end binds divides with them. A TCP or
Unix-stream payload listens at the configured address and the handler connects
to it. A UDP or Unix-datagram payload sends first, so the handler binds that
address and the payload reaches out to it.

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
    ``/dev/ttyS0`` to stand in for it would be worse than one that could not,
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

This is the weakest of the stand-ins and the one place the simulator is openly
not the thing it stands for. Nothing emulates a SPI peripheral: a peripheral is
what a controller clocks, and nothing in user space can be clocked. What a pty
offers is the bytes -- a packet, in its segments, at its pace -- and no clock,
no mode, no chip select and no word width.

Requirement
    A handler takes a terminal at the end of a SPI path as a stand-in for a
    peripheral: it carries bytes on it, says once in the log that it is doing
    so, and does not apply the configured clocking to it, there being nothing
    to apply it to. Anything else that is not a spidev node is refused.

The narrowness is the point. A terminal is what a stand-in is, and a handler
that accepted any node it could open would open a wrongly configured path
confidently -- which is the fault the types were separated to prevent.

What such a link tests is a handler's reading and writing of a peripheral and
the pacing of a payload that produces data in blocks. What it cannot test is
whether the peripheral would stand being clocked that way, which is a question
for the peripheral.

So every type of endpoint, and every protocol of the network type, can be
simulated. The stand-ins are not all of one quality, and that is the thing to
keep in view rather than the coverage: a socket is a socket and a pty is a
line in all but its rate, while a bus is a register bank that every master
shares and a SPI peripheral is a pty with no clock behind it. Each type's own
file says which it is.

Requirement
    A handler opens a payload device with ``O_NOCTTY``. A device that is a
    terminal -- a real serial port, or a pty standing in for one -- otherwise
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
    The window decides neither. It asks for the two sizes to be judged and
    obeys the answer -- showing what it says and disabling the button while
    it says anything -- so there is one rule rather than one in each place
    that acts on it.

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
    A payload that failed to start offers ``Enable`` again. It is not
    sending, which is what its status says, and the useful thing to offer is
    the start that failed rather than a stop of something that never began.

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
    controls; the other is how a stand-in behaves and reaches no spacecraft at
    all. A list that mixed them would have someone reading a drop percentage
    as something a payload does.

Requirement
    Nothing shown is a measurement. What a payload has done is on the panel
    itself, and a list that mixed the two would leave no way to tell a
    configured value from an observed one.

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

The four payloads of tcspecial1.yaml give a row of three and a row of one in a
1062x668 window. The size is the one ``window_size`` gives, which a test reads
out of this document so that the number here cannot quietly stop being the one
the program opens at.



tcsmoc
------
The tcsmoc is a GUI program used to control simulated payloads
interacting with tcspecial using
the tcslib library over a datagram connection to tcspecial.  For testing
purposes, tcsmoc uses tcslib, along with simulated payloads, to support a simple GUI.

Tcsmoc gets the payload definition from a file named on its command line, and
reads tcspecial1.yaml when the command line names none. It takes an argument
rather than an environment variable because it starts tcspecial and tcssim as
subprocesses and they inherit its environment: a variable naming tcsmoc's file
would name theirs too, and so could not point tcsmoc at one file and its
children at another. An argument belongs to tcsmoc alone.

Requirement
    Tcsmoc accepts one tcspecial configuration file. A second path is an error
    rather than an argument with no effect, because it is more likely a mistake
    about which file is being read than something meant to be ignored.

The beacon box carries the indicator and, beside it, the time the last beacon
arrived.

Requirement
    The time the last beacon arrived is shown, and is the time it arrived at
    the MOC rather than the spacecraft time the beacon carries. The line is
    read to find out whether beacons are still coming, so a spacecraft whose
    clock had stopped must not look as though its beacons had. It is shown the
    way every other time in the window is, by the one function that shows them.

Requirement
    The time survives the passes that receive nothing, and a socket error as
    well. The colour beside it is what ages; the beacon arrived when it
    arrived, and blanking the line on a timeout would say that none ever had.

Requirement
    Until a beacon has arrived the line says so, in the words a panel uses for
    a direction that has carried nothing. It was this and nothing else for the
    life of the program before the time was ever set: the window declared the
    line, the receiver set the colour beside it, and the line itself said no
    beacon had arrived while the colour said they were arriving steadily.

Requirement
    The beacons also say whether the spacecraft is being heard from at all,
    which the panels act on. Contact is held for as long as the indicator
    shows the colour of being heard from -- the leading states of it, blinking
    or steady, a blinking green being beacons that are arriving and ageing
    rather than gone -- and is lost when the indicator leaves that colour,
    when no beacon has ever arrived, and when the system clock has moved so
    that the last one is in the future. One reading serves both, taken where
    the colour is taken, so the light and the panels cannot say different
    things about one beacon.

Requirement
    A panel starts unknown and the poller's first pass asks what its handler
    is doing. A MOC that has just opened has been told nothing about any
    handler, so a panel claiming anything is a panel claiming what nobody
    said: it claimed Stopped, and was commonly wrong, since the MOC attaches
    to a tcspecial that is already serving the set rather than starting a
    second one, and that one's handlers may have been moving data for hours.
    The asking does not wait for a beacon -- a set whose beacons never arrive
    is still a set whose handlers can be asked about -- and it does not wait
    out a poll interval either: the pass happens as the window opens and the
    interval follows it.

Requirement
    A panel's status is what a START_DH or STOP_DH was answered, and the
    beacons are what can take it away. Beacons stopping makes every panel's
    status unknown: the status is the answer to a command and is worth as much
    as the ground's hearing from the spacecraft, where a handler may since
    have stopped or gone with a process that restarted, and a panel still
    saying Active would be read as a reading. It is said once, when contact is
    lost, rather than every pass of the silence.

Requirement
    The panels are made unknown whether or not there is a command link. A MOC
    that has been disconnected cannot find out either, and the beacon is what
    it has to go on.

Requirement
    When contact returns, the spacecraft is asked what its handlers are doing
    and the panels say what it answers. Going back to what they said before
    the silence would be a claim nothing has supported since: handlers may
    have been stopped in it, or the process may have restarted with none of
    them running. The answer's own state is what a panel then shows -- active
    for a handler that is active, stopped for one created or stopped, and
    unknown for a handler the spacecraft has not got or a command it could not
    answer.

Requirement
    A restoration owed to the panels survives a pass that could not ask. The
    beacon says the spacecraft is there; what its handlers are doing is a
    question for the command link, which may be down at that moment or may
    leave the first pass unanswered. A restoration that was forgotten would
    leave every panel unknown until the next time beacons stopped and came
    back.

Requirement
    No other pass of the poller touches a panel's status. The ground asked for
    a handler to start or stop and was answered, and a poll is not a second
    opinion on that: the statistics a pass carries are what it is for. The
    Query all button sets the status as well, that being somebody asking the
    spacecraft directly.

Every program is pointed at its tcspecial configuration the same way: as a
command line argument, failing that the program's own environment variable --
``PAYLOAD_CONFIG_PATH`` for tcspecial and ``SIM_PAYLOAD_CONFIG_PATH`` for
tcssim, while tcsmoc has none for the reason below -- and failing that
tcspecial1.yaml. Tcssim names its simulator configuration separately again,
through ``PAYLOAD_SIM_YAML``.

Requirement
    An argument naming the tcspecial configuration takes precedence over any
    environment variable naming one. The argument is the unambiguous of the
    two: tcsmoc starts the other programs and they inherit its environment, so
    a variable can reach further than it was meant to, where an argument
    reaches exactly the program it is given to.

Requirement
    One configuration file is named, and nothing else. A further argument is an
    error rather than one with no effect, because it is more likely a mistake
    about which file is being read than something meant to be ignored.

Where the Addresses Are
^^^^^^^^^^^^^^^^^^^^^^^
Requirement
    No address appears on tcspecial's command line. The configuration file is
    its only argument.

Requirement
    The address and ports the command interpreter binds are in its own
    configuration file.

Requirement
    The command interpreter takes commands on two links. A command about the
    spacecraft -- ``PING``, ``RESTART_ARM``, ``RESTART``, ``CONFIGURE`` --
    arrives on ``port``; a command about a payload -- ``START_DH``,
    ``STOP_DH``, ``QUERY_DH``, ``QUERY_DH_SAMPLE``, ``CONFIGURE_DH`` --
    arrives on ``payload_port``. ``CONNECT`` is answered on either.

Two links because what the ground needs in a hurry must never be queued
behind what it asked for at leisure. A ``RESTART`` waiting behind a payload's
statistics is a spacecraft that cannot be rescued while it is busy, and the
two types of command are unlike in every way that matters: one is rare,
urgent, and about the vehicle, the other routine, bulky, and about an
instrument. Missions divide them for the same reason, often between different
operators with different authority.

On a space link the division is by virtual channel, which is where the
priority between them is really decided and where a command link gets COP-1's
sequence control. Over IP there is no such thing, so it is two ports -- what
an IP network has to divide traffic with. A payload set that is flown rather
than tested would map ``port`` to the command virtual channel and
``payload_port`` to a payload one.

Requirement
    One table says which link a command belongs on, read by the ground when
    it sends and by the spacecraft when it answers:
    ``CommandType::link``. The two ends cannot come to disagree about where a
    command belongs, which is a disagreement nothing could report except as a
    command that went unanswered.

Requirement
    The table is a match over every type of command, not a rule with a
    default. A command added later is placed deliberately, by someone who has
    to decide, rather than by whichever side of a default it fell on.

Requirement
    Both ports are required of every command interpreter configuration, with
    no default, as the beacon attributes are: where commands are taken is not
    a question to be answered by whatever a file was silently given.

Requirement
    The two ports are two ports. A configuration giving both the same number
    describes a split it has not made, and the half of it that bound second
    would fail at startup with the other half's address.

Requirement
    No payload may bind a port a command link binds. It is the one collision
    that does not stop a payload from working -- it stops the commanding that
    would have started it -- and which of the two fails depends on the order
    the programs happened to start in. A command link bound to every
    interface, which ``0.0.0.0`` is, takes its port on every address, so a
    payload at ``localhost:4000`` collides with a link at ``0.0.0.0:4000``
    without either naming the other's host.

Requirement
    Each link is served by a loop of its own, in a thread of its own. Two
    sockets read by one loop are two queues into one bottleneck: a payload
    command that takes its time -- ``START_DH`` opens the payload's endpoint
    and the handler's pair of OC sockets -- would still be ahead of whatever
    the ground sent next. The split is only worth having if what it separates
    cannot wait for the other.

Requirement
    Both sockets are bound as the interpreter is created, before either loop
    runs, so a port already in use is reported where every other
    configuration error is -- at startup, naming itself -- rather than when
    the first command of that type arrives.

Requirement
    A command that arrives on the link it does not belong on is refused with
    ``InvalidCommand``, and the refusal is of the telemetry type the command
    asks for. The type is how the ground pairs an answer with what it asked,
    so a refusal it cannot place reads as a command that went unanswered --
    which is the one thing a refusal exists to deny.

Serving it instead would be worse than refusing it. A command on the wrong
link means one end is not reading the configuration the other is, and that is
exactly the fault this split must not hide: the link the ground restarts the
spacecraft over is the last place to learn that the two ends disagree about
where commands go.

Requirement
    The ground sends each command on the link the table names. A client holds
    a connection to each, and which one a command goes on is read from the
    same ``CommandType::link`` the spacecraft refuses it by, so the ground
    cannot spend a round trip learning what it already knows.

Requirement
    A client has both links or neither. One would send only half of what
    there is to send, and the half it could not send is the half the panels
    use.

Requirement
    The ground asks ``CONNECT`` on both links, and the two answers have to
    agree. Two links pointed at two spacecraft is the one way this
    arrangement can be wrong that nothing else would catch: each link works,
    each answer looks right, and the payloads being commanded are not the
    ones being restarted.

Requirement
    The MOC takes the payload port from the command interpreter's own
    configuration file, which is where tcspecial takes it from. The host is
    the operator's -- it is what the address box says, and a bind address of
    ``0.0.0.0`` is no destination -- so moving the link moves both of them by
    typing one address.

Requirement
    ``tcsverify`` reports a missing or colliding port from the line that
    states it.

Requirement
    A beacon is sent to a multicast group. ``beacon_address`` is a group
    address -- in ``224.0.0.0/4``, and ``239.0.0.0/8`` for one of local scope
    -- and an address that is not is refused.

A beacon is an announcement to whoever is listening, and the spacecraft does
not know how many ground stations there are or where they are. A unicast
beacon can reach exactly one listener, chosen in advance by whoever wrote the
configuration, because a port can be bound once: a second station cannot even
listen. Multicast is the primitive for one-to-many with no knowledge of the
many, it is a UDP-only facility, and a beacon was UDP already -- nothing about
it wants an acknowledgement.

Requirement
    Both ends name the local interface the beacon goes out on and is joined
    on: ``beacon_interface``, the address of an interface on the host.

Neither end can be left to choose. A sender bound to every interface sends a
multicast datagram out the default route; a listener that joins with
``INADDR_ANY`` joins on the default route; and on a host where those differ --
one with a second interface, or a tunnel -- not one beacon arrives, with
nothing anywhere to say so. Measured on a development machine with three
interfaces: of the four combinations of naming and not naming, only naming the
same interface at both ends delivers anything.

There is no ``set_multicast_if`` in the standard library, so the sender selects
the interface by binding that interface's address rather than every address.
The TTL is set to 1, which keeps a beacon on the network it was sent on; it is
stated rather than left to the default because it is the one option that
decides how far the thing travels.

Requirement
    Every command interpreter configuration states where beacons go, on what
    interface, and how often: ``beacon_address``, ``beacon_interface`` and
    ``beacon_interval_ms``. None has a default and none may be left out, in
    the command interpreter's own file or in a payload set's ``tcspecial``
    section.

Beacons are how the ground knows the spacecraft is alive. A configuration that
has not been asked where to send them has not answered, and a default is a way
of finding that out later -- by the beacons arriving somewhere nobody is
listening, which looks from the ground exactly like a spacecraft that has
stopped.

Requirement
    A payload set's section wins over the command interpreter's own file. A
    set's own ground station is what listens for its beacons, so the set is
    where it is said; the file's is what places a set whose own configuration
    states no section at all.

Requirement
    Tcsmoc reads the beacon group and interface from the same two
    configurations, resolved the same way. The two programs call one function
    over the same files, so they cannot name different ones.

That is what makes the earlier drift impossible rather than merely unlikely.
The MOC used to bind a compiled-in address and read neither file, so a mission
that moved the beacon moved it for tcspecial alone and the MOC said only that
nothing was arriving. A MOC that cannot work out where to listen says so and
runs without beacons, as it does without a simulator file: controlling payloads
does not depend on it.

Requirement
    A ``beacon_address`` that is not an address and a port, or not a multicast
    group, is refused where it is read, naming the attribute. So is a
    ``beacon_interface`` that is not an address.

Refused there rather than where a beacon is sent, because a beacon goes out on
a timer with nobody to report to: the address used to be a constant parsed with
``unwrap``, which could only ever have panicked, and a configured one has to be
caught while someone is still reading errors.

The command address was an argument for a while, and tcsmoc passed the address
it was about to send to, so that the end that listens and the end that sends
could not differ. One address in one place is the simpler rule, and it is the
rule now; what it gives up is that agreement. The MOC and a tcspecial it starts
agree about the command address by reading the same default, and about the
beacon address the same way, so a ``tcspecial.yaml`` that moves either moves it
for tcspecial alone -- commands then go to a port nothing is bound to, which no
error on either end reports, since a UDP datagram sent where nobody is
listening is not refused. What the two ends do check on connecting is each
other's *configuration*; see `What Each End Read`_.

Requirement
    Tcspecial says both addresses as it starts, on stderr, whether or not
    anything was asked of the log, and names the file they came from.

Said on the way past rather than only when something fails, and said before
either socket is made, so that a failure to bind follows the address it was
trying to bind. They are the addresses the ground has to be pointed at, and
until this was printed, finding out what a running tcspecial had bound meant
asking the operating system.

Requirement
    Tcsmoc passes its own configuration file to each program it starts, as that
    program's argument. Tcsmoc builds its panels from that file, so a child
    reading a different one would serve or simulate payloads the panels do not
    describe: handlers that never connect, with nothing on screen to say why.

Requirement
    Tcsmoc does not start a tcspecial when one is already running. It asks by
    PING, and attaches to the one that answers instead of starting a second.

Starting a second never worked: the command interpreter's address can be bound
once, so the second exits and tcsmoc comes up with a window, no spacecraft
behind it, and the reason on a terminal nobody is reading. Asking by PING
rather than by looking for a process or a bound port is deliberate -- what
matters is whether a command interpreter answers, where a bound port might be
anything and a process of that name might be wedged -- and it finds a
tcspecial that is not a local process at all.

Requirement
    Tcsmoc stops only what it started. A tcspecial it attached to rather than
    started is left running when tcsmoc closes: something else started it,
    something else may still be using it, and shutting down a command
    interpreter because a ground display was closed is not the display's
    decision.

Requirement
    A ``START_DH`` for a handler tcspecial does not have says which payload was
    asked for, which id it came in as, what this tcspecial does serve, and that
    the two ends are reading different configurations.

Attaching is what makes this worth saying. A tcspecial left running from an
earlier payload set is attached to rather than replaced, and it goes on serving
the set it was started with while the panels in front of the operator are the
new set's -- so a payload the configuration plainly contains is answered
``NotFound``. The command carries the payload's name, which put the name in the
answer and made it read as a name that had been looked up and missed; nothing
is looked up by name, the id is what addresses a handler, and the file tcspecial
read twenty hours ago is the fact that explains it. A status cannot carry that,
so the log says it.

An argument is what makes this possible. Tcsmoc has no variable of its own
because its children inherit its environment: a variable naming tcsmoc's file
would name theirs as well, and could not point one at a different file from
the other. An argument reaches exactly the program it is given to, and beats
anything that program would otherwise have taken from the environment, so for
the run of a payload set tcsmoc's file is the one that counts.

Tcsmoc builds each child's command without running it, so what a child would
be started with is checked by a test rather than by starting it: the programs
open windows and bind fixed ports, which makes running them a poor way to test
anything.

The one file tcsmoc does not pass on is tcssim's simulator configuration.
Tcssim takes it from ``PAYLOAD_SIM_YAML``, inherited from tcsmoc's environment
like any other variable, and so is already looking at the file tcsmoc was
pointed at. Tcsmoc reads that file as well, but only to show what it says; it
acts on none of it and so has nothing to say about which one is right.

The Makefile names a set once for this reason, and names it as a stem:
``TCSPECIAL_CONFIG_FILE`` is the set, ``$(TCSPECIAL_CONFIG_FILE).yaml`` the
tcspecial configuration that reaches every program as its argument, and
``$(TCSPECIAL_CONFIG_FILE)sim.yaml`` the simulator file that reaches tcssim in
the environment. One target then runs a whole set, and its last letter is the
spelling of the configuration the MOC reads -- ``runmocy`` the YAML and
``runmocx`` the XML -- both adding the suffix to the same stem, so they are one
set read two ways:

.. code-block:: console

   $ make runmocy
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2

Requirement
    The two files of a set are named by one variable rather than two. Naming
    one of a pair is how the two come to describe different sets, which is
    reported rather than run -- every data handler of the tcspecial
    configuration must have a simulated payload naming it -- but it is reported
    by tcssim, after tcsmoc has already come up, which makes it a confusing way
    to find out.

The sets live under ``tests/manual`` rather than at the repository root
because running one is something a person does at a terminal: each opens
sockets or device nodes, so none of them belongs in a test that runs on every
build. The tests that read them only parse them.

``make run`` runs tcspecial alone and ``make runsim`` the simulator alone, both
from the same stem -- tcssim run by itself needs the tcspecial configuration as
well as the simulator file, and one variable is what keeps the two from being
given for different sets:

.. code-block:: console

   $ make run TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1
   $ make runsim TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1

Started in that order, the three run as separately as they can: tcsmoc finds
the tcspecial already there, attaches to it rather than starting a second, and
leaves it running when its window closes. Which is what one program at a time
is for -- a debugger on the one being worked on, and the others left alone.

``make help`` lists the targets, and ``RUST_LOG`` reaches every one of them:

.. code-block:: console

   $ make runmocy RUST_LOG=debug
   $ make run TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1 RUST_LOG=tcspecial::ci=trace

The manual says the same thing from the other end -- what to type to run a
set -- under "Running the Programs".

The window is built from that file. Nothing in the GUI names a data handler or
fixes how many there are, so a handler added to or removed from tcspecial1.yaml
adds or removes a rectangle.

Requirement
    Panels are laid out across the window before they are laid out down it,
    three to a row, and a fourth starts a row under the first.

Requirement
    The MOC opens the payload data path of every handler that has an OC
    address: one socket per handler, which says hello and then reads what
    arrives. A panel shows the latest arrival with the time it arrived.

Two things were wrong without it. A handler's OC endpoint binds the address
the ground sends to and learns where to answer from what arrives there, so
with nobody on the ground it had no peer at all: every write to the ground
failed, and a payload's data went into a handler and no further. And every
reading in a panel was the spacecraft's own account -- the statistics it
counted, the samples it kept -- which is worth having and is not evidence
that anything reached the ground.

Requirement
    The hello is a datagram of no bytes. It teaches the handler where the
    ground is, which is all it is for, and a handler hands nought bytes up to
    a conduit that drops them rather than writing them on -- so nothing of
    the ground's own making enters the payload's link.

Requirement
    The hello is said again while nothing is arriving. A handler stopped and
    started rebinds its OC socket, and the peer it had learnt goes with the
    socket; a ground station that said hello once would be silently cut off,
    which is the fault this exists to remove.

Requirement
    The window asks to be put against the top of the display, and tcssim's
    against the bottom. The two are read together -- what the ground sees of
    a payload and what the payload is doing about it -- and left where a
    window manager puts them they land on top of each other.

Requirement
    It is a request and nothing depends on it. X11 honours a client's
    position; Wayland leaves placement to the compositor and may ignore it,
    and a tiling window manager places windows as it sees fit. A window that
    lands elsewhere is a window in the wrong place, not a program that
    failed.

Requirement
    The display's height is read from ``/sys/class/drm``: the tallest
    connected output's first mode. Nothing can know which display the windows
    opened on, and the tallest is the one that makes a window asking for the
    bottom land on a display rather than above it. With no height to be had,
    neither window asks for anything.

Requirement
    The position is asked for once the event loop is running, because that is
    when a window has a size. Before it is shown it reports what was
    requested rather than what it was given, and the bottom of the display is
    the display's height less the window's.

Requirement
    The window opens wide enough for the row.

It already did. Three columns come to 798 pixels including the margins, which
is wider than the window's own width floor: the command interpreter's controls
need 700, and the floor is at least what two rows of panels stand, so that a
two-row grid is wider than it is tall. That was not obvious before the panels
were measured: the constant the window width was worked out from said a panel
was 320 wide, with a comment claiming the layout stretched them to fill it, and
a panel is as wide as its widest row and does not stretch. Three columns of 320
would have opened a 960-wide window with a third of it empty to the right of
the panels.

A panel's widest row is its three buttons, which is why spelling Config out as
Configuration moved the window: 206 pixels became 256, and three columns 644
became 798. A row of buttons that does not fit keeps them at their own width
and hangs the last one over the panel's edge onto the panel beside it, so a
test holds every button to the panel that contains it and not merely to the
window.

So the shape follows from the count alone: three handlers are one row of three,
and the four of the shipped tcspecial1.yaml are a row of three and a row of
one, in a 821x884 window. A test reads that size out of this document and
checks it against the one the program opens at, as one does for tcssim.

This replaced a rule that chose the shape nearest square among those at least
as wide as they were tall. It read well and laid the panels out badly: two
panels came out as one column of two, because a 739x739 window is squarer
than a 739x549 one, so the set the Makefile runs went down the window instead
of
across it. Which row and column each panel is given was row-major all along --
the shape was the whole of the fault. Squareness was never the thing wanted
either; it was a proxy for a window that fits a screen, and a column count says
that directly.

What is given up is that the grid is no longer always wider than it is tall.
Seven panels or more take a third row, which is 929 against a 798-wide window;
they scroll, as too many panels always have. Up to six it still is, and a test
says so. The window the MOC opens has been taller than wide since it gained its
three-row floor, which is a different thing: the floor is room to grow into.

A test measures where the panels land in a laid-out window rather than reading
the arithmetic back, because the arithmetic was not the part that was wrong.

The GUI has a section at the
top of its single window that allows issuing of CI commands and viewing responses.
Below that are as many rectangles as there are data handlers.
Each displays the DH name, its status, configuration information, and packet
size, whether or not it has been started: what the panels are for is comparing
a handler against the file that describes it, and a panel that showed nothing
until it was started would say nothing about the set that had been loaded. It
shows no packet interval: that is a simulator setting, stated in
tcspecial1sim.yaml and shown by tcssim.
Below that it displays the time and the data most recently sent. Underneath
that is the time and data most recently received. Both come from
`QUERY_DH_SAMPLE`_ telemetry, which tcsmoc asks for alongside the statistics;
the times are when the data moved rather than when it was asked about, and
data longer than a sample is shown as a head followed by an ellipsis.

A handler whose conduits have carried nothing in a direction shows
``--:--:--`` and no data for it, which is also what a panel shows before it
has been queried at all.

Requirement
    A panel says which payload of the configuration it is, beside the name:
    ``auto-send (dev id 0)``. The number is the payload's place in the payload
    configuration, counted from nought in the order the file describes them,
    and nothing in the file states it -- a payload stated a ``dh_id`` once and
    that was a second identity to keep consistent by hand. Both windows count
    the same way, which is the use of it: the two are read side by side, and a
    payload is then the same number in each.

Each panel has three buttons: ``Enable``, ``Disable``, and ``Configuration``,
which shows what the configuration files said about the handler and is
described under `tcssim`_ where the same button is. It read ``Config`` while
the panel was narrower than the word.

Requirement
    A panel has a button per command, each labelled with the command it
    sends. ``Enable`` sends START_DH and ``Disable`` sends STOP_DH, with
    Enable first: that is the press a panel is opened to make, and the status
    beside the name is what says which of the two has taken effect.

Requirement
    Both are pressable whatever the handler is doing. tcspecial answers a
    START_DH for a handler already running and a STOP_DH for one already
    stopped without harm, and what an operator wants when a panel looks wrong
    is what the spacecraft says to the command -- which a button that cannot
    be pressed does not say.

There was one button, labelled with the one thing worth doing next:
``Discard`` for a handler that was moving data and ``Receive`` for one that
was not. It had to be read both ways -- Slint built the label from the status
and the program decided the command from the same status -- so the two had to
be held to one rule or they would drift into offering one thing and doing the
other. Two buttons have nothing to drift: a press is the command on the
label. What a panel gives up is that it no longer suggests which press to
make, and what it gains is that either can be made without first getting the
handler into the state that offers it.

Requirement
    The MOC has a button per command where the simulator has one button for
    both, and the difference is the link each press crosses. Pressing a button
    in tcssim starts or stops a payload in that same program: the press is the
    act, it cannot be lost, and one button labelled with what it will do says
    everything there is to say. A press in the MOC sends a datagram to a
    spacecraft, which may not arrive -- so the same START_DH or STOP_DH may
    have to be sent again, and again, before tcspecial has it. A single button
    offering the press that suits the status cannot be pressed twice: the
    status it reads is the ground's own guess about a handler it may not have
    reached, and the press worth making is exactly the one such a button
    refuses. Two buttons, both always pressable, are a command an operator can
    repeat.

Requirement
    The panels are refreshed without being asked. A handler moving data must
    not look like one doing nothing, which is what a panel that changed only
    when a button was pressed showed.

Requirement
    Asking a handler what to show does not happen on the window's own thread.
    Each command waits on the spacecraft for as long as the client's timeout
    allows, and a window that stops repainting while tcspecial is slow to
    answer is a worse fault than a panel a second out of date.

Requirement
    A panel shows what arrived from its payload, the time it arrived, and how
    many packets have arrived -- and not the spacecraft's account of what it
    last sent and last received, nor its count of the bytes it sent to the
    payload. Three rows and a counter carried that account and are gone: a
    panel is watched for what reached the ground, and the claim about it took
    a third of the panel's height to say nearly the same thing.
    QUERY_DH_SAMPLE is not asked for any more either -- a command a second
    per handler whose answer nothing showed.

Requirement
    All three of those readings are kept by the end that received them, which
    is the ground. The count was taken from the QUERY_DH statistics instead,
    and from the wrong half of them: a handler's ``bytes_received`` is what it
    read from the ground, which is the direction nobody sends in, so a panel
    beside a payload sending steadily read ``0 bytes``. The listener that
    already kept the time and the data counts the packets as it receives
    them.

Requirement
    The window says when the last command went out and when its answer came
    back, under ``Last Response``. A response that is slow to come and one
    that never comes read alike until those two times are beside it.

Requirement
    Both are the ground's own clock, and they are the window link's own
    commands -- what the operator pressed -- not the panels' polling. The
    times are read from the client that sends them, which is the one place
    every command passes, so a press that forgot to record one cannot
    happen.

Testing requires starting up tcssim before other operations and shutting it down
when tcsmoc is halted.

tcsverify
---------
Tcsverify reads a payload set -- the tcspecial configuration file, and the
simulator file beside it when one is given -- and prints what is wrong with
it. It opens no socket and no device node, so it can be run anywhere, by a
person editing a file or by a recipe: it is given the files to read and says
nothing when they are good.

Requirement
    Tcsverify takes one file or two. The first is the tcspecial configuration
    file and the second, if there is one, the simulator configuration. Which
    they are follows from the order they are given in and not from their
    names, a set being free to name its files as it likes.

Requirement
    Every problem is printed as the file it is in, the line of that file, and
    what is wrong: ``file:line: message``. The shape an editor jumps to and a
    build log is read with.

Requirement
    A hundred problems are printed at most. The hundred and first stops the
    run and ``Too many errors, halting`` is printed in its place.

Requirement
    Tcsverify exits with a status of nought when it found nothing wrong and
    non-nought when it found something, so that a recipe or a shell ``&&`` can
    be told by it. A command line that is neither of the two shapes above is
    a third status, which is not a statement about any file.

Requirement
    Tcsverify states no rule of its own. Every problem it reports comes from
    the same rules in ``tcslibgs`` that the programs reading these files use,
    so a set it accepts is a set they accept. A verifier with rules of its own
    would eventually disagree with them, and then a passing check would mean
    nothing.

That last one is what the library's ``verify::Problem`` type is for. A loader wants the
first thing wrong with a file and nothing else -- it cannot run, and which of
several mistakes it names changes nothing -- where a reader wants all of them.
So the rules are stated once and collect their answers:
``PayloadConfig::to_dh_configs`` and ``SimConfigFile::resolve`` return the
first problem, which is what they always returned, and
``PayloadConfig::check`` and ``SimConfigFile::problems`` return the lot.

**Where a line number comes from**

Nothing in the library has seen the file. A parser hands up a document and the
lines are gone by then, so a problem says which payload or group it is *about*
and tcsverify works out where that name is written -- the same thing a reader
does with the error a program prints. The alternative, carrying a line number
through every struct in the library, would put one file format's shape into
the types that exist so that two formats can describe one thing.

Requirement
    A problem about a payload or a group is reported from the line its name is
    written on. One about a section is reported from the line the section
    begins on, and one about the file as a whole from its first line.

Requirement
    A file that does not parse is reported from where the parser stopped, when
    the parser says where that was. YAML says; XML does not, and a missing
    element has no position even in principle, so such a file is reported from
    its first line.

Requirement
    The simulator file is checked against the tcspecial configuration only when
    that configuration has no problems of its own. There would be nothing to
    check it against otherwise -- the payloads that did not settle are not
    handlers -- and a join against the rest reports every one of them as
    unsimulated, which is a page of problems the verifier caused rather than
    found. It says that it did not do it, rather than leaving it unsaid.

GUI Framework
-------------
The testing GUIs will all use
the Rust slint crate
with black on white. All windows will have a go away box which will shut down
the entire application, i.e. tcspecial, tcsmod, and tcssim.

Hints
=====
These are some things that Claude doesn't seem to figure out by itself.

* Do not pass an i32 to PollFd::new() as the first argument. Instead, pass the i32
  value as the result of applying BorrowedFd::borrowed_raw() to it.

* The crate libc must be included to Cargo.toml for all crates to get definitions of AF_UNIX and other address families.

* The crate serde_json is in Cargo.toml for the command link, which carries
  JSON between the ground and the spacecraft. No configuration file is JSON.

* into_raw_fd() must not be used to convert TCPStream and UDPSocket types to RawFDs.

* as_raw_fd() must be used to convert TCPStream and UDPSocket types to RawFDs.

* Do not use tokio. Use threads directly.


Possible Enhancements
=====================

Build-time selection of CI and DH protocols to OC/spacecraft radio

    Though most or all of on-board spacecraft protocols are based on datagrams, use
    of error correcting stream-based protocols are also a reasonable choice for this
    purpose. Build-time selection of these seems useful.

Special tty-based timing of input

    Could use the ioctl_tty/termios maximum number of characters to read a message.

Tcspecial manual/auto fail over

    Requires sharing the state in a consistent way.

Support for non-Linux ReadyWait

    The ReadyWait trait could be extended to other operating systems.


Development Approach
====================
As of this writing, this design document is all there is of TCSpecial. The intent
is to use AI to generate it from this code. Going from design to code, if
AI is ready, should both save time and improve quality but these will only
be true to the extent that the design is accurate and complete. Right now
I am using Claude Code as the AI code generator.

Requirements
============

These are the requirements TCSpecial was written against, as they were
written, with the implementation notes that say where the software does
something else. They are here rather than in a file of their own so that a
requirement and the design that answers it are read together: a requirement
kept somewhere else is a requirement nobody reads while making a decision.

TCSpecial is a framework for passing commands to payload devices from an
operations center and for conduiting telemetry from the payloads back to it.
What follows sets out the requirements for its operation.

About the names and the notes
-----------------------------

The requirements below are as they were written, with two types of change.

The names are the ones the software goes by. Three were settled differently
after this was written: the spacecraft process is **tcspecial**, not
``tcssvr``; the library both ends share is **tcslibgs**, not ``tcsdefs``; and
the testing GUI became two programs, **tcsmoc** for the ground and **tcssim**
for the payloads, rather than one ``tcstest``. A third test program,
**tcsverify**, checks the configuration files without running any of them.

Four words changed as well, and these requirements use the old ones throughout.
What a requirement calls a **payload** is a **device** where it is hardware,
and a **node** where the statement is about anything the configuration can name
-- a device, a MOC link, or a compute element. Where it says *device* and means
an entry in the filesystem, that is a **device file**. Where it says **OC** it
means the **MOC**, the mission operations center. The requirements stand as
written; ``manual.rst`` gives the vocabulary in force.

A ``Not implemented`` or ``Implemented differently`` note says where the
software does not do what a requirement says. The requirement stands as
written: a note records what is so today, and is not an amendment. What each
of them does instead is described in ``design.rst``.

Where a note calls something **experimental**, it means what ``design.rst``
and ``manual.rst`` mean by it: written, parsing, reaching a handler and tested,
but never run against the hardware it describes and in no shipped payload set.
Six things are in that state, all of them types of payload interface:
RS-485 on a serial line, and the ``can``, ``iio``, ``rpmsg``, ``gpio`` and
``hdlc`` types.

The name of the project is TCSpecial
------------------------------------

TCSpecial shall include a process named tcspecial that runs on the spacecraft
-----------------------------------------------------------------------------

TCSpecial shall include a library named tcslib that links with misssion control software
----------------------------------------------------------------------------------------

TCSpecial shall include a library named tcslibgs that is shared between tcspecial and tcslib
--------------------------------------------------------------------------------------------

TCSpecial shall include applications named tcsmoc and tcssim that provide GUIs for testing tcspecial, tcslibgs, and tcslib
--------------------------------------------------------------------------------------------------------------------------

.. note::
   Implemented as two programs rather than one. Tcsmoc stands in for the
   operations center and tcssim for the payloads, because a test needs both
   ends of every link and one window driving both says less about either.

TCSpecial must have a trait named Command Interpreter (CI) that parses commands sent by tcslib
----------------------------------------------------------------------------------------------

.. note::
   Implemented differently. ``CommandInterpreter`` is a struct, not a trait:
   there is one command interpreter and nothing has needed a second
   implementation of it. The traits in tcspecial are at the layer below --
   ``EndpointWaitable``, ``EndpointReadable``, ``EndpointWritable`` -- where
   there really are many implementations, one per type of endpoint.

TCSpecial must have a trait named data handler (DH) that is called by CI to process commands
--------------------------------------------------------------------------------------------

.. note::
   Implemented differently, for the reason above: ``DataHandler`` is a struct.
   What differs between handlers is the endpoints they open, which is where
   the traits are.

CI must be implemented by CI_UDP that communicates with tcslib code via UDP/IP
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. note::
   Implemented differently. There is no ``CI_UDP``: the command interpreter
   speaks UDP itself, on two sockets -- one for commands about the spacecraft
   and one for commands about a payload.

DH_UDP must implement DH and use UDP to send telemetry to tcslib
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. note::
   Implemented differently. There is no ``DH_UDP``. A handler's endpoints are
   per type -- ``UdpEndpoint``, ``TcpEndpoint``, ``SerialEndpoint``,
   ``I2cEndpoint``, ``SpiEndpoint``, ``DeviceEndpoint``, the Unix socket pair,
   and the experimental ``CanEndpoint``, ``IioEndpoint``, ``RpmsgEndpoint``
   and ``GpioEndpoint`` -- and a handler's OC endpoint is UDP whichever type
   its payload is.

CI must support the following functions for implementing commands: ping(), exit(), status(), config(), start_dh(), stop_dh(), status_dh()

.. note::
   ``ping``, ``config``, ``start_dh`` and ``stop_dh`` are there.
   ``status_dh`` is ``query_dh``, which also has a companion,
   ``query_dh_sample``, for what a handler last sent and received. ``exit``
   and ``status`` are not implemented; see below. Two commands this did not
   ask for were added: ``connect``, which is each end saying what
   configuration it read, and ``restart_arm`` with ``restart``.

CI must have an element named dh of type BTreeMap
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. note::
   Implemented, named ``data_handlers``, and behind a mutex and a reference
   count: ``Arc<Mutex<BTreeMap<DHName, DataHandler>>>``. Keyed by name, a
   handler having no number; two command links and the handlers' own threads
   reach it.

CI must support sending telemetry responses to all CI commands
..............................................................

Each CI telemetry response to a command must contain a timestamp, command sequence number, a success or error indication
........................................................................................................................

.. note::
   Implemented. ``TelemetryHeader`` carries the sequence number, the
   telemetry type, the status and the time the spacecraft made the answer.
   Two responses used to carry a time of their own and the other eight
   carried none; it is in the header for all of them now. A timestamp is
   milliseconds since the UNIX epoch, wherever one is carried.

CI must periodically send a Telemetry::Beacon message with a timestamp and unique sequence number
.................................................................................................

.. note::
   Implemented, and to a multicast group so that any number of ground
   stations can hear it. The number is counted by the sender from one, so it
   starts again when tcspecial does, and the MOC tells a gap from a restart
   by that. A beacon also carries what a CONNECT is answered with -- the
   software version, the version the payload set states, and the digest of
   the configuration file the spacecraft read -- which this did not ask for.

CI must parse commands from the ground receved via UDP/IP
.........................................................

All CI commands must include a sequence number that increments for each command sent
....................................................................................

CI must support an EXIT command that terminates the tcspecial process
.....................................................................

.. note::
   Not implemented. There is no EXIT command. RESTART comes nearest, and is
   deliberately not the same thing: it has to be armed by a RESTART_ARM
   first, so that one lost or malformed datagram cannot stop the flight
   software.

CI must support a STATUS command that sends telemetry containing status to ground

.. note::
   Not implemented. There is no STATUS command for the interpreter as a
   whole. QUERY_DH answers for one handler -- what state it is in and what it
   has moved -- and QUERY_DH_SAMPLE for what it last sent and received; a
   ground station asks about every handler by asking about each.

Tcspecial includes zero or more components named Data Handler (DH)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^


The top part of the tcsmoc window is for sending commands to tcspecial's CI and viewing telemetry from tcspecial's CI
---------------------------------------------------------------------------------------------------------------------

The middle part of the tcsmoc window is for viewing telemetry from individual DHs
---------------------------------------------------------------------------------
