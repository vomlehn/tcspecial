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

    * tcspecial: process running on spacecraft

* Testing software is:

  * tcsmoc: Simple interface using tcslib for visualizing tcspecial operation and doing testing. 

  * tcssim: Simulated payloads

* There are two libraries and a YAML file with payload configuration information:

    * tcslib: ground software library providing simple integration with mission control software

    * tcslibgs: sofware library containing command, telemetry, and any other definitions shared between tcspecial and tcslib

    * tcspayload.yaml: Configuration information

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
   |  | tcspayload.yaml |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    | tcspayload.yaml |  |     |                 |
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

tcspecial
---------
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

tcslibgs
--------
The TCSpecial ground/space library contains definitions used by both
tcslib and tcspecial.

tcspayload.yaml
---------------
This is a YAML file that defines the actual payloads. It is considered part
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
    A group definition carries every attribute of its endpoint type except the
    one that locates an endpoint: its device name, its network address, or, on
    a bus that addresses its devices, the address of the device on that bus.

Requirement
    What locates an endpoint is carried by the endpoint definition.

Requirement
    An endpoint of an I2C group gives both the bus device and the slave
    address, because two endpoints of one group commonly sit on the same bus
    and differ only in which device the master addresses.

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
    An attribute that does not apply to a group's type is an error rather than
    being ignored, so that a misspelled or misplaced attribute is reported
    where it was written.

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
       stream:
         max_length: 64
         timeout: none

     # A stream protocol, so it needs a rule for where a read ends.
     - name: payload_tcp
       type: network
       protocol: tcp
       stream:
         max_length: 4096
         timeout: 1s

     # A datagram protocol: the datagram is already the frame, so no stream
     # section applies.
     - name: payload_udp
       type: network
       protocol: udp

     # A stream protocol again, but a Unix-domain socket is named by a path
     # rather than by a host and port, so its endpoint gives a device.
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
       pec: true
       retries: 2
       timeout: 50ms
       bus_speed: 400000

     # A SPI bus. The mode must be stated, because no default is right for every
     # peripheral, and the speed is an upper bound rather than an exact rate.
     - name: payload_spi
       type: spi
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
              datarate="115200" stop_bits="1" byte_length="8">
         <stream max_length="512" timeout="250ms" terminators="0x0D,0x0A"/>
       </group>

       <!-- A reader that simply hands over 64 bytes at a time. Saying
            timeout="none" is how a file asks for that; leaving the timeout
            out would be an error. -->
       <group name="rs422_blockmode" type="serial"
              datarate="38400" stop_bits="2" byte_length="8">
         <stream max_length="64" timeout="none"/>
       </group>

       <!-- A stream protocol, so it needs a rule for where a read ends. -->
       <group name="payload_tcp" type="network" protocol="tcp">
         <stream max_length="4096" timeout="1s"/>
       </group>

       <!-- A datagram protocol: the datagram is already the frame, so no
            stream section applies. -->
       <group name="payload_udp" type="network" protocol="udp"/>

       <!-- A stream protocol again, but a Unix-domain socket is named by a
            path rather than by a host and port, so its endpoint gives a
            device. -->
       <group name="payload_unix" type="network" protocol="unix_stream">
         <stream max_length="1024" terminators="0x0A"/>
       </group>

       <!-- An I2C bus. There is no byte length, parity, or stop bits to give:
            the protocol fixes the framing of a byte. The bus speed is recorded
            so that it can be checked against the platform, which is what
            actually sets it. -->
       <group name="payload_i2c" type="i2c"
              pec="true" retries="2" timeout="50ms" bus_speed="400000"/>

       <!-- A SPI bus. The mode must be stated, because no default is right for
            every peripheral, and the speed is an upper bound rather than an
            exact rate. -->
       <group name="payload_spi" type="spi"
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
   |  | tcspayload.yaml |  |  :    | |  |      .          |  |     |                 |
   |  +-----------------+  |  :    |           .             |     |                 |
   +-----------------------+  :    | |  |      .          |  |     |                 |
                              :    | |  +-----------------+  |     |  +-----------+  |
                              :    | +->| Data            |<--------->| Payload n |  |
                              :    |    | Handler n       |  |     |  +-----------+  |
                              :    |    +-----------------+  |     |                 |
                              :    |    | tcspayload.yaml |  |     |                 |
                              :    |    +-----------------+  |     |                 |
                              :    +-------------------------+     +-----------------+

tcssim
------
Tcssim is a GUI simulating the payloads. It gets the payload definition from
tcspayload.yaml.

Each payload occupies a portion of the window, displaying its name, configuration,
and statistics. It also displays the most recent packets sent and received.
The DHs are named "DH" plus the DH #.

The packet size and interval can be changed.



tcsmoc
------
The tcsmoc is a GUI program used to control simulated payloads
interacting with tcspecial using
the tcslib library over a datagram connection to tcspecial.  For testing
purposes, tcsmoc uses tcslib, along with simulated payloads, to support a simple GUI.

Tcsmoc gets the payload definition from tcspayload.yaml.

The GUI has a section at the
top of its single window that allows issuing of CI commands and viewing responses.
Below that are as many rectangles as there are data handlers.
The rectangle is blank if the DH has
not been started or has been stopped after having been started. Otherwise, it
displays the DH name, configuration information, packet size, and packet interval.
Below that it displays the time and the data most recently sent. Underneath
that is the time and data most recently received.

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
