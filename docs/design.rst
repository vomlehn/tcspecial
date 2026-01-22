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

Features
========

* Centralized control

  * Statistics
    
    * Bytes transferred and I/O operations attempted and completed

    * Maintained on global and per payload basis

  * Memory allocation complete during initialization

* TCSpecial code is comprised of:
  
  * Running on spacecraft

    * tcsspecial: process running on spacecraft

* Testing software is:

  * tcsmoc: Simple interface using tcslib for visualizing tcsspecial operation and doing testing. 

  * tcssim: Simulated payloads

* There are two libraries and a JSON file with payload configuration information:

    * tcslib: ground software library providing simple integration with mission control software

    * tcslibgs: sofware library containing command, telemetry, and any other definitions shared between tcsspecial and tcslib

    * tcspayload.json: Configuration information

      * Network connections support Stream and datagram
    
      * Network and device
  
        * Support for <n> interfaces on Linux
  
        * Network devices are specified by node and server, like getaddrinfo(), along
          with the other getaddrinfo() hints.
  
        * Other devices are specified by a pathname.
  
        * Streams, both network streams, and devices, have a message length and
          a wait interval.


* Radio interfaces

  * UDP standard

* Straight-forward extensibility for custom payload and uplink/downlink interfaces

* Written in Rust:

    * Provides a high degree of portability

    * Take advantage of Rust memory ownership features to eliminate many errors

    * Leverage the many features Rust has to produce clean, well documented code, with many modern features to make it easy to write code.


High-Level View of TCSpecial
----------------------------
Tcsspecial fits into the ground and space portions of the command and telemetry
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
   |  | Mission |          |  :  ----+->| tcsspecial      |  |     |                 |
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
   |  | tcspayload.json |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    | tcspayload.json |  |     |                 |
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
software, tcslib, and the space portion, tcsspecial.

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
Return statistics from the indicated data handler:

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

tcsspecial
---------
Tcsspecial hass a command interpreter (CI) running on the spacecraft where the
payloads are located. CI has one or more threads to handle OC communications, i.e.
data exchanged with tcslib over a bi-directional communication link. It
defaults to using datagram communication to the tcslib, though this is usually actually
a link to the spacecraft radio. The radio may use a different protocol.

Tcsspecial also has threads associated with each data handler (DH). Each DH
communicates to payloads via a bi-direction channel. A key feature of tcsspecial
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

After activation, it enters a loop relaying data between OC
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
Data handlers (DHs) relay data between the OC and
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
a bigger buffer but tcsspecial does not support this capability.

Stream Endpoints
................
Streams do not have embedded markers to indicate data boundaries, so select()-like
calls indicate only that one or more bytes are available. Transferring one byte
at a time will incur a significant amount of overhead, so tcsspecial can delay for
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
   Tcsspecial and tclib must translate the operating system dependent values, such as
   the address families, types, and protocols, to canonical values, and back again
   to avoid building in operating system dependent code. All values transmitted
   must use the canonical values.

Device Endpoints
"^^^^^^^^^^^^^^^^
Linux device Endpoints open device entries in the /dev directory. This could be:

**Linux Device Endpoints**

+----------------+---------------------------------------------+
| Name           | Description                                 |
+================+=============================================+
| /dev/ttyS0     | Serial port, such as RS-232 or RS-422       |
+----------------+---------------------------------------------+
| /dev/ttyUSB0   | USB serial adapter (to RS-232 or RS-422     |
+----------------+---------------------------------------------+
| /dev/i2c-2     | I2c bus                                     |
+----------------+---------------------------------------------+
| /dev/spidev0.1 | SPI device                                  |
+----------------+---------------------------------------------+


Relays
^^^^^^
Relays contain two Endpoints, an EndpointReadable and an EndpointWritable. Data
flows in just one direction, from the EndpointReadable to the EndpointWriteable.
A Relay is implemented as a thread that simply looks between the Read and
EndpointWriteables, handling commands from the Command Interpreter as necessary.
The directions are denoted "Ground to Payload" and "Payload to Ground"

Data Handlers
^^^^^^^^^^^^^
Data Handlers package two Relays, one in one direction and one in the
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

This approach allows TeleNex resources to be allocated statically, but
the underlying operating system may use dynamic resource allocation. These
may fail, so the CI and DH code must be prepared to handle failures in
in the operating system and retry at intervals if the various protocols do
not already support this.

The software running on the spacecraft is TCSpecial. This has a command
interpreter and some number of data handlers (DHs). The CI talks to ground
software and to the DHs. The DHs talk to the CI and to the payloads.

DH Links
^^^^^^^^
Supported DH protocols include networking protocols and device interfaces, such
as RS-422, as well as stackable protocols. These links can be divided into stream
and datagram types. Stream data has no delimiters, no error detection and correction
codes, etc. There may be gaps between bytes and sequences of bytes that can be
used to identify groups of data.

Datagrams are groups of data with a count or other mechanism to identify the start
and end locations. Under normal situations, datagrams are limited to a specific
length, but the DH interfaces optionally 
allow examining incoming packets to determine whether
they exceed the previously expected length and reallocating a larger buffer to be
able to read the whole packet with truncation. This is only effective up to some
size, however, the kernel itself will have limitations on the packet size it can read.


Provided Stacked DHs
==≈=≈=============
TeleNex offers several several stacked DHs that can be used as-is and as examples.

Packetizing U32 Streaming Data
"^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
The payload is sending big-endian u32 data items every three seconds. The DH accumulates up to four of these. If the bytes have a time interval > between 100 mS, that data item is discarded. The resulting packet has a leading byte with a bit set for each valid data item, followed by valid data item values.

Byte Swapping Values
"^^^^^^^^^^^^^^^^^^^^
This DH is composited with the DH that packetizes streaming data. It converts the big-endian u32s to little-endian u32s.

Packetizing Arbitrary Streaming Data With Constant Overhead Byte Stuffing
‐-----‐------------------------------------------------------------------------------------------------------------------
Read 254  * 4 bytes from the payload, write to a buffer with COBS, stick a u16 header which is a byte count and send to the OC.

tcslib
------
The TCSpecial library has a set of operations for global control and status
and a set of per-payload interface operations.  It is used for building control applications using mission control
software such as YAMCS or MCT.

tcslibgs
--------
The TCSpecial ground/space library contains definitions used by both
tcslib and tcsspecial.

tcspayload.json
---------------
This is a JSON file that defines the actual payloads. It is considered part
of TCSpecial as it must be supplied, but is also used by the
test software. The data handlers for payloads have the following configurations:

+------+---------+-------------------------+-------------+------------------+
| DH # | Type    | Configuration           | Packet Size | Packet interval  |
+======+======+============================+=============+==================+
| 0    | Network | TCP/IP | localhost:5000 | 12 bytes    | 1 packet/second  |
+------+---------+--------+----------------+-------------+------------------+
| 1    | Network | UDP/IP | localhost:5001 | 11 bytes    | 1 packet/second  |
+------+---------+--------+----------------+-------------+------------------+
| 2    | Device  | n/a    | /dev/urandom   | 1 byte      | continuous       |
+------+---------+--------+----------------+-------------+------------------+
| 3    | Network | UDP/IP | localhost:5003 | 15 bytes    | 2 packets/second |
+------+---------+--------+----------------+-------------+------------------+

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
   |  | tcsmoc  |          |  :  ----+->| tcsspecial      |  |     |                 |
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
   |  | tcspayload.json |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    | tcspayload.json |  |     |                 |
                              :    |    +-----------------+  |     |                 |
                              :    +-------------------------+     +-----------------+

tcssim
------
Tcssim is a GUI simulating the payloads. It gets the payload definition from
tcspayload.json.

Each payload occupies a portion of the window, displaying its name, configuration,
and statistics. It also displays the most recent packets sent and received.
The DHs are named "DH" plus the DH #.

The packet size and interval can be changed.

tcsmoc
------
The tcsmoc is a GUI program used to control simulated payloads
interacting with tcsspecial using
the tcslib library over a datagram connection to tcsspecial.  For testing
purposes, tcsmoc uses tcslib, along with simulated payloads, to support a simple GUI.

Tcsmoc gets the payload definition from tcspayload.json.

The GUI has a section at the
top of its single window that allows issuing of CI commands and viewing responses.
Below that are up to eight rectangles. The rectangle is blank if the DH has
not been started or has been stopped after having been started. Otherwise, it
displays the DH name, configuration information, packet size, and packet interval.
Below that it displays the time and the data most recently sent. Underneath
that is the time and data most recently received.

Testing requires starting up tcssim before other operations and shutting it down
when tcsmoc is halted.

GUI Framework
-------------
The testing GUIs will all use
slint
with black on white. All windows will have a go away box which will shut down
the entire application, i.e. tcsspecial, tcsmod, and tcssim.

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


Possible Enhancements
=====================

Build-time selection of CI and DH protocols to OC/spacecraft radio

    Though most or all of on-board spacecraft protocols are based on datagrams, use
    of error correcting stream-based protocols are also a reasonable choice for this
    purpose. Build-time selection of these seems useful.

Special tty-based timing of input

    Could use the ioctl_tty/termios maximum number of characters to read a message.

Tcsspecial manual/auto fail over

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
