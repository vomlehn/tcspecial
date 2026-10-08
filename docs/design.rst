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

Terminology
===========

These words are used in one sense each, here and in the code. Several of them
had been used in two, which is why they are written down: "device" in
particular had been both a kind of endpoint and a protocol beside TCP and UDP,
and "endpoint" had been both one end of a data handler's path and an entry in a
configuration file.

endpoint
    One end of the path a data handler's data travels. A handler has two: the
    OC endpoint, where the ground's data arrives and leaves, and the payload
    endpoint, where the payload's does. An endpoint is of exactly one kind, and
    the kind decides what is needed to open it and what terms it is operated
    on.

endpoint kind
    What an endpoint is made of, and so what locates it and what else must be
    said about it. There are five: ``network``, ``device``, ``serial``,
    ``i2c`` and ``spi``. A kind is not a protocol -- a network endpoint carries
    the protocol it runs over, and no other kind is addressed that way.

device
    An endpoint that is a device file, opened and read as it comes. Not the
    general word for hardware: a serial line, an I2C device and a SPI
    peripheral are all reached through device files and are none of them a
    device endpoint, because each has terms of its own that a plain device has
    not. A device *file* is the thing in the filesystem; a device *endpoint* is
    one of the five kinds.

bus
    A device that several devices are reached through, each by an address on
    it. I2C is the one in use. The bus is named by a device file and the device
    on it by an address, which is why an endpoint on a bus carries both.

payload
    What is at the far end of a payload endpoint: the instrument, recorder or
    radio whose data a handler carries. Also what tcssim stands in for, and
    what a payload configuration file describes.

data handler
    The pair of conduits between one payload endpoint and one OC endpoint,
    named by a ``dh_id`` and a name. "Handler" alone always means this.

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

payload configuration file
    The file describing payloads: which data handlers exist, how tcspecial
    reaches each one, and how large its packets are. ``payload1.yaml`` and the
    rest.

endpoint configuration file
    The richer file describing endpoints in groups, which can state every term
    of every kind -- a serial line's framing, a bus's addressing, a SPI
    peripheral's clock. Its endpoints become data handlers.

simulator configuration file
    The file describing how a simulated payload behaves: how often it produces
    a packet and how it divides one into segments. Properties of a simulation
    rather than of a payload, which is why they are not in the payload file.

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

* There are two libraries and two YAML files with payload configuration
  information:

    * tcslib: ground software library providing simple integration with mission control software

    * tcslibgs: sofware library containing command, telemetry, and any other definitions shared between tcspecial and tcslib

    * payload1.yaml: Configuration information for the payloads tcspecial
      serves, tcsmoc controls, and tcssim simulates

    * payload1sim.yaml: What simulating those payloads takes, which is not part
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
   |  |  payload1.yaml  |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    |  payload1.yaml  |  |     |                 |
                              :    |    +-----------------+  |     |                 |
                              :    +-------------------------+     +-----------------+

On the ground, the tcslib library is used by the mission control software (such as YAMCS
or MCT) to issue commands and receive telemetry. These are transmitted over
what is shown as a single communications link, though these could be using
multiple frequencies or channels.

Commands sent through tcslib go to the tcscmd process, directed as appropriate to
either the command interpreter or the the
various data handlers. This is shown as a multiplexed link, such as something
using an IP address for the command interpretter and each of the
data handlers, the but other options
can be easily implemented.

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
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | timestamp  | Timestamp | Spacecraft time when response was  |
|                |            |           | sent                               |
+----------------+------------+-----------+------------------------------------+

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
|                | dh_id      | DHId      | Identifer to assign to the new     |
|                +------------+-----------+------------------------------------+
|                | type       | DHType    | Type of DH, network or device      |
|                +------------+-----------+------------------------------------+
|                | name       | DHName    | If the DH is a network interface   |
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
produce a successful value. This, in turn, means the dh_id must not be reused.

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| STOP_DH_CM     | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | dh_id      | DHId      |                                    |
+----------------+------------+-----------+------------------------------------+
| STOP_DH_TM     | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | None                                                        |
+----------------+------------+-----------+------------------------------------+

QUERY_DH
^^^^^^^^
Return statistics from the indicated data handler.

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

+----------------+-------------------------------------------------------------+
| Name           | Parameters                                                  |
+================+============+===========+====================================+
| QUERY_DH_CM    | Name       | Type      | Description                        |
|                +------------+-----------+------------------------------------+
|                | dh_id      | DHId      | Value that must match the RESTART  |
|                |            |           | command key                        |
+----------------+------------+-----------+------------------------------------+
| QUERY_DH_TM    | Parameters                                                  |
|                +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
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
|                | dh_id      | DHId      | Data handler to ask about          |
+----------------+------------+-----------+------------------------------------+
| QUERY_DH_      | Parameters                                                  |
| SAMPLE_TM      +------------+-----------+------------------------------------+
|                | Name       | Type      |  Description                       |
|                +------------+-----------+------------------------------------+
|                | dh_id      | DHId      | Data handler this answers for      |
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
|                | timestamp  | Timestamp | Spacecraft time at which the       |
|                |            |           | beacon message was sent            |
+----------------+------------+-----------+------------------------------------+

TCSpecial
=========

tcspecial
---------
Tcspecial reads two configuration files. Its own is
``tcspecial/src/tcspecial.yaml``, named by ``TCSPECIAL_CONFIG_PATH`` when that
is set, and holds the address the command interpreter listens on, the beacon
interval, and where the telemetry log goes. The other describes the payloads
and is named on the command line; see `Payload Configuration Files`_.

Both choose their parser from their extension, as every configuration file in
the project does, so either may be written in YAML, JSON or XML. The shipped
one is YAML because that is what the rest of the project's configuration is
written in; it was JSON when JSON was the only format the project read.

This is the shipped ``tcspecial/src/tcspecial.yaml``:

.. code-block:: yaml

   address: "0.0.0.0"
   port: 4000
   protocol: udp
   beacon_interval_ms: 5000
   log_dir: telemetry
   log_segment_bytes: 65536

The address and port are where the command interpreter binds, which is where
the ground sends commands. The quotes on the address are deliberate: unquoted,
``0.0.0.0`` is a string in YAML only because it happens not to be a number,
which is a thin reason to rely on. ``log_dir`` holds the telemetry log's
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
    A START_DH naming a kind that is not the handler's kind is refused with
    ``InvalidParameter``, and starts nothing. The ground and the spacecraft
    read the same configuration file, so a mismatch is not a handler to be
    started differently: it is the two ends disagreeing about what that file
    says, or a command made by hand. Starting it anyway would open the
    endpoint the configuration describes while the ground believed it had
    started something else.

The kind a START_DH carries is worked out from the endpoint by one mapping,
``EndpointConfig::kind``, which both ends use: the ground to say what it is
asking for and the spacecraft to check what was asked. Two copies of it would
be two chances for the ground to ask for something the spacecraft refuses. The
check is made before the handler's state is looked at, so a mis-named START_DH
is refused whether or not the handler happens to be running.

Requirement
    An endpoint is of one of five kinds: ``network``, ``device``, ``serial``,
    ``i2c`` or ``spi``. The kind decides what locates the endpoint and what
    terms it is opened on, and a handler carries both: a serial line's framing,
    the address of a device on a bus, and a SPI peripheral's clock are as much
    a part of where the payload is as the name of the file.

A serial line, an I2C device and a SPI peripheral are all reached through
device files, and all three used to become device endpoints on the way to
being started -- which opened the right file and then talked to it on whatever
terms it had been left on: a line at the wrong rate, a peripheral in the wrong
mode. An I2C endpoint could not be started at all, there being nowhere in a
handler's configuration to put the address of a device on a bus. Each is its
own kind now, and tcspecial has an endpoint implementation for each.

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
would remove that, at the cost of stating in every payload file where the
ground is.

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

In order for tcscmd to notify a given endpoint that it has some action
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
Four of the five endpoint kinds are opened through an entry in ``/dev``. Which
kind an entry is depends on what is at the end of it, and not on the entry's
name: the name says where to look, and the kind says what else must be said
about it and which implementation opens it.

**Endpoints Opened Through Device Files**

+----------------+-------------------------------+--------------------------+
| Name           | What is there                 | Endpoint kind            |
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
| /dev/urandom   | A character device read as it | ``device``: nothing but  |
|                | comes                         | the path is needed       |
+----------------+-------------------------------+--------------------------+

A ``device`` endpoint is the last of these and only that: a file opened and
read as it comes. The other three are device files too, and were device
endpoints until each was given a kind of its own, which is what let their terms
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

payload1.yaml
-------------
This is a YAML file that defines the actual payloads. It is considered part
of TCSpecial as it must be supplied, but is also used by the
test software. All three programs read it: tcspecial serves the payloads it
describes, tcsmoc controls them, and tcssim simulates them, so a handler
changed in it changes for all three at once. Its format is given under
`Payload Configuration Files`_.

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

Payload Configuration Files
===========================
A payload configuration file describes the payloads themselves: which data
handlers exist, how tcspecial reaches each one, and how big its packets are.
What it deliberately does not describe is how a simulated payload behaves; see
`Simulator Configuration Files`_ for that. The parser is ``tcslibgs::config``.

Two sets are shipped. ``payload1.yaml`` describes four handlers of three
kinds, which is what exercises the panels and the grid; ``payload2.yaml``
describes one, which is what a single link is debugged against. Each has a
simulator configuration beside it, named for it -- ``payload1sim.yaml``,
``payload2sim.yaml`` -- and a set added by that convention needs no change
anywhere to be run. ``payload1.yaml`` is what every program falls back to when
nothing names a file; the Makefile names a set explicitly and defaults to the
second.

The format is chosen from the extension exactly as every other configuration
file's is, so a payload configuration may be written in YAML, JSON, or XML.

File Structure
--------------
A payload configuration file has a version, a description, and two sections,
one of them optional.

``payload_groups``
    Named groups of attributes. A group carries what several payloads have
    in common. Optional: a file whose payloads share nothing, or that prefers
    to spell every one of them out, has no groups.

``payloads``
    The payloads themselves. Each has an id and a name, may name a group,
    and states whatever attributes it does not take from that group.

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
| type                | string | ``network`` or ``device``                   |
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
| trigger             | string | What to send to make the payload answer.    |
|                     |        | Triggered only                              |
+---------------------+--------+---------------------------------------------+
| trigger_interval_ms | number | How often to send it. Triggered only        |
+---------------------+--------+---------------------------------------------+

A group states any of these; a data handler states any of these plus its
``dh_id``, its ``name``, and the ``group`` it takes attributes from.

The OC address is where a handler is reached from the ground, and is a
different thing from the address at which it reaches its payload. It is
commonly shared by every handler of a group, which is why a group may carry
it, while the port is what tells one handler of a group from another.

Requirement
    A data handler states both an OC address and an OC port, or neither. Half
    of an address reaches nothing, and nothing in the file says which half was
    meant.

Requirement
    A payload configuration need not give any handler an OC address. Only
    starting a handler needs one, so a file describing payloads is complete
    without it and a handler without one is refused at the point it would be
    started.

How a Payload Is Made to Send
-----------------------------
Payloads are generally one of two kinds, and which kind a payload is decides
where the timing of it is written down.

Requirement
    A payload is periodic or triggered, and says which. A file that says
    neither describes a periodic payload, which is both the commoner kind and
    what every payload file written before there was a mode describes.

Requirement
    A periodic payload sends of its own accord and tcspecial reads what
    arrives. It states no trigger and no trigger interval: there is nothing to
    send it, and how fast a simulated one produces data is a property of the
    simulation, stated in the simulator configuration file.

Requirement
    A triggered payload answers a request. It states the trigger to send and
    how often to send it, both of which are flight behaviour: tcspecial does
    the sending, and what to send comes from the payload's interface document.
    Its simulator configuration states no interval at all.

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

Requirement
    A trigger's bytes go out as the file wrote them. A trigger ending in a
    carriage return is written with one, in double quotes, where YAML and
    JSON read ``\r`` and ``\n`` as the characters they name.

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

The format has no packet interval. How fast a payload produces packets is a
property of a simulation rather than of the payload, and belongs to a
simulator configuration file.

Requirement
    A payload configuration file states no packet interval, and one stated
    here is an error naming the file it belongs in. How fast a payload sends
    is a property of the simulation, so a payload file that states an interval
    is describing a simulation. It was ignored for a while, which was the same
    silence the mode rules exist to prevent: a file that states an interval
    has said something about the payload's timing, and reading it as though it
    had not is reading a different file than the one that was written. An
    interval a payload takes from its group is refused as one it states
    itself is.

Requirement
    A ``dh_id`` and a ``name`` belong to a data handler and never to a group.
    They are what tell one handler of a group from another.

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
    A group name is defined once. Two groups sharing a name is an error rather
    than one silently shadowing the other.

Requirement
    A payload name and a ``dh_id`` are each defined once. Both pick out one
    payload, and a repeat of either parses cleanly and then loses a payload:
    tcspecial keeps its handlers by id, so a repeated id has one silently
    replace the other, and a simulator configuration is joined to this file by
    name, so a repeated name has one entry drive two payloads. Reported in the
    words the endpoint configuration format uses for the same rule, which has
    always had it.

Requirement
    An attribute of another kind of payload is an error. A device payload
    stating a protocol, an address or a port, or a network payload stating a
    path, has written something nothing will read -- and a payload that looks
    configured and is not is worse than one that is refused. The rule is the
    one the endpoint configuration format applies to its own groups.

Requirement
    The check is made after a group has been laid under a payload, because an
    attribute inherited from a group reaches the payload exactly as one it
    states itself does. A payload of one kind joining a group written for
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
    kind of claim for that reason, whichever kind of handler opens it.

Requirement
    A bus is shared and a place on it is not. Several devices on one
    I\ :superscript:`2`\ C bus is what a bus is for, so a claim is on the bus
    and the address together rather than on the bus device.

Requirement
    The rule is the same for both configuration formats and is applied in one
    place. The hazard belongs to the handlers rather than to the words that
    described them, and the endpoint configuration format is the only one that
    can describe the kinds where it matters most.

Requirement
    A group is named by a data handler. A group no handler names is an error
    rather than a section with no effect, because that is also what a group
    whose name a handler misspelled looks like.

A misspelled group name leaves the group unnamed and the name undefined at
once. The handler's end is where the misspelling actually is, so that is the
error reported.

Example
-------
This is the shipped payload1.yaml. DH1 and DH3 are reached the same way, over a
UDP socket on this host, so what they share is a group and what tells them
apart stays with each of them. DH0 and DH2 share nothing with anything and so
state every attribute for themselves.

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

Every handler here shares an OC address and has a port of its own, numbered to
match its ``dh_id``. The OC address could have been a group attribute for that
reason; it is written out per handler so that the file reads as the four
handlers it describes rather than as three attributes and an exception.

Simulator Configuration Files
=============================
A payload configuration file describes payloads. Simulating one takes more than
that description holds, and the extra is not payload configuration: how often a
payload produces a packet, and how it divides a packet into segments, are
choices about a simulation. They are stated in a separate file, which only
tcssim acts on. The shipped pair is payload1.yaml and payload1sim.yaml; the
parser is ``tcslibgs::sim_config``.

Requirement
    The parser lives with the other three configuration languages rather than
    inside tcssim. Both GUIs read this file now -- one to simulate payloads
    and one to show what a payload set says about them -- and a configuration
    language read by two programs cannot live inside one of them.

The format is chosen from the extension exactly as every other configuration
file's is, so a simulator configuration may be written in YAML, JSON, or XML.

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
|                     |        | packet, whose size the payload file gives   |
+---------------------+--------+---------------------------------------------+

**What it stands in for.** Neither of these is a setting of the simulation.
The payload configuration decides both, and they are stated here again only so
that the two files can be checked against each other.

+---------------------+--------+---------------------------------------------+
| Attribute           | Type   | Meaning                                     |
+=====================+========+=============================================+
| type                | string | The kind of payload this stands in for, in  |
|                     |        | the payload file's own word: ``network``,   |
|                     |        | ``device``, ``serial``, ``i2c``, or ``spi`` |
+---------------------+--------+---------------------------------------------+
| protocol            | string | Which transport, for a network payload.     |
|                     |        | Refused for every other kind                |
+---------------------+--------+---------------------------------------------+
| sim_address         | string | Where the payload is: the host, or the      |
|                     |        | socket's path for a Unix socket. Optional   |
+---------------------+--------+---------------------------------------------+
| sim_port            | number | Which port the payload is at. Optional, and |
|                     |        | refused for a Unix socket                   |
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
    understood which kind of payload it is simulating. The segment settings
    still apply: how a payload divides an answer is its own business whichever
    way it was prompted.

Requirement
    A simulated triggered payload sends one packet for each request it is
    sent, and nothing otherwise. A payload that answered on a clock as well
    would be two kinds at once, and no file could say which rate was which.

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
against the other's payload file, and the run that follows looks like a
payload that never sends. So each simulated payload says what it stands in
for, and the statement is compared rather than used.

Requirement
    A simulated payload states the kind of payload it stands in for, and a
    network payload the transport as well. Neither is taken from this file:
    the payload configuration decides both, and a payload that stated neither
    could not be checked against anything.

Requirement
    The two files cannot disagree about a payload's kind or its transport. A
    disagreement is an error naming the payload and what each file says of it,
    rather than one file overriding the other: both describe the one payload,
    so a disagreement means one of them is not the file that was meant.

Requirement
    A transport is stated by a network payload and refused for every other
    kind. A line, a bus or a peripheral is not reached by a transport, so one
    stated for it would govern nothing.

Requirement
    The kind and the transport are written in the same words the payload
    configuration uses, and the error that refuses a word that is neither
    lists the words that would have worked. The spellings are one table read
    from both files, so the two cannot come to disagree about what a kind is
    called.

Requirement
    Where the payload is may be stated as ``sim_address`` and ``sim_port``,
    and is then compared like the kind and the transport. Unlike them it need
    not be stated at all: an address is commonly the same in two payload sets
    that differ in every other way, so requiring it would be duplication asked
    for and nothing caught.

Requirement
    This file writes them with the ``sim_`` prefix, where the payload
    configuration writes ``address`` and ``port``, because the two files mean
    different things by stating it. The payload configuration decides where a
    payload is; this file only says where it understands the payload to be. A
    plain ``address`` here would read as though the simulator chose the
    address, which it does not.

Requirement
    ``address`` or ``port`` written here is refused with the prefixed name.
    They are the two attributes someone writing a simulator file beside a
    payload file is likeliest to carry across, so such a line is less a
    mistake than the wrong spelling of something real, and is answered as one.

Requirement
    Hosts that certainly mean this host are one host, as they are for two
    handlers claiming one port. A Unix socket's ``sim_address`` is its path
    and is compared as written, there being nothing to canonicalise.

Requirement
    A Unix socket takes no ``sim_port``. It is named by a path, and the port a
    payload file gives it means nothing there, so a port stated here could
    only be compared against a number nobody chose.

Requirement
    Neither belongs to a payload that is not reached over a network, and
    stating one for a device, a line, a bus or a peripheral is an error for
    the same reason a protocol is.

Requirement
    A group may carry the kind and the transport, like every other setting a
    payload does not state. A group whose payloads are not all reached the
    same way leaves them with the payloads: the shipped ``payload1sim.yaml``
    has two payloads sharing a rate that do not share a transport.

Fault Injection
---------------
A payload that always works exercises only the path where everything works.
The settings above have a simulated payload misbehave in the ways a real one
does, so that a handler's own counting, framing, timeouts and reconnection can
be made to happen on purpose rather than waited for.

Requirement
    Fault injection settings appear in the simulator configuration file only.
    A payload configuration file describes the payloads a mission flies, and
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
    kind; only a triggered payload is asked for anything, so
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
    A simulated payload names a data handler of the payload configuration file.
    A name matching no data handler is an error, because a line that matched
    nothing would otherwise be indistinguishable from a misspelled name.

Requirement
    Every data handler of the payload configuration file has a simulated payload
    naming it. A handler nothing names is an error, because the simulator would
    otherwise start with a payload that never produces anything.

Requirement
    A name is defined once. Two groups or two simulated payloads sharing a name
    is an error rather than one silently shadowing the other.

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
This is the shipped payload1sim.yaml, which simulates the four data handlers of
payload1.yaml. Two of them are driven alike and so share a group; the last
states its own rate instead.

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
       segment_interval_ms: 500

Endpoint Configuration Files
============================
Endpoints may be configured from a file in YAML, XML, or JSON. The three
formats describe exactly the same thing and are parsed into exactly the same
Rust types, so which one a mission uses is a matter of local preference and
tooling, never of capability. The parser is ``tcslibgs::endpoint_config``; the
examples at the end of this section are the files in
``tcslibgs/tests/actual/``, which a test parses on every build so that a
documented example cannot quietly stop being valid.

Requirement
    The format of a configuration file is chosen from its extension. ``.yaml``
    and ``.yml`` are YAML, ``.xml`` is XML, and any other extension, or none
    at all, is JSON.

The fallback to JSON is deliberate rather than arbitrary: it is the format
every configuration file in the project used before the others were accepted,
so a file that predates them, or that carries no extension, keeps being read
the way it always was.

File Structure
--------------
Every endpoint configuration file has three sections.

general
    Configuration that belongs to the file as a whole rather than to any one
    endpoint or group.

endpoint groups
    Named groups of endpoints. A group carries every attribute shared by the
    endpoints of one type.

endpoints
    The endpoints themselves. Each names the group it draws its attributes
    from, and supplies the one thing the group deliberately omits.

The division between the last two sections is the point of the format.

Requirement
    A group definition carries every attribute of its endpoint kind except the
    one that locates an endpoint: its path, its network address, or, on a bus
    that addresses its devices, the address of the device on that bus.

Requirement
    What locates an endpoint is carried by the endpoint definition.

Requirement
    An endpoint of an I2C group gives both the bus device and the address of
    the device on that bus, because two endpoints of one group commonly sit on
    the same bus and differ only in which device the master addresses. Both
    reach the data handler the endpoint becomes.

Requirement
    An endpoint of a SPI group gives the device node alone. The bus and the
    chip select are both named by it, so there is no separate address.

The reason for the split is that the device name or address is precisely what
distinguishes one endpoint of a group from another. Four RS-422 payload links
running at the same rate, with the same framing and the same stream protocol
rules, differ only in which ``/dev`` entry they open. Writing the shared
attributes once, in a group, means a change to the line rate is a change in one
place, and means two endpoints that are meant to be configured alike cannot
drift apart.

The General Section
-------------------
**General section attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| version       | No       | Version of the configuration file format         |
+---------------+----------+--------------------------------------------------+
| description   | No       | Free text describing what the file configures    |
+---------------+----------+--------------------------------------------------+

.. note::
   The general section is where settings that apply to a whole file belong.
   Only the two above are defined so far; it exists as a named section so that
   adding one later is not a change to the file structure.

The Endpoint Groups Section
---------------------------
Each group has a name and a type. The name is what endpoints refer to; the type
selects which further attributes apply.

Requirement
    Each endpoint group has a name, unique within the file.

Requirement
    Each endpoint group has a type, which is one of ``serial``, ``network``,
    ``i2c``, or ``spi``.

Requirement
    Each endpoint group has an endpoint in it. A group no endpoint names is an
    error rather than a definition with no effect, because that is also what a
    group whose name an endpoint misspelled looks like. A misspelled name
    leaves the group unused and the name undefined at once; the endpoint's end
    is where the misspelling is, so that is the error reported.

    The payload and simulator configuration formats state the same rule for
    their own groups, for the same reason.

Requirement
    An attribute that does not apply to a group's type is an error rather than
    being ignored, so that a misspelled or misplaced attribute is reported
    where it was written.

Two attributes are shared rather than belonging to one type. The packet size,
below, applies to every type. The stream payload protocol attributes have a
section of their own, and apply to the types that carry a stream rather than to
all four.

**Shared group attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| packet_size   | No       | Bytes in one packet exchanged with an endpoint   |
|               |          | of this group                                    |
+---------------+----------+--------------------------------------------------+

Requirement
    Any type of group may specify a packet size. A packet has a size whether it
    travels over a serial line, a socket, or a bus, so this is not an attribute
    of one type.

Requirement
    The packet size belongs to the group rather than to the endpoint, because
    endpoints of one group are configured alike in everything but where they
    are.

Requirement
    A packet size of zero is an error. A packet of no bytes is not a packet.

Requirement
    The packet size is optional. A file describing only how to reach a device
    need not state one; a data handler built from an endpoint does need it.

.. note::
   A stream group's ``max_length`` is a different measurement and both may be
   given. ``max_length`` bounds what a single read of the stream may deliver,
   which is a property of the framing; ``packet_size`` is how much data a
   packet carries. For a group reading a fixed number of bytes at a time the
   two commonly agree, and nothing requires them to.

Serial Groups
^^^^^^^^^^^^^
**Serial group attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| datarate      | Yes      | Line rate in bits per second                     |
+---------------+----------+--------------------------------------------------+
| stop_bits     | Yes      | Stop bits after each byte: ``1``, ``1.5``, ``2`` |
+---------------+----------+--------------------------------------------------+
| byte_length   | Yes      | Data bits in each byte, from 5 to 8              |
+---------------+----------+--------------------------------------------------+
| stream        | Yes      | Stream payload protocol attributes, below        |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet exchanged with an endpoint   |
|               |          | of this group. Shared by every type; see above   |
+---------------+----------+--------------------------------------------------+

Requirement
    A serial group has a stream section, because a serial port is a stream and
    so there is always a rule deciding where one read ends.

Network Groups
^^^^^^^^^^^^^^
**Network group attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| protocol      | Yes      | ``tcp``, ``udp``, ``unix_stream``,               |
|               |          | ``unix_dgram``                                   |
+---------------+----------+--------------------------------------------------+
| stream        | See      | Stream payload protocol attributes, below        |
|               | below    |                                                  |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet exchanged with an endpoint   |
|               |          | of this group. Shared by every type; see above   |
+---------------+----------+--------------------------------------------------+

Requirement
    A network group using a stream protocol, ``tcp`` or ``unix_stream``, has a
    stream section.

Requirement
    A network group using a datagram protocol, ``udp`` or ``unix_dgram``, has
    no stream section, because a datagram is already a frame and needs no rule
    for where it ends.

I2C Groups
^^^^^^^^^^
A serial port is configured by describing how a byte is framed on the wire.
I2C needs none of that, because the protocol fixes it: every byte is eight
data bits, most significant bit first, followed by an acknowledge bit. What a
group does carry is how the master addresses the device and how far it will go
to complete a transfer.

**I2C group attributes**

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
| bus_speed     | No       | Bus clock rate in Hz. Recorded, not applied      |
+---------------+----------+--------------------------------------------------+
| packet_size   | No       | Bytes in one packet exchanged with an endpoint   |
|               |          | of this group. Shared by every type; see above   |
+---------------+----------+--------------------------------------------------+

Requirement
    An I2C group specifies no byte length, parity, or stop bits, because the
    protocol fixes the framing of a byte and leaves nothing to configure.

Requirement
    An I2C group has no stream section, because the master clocks exactly as
    many bytes as it asks for and a transfer is therefore already bounded.

Requirement
    With 7-bit addressing, an address names a device only in the range
    ``0x08`` to ``0x77``. The specification keeps ``0x00`` to ``0x07`` for the
    general call, the CBUS address, and the high-speed master code, and
    ``0x78`` to ``0x7F`` for the 10-bit addressing prefix and for future use.
    An endpoint given one of those is reported as an error rather than
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
    The bus clock rate is recorded rather than applied. It belongs to the bus
    controller, which the platform configures from the device tree or from
    ACPI, and a program holding an endpoint open cannot change it.

The last requirement is the one place where this type departs from the others,
and it is worth saying why the attribute exists at all given that writing it
changes nothing. A bus runs at one rate for every device on it, so the rate is
not an endpoint's to choose. It is still worth stating, because a device that
cannot keep up with the bus it has been wired to fails in ways that are hard to
read from the symptoms. Recording the rate a group expects lets that be checked
against the platform at startup and reported plainly, rather than inferred later
from corrupted transfers.

SPI Groups
^^^^^^^^^^
SPI is clocked and full duplex, so, as with I2C, there is no parity and there
are no stop bits; a transfer is delimited by the chip select rather than by
framing bits. What a group carries is the shape of the clock and of a word.

**SPI group attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| max_speed     | Yes      | Greatest clock rate the peripheral accepts, in   |
|               |          | Hz                                               |
+---------------+----------+--------------------------------------------------+
| mode          | Yes      | Clock polarity and phase: ``0``, ``1``, ``2``,   |
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
| packet_size   | No       | Bytes in one packet exchanged with an endpoint   |
|               |          | of this group. Shared by every type; see above   |
+---------------+----------+--------------------------------------------------+

Requirement
    A SPI group specifies the mode, because the controller has no default that
    is right for every peripheral and a mismatched mode fails the way a
    mismatched data rate fails on a serial line.

Requirement
    A SPI group has no stream section, because the master clocks exactly as
    many words as it asks for and a transfer is therefore already bounded.

Requirement
    The clock rate is an upper bound. A controller divides its own clock down
    and so runs at the greatest rate it can produce that does not exceed the
    rate the group gives.

Stream Payload Protocol Attributes
----------------------------------
A stream delivers bytes with no frame boundaries of its own, so a stream
endpoint needs to be told where one read ends. Three attributes say so.

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

The Endpoints Section
---------------------
An endpoint names itself, names the group it draws its attributes from, and
gives what locates it. Which of the locating attributes apply follows from the
type of that group, so an endpoint is short: everything else was written once,
in the group.

**Endpoint attributes**

+---------------+----------+--------------------------------------------------+
| Name          | Required | Description                                      |
+===============+==========+==================================================+
| name          | Yes      | Name of this endpoint, unique within the file    |
+---------------+----------+--------------------------------------------------+
| group         | Yes      | Name of the group supplying its attributes       |
+---------------+----------+--------------------------------------------------+
| device        | See      | Path of the device or socket this endpoint uses  |
|               | below    |                                                  |
+---------------+----------+--------------------------------------------------+
| address       | See      | Network host, or the address of a device on a    |
|               | below    | bus                                              |
+---------------+----------+--------------------------------------------------+
| port          | See      | Network port                                     |
|               | below    |                                                  |
+---------------+----------+--------------------------------------------------+
| dh_id         | No       | Identifier of the data handler this endpoint     |
|               |          | becomes; see `Endpoints as Data Handlers`_       |
+---------------+----------+--------------------------------------------------+
| oc_address    | No       | Host the data handler this endpoint becomes      |
|               |          | exchanges payload data with the OC on            |
+---------------+----------+--------------------------------------------------+
| oc_port       | No       | Port it does so on                               |
+---------------+----------+--------------------------------------------------+

Which of ``device``, ``address`` and ``port`` apply depends on the group.

**What locates an endpoint of each type**

+---------------------+-----------------------------------------------------+
| Group               | Locating attributes                                 |
+=====================+=====================================================+
| serial              | ``device``                                          |
+---------------------+-----------------------------------------------------+
| network, over a     | ``address`` and ``port``                            |
| host and port       |                                                     |
+---------------------+-----------------------------------------------------+
| network, over a     | ``device``                                          |
| Unix-domain socket  |                                                     |
+---------------------+-----------------------------------------------------+
| i2c                 | ``device``, the bus, and ``address``, the device on |
|                     | it                                                  |
+---------------------+-----------------------------------------------------+
| spi                 | ``device``                                          |
+---------------------+-----------------------------------------------------+

Requirement
    Each endpoint has a name, unique within the file, and names a group defined
    in the same file. Naming a group that does not exist is an error.

Requirement
    An endpoint of a network group speaking ``tcp`` or ``udp`` gives an address
    and a port. One speaking ``unix_stream`` or ``unix_dgram`` gives a device
    instead, because a Unix-domain socket is named by a path.

Requirement
    A locating attribute that does not apply to the endpoint's group is an
    error rather than being ignored, for the same reason it is in a group: a
    misplaced attribute is reported where it was written.

.. note::
   The protocol is not an endpoint attribute. It belongs to the network group,
   with everything else the endpoints of that group share, which is why two
   endpoints of one group cannot be reached over different transports. The same
   holds for the packet size.

Endpoints as Data Handlers
--------------------------
A payload configuration file says which data handlers exist and how tcspecial
reaches each one. An endpoint configuration says how to reach a device in far
more detail: a serial line's framing, a stream's rules for where one read
ends, the parameters of a bus. The two therefore overlap, and the endpoint
format is the richer description of the two.

``EndpointConfigDoc::to_dh_configs`` converts endpoints into the data handler
configurations the rest of the software already takes. The transport comes
from the group and the address from the endpoint, which is the division the
format is built around; the packet size comes from the group, and the id from
the endpoint, because an id distinguishes one handler from another exactly as
an address distinguishes one endpoint from another.

All three programs read data handlers this way. Each is given a file and takes
its handlers from it whichever kind it is, so a payload set may be written
either way and the three stay in step: tcsmoc's panels, tcssim's payloads and
tcspecial's handlers are the same handlers, which is what the payload set
mechanism exists to keep true. Tcsmoc passes the file it was given to the
programs it starts, so one argument names the set for all three.

Requirement
    A file's kind is read from the sections it carries -- ``payloads``
    for a payload configuration, ``endpoints`` or the groups of them for an
    endpoint configuration -- and not from its name. A file's extension says
    how it is spelled, YAML or JSON or XML, and both kinds can be written in
    any of the three.

Requirement
    A file carrying neither section is an error. It describes no data handlers,
    and reading it as an endpoint configuration with no endpoints would make an
    empty document look like a valid one.

The loader is ``tcslibgs::config::load_dh_configs``, and
``handler_source`` answers which kind a file holds without loading it.

Requirement
    An endpoint becoming a data handler has a ``dh_id``. An endpoint
    configuration that assigns none is still valid -- describing how to reach a
    device needs no handler ids -- so this is an error when converting rather
    than when reading the file.

Requirement
    Two endpoints do not share a ``dh_id``. A duplicate converts cleanly and
    then has one handler shadow another, which is what a payload file rejects
    as well.

Requirement
    An endpoint becoming a data handler is in a group that states a packet
    size. The attribute is optional for the same reason ``dh_id`` is, and
    required for the same reason: a handler must know how much data a packet
    carries.

Requirement
    An endpoint becoming a data handler that is to be started states an
    ``oc_address`` and an ``oc_port``, or neither, exactly as a data handler of
    a payload configuration does. They belong to the endpoint rather than its
    group, for the reason the endpoint's own address does: they are what
    distinguish one handler from another.

Requirement
    An endpoint of an I2C group becomes an I2C data handler, carrying the bus
    device and the address of the device on it. Neither can be left behind: the
    bus says which file to open and the address says which device on it
    answers, and a handler holding only the first would talk to whichever
    device the bus was last pointed at.

It used not to become a handler at all, for want of anywhere in a handler's
configuration to put the two together. That made an I2C bus a thing that could
be described in a file and never run.

A Unix-domain socket is the one endpoint whose shape differs between the two
formats. Its group is a network group, but it is located by a path rather than
by a host and a port, so it becomes a network data handler whose address is
that path and whose port is zero -- which is how a payload file spells the
same thing, and why tcsmoc shows no port for one.

The shipped examples assign no ``dh_id``, which makes them endpoint
configurations rather than payload definitions. A test converts each of them
and requires the refusal, so that the examples cannot drift into being half of
each.

Value Syntax
------------
XML carries every attribute value as text, while YAML and JSON distinguish
numbers from strings. Both are accepted everywhere a value is expected, so the
same value may be written ``8`` or ``"8"`` without changing its meaning.

**Value syntax**

+---------------+--------------------------------------------------------------+
| Kind          | Syntax                                                       |
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
| Bus address   | Decimal or ``0x`` hexadecimal, within the width the group's  |
|               | addressing mode allows and not one the bus reserves          |
+---------------+--------------------------------------------------------------+

Requirement
    A timeout carries an explicit unit. A bare number is an error, so that a
    file cannot silently mean milliseconds where seconds were intended.

Requirement
    A timeout of zero is an error. A read that is not to wait is written
    ``none``.

Requirement
    A hexadecimal value is written as a string in JSON, which has no
    hexadecimal number syntax of its own: ``"0x48"``, not ``0x48``. YAML and
    XML accept either spelling, and all three mean the same value.

Attribute names may be spelled with either underscores or hyphens, so
``max_length`` and ``max-length`` are the same attribute.

How the Formats Correspond
--------------------------
The formats differ only in how the common structure is spelled.

**Format correspondence**

+---------------------+---------------------------+--------------------------+
| Structure           | YAML                      | XML                      |
+=====================+===========================+==========================+
| A section           | A mapping key             | An element               |
+---------------------+---------------------------+--------------------------+
| A list of entries   | A sequence                | Repeated child elements  |
+---------------------+---------------------------+--------------------------+
| An attribute        | A mapping key             | An XML attribute         |
+---------------------+---------------------------+--------------------------+
| A group's type      | ``type: serial``          | ``type="serial"``        |
+---------------------+---------------------------+--------------------------+
| A byte list         | ``[0x0D, 0x0A]``          | ``"0x0D,0x0A"``          |
+---------------------+---------------------------+--------------------------+

.. note::
   XML names the repeated children of the two list sections ``<group>`` and
   ``<endpoint>``, the singular of the section that contains them.

.. note::
   JSON spells structure the way YAML does, a section being an object and a
   list an array, so the YAML column above describes JSON as well. The one
   thing JSON cannot carry is a comment, so the remarks in the examples below
   have no JSON equivalent.

YAML Example
------------
.. code-block:: yaml

   # Endpoint configuration for the payload bay.
   general:
     version: "1.0"
     description: Flight payload endpoints

   endpoint_groups:
     # Everything an RS-422 payload link shares. No device name appears here:
     # that is what tells one endpoint of this group from another, so it belongs
     # to the endpoint.
     - name: rs422_payload
       type: serial
       datarate: 115200
       stop_bits: 1
       byte_length: 8
       packet_size: 512
       stream:
         max_length: 512
         timeout: 250ms
         terminators: [0x0D, 0x0A]

     # A reader that simply hands over 64 bytes at a time. Saying "timeout: none"
     # is how a file asks for that; leaving the timeout out would be an error.
     - name: rs422_blockmode
       type: serial
       datarate: 38400
       stop_bits: 2
       byte_length: 8
       packet_size: 64
       stream:
         max_length: 64
         timeout: none

     # A stream protocol, so it needs a rule for where a read ends.
     - name: payload_tcp
       type: network
       protocol: tcp
       packet_size: 1024
       stream:
         max_length: 4096
         timeout: 1s

     # A datagram protocol: the datagram is already the frame, so no stream
     # section applies.
     - name: payload_udp
       type: network
       protocol: udp
       packet_size: 256

     # A stream protocol again, but a Unix-domain socket is named by a path
     # rather than by a host and port, so its endpoint gives a device. This is
     # also the one group stating no packet size: the attribute is optional, and
     # a recorder takes whatever a read delivers.
     - name: payload_unix
       type: network
       protocol: unix_stream
       stream:
         max_length: 1024
         terminators: [0x0A]

     # An I2C bus. There is no byte length, parity, or stop bits to give: the
     # protocol fixes the framing of a byte. The bus speed is recorded so that
     # it can be checked against the platform, which is what actually sets it.
     - name: payload_i2c
       type: i2c
       packet_size: 32
       pec: true
       retries: 2
       timeout: 50ms
       bus_speed: 400000

     # A SPI bus. The mode must be stated, because no default is right for every
     # peripheral, and the speed is an upper bound rather than an exact rate.
     - name: payload_spi
       type: spi
       packet_size: 64
       max_speed: 10000000
       mode: 3
       bits_per_word: 8
       cs_active: low

   endpoints:
     - name: magnetometer
       group: rs422_payload
       device: /dev/ttyS0

     - name: star_tracker
       group: rs422_payload
       device: /dev/ttyS1

     - name: spectrometer
       group: rs422_blockmode
       device: /dev/ttyUSB0

     - name: camera
       group: payload_tcp
       address: 10.0.0.20
       port: 5000

     - name: housekeeping
       group: payload_udp
       address: 10.0.0.21
       port: 5001

     - name: recorder
       group: payload_unix
       device: /run/tcspecial/recorder.sock

     # Two sensors on one bus, differing only in the address the master uses.
     # That is exactly what an endpoint is for.
     - name: thermal_a
       group: payload_i2c
       device: /dev/i2c-2
       address: 0x48

     - name: thermal_b
       group: payload_i2c
       device: /dev/i2c-2
       address: 0x49

     # A SPI device node names the bus and the chip select together, so there is
     # no separate address to give.
     - name: imu
       group: payload_spi
       device: /dev/spidev0.1

XML Example
-----------
.. code-block:: xml

   <?xml version="1.0" encoding="UTF-8"?>
   <!-- Endpoint configuration for the payload bay. -->
   <endpoint-configuration>

     <general version="1.0" description="Flight payload endpoints"/>

     <endpoint-groups>

       <!-- Everything an RS-422 payload link shares. No device name appears
            here: that is what tells one endpoint of this group from another,
            so it belongs to the endpoint. -->
       <group name="rs422_payload" type="serial"
              datarate="115200" stop_bits="1" byte_length="8"
              packet_size="512">
         <stream max_length="512" timeout="250ms" terminators="0x0D,0x0A"/>
       </group>

       <!-- A reader that simply hands over 64 bytes at a time. Saying
            timeout="none" is how a file asks for that; leaving the timeout
            out would be an error. -->
       <group name="rs422_blockmode" type="serial"
              datarate="38400" stop_bits="2" byte_length="8"
              packet_size="64">
         <stream max_length="64" timeout="none"/>
       </group>

       <!-- A stream protocol, so it needs a rule for where a read ends. -->
       <group name="payload_tcp" type="network" protocol="tcp" packet_size="1024">
         <stream max_length="4096" timeout="1s"/>
       </group>

       <!-- A datagram protocol: the datagram is already the frame, so no
            stream section applies. -->
       <group name="payload_udp" type="network" protocol="udp" packet_size="256"/>

       <!-- A stream protocol again, but a Unix-domain socket is named by a
            path rather than by a host and port, so its endpoint gives a
            device. This is also the one group stating no packet size: the
            attribute is optional, and a recorder takes whatever a read
            delivers. -->
       <group name="payload_unix" type="network" protocol="unix_stream">
         <stream max_length="1024" terminators="0x0A"/>
       </group>

       <!-- An I2C bus. There is no byte length, parity, or stop bits to give:
            the protocol fixes the framing of a byte. The bus speed is recorded
            so that it can be checked against the platform, which is what
            actually sets it. -->
       <group name="payload_i2c" type="i2c" packet_size="32"
              pec="true" retries="2" timeout="50ms" bus_speed="400000"/>

       <!-- A SPI bus. The mode must be stated, because no default is right for
            every peripheral, and the speed is an upper bound rather than an
            exact rate. -->
       <group name="payload_spi" type="spi" packet_size="64"
              max_speed="10000000" mode="3" bits_per_word="8" cs_active="low"/>

     </endpoint-groups>

     <endpoints>
       <endpoint name="magnetometer"  group="rs422_payload"   device="/dev/ttyS0"/>
       <endpoint name="star_tracker"  group="rs422_payload"   device="/dev/ttyS1"/>
       <endpoint name="spectrometer"  group="rs422_blockmode" device="/dev/ttyUSB0"/>
       <endpoint name="camera"        group="payload_tcp"     address="10.0.0.20" port="5000"/>
       <endpoint name="housekeeping"  group="payload_udp"     address="10.0.0.21" port="5001"/>
       <endpoint name="recorder"      group="payload_unix"    device="/run/tcspecial/recorder.sock"/>

       <!-- Two sensors on one bus, differing only in the address the master
            uses. That is exactly what an endpoint is for. -->
       <endpoint name="thermal_a"     group="payload_i2c"     device="/dev/i2c-2" address="0x48"/>
       <endpoint name="thermal_b"     group="payload_i2c"     device="/dev/i2c-2" address="0x49"/>

       <!-- A SPI device node names the bus and the chip select together, so
            there is no separate address to give. -->
       <endpoint name="imu"           group="payload_spi"     device="/dev/spidev0.1"/>
     </endpoints>

   </endpoint-configuration>

Both files above describe the same nine endpoints in seven groups, and parse
into values that compare equal. So does ``endpoints.json`` in the same actual
directory, which is that configuration once more in JSON and which the same
test holds to the same standard. It is not printed here because a third listing
of one configuration would show a reader of the first two nothing new.

Testing
=======
There are two components of testing software. Tcssim is a GUI used to simulate
payloads and tcsmoc is used to simulate the MOC. Both allow user interaction
to change parameters and see what result the changes produce.

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
   |  |  payload1.yaml  |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    |  payload1.yaml  |  |     |                 |
                              :    |    +-----------------+  |     |                 |
                              :    +-------------------------+     +-----------------+

tcssim
------
Tcssim is a GUI simulating the payloads. It reads two files. The payload
definition comes from payload1.yaml, named by the ``SIM_PAYLOAD_CONFIG_PATH``
environment variable when that is set, and says which payloads exist, how each
is reached, and how big its packets are. What simulating them takes comes from
payload1sim.yaml, named by ``PAYLOAD_SIM_YAML``; see `Simulator Configuration
Files`_.

Requirement
    A payload configuration file describes payloads and says nothing about
    simulating them. How often a payload produces a packet, and how a packet is
    divided into segments, are properties of a simulation rather than of a
    payload, and are stated in the simulator configuration file.

The two are joined by name: a simulated payload names the data handler it stands
in for. Neither file is read in the order of the other, and neither may be
silently short.

Requirement
    Every data handler of the payload file has a simulated payload naming it,
    and every simulated payload names a data handler of the payload file.
    Either kind of mismatch is reported rather than simulated around, because a
    misspelled name is otherwise indistinguishable from a payload deliberately
    left out.

Which Kinds Can Be Simulated
^^^^^^^^^^^^^^^^^^^^^^^^^^^^
A simulated payload has to be whatever is at the far end of its handler's
payload endpoint, and the five kinds of endpoint are not equally easy to be.

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
    and a payload file meant for simulation can name a path the simulator may
    create.

Requirement
    A simulated payload of an I2C endpoint is a chip of the kernel's
    ``i2c-stub``. That adapter's chips are a bank of registers in memory and
    every master on the bus reads and writes the same bank, so the bank is the
    payload: the simulator writes packets into it and the handler reads them
    out.

Nothing in user space can answer an address on a bus, which is why this is the
one kind of simulated payload that is not the far end of anything. It has two
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
    their numbers are assigned as they are found and a payload file cannot know
    which number the stub will have. With none loaded, or with no chip at the
    configured address, what is reported is the command that would put it
    there.

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
confidently -- which is the fault the kinds were separated to prevent.

What such a link tests is a handler's reading and writing of a peripheral and
the pacing of a payload that produces data in blocks. What it cannot test is
whether the peripheral would stand being clocked that way, which is a question
for the peripheral.

So every kind of endpoint, and every protocol of the network kind, can be
simulated. The stand-ins are not all of one quality, and that is the thing to
keep in view rather than the coverage: a socket is a socket and a pty is a
line in all but its rate, while a bus is a register bank that every master
shares and a SPI peripheral is a pty with no clock behind it. Each kind's own
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
payload file, the rest from the simulator file.

A ``Params`` button shows the whole of what the two files said about that
payload.

Requirement
    Each panel has a button that shows every parameter read from the two
    configuration files for that payload, in both programs. What a panel has
    room for is a name, a size and what has moved; a payload set says a good
    deal more than that -- a trigger, a line rate, a slave address, a clock
    mode, a fault -- and until this there was nowhere to see it but the files
    themselves, which is no use for telling what the program actually read.

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
    Only the attributes of the payload's own kind are shown, and only the
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
    that is missing, unreadable, or written for another payload file costs it
    that half of what a panel can show and nothing else; it says so once on
    the way past, and the panel says so where the parameters are shown.

The panels are laid out in a grid whose shape follows from how many there are.
The window is kept wider than it is tall, and among the shapes that satisfy
that, the one nearest square is chosen; tcssim opens the window at the size that
shape asks for. The four payloads of the shipped files give a two-by-two grid in
a 640x496 window. The rule is the one tcsmoc follows, applied to the sizes of
tcssim's own panels.



tcsmoc
------
The tcsmoc is a GUI program used to control simulated payloads
interacting with tcspecial using
the tcslib library over a datagram connection to tcspecial.  For testing
purposes, tcsmoc uses tcslib, along with simulated payloads, to support a simple GUI.

Tcsmoc gets the payload definition from a file named on its command line, and
reads payload1.yaml when the command line names none. It takes an argument
rather than an environment variable because it starts tcspecial and tcssim as
subprocesses and they inherit its environment: a variable naming tcsmoc's file
would name theirs too, and so could not point tcsmoc at one file and its
children at another. An argument belongs to tcsmoc alone.

Requirement
    Tcsmoc accepts one payload configuration file. A second path is an error
    rather than an argument with no effect, because it is more likely a mistake
    about which file is being read than something meant to be ignored.

Every program is pointed at its payload file the same way: as a command line
argument, failing that the program's own environment variable --
``PAYLOAD_CONFIG_PATH`` for tcspecial and ``SIM_PAYLOAD_CONFIG_PATH`` for
tcssim, while tcsmoc has none for the reason below -- and failing that
payload1.yaml. Tcssim names its simulator configuration separately again,
through ``PAYLOAD_SIM_YAML``.

Requirement
    An argument naming the payload file takes precedence over any environment
    variable naming one. The argument is the unambiguous of the two: tcsmoc
    starts the other programs and they inherit its environment, so a variable
    can reach further than it was meant to, where an argument reaches exactly
    the program it is given to.

Requirement
    One payload file is named. A second argument is an error rather than one
    with no effect, because it is more likely a mistake about which file is
    being read than something meant to be ignored.

Requirement
    Tcsmoc passes its own payload file to each program it starts, as that
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
Tcsmoc never reads it and so has nothing to say about which one is right;
tcssim takes it from ``PAYLOAD_SIM_YAML``, inherited from tcsmoc's environment
like any other variable.

The Makefile names a set once for this reason. ``PAYLOAD_YAML`` reaches every
program as its argument and ``PAYLOAD_SIM_YAML`` reaches tcssim in the
environment, so one target runs a whole set:

.. code-block:: console

   $ make runmoc
   $ make runmoc PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml
   $ make runmoc PAYLOAD_YAML=payload2.yaml PAYLOAD_SIM_YAML=payload2sim.yaml

Both are named because the two files are a pair: tcsmoc passes the payload file
on to its children and never reads the simulator file, so naming one of a pair
is how the two come to describe different sets. That is reported rather than
run -- every data handler of the payload file must have a simulated payload
naming it -- but it is reported by tcssim, after tcsmoc has already come up,
which makes it a confusing way to find out.

``make run`` takes ``PAYLOAD_YAML`` for tcspecial alone, and ``make runsim``
takes both for the simulator alone -- both, because tcssim run by itself needs
the payload file as well as the simulator file, and the two must describe the
same set or their names will not match:

.. code-block:: console

   $ make run PAYLOAD_YAML=payload1.yaml
   $ make runsim PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml
   $ make runmoc PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml

Started in that order, the three run as separately as they can: tcsmoc finds
the tcspecial already there, attaches to it rather than starting a second, and
leaves it running when its window closes. Which is what one program at a time
is for -- a debugger on the one being worked on, and the others left alone.

``make help`` lists the targets, and ``RUST_LOG`` reaches every one of them:

.. code-block:: console

   $ make runmoc RUST_LOG=debug
   $ make run PAYLOAD_YAML=payload1.yaml RUST_LOG=tcspecial::ci=trace

The manual says the same thing from the other end -- what to type to run a
set -- under "Running the Programs".

The window is built from that file. Nothing in the GUI names a data handler or
fixes how many there are, so a handler added to or removed from payload1.yaml
adds or removes a rectangle.

The rectangles are laid out in a grid whose shape follows from how many there
are. The window is kept wider than it is tall, and among the shapes that
satisfy that, the one nearest square is chosen; tcsmoc opens the window at the
size that shape asks for. The four data handlers of the shipped payload1.yaml
give a two-by-two grid in a 640x630 window.

The GUI has a section at the
top of its single window that allows issuing of CI commands and viewing responses.
Below that are as many rectangles as there are data handlers.
The rectangle is blank if the DH has
not been started or has been stopped after having been started. Otherwise, it
displays the DH name, configuration information, and packet size. It shows no
packet interval: that is a simulator setting, stated in payload1sim.yaml and
shown by tcssim.
Below that it displays the time and the data most recently sent. Underneath
that is the time and data most recently received. Both come from
`QUERY_DH_SAMPLE`_ telemetry, which tcsmoc asks for alongside the statistics;
the times are when the data moved rather than when it was asked about, and
data longer than a sample is shown as a head followed by an ellipsis.

A handler whose conduits have carried nothing in a direction shows
``--:--:--`` and no data for it, which is also what a panel shows before it
has been queried at all.

Each panel has one button, which starts and stops the handler.

Requirement
    The button is labelled with what pressing it will do. A handler that is
    moving data offers ``Discard``, which stops it; a handler that is not
    offers ``Transmit``, which starts it. There were two buttons, Start and
    Stop, and either was pressable whatever the handler was doing: a Stop on
    a stopped handler asks tcspecial to stop something that is not running
    and reads as though it had done something.

Requirement
    Which command a press sends is decided by the status the label was made
    from, not by what the handler turns out to be doing. The window builds the
    label and the program decides the command, so the two are checked against
    each other: a button that offered one thing and did the other would be
    worse than either button alone.

Requirement
    A handler whose last command failed offers ``Transmit``. Its status is
    neither started nor stopped, and the useful thing to offer is the start
    that failed rather than a stop of something that never began.

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
    A handler that answers the statistics but not the samples keeps the sample
    lines it had. A blank line says a handler has moved nothing, which is a
    different thing from a question that went unanswered.

Testing requires starting up tcssim before other operations and shutting it down
when tcsmoc is halted.

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

* DHId must implement the trait Ord.

* The crate libc must be included to Cargo.toml for all crates to get definitions of AF_UNIX and other address families.

* The crate serde_json must be added to Cargo.toml for all crates to get JSON definitions.

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
