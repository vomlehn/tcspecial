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

    i2c

        path = "device-path"
            The bus, such as /dev/i2c-2. Two handlers commonly name the same
            bus and differ only in the address below.

        address = <n>
            7-bit addressing: 0x08 through 0x77. 10-bit: 0x000 through 0x3FF.

        ten-bit = true | false

        pec = true | false

        retries = <n>

        timeout = <n milliseconds>

        bus-speed = <n>
            Recorded, not applied. See the I\ :superscript:`2`\ C section
            below.

    spi

        path = "device-path"
            The device node, such as /dev/spidev0.1, which names the bus and
            the chip select together.

        max-speed = <n bits per second>

        mode = 0 | 1 | 2 | 3

        bits-per-word = <n>

        bit-order = msb | lsb

        cs-active = low | high

Neither bus interface takes any of the Datagram or Raw attributes below. A
master clocks exactly as many bytes as it asks for, so a transfer is already
bounded and needs no rule for where a read ends.

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

    path = "device-path"

    address = <n>
        With 7-bit addressing, 0x08 through 0x77. The bus reserves 0x00
        through 0x07 and 0x78 through 0x7F, so no device can answer to one
        of those. With 10-bit addressing, 0x000 through 0x3FF, none reserved.

    ten-bit = true | false
        Use 10-bit slave addressing instead of 7-bit.

    pec = true | false
        SMBus packet error checking, a CRC-8 appended to each transfer.

    retries = <n>

    timeout = <n milliseconds>
        Rounded up to a multiple of 10 ms by the kernel.

    bus-speed = <n>
        Informational only. The bus clock rate belongs to the controller and
        is set by the platform, in the device tree or ACPI; it cannot be
        changed through /dev/i2c-N.

    There is no parity or byte length to configure. I\ :superscript:`2`\ C
    fixes the framing at eight data bits, most significant bit first,
    followed by an acknowledge bit.

SPI

    path = "device-path"

    max-speed = <n bits per second>

    mode = 0 | 1 | 2 | 3
        Clock polarity and phase, CPOL and CPHA. This must match the
        peripheral; a mismatch fails the way a wrong data rate fails on a
        serial port.

    bits-per-word = <n>
        Commonly 8, though a controller may support other widths.

    bit-order = msb | lsb

    cs-active = low | high

    There is no parity and there are no stop bits. SPI is clocked and
    full duplex, and a transfer is delimited by the chip select rather than
    by framing bits.

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
