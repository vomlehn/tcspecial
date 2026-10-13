================
TCSpecial Manual
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

TCSpecial is **telemetry and control, special**. It connects the devices aboard
a vehicle -- sensors, actuators, and whatever else arrives on an
I\ :superscript:`2`\ C bus, a SPI link, a serial line, a CAN bus, a set of
discrete lines, a sensor buffer, a channel to another processor core, a socket
or a device file -- to the places their data has to reach. Some of those places
are on the ground: a network link carries telemetry to the mission operations
center, the **MOC**, and commands back. Others are aboard: software that takes
what a device produces, computes something from it, remembers what it needs to
remember, and sends the result on to the original device, to a different
device, or to the MOC. Others again keep what they are given -- a log node
records what reaches it, however many sources send to it, so that what happened
can be reconstructed later rather than only watched as it goes past.

Everything in that picture is a **node**, and what joins one node to another is
a **link**. Neither is written in the software. Both are read from a
configuration file at startup, which is the whole of the architecture: the
program is a set of node types and a machine for moving data along declared
links, and what a particular vehicle does is a file.

What that buys
--------------

The reasons are given in the order TCSpecial ranks them. Where two designs
compete, the earlier item wins, and this list is what an argument about them
should cite.

**Reliability.** The whole of what a vehicle will do is one artifact, and it is
checked before any of it runs. Every link must name nodes that exist; a node
that computes a mean must have its inputs bound, all of them; a set of discrete
lines driven as outputs may not also be read as inputs. ``tcsverify`` performs
those checks on the ground, against the same code that will perform them again
aboard, so a configuration that does not make sense is refused on a desk rather
than discovered in flight. What did fly is not a matter of memory either: the
beacon carries a digest of the configuration the spacecraft read, so the ground
can say which file is running. And a node can be reached only along a link
somebody declared -- there is no ambient path by which data arrives somewhere
nobody intended.

**Ease of use.** One file says what the system does, and it says it in the
words of the person who built the vehicle: a bus and an address, a chip and
three line numbers, an interval, a setpoint. Reading the file tells you what is
connected to what, which is the question actually asked of a telemetry system
at three in the morning. There is no code to read to find out where a sensor's
readings go.

**Maintainability.** A node knows nothing about its neighbours. It is handed
bytes and it produces bytes, and the links are somebody else's business, so a
type of interface is one module with one job and a test of its own. Adding a
type does not touch the machinery that moves data, and changing what a vehicle
does does not touch either.

**Flexibility and extensibility.** A new interface is a new node type; a new
computation is a new function or a program of its own. Neither disturbs the
graph. Rearranging a vehicle -- another sensor, a second heater circuit, the
same readings sent somewhere new -- is an edit to a configuration file, not a
build.

The vocabulary
--------------

These words are used precisely throughout, and some of them replaced older ones
that were used loosely.

node
    Anything the configuration can name and a link can join: a device, a MOC
    link, or a compute element. The generic term.

device
    A node that is real hardware, reached over one of the interfaces. These
    were called *payloads*, which said what they were for rather than what they
    are, and said nothing at all about a device being driven rather than read.

device file
    A node reached as an entry in the filesystem -- ``/dev/urandom``, a named
    pipe, a file a program writes. The word *device* was used for this as well
    as for hardware, which made "device" ambiguous in exactly the places it had
    to be exact.

MOC
    The mission operations center, on the ground. A MOC node is a network link
    carrying telemetry down and commands up. Earlier text calls this the OC.

compute element
    A node that transforms what it is given. It may keep state between
    arrivals, which is what makes a control loop possible: an average over
    time, a comparison against a setpoint, a filter that remembers.

group
    Devices that share one sampling interval, so that a set of sensors is read
    as close to simultaneously as the hardware allows rather than each on a
    clock of its own.

log node
    A node that records what reaches it rather than passing it on, built on the
    same ``tcslog`` the telemetry log uses. Any number of sources may link into
    one, so a flight's housekeeping is one log with the records interleaved as
    they happened. Its directory and its record format are configured, and
    every source must produce records that format can hold -- which is settled
    when the configuration is loaded, so a log never has to refuse a record in
    flight.

link
    A directed connection from one node to one or more destinations. A link
    with several destinations is how one reading reaches both the ground and an
    on-board computation; it is the "T" such an arrangement is usually drawn
    as. It is written as an attribute of the node it leaves rather than in a
    section of its own, so a node and what leaves it are read and written
    together.

terminal
    Where a link is attached: a node's **data source** is where data leaves it
    and a **data sink** is where data arrives. A terminal has a name, and the
    name says which way the data is travelling and not what is at the other
    end of it. There are four of them, two per stream:

    ``to_moc`` is where a payload's readings leave and ``from_payload`` is
    where they arrive; ``to_payload`` is where what the ground sends leaves and
    ``from_moc`` is where it arrives. A payload has ``from_moc`` and
    ``to_moc``; a MOC link has ``from_payload`` and ``to_payload``; a ``null``
    node has all four. With no vehicle node in the way, a payload's ``to_moc``
    connects to a MOC link's ``from_payload``.

    A link is written as an attribute of the node it leaves, named after the
    data source it leaves by -- ``to_moc: k_fore``, or ``to_moc: [moc,
    mean_temp.in0]`` where it reaches several sinks. The destination names the
    terminal it arrives at after a dot where it has to; ``node`` alone will do
    wherever the type has one data sink, which is every type but ``null``. A data sink takes one data source; a data source may reach
    several sinks; and a MOC link's sink takes as many as the configuration
    gives it, every payload's telemetry arriving at the ground on its own
    socket.

    Each terminal may be linked to one other: a ``to_moc`` to a
    ``from_payload``, a ``from_payload`` to a ``to_moc``, a ``to_payload`` to a
    ``from_moc``, a ``from_moc`` to a ``to_payload``. Anything else would carry
    one stream into the other. In tcsgui the terminals are drawn as small
    boxes in a node's corners, each with an arrow in it, and a link is made by
    dragging from one to another -- a drag that does not pair makes nothing.

``null`` node
    A vehicle node that carries both streams through and does nothing to what
    passes. It is the one type with all four terminals.

command binding
    A statement that a named command from the MOC sets a named attribute of a
    node. It is how a setpoint reaches a control loop in flight.

.. note::
   **What runs today.** The architecture above is what TCSpecial is being
   built into, and this manual describes it as the thing to configure against.
   What is in flight-ready shape at the time of writing is narrower: a node may
   be a device or a MOC link, and the links the software builds are the pair
   that joins one device to one MOC link in each direction. Compute elements,
   links with several destinations, groups with a shared interval, discrete
   lines driven as outputs, and command bindings are specified in
   ``design.rst`` and are being implemented in that order. Where this manual
   describes something not yet running it says so in a note like this one, and
   the rule this project has always followed applies: the description stands,
   and a note records what is so today.

Configuration File Formats
==========================
A configuration file may be written in YAML or XML. Both describe the same
settings and are read into the same values, so the choice is one of local
preference and tooling rather than of capability.

The format is taken from the file's extension:

.. code-block:: text

   .yaml, .yml     YAML
   .xml            XML
   anything else   refused

A file whose extension is none of those is refused, naming the file and what a
configuration file is called. JSON was read as well, and was what a file with
no extension or an unfamiliar one was assumed to be, which made a misspelled
extension not an error but a file parsed as the wrong language. Nothing was
written in it that was not also written in YAML.

(The command link carries JSON between the ground and the spacecraft. That is
not a configuration file and is unaffected.)

Which Files There Are
=====================
Two configuration files, and neither of them has a name of its own. Any name
will do so long as it ends in ``.yaml`` or ``.xml``, which is what says how to
read it; the names used throughout this manual are the shipped examples' and
are suggestions, nothing more.

the tcspecial configuration file
    Everything tcspecial is configured with. Its ``tcspecial`` section is the
    command interpreter's own settings -- the address and the two ports it
    takes commands on, where beacons go and how often, and where the
    telemetry log goes -- and its payload configuration section says which
    data handlers there are, how tcspecial reaches each one and how large its
    packets are. Named on the command line of every program that reads it,
    which is tcspecial, tcsmoc, tcssim and tcsverify.

the simulator configuration file
    How a simulated payload behaves: how often it produces a packet and how
    it divides one into segments. Read only by tcssim, which takes it from
    ``PAYLOAD_SIM_YAML``, and by tcsmoc, which shows it and acts on none of
    it. There is nothing in it about the payloads themselves.

A file is recognised by what is in it rather than by what it is called: a
configuration with no ``payloads`` section describes no data handlers and is
refused.

The command interpreter's own settings are in the one place with the payloads
today only for the beacons. The ``tcspecial`` section is read for where
beacons go and how often; the address, the two command ports and the log are
read from the file ``TCSPECIAL_CONFIG_PATH`` names, which states those same
attributes at its top level. The section is checked wherever it appears, so
one that could never place a command interpreter is refused where it is read
rather than carried.

The Payload Configuration
=========================
The payloads are a section of the tcspecial configuration file: a version, a
description, the ``tcspecial`` section, an optional section of groups, and the
data handlers themselves. A handler takes every attribute it does not state
from the group it names, so a group holds what several handlers share.

.. code-block:: text

   version        "1.0"
   description    free text

   tcspecial                       optional
     ...          the command interpreter's own settings, of which
                  beacon.send and beacon.recv are read

   payload_groups                  optional
     name         the name payloads refer to
     ...          any of the attributes below

   payloads
     name         the handler's name, which commands address it by and the
                  panels show
     group        the group to take unstated attributes from, if any
     ...          any of the attributes below

The shipped examples each carry a ``tcspecial`` section, first in the file, so
one file says what the set is and where its command interpreter belongs.
``beacon.send`` and ``beacon.recv`` are read from it, since where a set's
beacons go is the set's own business; the rest is not read yet, tcspecial being placed by the
file ``TCSPECIAL_CONFIG_PATH`` names. It is all checked when the file is
loaded -- a section that could never place a command interpreter is refused
where it is read.

What follows is the attributes every payload has, whatever it is, and then one
section per type of payload for the attributes only that type has. A payload
states its type in ``type`` and is then held to that type's attributes and no
others: a device stating a ``payload_interface``, or a network payload stating
a ``path``, is refused.

About the experimental interfaces
---------------------------------
Some of what follows is marked **experimental, not yet supported**. Each of
those is written, parses, reaches a handler and has tests; none has been run
against the hardware it describes, and none is shipped in a payload set. What
is marked that way is: RS-485 on a serial line, and the ``can``, ``iio``,
``rpmsg``, ``gpio`` and ``hdlc`` types of payload.

One of those is further out than the others. Every experimental type but
``hdlc`` has something in the kernel to be run against -- a virtual CAN bus, an
I\ :superscript:`2`\ C stub, a simulated GPIO chip, a dummy sensor -- so each
has been exercised against something. A synchronous line has none, and needs a
card; what is written for it is held to the kernel's structures by tests and to
nothing else.

Read them as a statement of what the configuration will be rather than as a
feature to rely on. They may change in ways the five supported types --
``network``, ``device-file``, ``serial``, ``i2c`` and ``spi`` -- will not.

Attributes Every Payload Has
----------------------------
.. code-block:: text

   name           the handler's name, which commands address it by and the
                  panels show
   group          the group to take unstated attributes from, if any
   type           network | device | serial | i2c | spi, or one of the
                  experimental can | iio | rpmsg | gpio
   packet_size    bytes in one packet
   mode           periodic | triggered; absent is periodic
   trigger        what to send to make the payload answer:    triggered only
                  bytes, as unquoted hexadecimal after 0x,
                  or text, as a quoted C string
   packet_interval_ms
                  how often the trigger is sent               triggered only

``name`` belongs to a handler and never to a group: it is what tells one
handler of a group from another, and what every command about a handler names
it by. There is no number beside it -- a handler used to state a ``dh_id`` as
well, which was a second identity for one payload to keep consistent by hand.
An OC address and an OC port are given together or not at all, and a handler
needs them before it can be started.

The two types of payload
------------------------
A payload either sends of its own accord or answers a request, and which it is
decides where its timing is written down.

**periodic** -- the payload sends and tcspecial reads. Nothing here says when:
how fast a simulated one produces data is a property of the simulation, so the
intervals are in the simulator configuration. A periodic payload takes no
``trigger`` and no ``packet_interval_ms`` here.

**triggered** -- tcspecial sends a request and the payload answers. Both the
request and how often it goes out are here, because tcspecial does the sending
and what to send comes from the payload's interface document. Its entry in the
simulator configuration states no interval at all, packet or segment, and
takes none from the group it names: such a payload sends when it is asked, so
a rate there would govern nothing. A group's settings reach a payload exactly
as its own do, so an inherited interval is refused as a stated one is -- a
triggered payload may share a group for the settings it can use, but not one
that states a rate.

The interval is ``packet_interval_ms`` in whichever configuration states it --
one name, in the place the payload's type puts it. It was
``trigger_interval_ms`` here; a file still using that name is told the name it
has now.

In a graph, a ``trigger`` is also what allows anything to be sent to a payload
at all. What is written to a payload that is read is the question it answers,
and the trigger is the bytes of that question -- so a payload that states none
is one the configuration describes as only read, and a link delivering into it
is refused, naming both ends. The usual cause is a link drawn to the wrong
node, which nothing else in a configuration would reveal. A payload that is
written and never read is a different thing and takes no trigger: a heater, a
valve or a latch is driven rather than asked, and a trigger on one would be
bytes written before a read that never happens.

**The trigger itself** is bytes, not text, since an interface document asks
for what it asks for. A series of bytes is written as a list of values
separated by spaces, in hexadecimal, in either case. One or two digits is one
byte -- ``5`` and ``05`` are the same byte -- and three or four digits is two
bytes, most significant first, which is both the order it is written in and
the order every other multi-byte quantity in this project uses::

   trigger: 55 AA 0F
   trigger: 0D 0A
   trigger: 03 0400

There is no ``0x`` on any of them. A trigger is a list of byte values and
nothing else, so a prefix saying "this is hexadecimal" would be saying it once
per value for a notation that has no alternative. A file that writes one is
told so by name rather than having ``0x55`` read as something.

A trigger that is text is written as a string in C's notation, **quotes and
all**, which is what its escapes need -- ``\\`` ``\"`` ``\'`` ``\n`` ``\r``
``\t`` ``\0`` ``\a`` ``\b`` ``\f`` ``\v`` and ``\xNN``::

   trigger: '"READ\r"'
   trigger: '"\x02ARM\x03"'

Which of the two a value is in follows from its first character and nothing
else: a quote opens a C string, and anything else is a list of values.
``52 45`` is a plausible pair of bytes and a plausible pair of digits, so
something has to say which, and the quote is it.

The quotes are therefore part of the **value** rather than part of the way the
file is written, which is why a YAML trigger of text is written inside the
format's own quotes -- ``'"READ\r"'`` -- and an XML one as
``<trigger>&quot;READ\r&quot;</trigger>``. A value whose quotes belong to the
format alone, such as ``"55 AA"``, arrives here as ``55 AA`` and is read as
the two bytes it is.

An escape C does not have is refused rather than passed through, a string that
opens and does not close is refused, and a trigger of no bytes at all is
refused: a payload that answers requests has to be asked something. Writing
the escapes yourself is also how an XML configuration states a trigger that a
YAML one states with the format's own escapes.

.. code-block:: yaml

   payloads:
     # Sends on its own: nothing here about timing.
     - name: auto-send
       type: network
       protocol: tcp
       payload_interface: localhost:5000
       packet_size: 12

     # Answers a request: both halves of the request are here, and the rate
     # is the rate the trigger goes out at.
     - name: triggered-send
       type: network
       protocol: tcp
       payload_interface: localhost:5001
       packet_size: 12
       mode: triggered
       trigger: '"READ\r"'      # or 52 45 41 44 0D, the same bytes
       packet_interval_ms: 500

A file that mixes the two is refused rather than run, in either direction: a
periodic payload carrying a trigger or an interval, or a triggered one with no
interval, has not said which type of payload it describes, and either guess
would operate it in a way nobody asked for.

A datagram payload -- ``udp`` or ``unix_dgram`` -- cannot be triggered. Its
handler learns where to send from the payload's first packet, so there is
nowhere to send a request until the payload has spoken, which a triggered
payload does not do.

Tcspecial counts the triggers it sends apart from the data it carries, which
``QUERY_DH`` reports: a handler whose triggers climb while nothing arrives is
one whose payload is not answering. The triggers that would not go are counted
apart again, since a payload that has gone away and a handler that has stopped
asking are different things -- and a handler goes on asking through either.

A Network Payload
-----------------
Keyword ``type: network``. A socket, which the transport decides the shape of.

.. code-block:: text

   protocol       tcp | udp | unix_stream | unix_dgram
   payload_interface
                  where this payload is reached: the host and the port with a
                  colon between them, as localhost:5000 -- or the path of a
                  Unix-domain socket, which has no port and so is written
                  alone

Which end waits and which reaches out follows from the transport, and so does
whether a handler can trigger the payload at all: a stream is connected, so
the handler reaches out and may ask the payload for data, while a datagram
handler waits to be spoken to and learns where its payload is from the first
packet that arrives.

A Device File Payload
---------------------
Keyword ``type: device-file``. A file in the filesystem, opened and read as
the data comes.

.. code-block:: text

   path           the device the handler opens

Nothing else: a plain device has no terms to be set and no address to be
reached at. A device cannot be triggered -- tcspecial opens it and reads it,
and there is nothing to write a request to.

Three other types are reached through a device file too and are not devices in
this sense: a serial line, a bus and a SPI peripheral each have terms of their
own, which is why each has a type of its own below.

A CAN Payload
-------------
Keyword ``type: can``. **Experimental, not yet supported.** A payload on a CAN
bus, which is not a device file at all: a CAN controller is a network
interface, and a payload on it is addressed by the identifier its frames carry.

.. code-block:: text

   interface      the controller: can0, or vcan0 for the kernel's virtual bus
   can_id         the identifier this payload's frames carry, decimal or 0x
   can_fd         true | false, default false -- CAN FD frames rather than
                  classic CAN, which is also what lets a packet be longer
                  than eight bytes
   loopback       true | false, default true -- whether the controller gives
                  frames to other sockets on this host, which is what lets
                  tcssim stand in for the payload

Both halves of where it is are required. Several payloads commonly share one
bus and differ only in the identifier, which is the arrangement an
I\ :superscript:`2`\ C payload has, so two payloads may name one ``interface``
and may not share a ``can_id``: two handlers filtering on one identifier would
each read the other's frames.

A frame is its own length and carries eight bytes, or sixty-four as CAN FD,
and CAN does not fragment -- so a ``packet_size`` larger than a frame is
refused where the file is read rather than discovered a frame at a time. The
segment settings still apply: a packet may go as several frames, each a
segment.

A CAN payload may be triggered, unlike a payload on an
I\ :superscript:`2`\ C bus. The handler sends its trigger with the same
identifier, and the socket does not receive its own frames, so a handler never
reads back the trigger it just sent.

The simulator joins the same bus as another member of it, so both ends are
real -- the only experimental type whose stand-in is as faithful as a socket.
What it cannot do is make the bus: a virtual one needs

.. code-block:: console

   $ sudo ip link add dev vcan0 type vcan
   $ sudo ip link set dev vcan0 up

and tcssim says exactly that if the interface is not there, as it does for the
I\ :superscript:`2`\ C stub module. ``bin/mkvcan`` runs both, after removing any
virtual bus left from a previous run, so

.. code-block:: console

   $ make mkvcan

is the shorter way to the same place. Either needs root -- the script asks for
it through sudo -- and neither survives a reboot, so the bus is something a
session makes again. ``make rmvcan`` removes the buses and leaves the driver
loaded, which is both the undoing of ``make mkvcan`` and the first thing it
does itself.

The second command is the one that gets forgotten, and a bus made without it
is worse than no bus at all: a socket binds to an interface that is down
without complaint, and the send then fails with ``ENETDOWN``. So the tests
take a bus that is down as no bus and skip, rather than fail in a way that
reads as a bug in the endpoint. Note that a virtual bus has no carrier and so
reports its operational state as ``UNKNOWN`` whether it is up or not, the way
the loopback interface does -- the ``UP`` to look for is the flag ``ip link``
prints in the angle brackets:

.. code-block:: console

   $ ip -brief link show type vcan
   vcan0            UNKNOWN        <NOARP,UP,LOWER_UP>

A GPIO Payload
--------------
Keyword ``type: gpio``. **Experimental, not yet supported.** A set of discrete
lines -- a deployment switch, an arming line, a payload's own "alive" line --
watched for changes, or a set of circuits driven: a heater, a valve, a latch.

.. code-block:: text

   path           the chip: /dev/gpiochip0
   lines          the offsets, separated by commas or spaces, decimal
                  or 0x. Required
   direction      input | output, default input
   edge           rising | falling | both, default both. Inputs only
   bias           none | pull_up | pull_down, default none. Inputs only
   debounce_us    how long a line must be steady before a change counts.
                  Default none, applied by the kernel, inputs only

The direction is not something a set of lines can be both of: a line
requested as an input is one the kernel will not write, and one requested as
an output produces no edges to read. So it is stated, and stating it is what
lets a file be refused for driving something it said it would watch. The
three watching terms are refused outright on an output rather than ignored:
an edge or a debounce on a line nothing reads is a file saying something it
does not mean, and the usual cause is a set of lines whose direction was
changed and whose other words were left as they were.

This is the one type whose payload data is made rather than carried. A line
has no bytes: it is high or low, and what the ground wants is when it changed.
So each change becomes a record of sixteen bytes -- eight of time in
milliseconds since the epoch, four of line offset, four of the value it became
-- and a ``packet_size`` must be a whole number of those, a packet that was
part of a record being a reading nobody could use.

Two payloads may watch one chip and may not watch one line: the kernel gives a
line to one request at a time, so the second handler's start would fail.

Driven lines are driven together, to the same state. What arrives is a value
as everything on a link is -- the text ``1`` to assert and ``0`` to unassert,
which is what the hysteresis compute element emits -- and anything else is
refused rather than taken for one of them: the thing on the other end is a
heater, and a datum that cannot be read is not guessed at. The lines are a
set and not a pattern because there is nothing in the file to say which line
is which, and because three heater circuits switched one at a time would be
three thermal states of a vehicle on the way to the one that was asked for.
The kernel counts them by their place in the request, so lines ``3,4,5``
named in that order are the low three bits of the call that drives them.

A GPIO payload cannot be triggered, and a read and a write are not both open
to it: an input refuses a write, because driving one would be commanding a
payload rather than reading it, and an output refuses a read, because
reporting a line this node itself drove would be reporting its own voice.
Each refusal says which direction does allow it.

The simulator moves lines through the kernel's ``gpio-sim``, which it finds
rather than makes: loading the module and making a chip need root, and tcssim
says so if there is none. ``bin/mkgpio`` makes one and prints the node it
became, which is numbered as chips appear and so has to be read off the run
rather than assumed; ``bin/rmgpio`` takes it away again. The packet and segment sizes mean nothing to a
simulated set of discretes, and neither do the faults -- there are no bytes to
drop or corrupt; what it does is pull each of its lines the other way every
packet interval.

An HDLC Payload
---------------
Keyword ``type: hdlc``. **Experimental, not yet supported.** A synchronous
serial line -- bits carried on a clock, framed as HDLC -- which Linux drives as
a network interface rather than as a device file.

.. code-block:: text

   interface      the interface the card presents: hdlc0
   standard       v35 | v24 | x21 | x21d -- how the port is wired. Required
   clock_type     external | internal | tx_internal | tx_from_rx -- whose
                  clock the bits are on. Required
   clock_rate     bits per second. Required where this end generates the
                  clock, and refused where the far end does
   encoding       nrz | nrzi | fm_mark | fm_space | manchester, default nrz
   frame_check    none | crc16_pr0 | crc16_pr1 | crc16_pr0_ccitt |
                  crc16_pr1_ccitt | crc32_pr0_ccitt | crc32_pr1_ccitt,
                  default none
   loopback       true | false, default false -- for proving a card with no
                  line attached

An ``interface`` and not a ``path``, which is the first thing to know about
this type. Linux calls the mechanism **generic HDLC**: a card presents a
network interface, the terms of the line are set with one ioctl on it, and the
data is frames on that interface. The attributes above are the kernel's own
two structures, from ``linux/hdlc/ioctl.h``: ``standard``, ``clock_type``,
``clock_rate`` and ``loopback`` are the port's wiring, and ``encoding`` and
``frame_check`` are what the bits mean once the port is clocked. The kernel
calls that last one a line's ``parity``; it is a frame check, and this calls
it one.

The rate belongs to whoever generates the clock. With ``internal`` or
``tx_internal`` this end drives it, so ``clock_rate`` is required; with
``external`` or ``tx_from_rx`` the far end drives it, and stating a rate is
refused -- a number this end cannot set is a number it has no business saying.

Both halves are read back after they are set, and a line that reports
something else is refused. A card may take the call and keep less than it was
given, and a line whose frame check is not the one the configuration asked for
is a line checking nothing while the file says it checks every frame.

A frame is its own length, so there is no stream section: a read hands over one
frame's payload and a write puts one payload into one frame, as on a CAN bus.
An HDLC payload may be triggered, both ends being peers on the line.

One interface is one payload. A raw HDLC frame carries no address, so there is
nothing to tell two payloads on one line apart -- which is what makes this
different from a CAN interface, where the identifier does exactly that.

Bringing the interface up is the platform's business, as it is for a CAN
controller: ``ip link set dev hdlc0 up``, from whatever starts the vehicle.

**There is no stand-in.** Every other experimental type has something in the
kernel to be run against -- ``vcan`` for a CAN bus, ``i2c-stub`` for a bus
device, ``gpio-sim`` for discrete lines, ``iio_dummy`` for a sensor -- and
generic HDLC has none. This type needs a card: a ``wanxl``, a ``pci200syn``, a
``pc300too`` or a FarSync. tcssim says so rather than pretending, and refuses
a set with one of these in it.

An rpmsg Payload
----------------
Keyword ``type: rpmsg``. **Experimental, not yet supported.** Software on
another core of the same package -- a real-time core beside the Linux one --
reached over an rpmsg channel, which Linux presents as a character device
carrying whole messages both ways.

.. code-block:: text

   path              the channel: /dev/rpmsg0
   service           the name the remote announced the channel with. Required
   max_message_bytes the greatest a message may be, where the transport is
                     not the usual one. Default 496, which is what 512-byte
                     buffers leave after the rpmsg header

The service name is required because a node's number is not an identity: it
is assigned as channels announce themselves, so ``/dev/rpmsg0`` is whichever
came first and may be another core's channel after a change in boot order.
tcspecial reads ``/sys/class/rpmsg/<node>/name`` and refuses a node that is
not the channel the file asked for. Where the kernel publishes no name --
and a simulated channel has none to publish -- nothing is refused.

A read hands over one whole message, so there is no stream section. rpmsg does
not fragment, so a ``packet_size`` larger than one message is refused where
the file is read.

An rpmsg payload may be triggered: the channel carries both directions, so the
handler writes a request and the remote answers.

The simulator stands in for the channel with a pseudo-terminal. What a pty is
not is a message boundary -- the rpmsg driver hands over one message per read
where a pty hands over whatever bytes are there -- which shows only where a
payload writes faster than the handler reads.

An IIO Payload
--------------
Keyword ``type: iio``. **Experimental, not yet supported.** A sensor's buffered
samples, read from a character device: what a thermistor, an accelerometer or
an ADC looks like on Linux.

.. code-block:: text

   path           the buffer: /dev/iio:device0

That is the whole of it: an IIO payload has no terms of its own. Which
channels are enabled and how fast the device samples are set through ``sysfs``
by whoever brought the sensor up -- a handler that wrote there would be
reconfiguring a sensor other programs may be reading -- so a payload says
neither. It said both once, as ``channels`` and ``sample_rate``, recorded and
never applied; a file that still states one is refused by name and told why.

A read hands over whole sample sets, as many as are ready, so there is no
stream section: nothing has to say where a read ends.

An IIO payload cannot be triggered. A buffer is a stream out of the sensor --
there is nothing on that descriptor to write -- so a file asking for one is
refused, as a datagram payload asking for one is.

The simulator stands in for the buffer with a pseudo-terminal, as it does for
a serial line: the path the handler was told to open leads to it while the
payload runs. What it does not stand in for is the ``sysfs`` half, the
channels and the buffer -- which the handler does not touch either.

A pty carries the bytes but is not a buffer, and the kernel offers something
that is: ``iio_dummy``, a sensor with no sensor in it, whose buffer a software
trigger fills on a timer. There is no simulator side to that one -- a buffer
is a stream out of the device and an IIO payload cannot be triggered, so the
dummy *is* the payload and tcspecial reads it, which makes it the more
faithful of the two stand-ins. ``bin/mkiio`` makes one, prints the node and
the channel count to name in a payload set, and ``bin/rmiio`` takes it away;
a host's own sensors sit in the same numbering and are left alone, which is
why the scripts go by name and not by number.

A Serial Payload
----------------
Keyword ``type: serial``. A line, with terms that say how it is driven. A
payload states them beside everything else it states, or takes them from its
``payload_groups`` group the way it takes the rest.

.. code-block:: text

   path           the serial port
   datarate       bits per second
   byte_length    5 | 6 | 7 | 8
   stop_bits      1 | 1.5 | 2
   parity         none | even | odd | mark | space, default none
   stream         where a read ends; see below

Start-stop framing, each byte delimited by its own start and stop bits, which
is what ``termios`` drives and what every one of these terms is. A line whose
bits are carried on a clock is not one of these: Linux drives such a line as a
network interface rather than as a device file, and it is an `An HDLC
Payload`_ here.

What a line may say is what the Linux driver for one can be told, and nothing
else. ``stop_bits`` is required -- a stop bit is what start-stop framing uses
in place of a clock, so a line without one is not this type -- and a clocked
line's words are refused on it by name.

Two things are worth knowing before setting a parity. A line is read back
after it is set, and a parity the port did not take is an error rather than a
handler quietly checking nothing. And a simulated line is a pseudo-terminal,
which will not take a parity at all -- it rejects ``even`` outright and drops
the others silently -- so a payload meant to be run against tcssim states
``parity: none``, which is what a payload link commonly runs anyway.

**RS-485: a two-wire bus** -- experimental, not yet supported

A start-stop line may be a half-duplex RS-485 bus rather than a point-to-point
RS-232 or RS-422 line. One pair of wires carries several transmitters, so a
transmitter has to stop driving the pair the instant it has finished; the
turnaround is a few bit times, too quick for a program, so Linux does it in
the UART driver and is told through an ioctl.

.. code-block:: text

   rs485             true | false, default false -- asynchronous lines only
   rs485_rts_on_send true | false -- the level of RTS while sending.
                     Required of a line that states rs485: true
   rs485_delay_before_send_ms
                     milliseconds to hold the bus before the first bit,
                     default 0
   rs485_delay_after_send_ms
                     and after the last, default 0

Which level of RTS drives the transceiver is a property of the board rather
than of the protocol -- the signal commonly reaches the transceiver's enable
pin through an inverter -- so a line that states ``rs485: true`` must state it,
and a line that got it the wrong way round would transmit exactly when it
should be listening. The delays default to nought, which is right where the
transceivers are quick enough, and a delay longer than a second is refused as
microseconds mistaken for milliseconds.

The turnaround details are refused on a line that does not state
``rs485: true``, and all of it is refused on a synchronous line: the kernel's
RS-485 support is part of its start-stop drivers, where a synchronous line is
driven through generic HDLC instead.

Unlike the synchronous terms above, these are applied rather than recorded,
and the line is read back after being set: a driver that does not support the
mode may take the call and clear the flag, and a line left in
point-to-point mode on a two-wire bus transmits into other transmitters. A
line that will not take what the file asked for is refused rather than
started. A simulated line is a pseudo-terminal, which will not take it at all,
so a set meant to be run against tcssim does not state ``rs485`` -- as it
states ``parity: none`` for the same type of reason.

**A stream section**

Where one read of the line ends.

.. code-block:: text

   max_length     the longest payload one read may deliver; always required
   timeout        how long a read waits, or the word none for a read that
                  does not time out
   terminators    bytes that each end a read, as a sequence or a delimited
                  string. Also spelled terminator

A file gives a timeout, terminators, or both. One that gives neither is
refused, because it would describe a read with no way to end but filling the
buffer -- which is a real intent, but one a file states by giving
``timeout: none``.

Only a line takes a stream section. A master on a bus clocks exactly as many
bytes as it asks for, so a transfer is already bounded and needs no rule for
where a read ends; a network payload's datagram is already a frame.

``tests/manual/tcspecial4.yaml`` is a worked example of a line, with its
group, its terms and its stream rule, beside one payload of each of the other
supported types; ``tcspecial4.xml`` is the same set in the other format.

An I\ :superscript:`2`\ C Payload
---------------------------------
Keyword ``type: i2c``. A device on a bus, which is why it gives two things to
be found by: the bus and the address on it. Two payloads commonly sit on the
same bus and differ only in which device the master addresses.

.. code-block:: text

   path           the bus
   payload_address the address of the device on it

   ten_bit        true | false, ten address bits rather than seven
   pec            true | false, an SMBus packet error check on each transfer
   retries        how many times the master tries a transfer
   timeout        a whole number with a unit: 50ms, 2s. Rounded up to a
                  multiple of 10 ms by the kernel

With seven-bit addressing the usable addresses are 0x08 through 0x77: the
specification reserves 0x00 through 0x07 and 0x78 through 0x7F, so no device
can answer to one of those. With ten-bit addressing the whole range 0x000
through 0x3FF is usable.

There is no parity or byte length. I\ :superscript:`2`\ C fixes the framing at
eight data bits, most significant first, followed by an acknowledge bit.

Such a payload cannot be triggered by a simulated stand-in: the master's own
read is the request, and a second master on the bus cannot see it.

What stands in for the device is the kernel's ``i2c-stub``, an adapter whose
chips are a bank of registers in memory: tcssim writes the bank and tcspecial
reads it. The simulator finds that adapter rather than making one -- loading a
module needs root -- and says how to make one if there is none.
``bin/mki2c`` makes it. There is a script per type -- ``bin/mki2c``,
``bin/mkgpio``, ``bin/mkiio`` and ``bin/mkvcan`` -- because a host that needs
one commonly does not need the others, and each has an ``rm`` beside it that
undoes exactly what it did. Where all of them are wanted,

.. code-block:: console

   $ make mkstubs

runs the four, and ``make rmstubs`` runs the four that undo them: the stub
module unloaded, and the simulated chip and dummy sensor this tree made taken
down while another program's is left alone. It prints the bus to name in a payload set, which is
numbered as adapters appear and so is not the same from one run to the next.
The stub's chips are fixed when the module is loaded, which is why the address
is the script's argument rather than something a configuration can choose
afterwards: ``./bin/mki2c 0x50`` reloads it with a chip there.

A SPI Payload
-------------
Keyword ``type: spi``. A peripheral on a clocked, full-duplex link.

.. code-block:: text

   path           the device node, which names the bus and the chip select
                  together

   max_speed      the greatest clock rate the peripheral accepts, in Hz
   spi_mode       0 | 1 | 2 | 3, the clock polarity and phase. Named so
                  because a payload's own mode says whether it sends of
                  its own accord or on a trigger
   bits_per_word  commonly 8, though a controller may support other widths
   bit_order      msb | lsb
   cs_active      low | high

The mode must match the peripheral; a mismatch fails the way a wrong data rate
fails on a serial line.

There is no parity and there are no stop bits. SPI is clocked and full duplex,
and a transfer is delimited by the chip select rather than by framing bits.

What the Configuration Will Not Let Pass
----------------------------------------
These mistakes are refused rather than ignored, because each of them
otherwise produces a payload that looks configured and is not:

* A **repeated name**. A name picks out one payload: tcspecial keeps its
  handlers by name, so a repeat has one silently replace the other, and a
  simulator configuration is joined to the payloads by name, so it would drive
  two payloads from one entry. The rule reaches no further than the file --
  two sets may name their payloads and their groups alike, and the shipped
  examples do.
* An **attribute of another type** -- a device file payload stating
  ``protocol``, ``payload_address`` or ``payload_interface``, or a network
  payload stating ``path`` or ``payload_address``. The check is
  made after the group has been laid under the payload, since an attribute
  inherited from a group reaches the payload just as one it states itself
  does: a payload of one type joining a group written for another is what this
  catches, and the error names the group when the group is where the attribute
  came from.
* A **term of a link on a type that is not on one**: a ``datarate`` or a
  ``spi_mode`` or a set of ``lines`` on a network or device file payload. Each
  type
  that *is* on a link refuses what belongs to another type as its own terms are
  read, and those two types have no terms of their own to read -- so a term
  written on one of them used to be taken in and dropped, which is a payload
  that looks configured and is not. A network payload's ``protocol`` is its
  own type's and is not a term of somebody else's link.
* A **word that is not an attribute**. A misspelled ``packet_size`` is a
  payload with nowhere to send its data, and ignoring the word says nothing
  about it.
* A **misspelled section**, for the same reason: ``payload`` is not
  ``payloads``, and a file whose only section is misspelled describes no
  payloads at all.
* A **packet interval** on a periodic payload. How fast a payload sends is a
  property of the simulation, so the interval belongs in the simulator
  configuration and the error says so. This was ignored for a while, which was
  the same silence the two modes exist to prevent -- a file stating an
  interval has said something about the payload's timing, and reading it as
  though it had not is reading a different file than the one that was written.
  An interval a payload takes from its group is refused as one it states
  itself is.
* **Two payloads wanting the same thing**. Two at one address cannot both be
  started; two on one device file, serial line or SPI peripheral is worse,
  each taking part of the one stream and reporting it as though it were the
  whole. A payload's own address and its end of the data link -- the section's
  ``payload_telemetry.dest_base`` counted on by the payload's id -- are compared
  together,
  since a port is a port whatever claims it, and ``localhost``, ``127.0.0.1``
  and ``::1`` count as one host. A Unix socket collides by its path, its port
  meaning nothing, and so does a device file opened twice. The exception is a
  bus: several devices on one I\ :superscript:`2`\ C bus is what a bus is for,
  so what is claimed there is the bus and the address together.

The rule is over the handlers a file produced rather than the words that
produced them, so it applies to every type of payload alike.

An Example
----------
This is the shipped ``tcspecial1.yaml``, without its ``tcspecial`` section.
``udp-steady`` and ``udp-fast`` are reached the same way, so what they share is
a group; ``tcp-stream`` and ``random-device`` share nothing and state
everything for themselves. Each is named for what it is, which is what a panel
in the MOC shows.

.. code-block:: yaml

   version: "1.0"
   description: TCSpecial payload set 1

   payload_groups:
     - name: udp_localhost
       type: network
       protocol: udp

   payloads:
     - name: tcp-stream
       type: network
       protocol: tcp
       payload_interface: localhost:5000
       packet_size: 12

     - name: udp-steady
       group: udp_localhost
       payload_interface: localhost:5001
       packet_size: 11

     - name: random-device
       type: device-file
       path: /dev/urandom
       packet_size: 1

     - name: udp-fast
       group: udp_localhost
       payload_interface: localhost:5003
       packet_size: 15

The same set in the other format is the shipped ``tcspecial1.xml``. It
describes the same four payloads and the same group, by the conventions of
`Configuration File Formats`_ -- every value a child element, a sequence as
repeated siblings, an absent optional field simply left out:

.. code-block:: xml

   <?xml version="1.0" encoding="UTF-8"?>
   <payload>
       <version>1.0</version>
       <description>TCSpecial payload set 1</description>
       <payload_groups>
           <name>udp_localhost</name>
           <type>network</type>
           <protocol>udp</protocol>
           <address>localhost</address>
       </payload_groups>
       <payloads>
           <name>tcp-stream</name>
           <type>network</type>
           <protocol>tcp</protocol>
           <address>localhost</address>
           <port>5000</port>
           <packet_size>12</packet_size>
       </payloads>
       <payloads>
           <name>udp-steady</name>
           <group>udp_localhost</group>
           <port>5001</port>
           <packet_size>11</packet_size>
       </payloads>
       <payloads>
           <name>random-device</name>
           <type>device-file</type>
           <path>/dev/urandom</path>
           <packet_size>1</packet_size>
       </payloads>
       <payloads>
           <name>udp-fast</name>
           <group>udp_localhost</group>
           <port>5003</port>
           <packet_size>15</packet_size>
       </payloads>
   </payload>

Both files are shipped and either may be run. The YAML is the one to read,
being the one that carries the comments; what the two have in common is
checked by a test, so an example that drifted in one format would fail the
build rather than mislead a reader.

Not Yet Read
------------
These have been described for this project and are not yet accepted by any
parser. A file giving one is refused rather than quietly running without it.

.. code-block:: text

   length-prefixed framing a read whose length is in its first one or two
                           bytes
   max_interbyte_interval  a read ended by a gap between bytes
   bidirectional           whether a handler carries data both ways; every
                           handler does at present

Starting and stopping a payload
===============================
In tcsmoc each panel has a button per command: ``Enable`` sends START_DH and
``Disable`` sends STOP_DH. Both are pressable whatever the handler is doing --
tcspecial takes a START_DH for one already running and a STOP_DH for one
already stopped without harm -- and the status beside the name says which of
the two has taken effect.

In tcssim a payload that is sending offers ``Disable``, which stops it, and
one that is not offers ``Enable``, which starts it: one button, labelled with
what pressing it will do, since a simulated payload is either sending or not
and there is nothing else to ask of it. The words are tcsmoc's, so that a
payload disabled in one window is disabled in the other rather than silent in
one and disabled in the other; they were the payload's own, ``Send`` and
``Silent``, which left the reader of two windows translating.

tcsmoc had one button that way too. It said ``Receive`` or ``Discard``
according to the status, which meant a press could not be made until the
handler was in the state that offered it -- and when a panel looks wrong,
what the spacecraft says to the command is the thing worth finding out.

Seeing what was read
====================
Each payload's panel has a ``Configuration`` button for it, in both windows
and spelled the same way in each. It shows every
parameter read for that payload: what the tcspecial configuration said about
the payload and how it is reached, and what the simulator configuration said
about how a stand-in for it behaves, each under the name of the file it came
from.

It shows nothing but what the files said. What a payload has actually done is
on the panel itself, and only the attributes of that payload's own type appear
-- a device has no protocol and a bus no port -- along with only the faults
that were asked for. Where a file stated nothing, the line says what the
program will use instead rather than passing it off as a statement: a handler
with no OC address is shown as one that cannot be started.

In tcsmoc the simulator half is shown when that file can be read. The MOC
simulates nothing, so a simulator configuration that is missing or written for
different payloads costs it that half and nothing else; the panel says so in
place of the settings.

The shipped examples
====================
Four payload sets are shipped under ``tests/manual``, each a pair: a tcspecial
configuration and the simulator configuration beside it, named for it. The
pairing is a convention of these files and not a rule -- a name only has to
end in ``.yaml`` or ``.xml`` -- but a set added that way needs no change
anywhere to be run.

Each tcspecial configuration is shipped in both formats -- ``tcspecial2.yaml``
and ``tcspecial2.xml`` are one set written two ways, and a test holds the two
to describing the same data handlers. The YAML is the one to read: it carries
the comments. Which spelling a run reads is the last letter of the MOC's
target, ``runmocy`` or ``runmocx``; every other target takes the file named in
``TCSPECIAL_CONFIG_FILE``. The simulator configurations are YAML alone:
nothing asked
for them in both spellings, and the pairing convention names them from the
other file's stem.

``tcspecial1.yaml``
  Four payloads of three types, which is what exercises the panels and the
  grid. Every program falls back to this set when nothing names a file.

``tcspecial2.yaml``
  Two payloads, one of each type: one that sends of its own accord and one
  that answers a request. The Makefile defaults to this set.

``tcspecial3.yaml``
  One payload, reached over a stream. It differs from set 2's first payload
  only in the transport, which is what makes it the set to debug a single link
  against -- and what the type and transport in a simulator configuration are
  there to tell apart.

``tcspecial4.yaml``
  One payload of every supported type -- a socket, a device file, a bus, a
  clocked link and a serial line -- which makes it both the set that exercises
  a link's own terms and the set that reaches every type at once.

  Three of the five need nothing: a datagram is a socket, and the device file
  is ``/dev/urandom``, which is always a source of bytes. The line and the
  clocked link need nothing either, in the sense that matters -- tcssim stands
  in for each with a pty and makes the path the file names a link to it while
  the payload runs. The bus is the one that needs something of the host: the
  kernel's I\ :superscript:`2`\ C stub, loaded at the address this set names,
  which ``bin/mki2c`` does -- or ``make mkstubs``, which runs it along with the
  other three -- and which tcssim says how to do if it is not there.

The Simulator Configuration
===========================
What simulating a payload takes, which is not a property of the payload: how
often it produces a packet, and how it divides one into segments. Only tcssim
acts on it; tcsmoc reads it too, to show what the tcssim it started is
simulating, and does without it if it is not there.

Every setting is optional, and **a setting written empty is a setting not
stated**: ``packet_interval_ms: ""`` and no ``packet_interval_ms`` at all mean
the same thing. That is what lets tcsgui write one of these files as a form --
every payload of the configuration, every setting of a payload, blank until
somebody fills one in -- and have the file load all the same. A value that is
there and wrong is still refused, naming it.

.. code-block:: text

   simulated_payload_groups        optional
     name                 the name payloads refer to
     ...                  any of the settings below

   simulated_payloads
     name                 the payload this one stands in for
     type                 the type of payload it is, as the tcspecial
                          configuration says
     protocol             which transport, for a network payload
     payload_address      the socket the payload answers from, if this file
                          cares to say
     payload_port         which port it answers from, likewise
     group                the group to take unstated settings from, if any
     ...                  any of the settings below

**What it stands in for**

Two of the attributes describe the payload rather than the simulation, and are
required for that reason. The two files are joined by handler name and by
nothing else -- and a name is what a set copied from another one keeps -- so a
simulator configuration written for one set parses happily against another
set's payloads, and the run that follows looks like a payload that never
sends. Stating the type here makes that pairing an error instead.

.. code-block:: text

   type                   the type of payload this stands in for, in the
                          tcspecial configuration's own word: network,
                          device, serial, i2c, spi, can, iio, rpmsg or
                          gpio. Must be what that configuration says
   protocol               which transport, for a network payload, and refused
                          for any other type. Must be what that configuration
                          says
   payload_address        the socket the simulated payload answers from: a
                          host for a UDP payload, a path for a Unix datagram
                          one. Optional, and only for those two types
   payload_port           the port it answers from. Optional, and refused for
                          a Unix datagram payload, which answers from a path

Where the payload *is* is not stated here at all: the simulator takes it from
the tcspecial configuration, which decides it. Writing ``payload_interface``
here -- or ``address`` or ``port``, which that file spelled them before -- is
refused with the place they belong; they are the likeliest thing to be carried
across from the other file.

What this file may say about an address is the other end of the same link: the
socket the simulated payload answers from.

.. code-block:: yaml

   simulated_payloads:
     - name: auto-send
       type: network
       protocol: udp
       payload_address: 127.0.0.1   # the socket this payload answers from,
       payload_port: 7000           # not the one the handler waits at
       packet_interval_ms: 1000

Nothing in the link needs it -- a handler learns where its payload is from the
first packet it gets -- but something outside the simulation may: a rule on a
firewall, or a capture being read afterwards. Only a datagram payload has an
address of its own to bind; a stream payload is the end that waits, so it
binds the address the tcspecial configuration gives it. A Unix datagram
payload answers from a path, so it takes an address and no port. Neither end
may be the other: a payload told to bind its handler's own socket is refused
rather than left to fail at the bind. Stating neither gets what every
simulator configuration asked for before this existed -- any interface and a
port the system chooses.

A word that is not a setting is refused here too -- in a payload, in a group,
and in the file's own sections. A misspelled setting that was ignored would
have the simulator do something other than what the file asked for and say
nothing: a payload asked through a misspelling to drop a tenth of its packets
drops none, and looks exactly like one that was never asked.

Neither the type nor the transport is taken from this file: the tcspecial
configuration decides both, and a disagreement about either is an error naming
the payload and what each file says of it. A group may carry them, like any
other setting -- though payloads that share a rate need not share a transport,
which is why the shipped ``tcspecial1sim.yaml`` leaves them with the payloads.

**Settings**

.. code-block:: text

   packet_interval_ms     milliseconds between packets; 0 is as fast as the
                          payload can be driven. Periodic payloads only: a
                          triggered one sends when it is asked, and stating
                          an interval for one is an error
   segment_interval_ms    milliseconds between the segments of one packet;
                          defaults to the packet interval
   segment_size           bytes in one segment; defaults to the whole packet,
                          whose size the tcspecial configuration gives

**Fault injection**

A payload that always works exercises only the path where everything works.
These settings have a simulated payload misbehave on purpose, so that a
handler's counting, framing, timeouts and reconnection can be made to happen
rather than waited for. Each defaults to no fault, so a file that mentions
none asks for a payload that works -- which is what every simulator
configuration written before these existed asks for.

.. code-block:: text

   drop_percent           packets in a hundred that are never sent. A dropped
                          packet is not counted as sent either, which is what
                          tells it from one lost on the way
   corrupt_percent        packets in a hundred with one byte altered. Never
                          left as it was: a corruption that changed nothing
                          would be a fault that did not happen
   truncate_percent       packets in a hundred sent short. Always shorter than
                          the packet size and never empty; a packet of one
                          byte cannot be shortened and goes whole
   jitter_ms              milliseconds a packet may be late, chosen afresh for
                          each one. Waited out inside the interval rather than
                          added to it, so the rate is unchanged and the packet
                          arrives late within it
   silent_after           packets after which the payload sends no more,
                          leaving its link open; 0 is never. What a wedged
                          instrument looks like from the handler's end
   close_after            packets after which the payload hangs up and then
                          listens again, so the handler has to reconnect;
                          0 is never. Streams only -- tcp and unix_stream
   ignore_trigger_percent requests in a hundred a triggered payload leaves
                          unanswered, which is what a handler's response
                          timeout exists for. Triggered payloads only

The percentages are nought to a hundred; a larger number is an error naming
the setting and the value. A fault the payload could not have is an error too:
``close_after`` for anything but a stream, and ``ignore_trigger_percent`` for a
payload that sends on its own. Faults belong in this file alone -- the
tcspecial configuration describes what a payload is meant to do, and tcspecial
reads it. A payload that starts with any fault set says so in one line of log,
so a run is not left to be guessed at afterwards.

.. code-block:: yaml

   simulated_payloads:
     - name: auto-send
       type: network
       protocol: udp
       packet_interval_ms: 500
       drop_percent: 10        # one packet in ten never arrives
       jitter_ms: 50           # and the rest are up to 50ms late

The two files are joined by name. Every data handler needs a simulated payload
naming it, and a name here that no handler has is an error rather than a line
with no effect, because a typo is otherwise indistinguishable from a payload
meant to be left out.

A simulated triggered payload sends one packet for each request it is sent and
nothing otherwise. The segment settings still apply to it -- how a payload
divides an answer is its own business however it was prompted -- but the packet
interval does not, and is refused. Two types cannot be asked at all and are
refused outright: a device, which tcspecial opens and reads directly, and a
device on a bus, where the master's own read is the request and a second master
cannot see it.

This is the shipped ``tcspecial1sim.yaml``. It states no version, and that is
deliberate: a configuration says which version it is so that it can be checked
against the software that will read it on the vehicle, before anything opens a
device. Nothing aboard reads this file -- the simulator does not fly -- so a
version here would answer a question nobody is asking, and a field that is
read, printed and never acted on is a field somebody will one day believe in. A
file that states one is refused, saying this.

.. code-block:: yaml

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

Running the Programs
====================
The Makefile names a payload set once and gives it to every program that needs
it, which is the point: tcsmoc builds its panels from the tcspecial
configuration and hands that same file to the tcspecial and tcssim it starts,
so a program
reading a different one would serve or simulate payloads the panels do not
describe.

``TCSPECIAL_CONFIG_FILE`` is the set, named without a suffix:
``$(TCSPECIAL_CONFIG_FILE).yaml`` is the tcspecial configuration, which
reaches every program as a command line argument, and
``$(TCSPECIAL_CONFIG_FILE)sim.yaml`` the simulator configuration,
which reaches tcssim through the environment -- and tcsmoc too, which it
inherits the same way: tcsmoc acts on none of it, and reads it only so a panel
can show what the tcssim it started is simulating. It defaults to
``tests/manual/tcspecial2``.

The sets live under ``tests/manual`` because running one is something a person
does at a terminal: each opens sockets or device nodes, so none of them belongs
in a test that runs on every build.

The two windows open at opposite edges of the display -- tcsmoc at the top and
tcssim at the bottom -- because they are read together and would otherwise
land on top of each other. It is a request to the window system: X11 honours
it, Wayland may ignore it, and a tiling window manager will do as it likes.

Everything at once
------------------
``make runmocy`` runs tcsmoc, which starts a tcspecial and a tcssim of its own:

.. code-block:: console

   $ make runmocy
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial4

The last letter is the spelling of the tcspecial configuration it reads:
``runmocy`` the YAML and ``runmocx`` the XML. Both take the same
``TCSPECIAL_CONFIG_FILE``, and add the suffix themselves, so the two are one
payload set
read two ways rather than two sets. The simulator configuration is not
spelled by it: only tcssim reads that one, and tcsmoc merely passes it on.

Both files are named because tcsmoc passes the tcspecial configuration on to
its children and never reads the simulator configuration, which reaches tcssim
by being in tcsmoc's environment. Naming only one of a pair is how the two
files come to describe different sets, which is reported rather than run:
every data handler must have a simulated payload naming it.

If a tcspecial is already running, tcsmoc attaches to it instead of starting a
second -- it asks by PING -- and leaves it running when the window is closed.
One it starts itself is started on the address it will send commands to, so
that half of the question never arises.
Starting a second never worked: the command interpreter's address can be bound
once. What it is serving, though, is the configuration it was started with: one
left over from an earlier payload set goes on serving that set's handlers,
and a payload the new file plainly contains is then answered ``NotFound``.
Stopping it and letting tcsmoc start a fresh one is the fix; tcspecial's log
says which handlers it does serve.

One program at a time
---------------------
Useful when a program is to be run under a debugger, or restarted without
disturbing the others, or watched with its own logging.

.. code-block:: console

   $ make run TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1
   $ make runsim TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1

``make run`` passes only the tcspecial configuration: tcspecial does not
simulate anything. ``make runsim`` passes both, because tcssim run on its own
needs the payloads as well as the simulator configuration -- and one variable
names both, so they cannot be given for different sets. Started in that order,
tcsmoc finds the tcspecial already running and attaches to it.

``make runmocy`` runs tcsmoc on the YAML spelling of the set and
``make runmocx`` on the XML:

.. code-block:: console

   $ make runmocx TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1

One target per spelling rather than one target and a variable, so that a
spelling is asked for the way everything else here is asked for -- by name,
with nothing to remember about which variable selects it. The two are
otherwise the same target. A set is transcribed by hand into the other format,
so running the one you have not run lately is how the transcription gets
exercised; the simulator configuration is YAML in both cases, there being only
one of it, and ``make runverifyy`` and ``make runverifyx`` check the two
spellings the same way.

Checking a set without running it
---------------------------------
``tcsverify`` reads a payload set and says what is wrong with it, without
opening a socket or a device node. It takes one file or two, the tcspecial
configuration first:

.. code-block:: console

   $ cargo run --bin tcsverify -- tests/manual/tcspecial2.yaml
   $ cargo run --bin tcsverify -- tests/manual/tcspecial2.yaml tests/manual/tcspecial2sim.yaml
   $ cargo run --bin tcsverify -- tests/manual/tcspecial2.xml tests/manual/tcspecial2sim.yaml

Either spelling of the configuration will do -- the last line reads the XML of
the same set -- and the simulator configuration is named in full rather than
worked out from the first file's name.

A good set prints nothing. A bad one prints a line per problem, each naming
the file, the line and what is wrong there:

.. code-block:: console

   $ cargo run --bin tcsverify -- /tmp/draft.yaml
   /tmp/draft.yaml:61: payload "udp-steady": packet_size must be greater than zero
   /tmp/draft.yaml:68: payload "random-device" is a device-file payload, so port does not apply to it

It exits 0 when nothing is wrong, 1 when something is, and 2 when the command
line is neither of those two shapes -- which a script can tell apart from a
bad file.

Every program that runs a set refuses the first thing wrong with it and stops,
which is what a program that cannot run without it should do and is not what a
person fixing the file wants: a set with three mistakes in it is three runs of
tcsmoc, each reporting one of them. This asks the same rules -- the library's,
the ones those programs use -- for all of their answers at once.

It reports a hundred problems at most, and then says ``Too many errors,
halting``. The hundred and first is what says there are more; beyond that a
page of them would mostly be consequences of the first few.

Both files of the set are checked, and the second against the first: every
payload simulated, every simulated payload a payload, and every setting one
that payload can have. A tcspecial configuration with problems of its own is
not joined to -- there would be nothing to join to, and a join against half of
a set reports the other half as unsimulated -- and tcsverify says so rather
than leaving it unsaid.

What the programs take without the Makefile
-------------------------------------------
.. code-block:: console

   $ cargo run --bin tcspecial -- tests/manual/tcspecial1.yaml
   $ PAYLOAD_SIM_YAML=tests/manual/tcspecial1sim.yaml \
       cargo run --bin tcssim -- tests/manual/tcspecial1.yaml
   $ PAYLOAD_SIM_YAML=tests/manual/tcspecial1sim.yaml \
       cargo run --bin tcsmoc -- tests/manual/tcspecial1.yaml

Every tcspecial configuration above may end in ``.xml`` instead, and the same
commands then run the same set in the other format:

.. code-block:: console

   $ cargo run --bin tcspecial -- tests/manual/tcspecial1.xml
   $ PAYLOAD_SIM_YAML=tests/manual/tcspecial1sim.yaml \
       cargo run --bin tcssim -- tests/manual/tcspecial1.xml
   $ PAYLOAD_SIM_YAML=tests/manual/tcspecial1sim.yaml \
       cargo run --bin tcsmoc -- tests/manual/tcspecial1.xml

The extension is what says how to read a file -- see `Configuration File
Formats`_ -- and it is read rather than guessed at, so a name ending in
neither is refused rather than parsed as whichever the program tried first.
The simulator configuration is YAML in these examples because
``PAYLOAD_SIM_YAML`` names a file whose extension says the same thing: a
simulator configuration may be XML as well, and the variable's name is older
than that.

An argument naming the configuration file beats any environment variable
naming one.
Failing an argument, tcspecial reads ``PAYLOAD_CONFIG_PATH`` and tcssim reads
``SIM_PAYLOAD_CONFIG_PATH``; failing that, every program reads
``tcspecial1.yaml``. Tcsmoc has no variable of its own, because the programs it
starts inherit its environment: a variable naming tcsmoc's file would name
theirs as well.

The configuration file is the only argument tcspecial takes. Where it serves
commands is in ``tcspecial.yaml``:

.. code-block:: yaml

   tcs_cmd:
     recv: "0.0.0.0:4000"           # commands about the spacecraft
     send: "127.0.0.1:4100"         # the MOC's own end of that link
   payload_command:
     source_base: "127.0.0.1:4300"  # a payload's commands come from here
     dest_base: "0.0.0.0:4200"      # and are taken here, plus its own id

Two links, because what the ground needs in a hurry must never be queued
behind what it asked for at leisure: a restart waiting behind a payload's
statistics is a spacecraft that cannot be rescued while it is busy. ``PING``,
``RESTART_ARM``, ``RESTART`` and ``CONFIGURE`` go to ``tcs_cmd.recv``;
``START_DH``, ``STOP_DH``, ``QUERY_DH``, ``QUERY_DH_SAMPLE`` and
``CONFIGURE_DH`` go to that payload's own socket under ``payload_command``;
``CONNECT`` is answered on either. One table in the library says which is which, so the two ends cannot
disagree about it.

Both are required, and they must differ -- one socket cannot be two links.
Nothing else may bind either of them: a payload given port 4000 is refused
where its file is read, since it would take the commanding with it.

Each link has a loop and a thread of its own, so a payload command that takes
its time waits for nothing on the other one. tcspecial says where both are as
it starts:

.. code-block:: console

   Commands are taken on 0.0.0.0:4000 and payload commands on 0.0.0.0:4001 as tcspecial/src/tcspecial.yaml asked, and beacons go from 0.0.0.0:5550 to 127.0.0.1:5650

A command sent to the wrong one of the two is refused rather than served: it
means one end is not reading the configuration the other is, and that is the
last fault to hide on the link the spacecraft is restarted over.

A spacecraft command is also taken only from where the configuration says the
MOC sends them -- ``tcs_cmd.send``, which the MOC binds for that link -- and
anything else arriving there is ignored, with nothing sent back to the sender.
The other three links are checked the same way: the MOC takes a response only
from the socket its command went to, a beacon only from ``beacon.send``, and a
payload's telemetry only from ``payload_telemetry.source_base`` counted on by
that payload's id. Where the configuration names every interface --
``0.0.0.0``, which the vehicle's side usually is -- the port is all there is to
compare against, the address a datagram carries then being whichever interface
it came in on.

Each ignored datagram says so on the console, naming where it came from and
where the configuration expected it from:

.. code-block:: console

   a command came from 127.0.0.1:53124 and not from 127.0.0.1:4100, which is where the configuration says the MOC sends from -- ignored

Which is the message to look for when a link is deaf: it tells a link nothing
is sending on from a link whose two ends have been given different files.

The MOC opens both when it connects, and says so:

.. code-block:: console

   Connected to 127.0.0.1:4000, payload commands to 127.0.0.1:4001

The address box is the host and the spacecraft's command port, as before;
payload commands go to the same host on the port ``tcspecial.yaml`` states,
so moving the link moves both by typing one address. The MOC sends each
command on the link it belongs on, and asks ``CONNECT`` of both -- if the two
answer differently they are not one spacecraft, and ``Last Response`` says
so.

Where its beacons come from, where they are received, and how often belong to
the payload set -- a set's ground station being what listens for its beacons -- and are
stated in the ``tcspecial`` section of the set's own payload configuration
file:

.. code-block:: yaml

   tcspecial:
     beacon:
       interval_ms: 5000
       send: "0.0.0.0:5550"
       recv: "127.0.0.1:5650"

Where the links are
-------------------

Five of the section's attributes are a host and a port written together, as
``address:port``, and most of them sit in a subsection with the rest of what
they are about:

.. code-block:: yaml

   tcspecial:
     tcs_cmd:
       recv: 0.0.0.0:4000
       send: 127.0.0.1:4100
     payload_command:
       source_base: 127.0.0.1:4300
       dest_base: 0.0.0.0:4200
     payload_telemetry:
       source_base: 0.0.0.0:6000
       dest_base: 127.0.0.1:6100

``tcs_cmd`` is the spacecraft command link, which is one conversation and two
sockets. ``tcs_cmd.recv`` is tcspecial's own: it receives commands about the
spacecraft there and sends its responses from there, a response going back out
of the socket its command arrived on. ``tcs_cmd.send`` is the MOC's own, which
it sends those commands from and receives the responses on. Both ends are in
one file because both ends read this file, and two places to say one thing is
one place for them to disagree.

``payload_command`` is where each payload is commanded, and it has two bases
for the same reason ``payload_telemetry`` does: the link has two ends. A
payload's socket on each side is that side's base plus **the payload's own
id**, so a set of four payloads based at 4300 and 4200 is commanded from 4300
to 4303 on the ground and takes those commands on 4200 to 4203 aboard.

Both ends come out of the file, including the host: this is the one link the
address box does not move. Every payload command link is written down at both
ends so that tcspecial can refuse a payload command that came from anywhere
else, which is the same rule the other three links are held to.

It was one socket for all of them, ``payload_interface``, which worked and
left two things undone. Nothing said which socket the ground sent from, so
tcspecial could not be told what a payload command should come from; and one
socket meant one queue, so a command about a slow payload was ahead of a
command about a quick one.

``payload_telemetry`` is where each payload's own data link is, and it has two
bases because the link has two ends. Every payload has a UDP interface for
exchanging data with the ground; tcspecial binds its end at
``payload_telemetry.source_base`` plus **the payload's own id** -- the position
the payload has in the configuration, counting from nought -- and the MOC binds
its end at ``payload_telemetry.dest_base`` plus the same id. So a set of four
payloads based at 6000 and 6100 has tcspecial at 6000 to 6003 and the ground at
6100 to 6103.

Both ends come out of the one file both ends read, so neither has to be told
the other's: tcspecial sends a payload's telemetry to the end the MOC binds,
from its first datagram, and the MOC sends nothing on that link at all.

It did have to be told. The ground's end used to be whatever port the system
chose, so nothing said where it was, and the MOC taught tcspecial by saying
hello -- an empty datagram to every payload, once a second, because Enable and
Disable rebind the handler's socket and what it had learnt went with it. A
ground station that said hello once was silently cut off by the next Enable.
Naming the ground's end ended that, and with it the only traffic the MOC sent
on its own.

That is why a payload entry states no address for its ground link. It used to
state ``oc_address`` and ``oc_port``, which for forty payloads was forty
chances to repeat a port -- and a repeated port is two payloads whose data
arrives interleaved, each reporting part of the other's stream as though it
were the whole. One base and one arithmetic rule cannot be written
inconsistently. A base so high that the payloads counted from it would pass
65535 is refused by name while somebody is reading the file.

An address and a port are one attribute apiece for the same sort of reason:
two things only ever used together are one fact about a configuration, and
writing them apart invites a file that states one and not the other. Each is
split at its last colon, so whatever a host is called the port is the part
after it. None of them states a protocol and none can: every link here is UDP,
and an attribute for it would be one that could say something untrue.

All three are required of every command interpreter configuration, with no
defaults: beacons are how the ground knows the spacecraft is alive, and a
configuration that has not been asked where to send them has not answered.
``tcspecial.yaml`` states them too, and a set's section wins over it; the
file's are what place a set whose configuration states no section at all.

``beacon.send`` is where beacons come from: tcspecial makes the socket it
sends them on out of it, binding that address and port, so every beacon leaves
from there. ``beacon.recv`` is where they are received: tcsmoc makes the socket
it receives them on out of it, binding that address and port, and it is where
tcspecial sends. Both are an address and a port, like the links above, and both
ends read both of them out of the same file -- which is how the two come to
name one pair.

Beacons went to a multicast group, with a second attribute naming the local
interface to join it on. That bought something real: any number of ground
stations could listen without being named, which a unicast beacon cannot offer,
a port being bindable once. It cost two things to get right -- a group address
that is nobody's, and an interface attribute whose being wrong lost every
beacon in silence, there being nothing to report when a datagram is simply not
joined for. One stated sender and one stated listener is what a configuration
can say plainly, and what a firewall rule or a capture filter can be written
against beforehand. A mission that needs more than one ground station listening
sends to a relay that fans the beacons out, which is a thing somebody
configured rather than a thing the network did silently.

``0.0.0.0:5550`` and ``127.0.0.1:5650`` are what the shipped files state, both
programs being run on one machine; a real ground link names the interface
facing it on each end.

Tcsmoc reads both addresses from the same two files, the same way, so moving a
beacon moves both ends of it. The command address is the one
that can still drift: the two ends agree about it by reading the same default,
so a file that moves it leaves the MOC sending where nothing is bound, and no
error on either end reports that.

Tcspecial says both as it starts, on the terminal and whatever ``RUST_LOG`` is
set to:

.. code-block:: console

   Commands are taken on 0.0.0.0:4000 and payload commands on 0.0.0.0:4001 as tcspecial/src/tcspecial.yaml asked, and beacons go from 0.0.0.0:5550 to 127.0.0.1:5650

A payload set states its own version, which every beacon carries:

.. code-block:: yaml

   version: "1.0"

One, two or three decimal parts -- ``1.0`` means 1.0.0 -- because it goes out
as three bytes. Anything else is refused where the file is read rather than
announced as a version the set has not got.

``RUST_LOG`` sets the logging, which the Makefile leaves at ``info``.

.. code-block:: console

   $ make runmocy RUST_LOG=debug
   $ RUST_LOG=tcspecial::ci=trace \
       cargo run --bin tcspecial -- tests/manual/tcspecial1.yaml

Testing
=======
Testing is done with two programs, and a third that checks the files they
read:

* tcsmoc - Simulates an operations center/mission control application. It
  communicates to TCSpecial via UDP datagrams.

* tcssim - Simulates payloads using various types of communication protocols
  and errors.

* tcsverify - Checks a payload set and prints what is wrong with it, without
  running any of it: see `Checking a set without running it`_.

tcsmoc
------

The tcsmoc window
^^^^^^^^^^^^^^^^^
Across the top, from the left:

the ``tcspecial link`` box
  The address of the command interpreter and one button, which reads
  ``Connect`` while the link is down and ``Disconnect`` while it is up. The
  status beside it says which, and is green when connected. Editing the
  address and pressing ``Connect`` is how the MOC is pointed at a different
  tcspecial; the MOC opens the link itself at startup, so the status normally
  reads connected before anything is pressed.

the beacon box
  The indicator, whose colours are listed under `Beaconing`_, the time the
  last beacon arrived, and what it said:

  .. code-block:: text

     Beacon:    (indicator)
     Last rcvd: 02:02:02
     Last msg:  v0.1.0 config v1.0.0 md5: 36999e46a4bf436854442d8f97a2248d

  Each beacon also carries its own number, counted from one by the
  spacecraft, and the MOC says on the console what the numbers say about the
  beacons between them:

  .. code-block:: console

     2 beacons did not arrive: #9 came after #6
     the beacons start again at #1, so tcspecial has restarted

  A beacon says only that the spacecraft is alive, so without the number a
  beacon that was lost on the way reads exactly like one that was never sent.

  ``Last msg`` reads ``<none>`` until a beacon arrives. ``v`` is the software
  flying, ``config v`` the version the payload set states, and ``md5`` the
  digest of the configuration file tcspecial read, so the line says what is up
  there without anything having to connect first. The answer to a ``CONNECT``
  says the same three in the same words: see `What each end read`_. A datagram the MOC cannot read is shown as
  what arrived, cut short, rather than left off the line: something else
  sending to the group is worth seeing beside a green light.

the ``Command`` menu
  The commands below.

``Last Response``
  What the last command answered, under the two boxes. The first thing it says
  in a session is whether tcspecial read the same configuration this MOC did:
  see `What each end read`_.

  Under it, when the last command went out and when its answer came back:

  .. code-block:: text

     Last Response: START_DH OK
     Last sent: 14:22:31   Last rcvd: 14:22:31

  The ground's own clock, and the window's own commands -- what was pressed.
  A response that is slow to come and one that never comes read alike until
  those two times are beside it.

Below that is one panel per data handler, filled across the window before
another row is started under it, and a ``Quit`` button at the foot of the
window, which stops the tcspecial and tcssim the MOC started.

Each panel shows its handler's name and number, its status, how it is
reached, and its packet size; then two readings and one count. The number is
the payload's place in the payload configuration, counted from nought in the
order the file describes them -- ``auto-send (dev id 0)`` -- and nothing in
the file states it. Both windows count the same way, so a payload is the same
number in each. The status is ``Active``,
``Stopped`` or ``Unknown``, and where each comes from is under `Beaconing`_:
the spacecraft's answer to the last ``Enable`` or ``Disable``, and ``Unknown``
for a handler nobody can speak for -- which is what the panels say while no
beacons are arriving.

``Last recv``
  When the MOC last received anything from that payload, on the handler's OC
  address. The MOC's own clock, and read for whether anything is still
  coming. Beside it is the **data indicator**, below.

``From payload``
  What arrived, as the ground received it.

``Recv``
  How many packets have arrived from that payload, counted from the moment
  the MOC started: ``3 recv``.

All three are the ground's end of it, counted and timed by the end that
received them. The spacecraft's account of what it last sent and last
received, and its count of the bytes it sent to its payload, were shown beside
them and are gone: a panel is watched for what reached the ground, and the
claim about it cost a third of the panel to say nearly the same thing. The
count was the spacecraft's too, and the wrong half of its statistics -- the
bytes its handler had read from the ground, a direction nobody sends in, so
the line read ``0 bytes`` beside a payload that was sending steadily. The
panels refresh themselves about once a second; nothing has to be pressed to
see traffic, and nothing is asked of the spacecraft to show it.

Each panel has three buttons. ``Enable`` and ``Disable`` are described under
`Starting and stopping a payload`_; ``Configuration`` shows what the files
said about that handler: see `Seeing what was read`_.

The data indicator
^^^^^^^^^^^^^^^^^^
.. note::
   **Needs examination.** This is a first statement of the rule rather than a
   settled one; what is unsettled is at the end of this section.

Beside ``Last recv`` on each panel is a dot, shaped like the beacon's, saying
what the ground can say about that payload's data:

grey
  Nothing to judge by: as far as the ground knows the payload is not enabled,
  or the payload states no ``stale_interval_ms``.

green
  Data has arrived from that payload within its interval.

yellow
  The payload is enabled and nothing has arrived from it for longer than its
  interval.

There is no red. A payload whose data has stopped may be a payload with
nothing to say, a payload somebody stopped, or a link that has failed, and
nothing on the ground can tell those apart; red is for a condition that is
dangerous, and this is one worth looking at.

How long to wait is the payload's own to say, in its entry or its group:

.. code-block:: yaml

   payloads:
     - name: auto-send
       type: network
       protocol: udp
       payload_interface: localhost:5000
       packet_size: 12
       stale_interval_ms: 3000

The figure is read by the ground and by nothing aboard. A payload that states
none is never judged -- its dot stays grey -- because a figure nobody chose
would make a payload that speaks once an hour look late every minute of it.

Staleness is counted from the later of the last arrival and the moment the
ground came to believe the payload was running, so a payload just enabled has
its whole interval to produce something rather than going yellow the instant it
is switched on.

The MOC asks nothing to work any of this out. An enabled payload sends on its
own, and its data arrives on that payload's own telemetry socket where the
ground times it, so the panels are kept current without a command going out.
There used to be a poller that asked every payload what it was doing once a
second; for forty payloads that was eighty datagrams a second of housekeeping,
and the arrivals had already said most of what it learnt. What only the
spacecraft can say -- its own counters, and whether it has the handler at all
-- is what ``Query all`` is for.

Three things about this are still to be settled: what interval suits a payload
that answers requests, since its data arrives only when tcspecial asks it;
whether *enabled* should mean what the ground last commanded or what the
spacecraft last said, which matters for a MOC that has just attached to a
tcspecial already running; and whether a payload that states no interval should
really look like a payload nobody has enabled, both being grey.

A triggered payload on the console
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
A payload that answers requests sends nothing until tcspecial asks it to, so
when such a panel shows no data the question is what the MOC sent and what came
back -- and a panel shows neither. For those payloads, and only those, tcsmoc
says both on the terminal it was started from.

At startup, once per triggered payload, the whole of what it will be addressed
by and asked with:

.. code-block:: console

   triggered payload triggered-send: TCP localhost:5003, trigger 0x67 6F 0A every 1000 ms, OC 127.0.0.1:6003, status Stopped

Then one line for every command sent about it. A press of its button:

.. code-block:: console

   triggered payload triggered-send: Enable pressed: sending START_DH as a Network handler
   triggered payload triggered-send: START_DH answered Success, so the panel now says Active

and one from ``Query all``, carrying the status each answer came back with
and the trigger count:

.. code-block:: console

   triggered payload triggered-send: QUERY_DH Success: 5 triggers sent, 0 bytes to the ground, 0 from it; QUERY_DH_SAMPLE Success

That one is said when it changes and not again until it changes once more:
what is worth reading is the answer where something moved. A payload that is
not transmitting therefore says this once and then nothing.

``NotFound`` there is worth knowing: the panel shows zeroes either way, but it
means the tcspecial being talked to has no such handler, which usually means it
was started from a different configuration -- most often one left running from
an earlier payload set, which tcsmoc attaches to rather than replacing. The
trigger count is the one number that says the asking is happening at all; it
appears in no other counter, because a trigger is a write with no read behind
it.

What each end read
^^^^^^^^^^^^^^^^^^
When a link comes up, tcsmoc sends its software version and an MD5 of the
payload configuration it read, and tcspecial answers with its own, and with
the version its payload set states. Each prints what it read on the terminal
it was started from before it makes the socket, so they are there to read even
when the link never comes up -- the MOC's first, then tcspecial's, then what
the answer said:

.. code-block:: console

   v0.1.0, configuration tests/manual/tcspecial2.yaml config v1.0.0 md5: 36999e46a4bf436854442d8f97a2248d
   v0.1.0, configuration tests/manual/tcspecial2.yaml config v1.0.0 md5: 36999e46a4bf436854442d8f97a2248d
   tcspecial answers v0.1.0 config v1.0.0 md5: 36999e46a4bf436854442d8f97a2248d

The first two lines are written the same way by the two programs, so two ends
reading one payload set print the same line twice -- which is what makes a
difference in any part of it worth reading. The third says what came back over
the link.

``Last Response`` says whether the two agree:

.. code-block:: text

   tcspecial agrees: v0.1.0, config v1.0.0 md5: 36999e46a4bf436854442d8f97a2248d

The set's version shown there is the one tcspecial sent, which is the version
of the file it read. The MOC does not send its own, so there is nothing to
compare it with: what the verdict is reached on is the software version and
the digest. They are the same payload
set if the digests match: the digest is of what a file says rather than of its
bytes, so the YAML and the XML of one set digest alike, and so do two files
differing only in comments, spacing, the order of their sections or the order
of one payload's attributes. The order of the payloads does count -- ids follow
it -- so each payload is given a sequence number as the file is read, the
digest takes them in that order, and a file may not state a number of its own.
Both formats number them alike, which is what lets the YAML and the XML of
one set digest the same.

A difference means the two ends are not serving the same set. The usual cause
is a tcspecial left running from an earlier one, which tcsmoc attaches to
rather than replacing; stopping it and letting tcsmoc start a fresh one is the
fix.

Commands
^^^^^^^^
From the ``Command`` menu. Three of the seven items send a command:

Ping
  Sends a PING and waits for the answer. ``Last Response`` says that one was
  sent, then what came back -- ``PING OK at 14:22:31``, the time the
  spacecraft made the answer, which every response carries -- and if nothing
  came back within the client's timeout it says when it stopped waiting.

ARM Restart
  Arms a restart, which a restart will not happen without.

Restart
  Restarts the spacecraft software, which only an armed restart does.

The remaining four -- ``Query``, ``Query DH``, ``Start DH`` and ``Stop DH`` --
answer in ``Last Response`` and send nothing. Starting and stopping a handler
is done from its own panel, and the panels show what a query would ask for
without anyone asking.

Beaconing
^^^^^^^^^
TCSpecial sends a beacon at the interval its configuration states, from the
address the payload set's ``tcspecial`` section names to the address beside it,
or ``tcspecial.yaml``'s pair for a set with no section. Tcsmoc binds that same
receiving address, having read the same files. Tcsmoc displays the
following beacon colors:

steady grey
  Either no beacon message has been received yet or the system time has
  changed backwards

steady green
  A beacon message has been received within the expected time

blinking green
  One or more beacon messages has not been received within the expected time
  but this is within the acceptable number of lost messages

blinking yellow
  The number of lost beacon messages indicate a possible issue.

blinking red
  The number of lost beacon messages indicates a likely issue.

Beside the indicator is the time the last beacon arrived, shown the way every
other time in the window is. It is the time the beacon reached the MOC rather
than the time the spacecraft stamped on it: the line is read to find out
whether beacons are still coming, and a spacecraft whose clock had stopped
would otherwise look as though its beacons had.

Until a beacon has arrived the line shows ``--:--:--``, as a panel does for a
direction that has carried nothing. Once one has, the time stays: a pass that
finds nothing ages the colour beside it and leaves the time alone, because the
beacon did arrive when it arrived.

Panels start at ``Unknown``: a MOC that has just come up has been told
nothing, and the tcspecial it attached to may have been running handlers for
hours. The first beacon is what fills them in -- the MOC then asks the
spacecraft what each handler is doing -- and until it arrives the data
indicator is what says whether a payload is delivering.

The indicator leaving green is also what the payload panels go by. A panel's
status is what the spacecraft answered the last ``Enable`` or ``Disable``, and
that answer is worth as much as the ground's hearing from the spacecraft: a
handler may since have stopped, or gone with a process that restarted. So when
the indicator leaves green every panel's status becomes ``Unknown``, whether
or not the command link is up -- a disconnected MOC cannot find out either.

When the indicator is green again, the MOC asks the spacecraft what each
handler is doing, and the panels say what it answers: ``Active`` for one that
is running, ``Stopped`` for one that is not, and ``Unknown`` for a handler
this tcspecial has not got -- which is what an operator sees when the MOC has
attached to a tcspecial left running for another payload set.

That asking happens at two moments and no others: the first beacon, and the
end of an outage. It is not a poll -- nothing is asked while contact simply
holds -- and a payload whose last press was ``Disable`` is left out of it: the
operator said to stop that one, so there is nothing to ask. ``Query all`` asks
about every payload whenever it is pressed.

If the link is down when contact returns, nothing is asked and nothing is
owed: the panels keep saying ``Unknown`` until the next time contact is made
or somebody presses ``Query all``.

Nothing else changes a panel's status. The once-a-second refresh shows what has
arrived and leaves the status to the commands and to the beacon.

tcssim
------

The tcssim window
^^^^^^^^^^^^^^^^^
One panel per simulated payload, in a grid, and a ``Quit All`` button at the
foot of the window. Nothing in the window names a payload or fixes how many
there are: the two configuration files decide that, so a payload added to or
removed from them adds or removes a panel.

Each panel shows, from the top:

the name, the number and the status
  The number is the payload's place in the payload configuration, as tcsmoc's
  panels show it, so the two windows name the same payload the same way. The
  status is red while the payload is not sending and black while it is.

how the payload is reached
  The same description tcsmoc's panels carry for the handler at the other end
  of the link.

``Pkt`` and ``Seg``
  The packet size, from the tcspecial configuration, and the segment size,
  from the simulator configuration.

``Pkt Int`` and ``Seg Int``
  The interval between packets and between the segments of one packet, both
  from the simulator configuration.

Those four can be changed while a payload is running, and take effect on the
next packet: that is what the simulator is for, and it is quicker than editing
a file and starting again. Nothing is written back to the files, so the next
run starts from what they say.

A segment is a piece of a packet, so the segment size may not be larger than
the packet size. Equal is the ordinary case -- a packet sent in one segment.
Make them disagree and a message says so at once, naming both sizes and
either way out of it, and the button that starts the payload is taken away
until they agree again; the edit that fixes them takes the message back. The
sizes are checked when the files are read as well, so a set whose simulator
configuration states a segment larger than the packet it is given says so
before anything is touched.

Below them are the two buttons -- one to start and stop the payload, described
under `Starting and stopping a payload`_, and ``Configuration``, described
under `Seeing what was read`_ -- and then three lines:

``Sent``
  When the last packet went, and the first few bytes of it.

``Recv``
  When the last packet arrived, and the first few bytes of it -- for a payload
  that answers requests. A payload that sends of its own accord is never
  spoken to, so this line is left blank for it rather than showing
  ``--:--:--`` for the whole run. The line stays so that every panel is the
  same shape; it is the two items on it that go.

  Tcsmoc's panels do the same thing in the other direction: a handler whose
  payload sends of its own accord is never asked anything, so its ``Last
  sent`` line is blank.

``Pkts``
  How many packets have gone and arrived since the payload started. The
  arrived half is there for a payload that answers requests, as the ``Recv``
  line is and for the same reason: a payload that sends of its own accord is
  never spoken to, so a count of what arrived could only ever read nought.

The times and bytes are the simulator's own account of what it did, where
tcsmoc's panels show the same transfers as the spacecraft reported them. Two
panels disagreeing about one transfer is worth knowing about, which is why
both are shown the same way.
