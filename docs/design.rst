=========================
Telenex (Telemetry Nexus)
=========================

.. contents:: Table of Contents
   :depth: 2
   :backlinks: entry

Telenex is a framework for passing commands to payload devices from
an operations center (OC) and relaying
telemetry`from the payloads to the OC. It is designed for extensible
protocol translation for both stream and datagram-oriented operation.
Telenex has a library for linking with other OC software and multi-threaded
process that runs on the system containing payloads.

Telenex is designed so that it can allocate all resources (buffers, thread
data, etc.) before entering its main loop, though it can also allocate
and free resources afterwards, if needed.

Telenex System Software
=======================

Telenex itself is a command interpreter (CI) running on the spacecraft where the
payloads are located. CI has one or more threads to handle OC communications. It
defaults to using datagram communication to the OC, though this is usually actually
a link to the spacecraft radio. The radio may use a different protocol.

Telenex also has threads associated with each data handler (DH). Each DH
communicates to payloads via one bi-direction channel. A key feature of Telenex is that
each DH may use a different protocol to communicate with its payload. This includes
not only the core communication protocols supported by the operating system, such
as stream or datagram protocols, or serial or parallel interfaces,
but also stackable DH protocols that can be
employed to build custom protocol stacks.

In addition to Telenex, there is also Telenexlib, which is a library used in the
OC for building control applications. For testing purposes, Telenexgui uses
Telenexlib, along with simulated payloads, to support a simple graphical user interface.

Resource Allocation
===================

In an ideal world, resources would be allocated at link time (really, process
load time). From a practical standpoint, however, the constraint of
pre-execution allocation is not possible to meet for verious reasons. For
example, configuration-dependent code may require runtime allocations. The
constraint Telenex is intend to obey is to have all allocations done before
entering the main loop.

This approach allows TeleNex resources to be allocated statically, but
the underlying operating system may use dynamic resource allocation. These
may fail, so the CI and DH code must be prepared to handle failures in
in the operating system and retry at intervals if the various protocols do
not already support this.

Interrupting I/O
================
It is a requirement that the CI be able to order DHs to perform various operations, such
as sending statistics and shutting down, even if the DH has I/O to perform. This
is done by passing a pipe file descriptor and writing a byte to it to indicate
the DH has something to do. The sequence is:

#. Set up DH I/O parameters for DH I/O file descriptor
#. Enter a loop
   #. Perform file descriptor "wait for I/O ready" operations, such as
      select(), epoll(), etc. The mio crate might also be useful.
   #. When the "wait for I/O ready" operation completes:
      #. If the pipe file descriptor is ready:
         
      uu#. Read one byte from the pipe file descriptor.
         #. Call a CI function to perform the desired function. This may return
            a value indicated the DH should exit its threads.
      #. Else, if the DH I/O file descriptor is ready:
         #. Exit the loop
#. Perform the DH I/O file descriptor operation, reading any pending data in a
   a non-blocking mode, or writing the whole output buffer.

Statically vs. Dynamically Sized I/O Buffers
============================================
I/O buffers may have statically or dynamically determined sizes. They are
implementations of the following trait:

.. code-block:: rust
   trait BufferBase {
       fn buffer() -> &mut[u8],
       fn len() -> usize,
       fn size() -> usize,
       fn grow(desired_size: usize) -> Result<&mut[u8], TelenexError>,
       fn max_size() -> usize,
   }

The functions are:

:buffer():
Pointer to the buffer

:len():
Number of valid bytes in the buffer

:size():
Number of bytes that is allocated for the buffer

:grow(desired_size):
Changes the number of bytes allocated for the buffer to the desired size

:max_size:
Limit to the number of bytes that can be allocated for the buffer.  Clearly,
size() <= max_size()

Statically Sized Buffers
------------------------
Stream buffers are always of a fixed size, whereas datagram buffers may be
either statically or dynamically sized. They look like:

.. code-block:: rust

   struct BufferStaticSized {
      p:      &mut[u8],
      size:   usize,
      len:    usize,
   }

   impl BufferStaticSized {
       fn new(max_size: usize) -> Result<BufferStaticSized, TelenextError> {
           BufferStaticSized {
               p:       &[max_size; u8],
               size:    max_size,
               len:     0,
           }
       }
   }

   impl BufferBase for BufferStaticSized {
       fn buffer(&self) -> &mut[u8] { self.p }
       fn len(&self) -> usize { self.len }
       fn size(&self) -> usize { self.size }
       fn grow(&self, desired_size: usize) Result<(), TelenexError> {
           self.desired_size <= self.size { Ok(self.buffer()) }
           else { Err(TelenexError::NoMem) }
       }
       fn max_size(&self) -> usize { self.size() }
   }

Note that grow does not reallocate memory if the buffer is already big enough,
and it returns an error if it isn't. Thus, all I/O to the buffer will be limited
in size. Statically sized buffers don't do memory allocations after they are created
and so are more performant.

Dynamically Sized Buffers
-------------------------
Unlike buffers used for stream I/O, dynamically sized buffers may be used for datagrams.
In this case, the usual recv() operation is performed only after a recv()
operation with the MSG_PEEK and MSG_TRUNC options indicate that more data is
available than will fit in the buffer. The buffer is then grown, up to the maximum
allowed size, and recv() used to read the entire datagram.

.. code-block:: rust

   struct DatagramBuffer {
      v:    Vec<u8>,
      max:  usize,
      size: usize,
      len:  usize,
   }

   impl DatagramBuffer {
       fn new(max_size: usize) -> Result<DatagramBuffer, TelenextError> {
           DatagramBuffer {
               p:       Vec<u8>::new(),
               max:     max_size,
               size:    max_size,
               len:     0,
           }
       }
   }

   impl BufferBase for DatagramBuffer {
       fn buffer<'a>(&self) -> &'a mut[u8] { self.p.as_mut_slice() }
       fn len(&self) -> usize { self.len }
       fn size(&self) -> usize { self.size }
       fn grow(&self, desired_size: usize) Result<(), TelenexError> {
           self.desired_size <= self.size { Ok(self.buffer) }
           else {
               match self.p.try_reserve_exact(size) {
                   Err(e) => Err(TelenexError::ReserveFailed(e)),
                   Ok(()) => {
                       self.p.resize(desired_size, 0),
                       Ok(self.buffer()),
                   }
               }
           }
       }
       fn max_size(&self) -> usize { self.max() }
   }

Note that grow does not reallocate memory if the buffer is already big enough and
doesn't free it if there is unused space.

Command Interpreter (CI)
========================

The payload system
software has a command interpreter with two threads. The threads manage
commands from the OC and status messages to the OC. I/O is done
with datagrams. Status messages are queued with a fixed-length queue.

Ground/Space Link
-----------------
The connection between the OC and the CI is usually implemented with a UDP/IP
datagram since it is generally the ground/space link, for which TCP/IP
is unsuitable beyond MEO. However, TCP/IP may be suitable if the link is
indirect, that is, to the radio, or for LEO and MEO orbits.

The CI has a Mutex<BTreeMap<<DH>>> which holds all of the allocated DHs. The
use of a Mutex allows status of all DHs to be determined atomically.

Initialization
--------------
When the CI starts up, it will allocate all resources, including threads
and communication links. It then enters the main loop.

Main Loop
---------
The main CI loop simply reads and processes command from the OC, along with
periodic sending Beacon Telemetry. The Exit command causes the CI to exit.

Shut Down
---------
During CI shutdown, all DHs are also shut down.

Data Handlers (DHs)
===================
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
--------------
When the CI Allocate command is given, a DH is sets up all require resources
and then waits for a DH Activate command.

Main Loop
---------
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
   :dedent: 4

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

In another case, the payload may write a stream but a DH could be composited
to packetize the data, say, by adding a byte count before the data.

.. code-block:: text
   :dedent: 4

         ______     ___________________     ___________________     _________
        |      |   |                   |   |                   |   |         |
        |      |-->| OC        OC      |-->| OC        OC      |-->|         |
        |      |   | read      write   |   | read      write   |   |         |
        |      |   |                   |   |                   |   |         |
        |  OC  |   |        DH1        |   | DH2 (Packetizer)  |   | Payload |
        |      |   |                   |   |                   |   |         |
        |      |<--| Payload   Payload |<--| Payload   Payload |<--|         |
        |      |   | write     read    |   | write     read    |   |         |
        |______|   |___________________|   |___________________|   |_________|


A call to oc_write() in DH1 then becomes a call to oc_read() in DH2 and
a call to payload_write() in DH2 becomes a call to payload_read() in DH1

Any number of DHs may be composited to create complex protocol
stacks. A stacking DH must be added to the payload side of another DH.

Before the read and write interfaces to a non-compositing DHs are called,
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

Payload data can be sent by the payload as datagrams or as a stream. Datagrams
may be a fixed maximum size or a dynamic size. Use of dynamic message sizes
uses the MSG_PEEK and MSG_TRUC options for recv(). Since it may cause memory
allocaton operations, it should not be used in applications which require
that no allocations be done after initialization.

When data is being read as a stream, there are two options:

:Send any data: All pending data on the file descriptor will
                be read when there is at least one byte pending.

:Wait for full: Wait for the buffer to fill up. A timer is set so that, if
                the buffer does not fill up, all data in the input buffer
                is sent

DH Links
========
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

Core DH Types
-------------

Core Link Types
---------------
Core link types are those provided by the operating system. For Linux-based
systems, these are network link types and device link types.

Network Link Types
^^^^^^^^^^^^^^^^^^
The following table lists network protocols supported on Linux-based systems. The
protocols below are generally available, see the man page for socket(2) and
other, associated, documentation. The protocol family, socket type, and protocol
information are those provided to the socket(2) system call.

Protocol support may require configuring the Linux kernel
to include protocol drivers. Of course, the hardware supporting the protocol
must also be present. For more information on each of the address families, consult
the Linux man page address_families(7).

+------------+--------------+----------------+-----------+-----------+
| Interface  | Protocol     | Socket type    | Stream or | Protocol? |
|            | Family       |                | Datagram  |           |
+------------+--------------+----------------+-----------+-----------+
| Networking | AF_UNIX      | SOCK_STREAM    | stream    | 0         |
|            |    or        +----------------+-----------+-----------+
|            | AF_LOCAL     | SOCK_DGRAM     | datagram  | 0         |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_SEQPACKET | datagram  | 0         |
|            +--------------+----------------+-----------+-----------+
|            | AF_INET      | SOCK_STREAM    | stream    | 0         |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_DGRAM     | datagram  | 0         |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_RAW       | datagram  | yes       |
|            +--------------+----------------+-----------+-----------+
|            | AF_AX25      | TBD            | TBD       | TBD       |
|            +--------------+----------------+-----------+-----------+
|            | AF_IPX       | TBD            | TBD       | TBD       |
|            +--------------+----------------+-----------+-----------+
|            | AF_APPLETALK | SOCK_DGRAM     | datagram  | yes       |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_RAW       | datagram  | yes       |
|            +--------------+----------------+-----------+-----------+
|            | AF_X25       | SOCK_SEQPACKET | datagram  | 0         |
|            +--------------+----------------+-----------+-----------+
|            | AF_INET6     | SOCK_STREAM    | stream    | yes       |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_DGRAM     | datagram  | yes       |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_RAW       | datagram  | yes       |
|            +--------------+----------------+-----------+-----------+
|            | AF_DECnet    | TBD            | TBD       | TBD       |
|            +--------------+----------------+-----------+-----------+
|            | AF_KEY       | TBD            | TBD       | TBD       |
|            +--------------+----------------+-----------+-----------+
|            | AF_NETLINK   | SOCK_DGRAM     | datagram  | yes       |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_RAW       | datagram  | yes       |
|            +--------------+----------------+-----------+-----------+
|            | AF_PACKET    | SOCK_DGRAM     | datagram  | yes       |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_RAW       | datagram  | yes       |
|            +--------------+----------------+-----------+-----------+
|            | AF_RDS       | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_PPPOX     | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_LLC       | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_IB        | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_MPLS      | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_CAN       | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_TIPC      | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_BLUETOOTH | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_ALG       | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_VSOCK     | SOCK_DGRAM     | datagram  | yes       |
|            |              +----------------+-----------+-----------+
|            |              | SOCK_RAW       | datagram  | yes       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_KCM       | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+
|            | AF_XDP       | TBD            | TBD       | TBD       |
+------------+--------------+----------------+-----------+-----------+

Device Link Types
^^^^^^^^^^^^^^^^^
Device communication links use direct file interfaces. These can be things like:

* RS-422
* I2C
* SPI

or many others. They are specified by supplying a path name and are all stream
interfaces

Provided Stacked DHs
==≈=≈=============
TeleNex offers several several stacked DHs that can be used as-is and as examples.

Packetizing U32 Streaming Data
--------‐--‐----------------------------------------
The payload is sending big-endian u32 data items every three seconds. The DH accumulates up to four of these. If the bytes have a time interval > between 100 mS, that data item is discarded. The resulting packet has a leading byte with a bit set for each valid data item, followed by valid data item values.

Byte Swapping Values
------------------------------------
This DH is composited with the DH that packetizes streaming data. It converts the big-endian u32s to little-endian u32s.

Packetizing Arbitrary Streaming Data With Constant Overhead Byte Stuffing
‐-----‐------------------------------------------------------------------------------------------------------------------
Read 254  * 4 bytes from the payload, write to a buffer with COBS, stick a u16 header which is a byte count and send to the OC.
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
All commands start with a CmdSN indicating the command serial number. This
value is initially 1 and increments for each command sent.
If the OC software fails, it will
lose the CmdSN. Thus, it starts by sending an Exit command until it receives
a sucessful ExitStatus message back. (FIXME: check that this works)

:Config:    Set CI configuration:
* Set Beacon interval. It is not possible to disable the beacon entirely,
  but it can be set to a very long value.

:Exit:      Shut down Telenex, including all DHs.

:Ping:      Request status from the CI. This returns the 

:SendMap:   Send a vector of DH states:

Telemetry
^^^^^^^^^
Telemetry messages are of two types. One is a status reply to a command,
the other is generated autonomously.

Command Status
""""""""""""""

:ConfigStatus:

:ExitStatus:

:InitStatus:

:PintStatus:

:SendMapStatus:

:ShutdownStatus:

Autonmously Generated Messages
""""""""""""""""""""""""""""""

:Beacon:    This telemetry is sent automatically. It consists of a vector
            with elements consisting of:
* DH name: Name of the DH
* OC Data received: bool--0 if nothing was received from the OC by this DH
  since the last beacon was sent for this DH, 1 if something was received
* Payload Data received: bool--0 if nothing was received from the payload
  since the last beacon was sent for this DH, 1 if something was received

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

Common CI and DH Types
======================
I/O statistics are maintained for each DH and for the CI. Statistics are
are zeroed with the respective Allocate commands and not reset afterwards.

.. code-block:: rust

   type IoCount = u32;
   type CmdSN = u32;
   type DHName = [u8; 32];

    struct IoStats {
        bytes_read:     IoCount;
        bytes_written:  IoCount;
        io_reads:       IoCount;
        io_writes:      IoCount;
    };

Command Interpreter (CI) Types
==============================
The Command Interpreter uses

.. code-block:: rust

    trait CIBase {
        fn read_oc(&mut [u8], n: usize) -> Result<usize>,
        fn write_oc(&[u8], n: usize) -> Result<usize>,
        fn cmd_cn() -> CmdSN;
        fn do_cmd(&[u8]
    }

    struct CI {
        stats:  ItStats,
        cmd_sn: CmdSN,
        fd;     FDESC,
        dh:     Vec<DH>,




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

DHs are supplied with configuration information as follows:
`
.. code-block:: rust

    use socket2::{Socket, Domain, Type, Protocol};
    use std::net::SocketAddr;

    trait DH {
        fn oc_read(&mut [u8], usize) -> Result<usize, TelenexError>;
        fn oc_write(&[u8], usize) -> Result<usize, TelenexError>;
        fn payload_read(&mut [u8], usize) -> Result<usize, TelenexError>;
        fn payload_write(&[u8], usize) -> Result<usize, TelenexError>;
    }

    enum Endpoint {
        Socket(domain: Domain, type: Type, protocol: Option<Protocol>, addr: &str) ->
            Result<Endpoint, TelenexError>;
        Device(path: &str) -> Result<Endpoint, TelenexError>
        Stacked(dh_name, &str) -> Result<Endpoint, TelenexError>;
    }

The control interpreter passes two file descriptors to each DH as it is
starting up: a file descriptor to be used to write data to the OC and
another one used to wake up a DH when the command interpreter needs the
DH to do something. The file descriptor for payload communication is
opened by the appropriate DH. These two file descriptors are passed using:

.. code-block:: rust
    struct DHFds {
        oc_fd:      FDESC,
        ci_fd:      FDESC,
    }

Stream DHs
-----------
Buffers are managed with various types. Stream buffers are allocated once
when the DH is created:

.. code-block:: rust
    struct DHBufferStaticSized {
       alloc_size:  usize,
       buf:         Vec<u8>,
    }

    impl DHBufferStaticSized {
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
        buffer:     DHBufferStaticSized;
        dhu_fds:    DHFds,
        payload_fd: i32,
    }

    impl FDStreamDH {
        fn new(name: &str, max_time: Time, DHBufferStaticSized, dhu_fds: DHFds) -> Result<FDStreamDH>;
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
       buffer:      DHBufferStaticSized,
       realloc_ok:  bool,
    }

    impl DHDatagramBuffer {
        fn new(alloc_size: usize, realloc_ok: bool, dhu_fds, address: IPAddress) -> Result<DHBuffer, TelenexError> {
            let buffer = DHBufferStaticSized::new(alloc_size)?;
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

Testing
=======
The telenexcmd application is a GUI program used to control simulated payloads
interacting with telenex using
the telenexlib library over a datagram connection to telenex. It has a section at the
top of its single window that allows issuing of CI commands and viewing responses.
The rest of the window
is devoted to eight rectanges for issuing of commands to one of up to eight
DHs and view their responses. The DHs are named dh0 through dh7.

Telenex and telenexcmd are started asynchronously.
Closing the window causes telenexcmd to terminate immediately.

Possible Enhancements
=====================

:Build-time selection of CI and DH protocols to OC/spacecraft radio:

Though most or all of on-board spacecraft protocols are based on datagrams, use
of error correcting stream-based protocols are also a reasonable choice for this
purpose. Build-time selection of these seems useful.
