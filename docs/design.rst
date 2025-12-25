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

Command Interpreter
-------------------

The payload system
software has a command interpreter with two threads. The threads manage
commands from the OC and status messages to the OC. I/O is done
with datagrams. Status messages are queued with a fix.

Data Handling Units
-------------------

Data Handling Units (DHUs) receive a command from the OC via a Command Origin.
They respond to commands and send synchronous and asynchronous telemetry
by sending it to a Command Destination, which results in it being sent
to the OC.

Similarly, command responses, and synchronous and asynchronous telemetry
comes from a payload via a Telemetry Origin. Data sent to a payload goes
through a Telemetry Destination.

Command Origins and Destinations, and Telemetry Origins and Destinations,
may be either connections to other DHUs, which allows compositing protocols,
or file descriptors. In the case of DHU connections, sending and receving
data involves a call to the DHU on the other side of the connection. This
will resolve to a DHU with a file descriptor.

Each DHU is associated with a pipe, from which it reads one byte. This pipe
is written to by the command interpreter when it wants to wake up the DHU
to shut the threads down or perform another action. Each DHU thus has one
file descriptor for reading and writing data to the OC, one file descriptor
for reading and writing payload data, and a third file descriptor for the
pipe written by the command interpreter. Waiting for asynchronous I/O
is done using the mio crate.

Each static DHU is assign static address information. This can be a path to a
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

Global Control and Status
-------------------------
:Init:              Go through initialization and bring up all static DHUs

:Shutdown:          Disconnect all static and dynamic DHUs

:Map:               Send a vector of DHU statues: inactive static, active
                    static, active dynamic

Per-DHU Control and Status
--------------------------
:StartStatic S:     Start static DHU S. Configuration data included.  S is a
                    value from 1 to n, where n is the number of static DHUs.

:StopStatic S:      Stop static DHU S. This must be between 1 and n.

:StartDynamic D:    Start dynamic DHU D. Configuration data included.  A DHU
                    identifer D is supplied in the command. It must not be in
                    use and be >n.

:StopDynamic D:     Stop dynamic DHU D, where D is >n and is in use.

:StatusDHU I:       Return status for DHU I. Information returned:

* Total number of bytes read

* Total number of bytes written

* Total number of I/O reads

* Total number of I/O writes
