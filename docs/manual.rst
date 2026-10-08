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
their names: a file naming ``data_handlers`` is the first, and one naming
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

   data_handler_groups             optional
     name         the name handlers refer to
     ...          any of the attributes below

   data_handlers
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

``dh_id`` and ``name`` belong to a handler and never to a group: they are what
tell one handler of a group from another. An OC address and an OC port are
given together or not at all, and a handler needs them before it can be
started.

Only ``network`` and ``device`` handlers can be described this way. A serial
line, an I\ :superscript:`2`\ C bus or a SPI peripheral has terms this format
has nowhere to put, and is described in an endpoint configuration file
instead.

This is the shipped ``payload1.yaml``. DH1 and DH3 are reached the same way,
so what they share is a group; DH0 and DH2 share nothing and state everything
for themselves.

.. code-block:: yaml

   version: "1.0"
   description: TCSpecial payload set 1

   data_handler_groups:
     - name: udp_localhost
       type: network
       protocol: udp
       address: localhost

   data_handlers:
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

Simulator Configuration Files
=============================
What simulating a payload takes, which is not a property of the payload: how
often it produces a packet, and how it divides one into segments. Only tcssim
reads this.

.. code-block:: text

   simulated_payload_groups        optional
     name                 the name payloads refer to
     ...                  any of the settings below

   simulated_payloads
     name                 the data handler this payload stands in for
     group                the group to take unstated settings from, if any
     ...                  any of the settings below

**Settings**

.. code-block:: text

   packet_interval_ms     milliseconds between packets; 0 is as fast as the
                          payload can be driven
   segment_interval_ms    milliseconds between the segments of one packet;
                          defaults to the packet interval
   segment_size           bytes in one segment; defaults to the whole packet,
                          whose size the payload file gives

The two files are joined by name. Every data handler of the payload file needs
a simulated payload naming it, and a name here that no handler has is an error
rather than a line with no effect, because a typo is otherwise
indistinguishable from a payload meant to be left out.

This is the shipped ``payload1sim.yaml``.

.. code-block:: yaml

   version: "1.0"
   description: Simulator settings for the payloads of payload1.yaml

   simulated_payload_groups:
     - name: steady_1hz
       packet_interval_ms: 1000
       segment_interval_ms: 1000

     - name: continuous
       packet_interval_ms: 0
       segment_interval_ms: 0

   simulated_payloads:
     - name: DH0
       group: steady_1hz

     - name: DH1
       group: steady_1hz

     - name: DH2
       group: continuous

     - name: DH3
       packet_interval_ms: 500
       segment_interval_ms: 500

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
line argument. ``PAYLOAD_SIM_YAML`` is the simulator file, which only tcssim
reads and which reaches it through the environment. Both default to the
``payload2`` set.

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

Commands
^^^^^^^^
Tcsmoc supports the following commands:

PING
  Send a PING message and wait for receipt.

PING
""""
Click on the Command menu, then click on the Ping menu item. The status
box will indicate a PING message has been sent. When received, the status
box will change to indicate a response has been received. If a response
wasn't received within the command timeout window, the status box will
indicate the time it stopped waiting.

Beaconing
^^^^^^^^^
TCSpecial sends a beacon at a configurable interval. Tcmoc displays the
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
