=========================
Telenex (Telemetry Nexus)
=========================

Telenex is a framework for passing commands to payload devices from
an operations center (OC) and relaying
telemetry`from the payloads to the OC. It is designed for extensible
protocol translation for both stream and datagram-oriented operation.
Telenex has a library for linking with other OC software and multi-threaded
process that runs on the system containing payloads.

Telenex is designed so that it can operate with all resources (buffers, thread
data, etc.) statically allocated before entering its main loop, with
dynamically allocated resources, or a combination of static and dynamic
allocation.

Payload System Software
=======================

Payload system software consists of a command interpreter and some number
of data handling units.

Command Interpreter (CI)
------------------------

The payload system
software has a command interpreter with two threads. The threads manage
commands from the OC and status messages to the OC. I/O is done
with datagrams. Status messages are queued with a fixed-length queue.
The connection between the OC and the CI is implemented with a UDP/IP
datagram.

The CI has a Vec<DH> which holds all of the allocated DHs.

Initialization
^^^^^^^^^^^^^^
When created, a CI uses a socket interface to initialize a datagram
connection to
OC. The connection

Data Handlers (DHs)
-------------------

Data handlers (DHs) are responsible for relaying data between the OC and
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
   :dedent: 4

    ....|    |
    ...|    |

.. code-block:: text
   :dedent: 0

         ______            _____            _________
        |      |          |     |          |         |
        |  OC  |<-------->| DH1 |<-------->| Payload |
        |______|  UDP/IP  |_____|  serial  |_________|

The serial interface consists of bytes written at arbitrary times.

DHs may also be composited to perform protocol translation. For example,
instead of a simple stream of bytes, the stream of data may consist of
a byte count followed by the counted number of bytes. This could be used
to assemble packets from the stream of bytes. This might look like:

.. code-block:: text
   :dedent: 4

         ______            _____                _____            _________
        |      |          |     |              |     |          |         |
        |  OC  |<-------->| DH1 |<------------>| DH2 |<-------->| Payload |
        |______|  UDP/IP  |_____|  packetizer  |_____|  serial  |_________|

A call to oc_read() in DH2 then becomes a call to oc_read() in DH1.



aaa
^^^

bbb
"""


It is also possible to stack any number of DHs together to create protocol
stacks. A stacking DH must be added to the payload side of another DH.




Command Origins and Destinations, and Telemetry Origins and Destinations,
may be either connections to other DHs, which allows compositing protocols,
or file descriptors. In the case of DH connections, sending and receving
data involves a call to the DH on the other side of the connection. This
will resolve to a DH with a file descriptor.

Each DH is associated with a pipe, from which it reads one byte. This pipe
is written to by the command interpreter when it wants to wake up the DH
to shut the threads down or perform another action. Each DH thus has one
file descriptor for reading and writing data to the OC, one file descriptor
for reading and writing payload data, and a third file descriptor for the
pipe written by the command interpreter. Waiting for asynchronous I/O
is done using the mio crate.

Each static DH is assign static address information. This can be a path to a
device or a network address.

Payload data can be sent by the payload as datagrams or as a stream. Datagrams
may be a fixed maximum size or a dynamic size. Use of dynamic message sizes
uses the MSG_PEEK and MSG_TRUC options for rcv(). Since it may cause memory
allocaton operations, it should not be used in applications which require
that no allocations be done after initialization.

When data is being read as a stream, there are two options:

:Send any data: All pending data on the file descriptor will
                be read when there is at least one byte pending.

:Wait for full: Wait for the buffer to fill up. A timer is set so that, if
                the buffer does not fill up, all data in the input buffer
                is sent

Telenex Library
===============
The Telenex library has a set of operations for global control and status
and a set of per-payload interface operations.

Commands and Telementry
-----------------------
Commands are sent to the CI and telemetry returned from the CI as network
messages, using the socket interface. The communication system must use the
same protocol, e.g. UDP/IP.

Note that all commands must be idempotent. That is, if a command is sent twice
without any intervening commands, the Telenex state will be the same as if
it had just been sent once. This allows the OC to handle lost commands and
telemetry without requiring closed loop communications.

The CI and each DH maintain a separate serial number which is sent with 
each command. That serial number is returned in the telemetry that corresponds
to that command. The CI also has beacon telemetry that includes a separate
serial number, allowing for detection of lost beacon telemetry.


CI Commands and Telemetry
-------------------------
Each CI command has a corresponding telemetry message indicating the
success or failure of the command. There is also a beacon telemetry message
send periodically without a command.

Commands
^^^^^^^^
:Init:      Go through initialization and bring up all static DHs

:Config:    Set CI configuration:

* Set Beacon interval. It is not possible to disable the beacon entirely,
  but it can be set to a very long value.

:Shutdown:  Disconnect all static and dynamic DHs

:Ping:      Request status from the CI        

:SendMap:   Send a vector of DH states:

Telemetry
^^^^^^^^^
:Beacon:    This telemetry is sent automatically

:InitTelem: Return

Per-DH Control and Status
--------------------------
Allocate NAME:  Start thread and allocate buffers for the DH with the given
                name. Includes:

* DH name
* Stream/Datagram
* Timeout (in nanoseconds)
* 
* Address/device/DH specifier
  * If network: <Host>:<Port>
  * If device: name of device
  * If DH push: name of 
* 

Free NAME:

Activate NAME:

Deactivate NAME:

:StatusDH NAME:       Return status for DH I. Information returned:

* Total number of bytes read

* Total number of bytes written

* Total number of I/O reads

* Total number of I/O writes

Command Interpreter (CI) Types
==============================
The Command Interpreter uses

.. code-block:: rust
    trait CI {
        read_oc(&mut [u8

Data Handler (DH) Types
=======================
There are several DH types. The individual DH types are derived from the
following trait:

.. code-block:: rust
    trait DH {
        fn name() -> &str;
        fn read_oc(&mut [u8], usize) -> Result<usize, TelenexError>;
        fn write_oc(&[u8], usize) -> Result<usize, TelenexError>;
        fn read_oc(&mut [u8], usize) -> Result<usize, TelenexError>;
        fn read_oc(&[u8], usize) -> Result<usize, TelenexError>;
        fn get_stats() -> TelenexStats;
        fn start() -> TelenexError;
        fn stop() -> TelenexError;
    }

    struct TelenexStats {
        bytes_read:     usize;
        bytes_written:  usize;
        io_reads:       usize;
        io_writes:      usize;
    };

The control interpreter passes two file descriptors to each DH as it is
starting up: a file descriptor to be used to write data to the OC and
another one used to wake up a DH when the command interpreter needs the
DH to do something. The file descriptor for payload communication is
opened by the appropriate DH. These two file descriptors are passed using:

.. code-block:: rust
    struct DHFds {
        oc_fd:      usize,
        ci_fd:      usize,
    }

Stream DHs
-----------
Buffers are managed with various types. Stream buffers are allocated once
when the DH is created:

.. code-block:: rust
    struct DHStreamBuffer {
       alloc_size:  usize,
       buf:         Vec<u8>,
    }

    impl DHStreamBuffer {
        fn new(alloc_size: usize) -> Result<DHBuffer, TelenexError> {
            let mut buf = Vec::new();
            buf.try_reserve(alloc_size).
                .map_error(|e| TelenexError::AllocFailed(e, alloc_size))?;
            DHBuffer {
                alloc_size,
                buf,
            }
        }
    }

Stream DHs use the following type to hold the DH name and statistics:

.. code-block:: rust
    struct StreamDH {
        name:        &str,
        stats:      TelenexStatus,
    }

File descriptor-based stream DHs need buffer. These are statically sized.
We try to fill the buffer entirely, but set a timer to indicate we should
send whatever is in the buffer if it isn't full.

.. code-block:: rust
   struct FdStreamDH {
        stream_dhu: StreamDH,
        max_time:   Time,
        buffer:     DHStreamBuffer;
        dhu_fds:    DHFds,
        payload_fd: i32,
    }

    impl FDStreamDH {
        fn new(name: &str, max_time: Time, DHStreamBuffer, dhu_fds: DHFds) -> Result<FDStreamDH>;
    }

Streams using socket interfaces use the following:

.. code-block:: rust
    struct SocketStreamDH {
        fd_stream_dhu:  FdStreamDH,
        address:        IPAddress,
    }

    impl SocketStreamDH {
        fn new(stream_dhu: &StreamDH, address: IPAddress) -> Result<SocketStreamDH, TelenexError>;
    }

Streams using device interfaces use the following:

.. code-block:: rust
    struct DeviceStreamDH {
        fd_stream_dhu:  FdStreamDH,
        path:           &str,
   }

FIXME: need to figure out how to propogate the CI pipe file descriptor,
especially since there may be multiple composites that lead to the same
DH.

DHs that are composited don't use file descriptors. Instead, they call the
various OC and payload read and write interfaces directly:

.. code-block:: rust

    struct CompositeDH {
        stream_dhu:     StreamDH,
        ci_fd:          FDESC,
    }

    impl FdCompositeDH {
        fn new(name: &str, ci_fd: FDESC, oc_dhu: &DH, payload_dhu: &DH) -> Result<FdStreamDH, TelenexError>;
    }

Datagram DHs
-------------
Datagram buffers can be allocated once, or reallocated when datagrams are too
large to read, depending on the alloc_ok flag:

.. code-block:: rust
    struct DHDatagramBuffer {
       buffer:      DHStreamBuffer,
       realloc_ok:  bool,
    }

    impl DHDatagramBuffer {
        fn new(alloc_size: usize, realloc_ok: bool, dhu_fds, address: IPAddress`) -> Result<DHBuffer, TelenexError> {
            let buffer = DHStreamBuffer::new(alloc_size)?;
            DHBuffer {
                buffer,
                realloc_ok,
            }
        }
    }

Datagram DHs are similar to stream DHs:

.. code-block:: rust
    struct DatagramDH {
        name:           &str,
        stats:          TelenexStatus,
        buffer:         DHDatagramBuffer,
        dhu_fds:        DHFds,
        payload_fd:     i32,
        address:        IPAddress,
    }

    impl DatagramDH {
        fn new(datagram_dhu: &DatagramDH, address: IPAddress, alloc_ok) -> Result<DatagramDH, TelenexError> {
        }
    }

    impl DH for DatagramDH {
    }
