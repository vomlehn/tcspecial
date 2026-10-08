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

Configuration File Formats
==========================
A configuration file may be written in JSON, YAML, or XML. All three describe
the same settings and are read into the same values, so the choice is one of
local preference and tooling rather than of capability.

The format is taken from the file's extension:

.. code-block:: text

   .yaml, .yml     YAML
   .xml            XML
   anything else   JSON

JSON is the fallback, so a file with no extension, or with an unfamiliar one,
is read as JSON.

Two differences are worth knowing when moving a file between the formats.
JSON has no comments, so remarks in a YAML or XML file have nowhere to go in
JSON. JSON also has no hexadecimal numbers, so a value such as an
I\ :superscript:`2`\ C address is written there as the string ``"0x48"``
rather than as ``0x48``; YAML and XML accept either spelling.

Which Files There Are
=====================
Four kinds of configuration file, each read by the programs that need it.

``tcspecial.yaml``
    The command interpreter's own settings: the address it listens on, the
    beacon interval, and where the telemetry log goes. Read by tcspecial at
    startup, from the path ``TCSPECIAL_CONFIG_PATH`` names when that is set.
    Nothing about the payloads is in it.

a payload configuration file
    Which data handlers exist, how tcspecial reaches each one, and how large
    its packets are. Named on the command line of every program that reads it;
    ``payload1.yaml`` when nothing names one. The shipped sets are
    ``payload1.yaml`` and ``payload2.yaml``.

a simulator configuration file
    How a simulated payload behaves: how often it produces a packet and how it
    divides one into segments. Read only by tcssim, which takes it from
    ``PAYLOAD_SIM_YAML``. Named for the payload file it goes with:
    ``payload1sim.yaml`` beside ``payload1.yaml``.

an endpoint configuration file
    The richer description of what is at the end of each link: groups of
    endpoints by kind, with every term a serial line, an I\ :superscript:`2`\ C
    bus or a SPI peripheral needs. Its endpoints can become data handlers, so
    it is an alternative to a payload configuration rather than an addition to
    one.

Every one of them may be written in any of the three formats, and a payload
file and an endpoint file are told apart by what is in them rather than by
their names: a file naming ``payloads`` is the first, and one naming
``endpoints`` is the second.

Payload Configuration Files
===========================
A payload configuration has a version, a description, an optional section of
groups, and the data handlers themselves. A handler takes every attribute it
does not state from the group it names, so a group holds what several handlers
share.

.. code-block:: text

   version        "1.0"
   description    free text

   payload_groups                  optional
     name         the name payloads refer to
     ...          any of the attributes below

   payloads
     dh_id        the handler's number, which commands name it by
     name         the handler's name, which the panels show
     group        the group to take unstated attributes from, if any
     ...          any of the attributes below

**Attributes**

.. code-block:: text

   type           network | device
   protocol       tcp | udp | unix_stream | unix_dgram     network only
   address        the host a network handler reaches, or the path of a
                  Unix-domain socket
   port           the port a network handler reaches; none for a Unix socket,
                  which is named by its path alone
   path           the device a device handler opens
   packet_size    bytes in one packet
   oc_address     the host this handler exchanges payload data with the
                  OC on, which is always UDP
   oc_port        the port it does so on
   mode           periodic | triggered; absent is periodic
   trigger        what to send to make the payload answer:    triggered only
                  bytes, as unquoted hexadecimal after 0x,
                  or text, as a quoted C string
   packet_interval_ms
                  how often the trigger is sent               triggered only

``dh_id`` and ``name`` belong to a handler and never to a group: they are what
tell one handler of a group from another. An OC address and an OC port are
given together or not at all, and a handler needs them before it can be
started.

The two kinds of payload
------------------------
A payload either sends of its own accord or answers a request, and which it is
decides where its timing is written down.

**periodic** -- the payload sends and tcspecial reads. Nothing here says when:
how fast a simulated one produces data is a property of the simulation, so the
intervals are in the simulator file. A periodic payload takes no ``trigger``
and no ``packet_interval_ms`` here.

**triggered** -- tcspecial sends a request and the payload answers. Both the
request and how often it goes out are here, because tcspecial does the sending
and what to send comes from the payload's interface document. Its entry in the
simulator file states no interval at all, packet or segment: such a payload
sends when it is asked, so a rate there would govern nothing.

The interval is ``packet_interval_ms`` in whichever file states it -- one name,
in the file the payload's kind puts it in. It was ``trigger_interval_ms`` here;
a file still using that name is told the name it has now.

**The trigger itself** is bytes, not text, since an interface document asks
for what it asks for. A series of bytes is written in hexadecimal after
``0x``, two digits to a byte, in either case, with spaces or underscores
allowed between bytes -- and **without quotes**, a series of bytes being no
more a string than a port number is::

   trigger: 0x55AA
   trigger: 0x55 AA 0F
   trigger: 0x0D0A

A trigger that is text is written as a string in C's notation, in quotes,
which is what its escapes need -- ``\\`` ``\"`` ``\'`` ``\n`` ``\r``
``\t`` ``\0`` ``\a`` ``\b`` ``\f`` ``\v`` and ``\xNN``::

   trigger: "READ\r"

Which of the two a value is in follows from its first two characters and
nothing else: ``52 45`` is a plausible pair of bytes and a plausible pair of
digits, so the ``0x`` is what says which. The quotes themselves are a
convention for writing the file and not a rule the programs enforce -- every
format hands the same string over whichever way it was written, so a quoted
``"0x55AA"`` is read as the same two bytes.

An escape C does not have is refused rather than passed through, and a trigger
of no bytes at all is refused: a payload that answers requests has to be asked
something. Writing the escapes yourself is also how an XML payload file states
a trigger that a YAML or JSON one states with the format's own escapes.

.. code-block:: yaml

   payloads:
     # Sends on its own: nothing here about timing.
     - dh_id: 0
       name: DH0
       oc_address: 127.0.0.1
       oc_port: 6000
       type: network
       protocol: tcp
       address: localhost
       port: 5000
       packet_size: 12

     # Answers a request: both halves of the request are here, and the rate
     # is the rate the trigger goes out at.
     - dh_id: 1
       name: DH1
       oc_address: 127.0.0.1
       oc_port: 6001
       type: network
       protocol: tcp
       address: localhost
       port: 5001
       packet_size: 12
       mode: triggered
       trigger: "READ\r"        # or 0x52 45 41 44 0D, the same bytes
       packet_interval_ms: 500

A file that mixes the two is refused rather than run, in either direction: a
periodic payload carrying a trigger or an interval, or a triggered one with no
interval, has not said which kind of payload it describes, and either guess
would operate it in a way nobody asked for.

A datagram payload -- ``udp`` or ``unix_dgram`` -- cannot be triggered. Its
handler learns where to send from the payload's first packet, so there is
nowhere to send a request until the payload has spoken, which a triggered
payload does not do.

Tcspecial counts the triggers it sends apart from the data it carries, which
``QUERY_DH`` reports: a handler whose triggers climb while nothing arrives is
one whose payload is not answering.

Only ``network`` and ``device`` handlers can be described this way. A serial
line, an I\ :superscript:`2`\ C bus or a SPI peripheral has terms this format
has nowhere to put, and is described in an endpoint configuration file
instead.

**What the file will not let pass**

These mistakes are refused rather than ignored, because each of them
otherwise produces a payload that looks configured and is not:

* A **repeated name or ``dh_id``** within the file. Both pick out one
  payload: tcspecial keeps its handlers by id, so a repeated id has one
  silently replace the other, and a simulator configuration is joined to this
  file by name, so a repeated name has one entry drive two payloads. The rule
  reaches no further than the file -- two payload sets may name their payloads
  and their groups alike, and the shipped sets do.
* An **attribute of another kind** -- a device payload stating ``protocol``,
  ``address`` or ``port``, or a network payload stating ``path``. The check is
  made after the group has been laid under the payload, since an attribute
  inherited from a group reaches the payload just as one it states itself
  does: a payload of one kind joining a group written for another is what this
  catches, and the error names the group when the group is where the attribute
  came from.
* A **word that is not an attribute**. A misspelled ``oc_address`` is a
  payload with nowhere to send its data, and ignoring the word says nothing
  about it.
* A **misspelled section**, for the same reason: ``payload`` is not
  ``payloads``, and a file whose only section is misspelled describes no
  payloads at all.
* A **packet interval**. How fast a payload sends is a property of the
  simulation, so the interval belongs in the simulator configuration file and
  the error says so. This was ignored for a while, which was the same silence
  the two modes exist to prevent -- a file stating an interval has said
  something about the payload's timing, and reading it as though it had not is
  reading a different file than the one that was written. An interval a
  payload takes from its group is refused as one it states itself is.
* **Two payloads wanting the same thing**. Two at one address cannot both be
  started; two on one device file, serial line or SPI peripheral is worse,
  each taking part of the one stream and reporting it as though it were the
  whole. A payload's own address and its ``oc_address`` are compared together,
  since a port is a port whatever claims it, and ``localhost``, ``127.0.0.1``
  and ``::1`` count as one host. A Unix socket collides by its path, its port
  meaning nothing, and so does a device file opened twice. The exception is a
  bus: several devices on one I\ :superscript:`2`\ C bus is what a bus is for,
  so what is claimed there is the bus and the address together.

The same collision rule applies to an endpoint configuration file, which is
the only one that can describe a serial line, a bus or a peripheral at all.

This is the shipped ``payload1.yaml``. DH1 and DH3 are reached the same way,
so what they share is a group; DH0 and DH2 share nothing and state everything
for themselves.

.. code-block:: yaml

   version: "1.0"
   description: TCSpecial payload set 1

   payload_groups:
     - name: udp_localhost
       type: network
       protocol: udp
       address: localhost

   payloads:
     - dh_id: 0
       name: DH0
       oc_address: 127.0.0.1
       oc_port: 6000
       type: network
       protocol: tcp
       address: localhost
       port: 5000
       packet_size: 12

     - dh_id: 1
       name: DH1
       oc_address: 127.0.0.1
       oc_port: 6001
       group: udp_localhost
       port: 5001
       packet_size: 11

     - dh_id: 2
       name: DH2
       oc_address: 127.0.0.1
       oc_port: 6002
       type: device
       path: /dev/urandom
       packet_size: 1

     - dh_id: 3
       name: DH3
       oc_address: 127.0.0.1
       oc_port: 6003
       group: udp_localhost
       port: 5003
       packet_size: 15

Starting and stopping a payload
===============================
Each panel has one button for it, labelled with what pressing it will do. In
tcssim a payload that is sending offers ``Silent`` and one that is not offers
``Transmit``; in tcsmoc, where the same two things are a handler carrying data
to the ground or dropping it on the spacecraft, they are ``Discard`` and
``Transmit``. Each panel also shows its own status beside the name, which is
where the label comes from.

There were two buttons in each, Start and Stop, and either was pressable
whatever the payload was doing: a Stop on something that was not sending did
nothing and read as though it had. A payload that fails to start offers to
start again, since that is what has not happened yet.

Seeing what was read
====================
Each payload's panel, in both programs, has a ``Configuration`` button. It shows
every parameter read from the two configuration files for that payload: what
the payload file said about the payload and how it is reached, and what the
simulator file said about how a stand-in for it behaves, each under the name
of the file it came from.

It shows nothing but what the files said. What a payload has actually done is
on the panel itself, and only the attributes of that payload's own kind appear
-- a device has no protocol and a bus no port -- along with only the faults
that were asked for. Where a file stated nothing, the line says what the
program will use instead rather than passing it off as a statement: a handler
with no OC address is shown as one that cannot be started.

In tcsmoc the simulator half is shown when that file can be read. The MOC
simulates nothing, so a simulator file that is missing or written for a
different payload file costs it that half and nothing else; the panel says so
in place of the settings.

Simulator Configuration Files
=============================
What simulating a payload takes, which is not a property of the payload: how
often it produces a packet, and how it divides one into segments. Only tcssim
acts on it; tcsmoc reads it too, to show what the tcssim it started is
simulating, and does without it if it is not there.

.. code-block:: text

   simulated_payload_groups        optional
     name                 the name payloads refer to
     ...                  any of the settings below

   simulated_payloads
     name                 the payload this one stands in for
     type                 the kind of payload it is, as the payload file says
     protocol             which transport, for a network payload
     payload_address      the socket the payload answers from, if this file
                          cares to say
     payload_port         which port it answers from, likewise
     group                the group to take unstated settings from, if any
     ...                  any of the settings below

**What it stands in for**

Two of the attributes describe the payload rather than the simulation, and are
required for that reason. The two files are joined by handler name and by
nothing else -- and a name is what a payload set copied from another one keeps
-- so a simulator file written for one set parses happily against another
set's payload file, and the run that follows looks like a payload that never
sends. Stating the kind here makes that pairing an error instead.

.. code-block:: text

   type                   the kind of payload this stands in for, in the
                          payload file's own word: network, device, serial,
                          i2c, or spi. Must be what the payload file says
   protocol               which transport, for a network payload, and refused
                          for any other kind. Must be what the payload file
                          says
   payload_address        the socket the simulated payload answers from: a
                          host for a UDP payload, a path for a Unix datagram
                          one. Optional, and only for those two kinds
   payload_port           the port it answers from. Optional, and refused for
                          a Unix datagram payload, which answers from a path

Where the payload *is* is not stated here at all: the simulator takes it from
the payload file, which decides it. Writing ``address`` or ``port`` here is
refused with the file they belong in -- they are the likeliest thing to be
carried across from the payload file beside this one.

What this file may say about an address is the other end of the same link: the
socket the simulated payload answers from.

.. code-block:: yaml

   simulated_payloads:
     - name: DH0
       type: network
       protocol: udp
       payload_address: 127.0.0.1   # the socket this payload answers from,
       payload_port: 7000           # not the one the handler waits at
       packet_interval_ms: 1000

Nothing in the link needs it -- a handler learns where its payload is from the
first packet it gets -- but something outside the simulation may: a rule on a
firewall, or a capture being read afterwards. Only a datagram payload has an
address of its own to bind; a stream payload is the end that waits, so it
binds the address the payload file gives it. A Unix datagram payload answers
from a path, so it takes an address and no port. Neither end may be the other:
a payload told to bind its handler's own socket is refused rather than left to
fail at the bind. Stating neither gets what every simulator file asked for
before this existed -- any interface and a port the system chooses.

A word that is not a setting is refused here too -- in a payload, in a group,
and in the file's own sections. A misspelled setting that was ignored would
have the simulator do something other than what the file asked for and say
nothing: a payload asked through a misspelling to drop a tenth of its packets
drops none, and looks exactly like one that was never asked.

Neither is taken from this file: the payload file decides both, and a
disagreement about either is an error naming the payload and what each file
says of it. A group may carry them, like any other setting -- though payloads
that share a rate need not share a transport, which is why the shipped
``payload1sim.yaml`` leaves them with the payloads.

**Settings**

.. code-block:: text

   packet_interval_ms     milliseconds between packets; 0 is as fast as the
                          payload can be driven. Periodic payloads only: a
                          triggered one sends when it is asked, and stating
                          an interval for one is an error
   segment_interval_ms    milliseconds between the segments of one packet;
                          defaults to the packet interval
   segment_size           bytes in one segment; defaults to the whole packet,
                          whose size the payload file gives

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
payload that sends on its own. Faults belong in this file alone -- a payload
configuration file describes what a payload is meant to do, and tcspecial reads
it. A payload that starts with any fault set says so in one line of log, so a
run is not left to be guessed at afterwards.

.. code-block:: yaml

   simulated_payloads:
     - name: DH0
       type: network
       protocol: udp
       packet_interval_ms: 500
       drop_percent: 10        # one packet in ten never arrives
       jitter_ms: 50           # and the rest are up to 50ms late

The two files are joined by name. Every data handler of the payload file needs
a simulated payload naming it, and a name here that no handler has is an error
rather than a line with no effect, because a typo is otherwise
indistinguishable from a payload meant to be left out.

A simulated triggered payload sends one packet for each request it is sent and
nothing otherwise. The segment settings still apply to it -- how a payload
divides an answer is its own business however it was prompted -- but the packet
interval does not, and is refused. Two kinds cannot be asked at all and are
refused outright: a device, which tcspecial opens and reads directly, and a
device on a bus, where the master's own read is the request and a second master
cannot see it.

This is the shipped ``payload1sim.yaml``.

.. code-block:: yaml

   version: "1.0"
   description: Simulator settings for the payloads of payload1.yaml

   simulated_payload_groups:
     - name: steady_1hz
       packet_interval_ms: 1000

     - name: continuous
       packet_interval_ms: 0

   simulated_payloads:
     - name: DH0
       type: network
       protocol: tcp
       group: steady_1hz

     - name: DH1
       type: network
       protocol: udp
       group: steady_1hz

     - name: DH2
       type: device
       group: continuous

     - name: DH3
       type: network
       protocol: udp
       packet_interval_ms: 500

Endpoint Configuration Files
============================
A group carries every attribute of its kind except the one that locates an
endpoint; the endpoint carries that, because it is what tells one endpoint of
a group from another.

Underscores and hyphens are both accepted in every attribute name, so
``stop_bits`` and ``stop-bits`` are the same attribute. In XML they are
written as attributes of the element.

.. code-block:: text

   general
     version      "1.0"
     description  free text

   endpoint_groups
     name         the name endpoints refer to
     type         serial | network | i2c | spi
     packet_size  bytes in one packet
     ...          the attributes of that kind, below

   endpoints
     name         the endpoint's name, which becomes a handler's name
     group        the group it belongs to
     dh_id        the handler number it becomes, if it is to become one
     ...          what locates it, below
     oc_address   as in a payload configuration
     oc_port

**What locates an endpoint**

.. code-block:: text

   device         a path: a serial port, a SPI device node, a plain device,
                  or a Unix-domain socket. Also spelled path
   address, port  a host and a port, for a network endpoint
   device,        a bus and the address of a device on it, for I2C
   address

**A serial group**

.. code-block:: text

   datarate       bits per second
   stop_bits      1 | 1.5 | 2
   byte_length    5 | 6 | 7 | 8
   stream         where a read ends; see below

There is no parity. The format does not carry one, and a line is set to none.

**A network group**

.. code-block:: text

   protocol       tcp | udp | unix_stream | unix_dgram
   stream         required for tcp and unix_stream, refused for the datagram
                  protocols, where the datagram is already the frame

**An I**\ :superscript:`2`\ **C group**

.. code-block:: text

   ten_bit        true | false, ten address bits rather than seven
   pec            true | false, an SMBus packet error check on each transfer
   retries        how many times the master tries a transfer
   timeout        a whole number with a unit: 50ms, 2s. Rounded up to a
                  multiple of 10 ms by the kernel
   bus_speed      recorded, not applied: the bus clock belongs to the
                  controller and is set by the platform

An endpoint of such a group gives both the bus device and the address on it,
because two endpoints of one group commonly sit on the same bus and differ
only in which device the master addresses. With seven-bit addressing the
usable addresses are 0x08 through 0x77: the specification reserves 0x00
through 0x07 and 0x78 through 0x7F, so no device can answer to one of those.
With ten-bit addressing the whole range 0x000 through 0x3FF is usable.

There is no parity or byte length. I\ :superscript:`2`\ C fixes the framing at
eight data bits, most significant first, followed by an acknowledge bit.

**A SPI group**

.. code-block:: text

   max_speed      the greatest clock rate the peripheral accepts, in Hz
   mode           0 | 1 | 2 | 3, the clock polarity and phase
   bits_per_word  commonly 8, though a controller may support other widths
   bit_order      msb | lsb
   cs_active      low | high

The mode must match the peripheral; a mismatch fails the way a wrong data rate
fails on a serial line. An endpoint of a SPI group gives the device node
alone, which names the bus and the chip select together.

There is no parity and there are no stop bits. SPI is clocked and full duplex,
and a transfer is delimited by the chip select rather than by framing bits.

**A stream section**

Where one read of a stream payload ends, for the kinds that are streams.

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

Neither bus kind takes a stream section. A master clocks exactly as many bytes
as it asks for, so a transfer is already bounded and needs no rule for where a
read ends.

A worked example of every kind at once is shipped, in all three formats, as
``tcslibgs/tests/actual/endpoints.yaml`` and its ``.json`` and ``.xml``
companions. A test parses each of them on every build, so an example there
cannot quietly stop being valid. None of its endpoints gives a ``dh_id``,
which is what makes it an endpoint configuration rather than a set of data
handlers: an endpoint becomes a handler only when it is given a number for
commands to name it by, and an OC address for the handler to reach the ground
at.

Not Yet Read
------------
These have been described for this project and are not yet accepted by any
parser. A file giving one is refused rather than quietly running without it.

.. code-block:: text

   parity                  on a serial line
   length-prefixed framing a read whose length is in its first one or two
                           bytes
   max_interbyte_interval  a read ended by a gap between bytes
   bidirectional           whether a handler carries data both ways; every
                           handler does at present

Running the Programs
====================
The Makefile names a payload set once and gives it to every program that needs
it, which is the point: tcsmoc builds its panels from the payload file and
hands that same file to the tcspecial and tcssim it starts, so a program
reading a different one would serve or simulate payloads the panels do not
describe.

``PAYLOAD_YAML`` is the payload file, which reaches every program as a command
line argument. ``PAYLOAD_SIM_YAML`` is the simulator file, which reaches
tcssim through the environment -- and tcsmoc too, which it inherits the same
way: tcsmoc acts on none of it, and reads it only so a panel can show what the
tcssim it started is simulating. Failing the variable, each looks for the file
beside the payload file and named for it. Both default to the ``payload2``
set.

Everything at once
------------------
``make runmoc`` runs tcsmoc, which starts a tcspecial and a tcssim of its own:

.. code-block:: console

   $ make runmoc
   $ make runmoc PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml
   $ make runmoc PAYLOAD_YAML=payload2.yaml PAYLOAD_SIM_YAML=payload2sim.yaml

Both files are named because tcsmoc passes the payload file on to its children
and never reads the simulator file, which reaches tcssim by being in tcsmoc's
environment. Naming only one of a pair is how the two files come to describe
different sets, which is reported rather than run: every data handler of the
payload file must have a simulated payload naming it.

If a tcspecial is already running, tcsmoc attaches to it instead of starting a
second -- it asks by PING -- and leaves it running when the window is closed.
Starting a second never worked: the command interpreter's address can be bound
once.

One program at a time
---------------------
Useful when a program is to be run under a debugger, or restarted without
disturbing the others, or watched with its own logging.

.. code-block:: console

   $ make run PAYLOAD_YAML=payload1.yaml
   $ make runsim PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml
   $ make runmoc PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml

``make run`` takes only the payload file: tcspecial does not simulate
anything. ``make runsim`` takes both, because tcssim run on its own needs the
payload file as well as the simulator file -- the names in the two must match.
Started in that order, tcsmoc finds the tcspecial already running and attaches
to it.

What the programs take without the Makefile
-------------------------------------------
.. code-block:: console

   $ cargo run --bin tcspecial -- payload1.yaml
   $ PAYLOAD_SIM_YAML=payload1sim.yaml cargo run --bin tcssim -- payload1.yaml
   $ PAYLOAD_SIM_YAML=payload1sim.yaml cargo run --bin tcsmoc -- payload1.yaml

An argument naming the payload file beats any environment variable naming one.
Failing an argument, tcspecial reads ``PAYLOAD_CONFIG_PATH`` and tcssim reads
``SIM_PAYLOAD_CONFIG_PATH``; failing that, every program reads
``payload1.yaml``. Tcsmoc has no variable of its own, because the programs it
starts inherit its environment: a variable naming tcsmoc's file would name
theirs as well.

``RUST_LOG`` sets the logging, which the Makefile leaves at ``info``.

.. code-block:: console

   $ make runmoc RUST_LOG=debug
   $ RUST_LOG=tcspecial::ci=trace cargo run --bin tcspecial -- payload1.yaml

Testing
=======
Testing is done with two programs:

* tcsmoc - Simulates an operations center/mission control application. It
  communicates to TCSpecial via UDP datagrams.

* tcssim - Simulates payloads using various types of communication protocols
  and errors.

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
  The indicator, whose colours are listed under `Beaconing`_, and the time the
  last beacon arrived.

the ``Command`` menu
  The commands below.

``Last Response``
  What the last command answered, under the two boxes.

Below that is one panel per data handler, and a ``Quit`` button at the foot of
the window, which stops the tcspecial and tcssim the MOC started.

Each panel shows its handler's name and status, how it is reached, and its
packet size; then the time and first few bytes of the last transfer each way,
and the byte counts. The two lines come from the spacecraft, so their times are
when the data moved rather than when it was asked about. The panels refresh
themselves about once a second; nothing has to be pressed to see traffic.

Each panel has two buttons. One starts and stops the handler, and is described
under `Starting and stopping a payload`_. The other, ``Configuration``, shows
what the files said about that handler: see `Seeing what was read`_.

Commands
^^^^^^^^
From the ``Command`` menu. Three of the seven items send a command:

Ping
  Sends a PING and waits for the answer. ``Last Response`` says that one was
  sent, then what came back, and if nothing came back within the client's
  timeout it says when it stopped waiting.

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
TCSpecial sends a beacon at a configurable interval. Tcsmoc displays the
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

tcssim
------

The tcssim window
^^^^^^^^^^^^^^^^^
One panel per simulated payload, in a grid, and a ``Quit All`` button at the
foot of the window. Nothing in the window names a payload or fixes how many
there are: the two configuration files decide that, so a payload added to or
removed from them adds or removes a panel.

Each panel shows, from the top:

the name and status
  The status is red while the payload is not sending and black while it is.

how the payload is reached
  The same description tcsmoc's panels carry for the handler at the other end
  of the link.

``Pkt`` and ``Seg``
  The packet size, from the payload file, and the segment size, from the
  simulator file.

``Pkt Int`` and ``Seg Int``
  The interval between packets and between the segments of one packet, both
  from the simulator file.

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
file states a segment larger than the payload file's packet says so before
anything is touched.

Below them are the two buttons -- one to start and stop the payload, described
under `Starting and stopping a payload`_, and ``Configuration``, described
under `Seeing what was read`_ -- and then three lines:

``Sent``
  When the last packet went, and the first few bytes of it.

``Recv``
  When the last packet arrived, and the first few bytes of it. A payload
  nothing speaks to shows ``--:--:--`` here for the whole run, which is what a
  payload that sends on its own looks like.

``Pkts``
  How many packets have gone and arrived since the payload started.

The times and bytes are the simulator's own account of what it did, where
tcsmoc's panels show the same transfers as the spacecraft reported them. Two
panels disagreeing about one transfer is worth knowing about, which is why
both are shown the same way.
