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

Data Handlers (DHs)
===================
Data handlers are responsible for passing data from the payload to the MOC
and, if appropriate, in the reverse direction, as well. Data from the MOC
to the payload is simply passed through, but the variety of payload
interfaces requires supporting a number of interfaces to payloads. The
following table details these

All

    name = <name>

    id = <id>

    bidirection = true | false

Networking-Related

    Networking

        address = "<node>:<port>"

        address_family = AF_<af>

        type = SOCK_<type>

        protocol = IPPROTO_<proto>
            See RFC 1700 for SOCK_RAW

    tty

        path = "device-path"

        data-rate = <n>

        parity = even | odd | none

        bits-per-byte = <n>

Datagram

    Fixed length
        max-length = <n-bytes>

    Terminated
        terminator = "string"

    Time-terminated
        time-terminator = <n nanoseconds>

    Counted
        length-length = 1 | 2

 Raw
    max_interbyte_interval
    max_interval = <interval>


Network

UDP/IP

TCP/IP

tty

I\ :superscript:`2`\ C

SPI

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
