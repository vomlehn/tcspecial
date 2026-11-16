#!/usr/bin/env python3
"""The consolidator example's battery age: how many seconds since the
battery last reported, sent once a second.

The consolidator sends each input's latest reading, held until another comes,
and says nothing of how old it is: a battery that stopped reporting an hour
ago looks, in the datum, just like one that reported a moment ago. What a
mission wants said about its readings is a member of its own, and this is one:
an ``app_all_veh`` node, linked from the battery and into the consolidator as
any input is, so that the battery's age goes down beside its charge.

It is a program rather than a device. tcspecial starts it from the
configuration and talks to it over the file interfaces an app node is run
with -- see ``tcsapp`` for the whole of that interface, which this program
speaks without the Rust library that describes it:

* It is run as ``battery_age.py <node name> <app-payload-arg>
  <app-gateway-arg>``.
* An ``app_all_veh`` reads payload data -- here, the battery's readings -- on
  descriptor 0, and writes payload data, groundward, on descriptor 1, one end
  of each of two socket pairs tcspecial made.
* What goes wrong is written on descriptor 2.
* Every interface carries packets: four bytes of length in this machine's own
  byte order, then that many bytes.

What it sends is a ``u16``, most significant byte first, as a link carries
one: the whole seconds since the last reading, up to 65534, and 65535 until
the first, which the configuration gives as its default too, so "never read"
is one value whether the program has spoken yet or not. ``app-payload-arg``
is how many seconds between reports. Nothing is linked to its ``moc-in``, so
``app-gateway-arg``, which every ``app_all_veh`` states, says nothing here.

It does not finish while the battery's interface is open: it reports on its
own clock, the battery or no battery, which is the point of it.
"""

import os
import select
import stat
import struct
import sys
import time

# The descriptors an app is run with.
PAYLOAD_INTERFACE = 0
GATEWAY_INTERFACE = 1
ERROR_INTERFACE = 2

# The age sent before the battery has reported: the most a u16 holds, a value
# no age reaches, as the consolidator's default for the input is.
NEVER = 0xFFFF

# The most an age is: one short of NEVER, so that a battery quiet for over
# eighteen hours still reads as old rather than as never read.
OLDEST = NEVER - 1

# A packet's length: four bytes, in this machine's own byte order.
LENGTH = struct.Struct("=I")


def packet(data):
    """One packet: four bytes of length in native byte order, then the data.

    Args:
        data: What the packet holds.

    Returns:
        The packet's bytes.
    """
    return LENGTH.pack(len(data)) + data


def write_all(fd, data):
    """Write every byte, however many writes that takes.

    Args:
        fd:   The descriptor to write to.
        data: The bytes to write.

    Raises:
        OSError: A write failed.
    """
    view = memoryview(data)
    while view:
        written = os.write(fd, view)
        view = view[written:]


def read_exactly(fd, count):
    """Read exactly ``count`` bytes, however many reads that takes.

    Args:
        fd:    The descriptor to read from.
        count: How many bytes.

    Returns:
        The bytes, or ``None`` where the interface ended first.

    Raises:
        OSError: A read failed.
    """
    got = b""
    while len(got) < count:
        more = os.read(fd, count - len(got))
        if not more:
            return None
        got += more
    return got


def read_packet(fd):
    """Read one packet.

    Args:
        fd: The descriptor to read from.

    Returns:
        What the packet holds, or ``None`` where the interface ended.

    Raises:
        OSError: A read failed.
    """
    head = read_exactly(fd, LENGTH.size)
    if head is None:
        return None
    (length,) = LENGTH.unpack(head)
    return read_exactly(fd, length)


def complain(message):
    """Say what went wrong, as a packet on the error interface.

    Unframed text would be worse than silence: its first four bytes would be
    read as a packet's length. Where the error interface is not open there is
    nowhere to say it, and nothing is said.

    Args:
        message: What went wrong, as text.
    """
    try:
        write_all(ERROR_INTERFACE, packet(message.encode("utf-8")))
    except OSError:
        pass


def is_handed_over(fd):
    """Whether this descriptor is an interface tcspecial handed over: a socket.

    Args:
        fd: The descriptor.

    Returns:
        ``True`` if it is an open socket.
    """
    try:
        return stat.S_ISSOCK(os.fstat(fd).st_mode)
    except OSError:
        return False


def age(last, now):
    """The age to send.

    Args:
        last: When the battery last reported, by the monotonic clock, or
              ``None`` where it never has.
        now:  The time now, by the same clock.

    Returns:
        The age as a link carries a u16: NEVER where the battery has not
        reported, and otherwise the whole seconds since it did, at most
        OLDEST.
    """
    if last is None:
        seconds = NEVER
    else:
        seconds = min(int(now - last), OLDEST)
    return struct.pack(">H", seconds)


def main(argv):
    """Send the battery's age every ``app-payload-arg`` seconds, taking each
    of its readings as it arrives, until its interface ends.

    Args:
        argv: The command line: the program, the node's name,
              app-payload-arg, the seconds between reports, and
              app-gateway-arg, which says nothing here.

    Returns:
        The exit status: nought when the battery's interface ends, one for a
        failure, which has been said on the error interface.
    """
    # Run the way an app_all_veh is run: both interfaces handed over. Said at
    # once rather than discovered as a read of a terminal.
    for fd, which in ((PAYLOAD_INTERFACE, "payload"),
                      (GATEWAY_INTERFACE, "gateway")):
        if not is_handed_over(fd):
            complain(
                "battery_age: the %s interface was not handed over at "
                "descriptor %d: this is an app_all_veh, run by tcspecial, "
                "which opens both" % (which, fd)
            )
            return 1
    if len(argv) != 4:
        complain(
            "battery_age: this app is run with 3 arguments, the node's name, "
            "app-payload-arg and app-gateway-arg, and it was given %d: the "
            "list is fixed by the kind of app it is" % (len(argv) - 1)
        )
        return 1
    name, payload_arg = argv[1], argv[2]
    try:
        every = float(payload_arg.strip())
    except ValueError:
        every = 0.0
    if every <= 0.0:
        complain(
            'battery_age %s: app-payload-arg is "%s", and it is the seconds '
            "between reports, more than nought" % (name, payload_arg)
        )
        return 1

    last = None
    due = time.monotonic()
    while True:
        now = time.monotonic()
        if now >= due:
            try:
                write_all(GATEWAY_INTERFACE, packet(age(last, now)))
            except OSError as why:
                complain("battery_age %s: the age could not be sent: %s"
                         % (name, why))
                return 1
            # From when it was due, so the reports do not drift; one so far
            # behind that the next is already past is put an interval from
            # now.
            due += every
            if due <= now:
                due = now + every
            continue
        # A reading, or the next report, whichever comes first.
        ready, _, _ = select.select([PAYLOAD_INTERFACE], [], [], due - now)
        if not ready:
            continue
        try:
            reading = read_packet(PAYLOAD_INTERFACE)
        except OSError as why:
            complain("battery_age %s: a reading could not be read: %s"
                     % (name, why))
            return 1
        if reading is None:
            return 0
        last = time.monotonic()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
