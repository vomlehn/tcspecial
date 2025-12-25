#Telenex (Telemetry Nexus)

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

##Payload System Software
Payload system software consists of a command interpreter and some number
of data handling units.

###Command Interpreter
The payload system
software has a command interpreter with two threads. The threads manage
commands from the OC and status messages to the OC. I/O is done
with datagrams. Status messages are queued with a fix

###Data Handling Units
Payload system software also has data handling units (DHU) which consist of a
thread handling data from OC to a payload and another thread operating in
the opposite direction. OC data is read and written with the same file
descriptor and data to and from the payload is written on the same
file descriptor.

Each DHU I/O operation is performed with a select()-like
operation that waits for a read or write file descriptor operation
to be possible. The select-like() operation also waits on another
file descriptor, which is written by the 

##Telenex Library
