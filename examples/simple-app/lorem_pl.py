#!/usr/bin/env python3
"""The simple-app example's payload: a program that sends a line of text, a
word at a time.

An ``app_pl`` node, which is a payload that is a program and is never asked:
nothing is linked into it, so nothing can ask it for a reading, and what it
reports and when is its own business. tcspecial starts it from the
configuration and talks to it over the file interfaces an app node is run
with -- see ``tcsapp`` for the whole of that interface, which this program
speaks without the Rust library that describes it:

* It is run as ``lorem_pl.py <node name> <app-payload-arg>``.
* An ``app_pl`` writes its readings on descriptor 1, one end of a socket pair
  tcspecial made, and has no descriptor 0.
* What goes wrong is written on descriptor 2.
* Every interface carries packets: four bytes of length in this machine's own
  byte order, then that many bytes.

What it reports is one word per packet, from the line its node names, and
then a packet holding nothing but the line terminator. The words are separate
data all the way to the ground, where ``lorem_moc`` writes them out with one
space between them.

Which line is ``app-payload-arg``, counting from one. The example's
configuration says ``1``, so it sends the first line.

The first word waits a little. The vehicle sends each word to the ground
station's address the moment it arrives, and a datagram sent to a port nobody
has bound yet is lost without a word. The pause gives ``lorem_moc`` time to
start listening, so the two can be started together in either order.

It does not finish when it has nothing left to say. A program that exited
would end its interface, and the vehicle would report every pass that nothing
more will come from it. So it waits where it is, the way a sensor with nothing
to report sits on its bus, until tcspecial stops.

The text is the example's own copy of the one in ``src/lib.rs``, which the
ground's program and the tests read. The tests run this program and compare
what it sends with that copy, so the two cannot drift apart unnoticed.
"""

import os
import stat
import struct
import sys
import time

# The first paragraph of the lorem ipsum text, one list per line, each the
# words on that line. No word holds a space: the ground puts a line back
# together by writing one space between them.
PARAGRAPH = [
    ["Lorem", "ipsum", "dolor", "sit", "amet,", "consectetur", "adipiscing",
     "elit,", "sed", "do"],
    ["eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
     "magna", "aliqua."],
    ["Ut", "enim", "ad", "minim", "veniam,", "quis", "nostrud",
     "exercitation", "ullamco"],
    ["laboris", "nisi", "ut", "aliquip", "ex", "ea", "commodo", "consequat.",
     "Duis"],
    ["aute", "irure", "dolor", "in", "reprehenderit", "in", "voluptate",
     "velit"],
    ["esse", "cillum", "dolore", "eu", "fugiat", "nulla", "pariatur.",
     "Excepteur"],
    ["sint", "occaecat", "cupidatat", "non", "proident,", "sunt", "in", "culpa",
     "qui"],
    ["officia", "deserunt", "mollit", "anim", "id", "est", "laborum."],
]

# What ends a line, as a packet of its own. The same as simple_app::TERMINATOR,
# which is what the ground compares each packet with.
TERMINATOR = b"\n"

# The descriptors an app is run with.
PAYLOAD_INTERFACE = 0
MOC_INTERFACE = 1
ERROR_INTERFACE = 2

# How long between one word and the next: slow enough that the line arrives
# visibly a word at a time, fast enough that a reader does not go away.
A_WORD_EVERY = 0.25

# How long before the first word: time for the ground station to be
# listening, started at the same moment as the vehicle.
BEFORE_THE_FIRST_WORD = 2.0


def packet(data):
    """One packet: four bytes of length in native byte order, then the data."""
    return struct.pack("=I", len(data)) + data


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


def complain(message):
    """Say what went wrong, as a packet on the error interface.

    Unframed text would be worse than silence: its first four bytes would be
    read as a packet's length, and tcspecial would wait for bytes that are not
    coming. Where the error interface is not open there is nowhere to say it,
    and nothing is said.

    Args:
        message: What went wrong, as text.
    """
    try:
        write_all(ERROR_INTERFACE, packet(message.encode("utf-8")))
    except OSError:
        pass


def is_handed_over(fd):
    """Whether this descriptor is an interface tcspecial handed over: a socket.

    Asked of what the descriptor is rather than whether it is open, because an
    interface that is read and written cannot be a pipe or a terminal.

    Args:
        fd: The descriptor.

    Returns:
        ``True`` if it is an open socket.
    """
    try:
        return stat.S_ISSOCK(os.fstat(fd).st_mode)
    except OSError:
        return False


def main(argv):
    """Send the line of the paragraph the node asks for, a word to a packet,
    on the MOC interface, then wait to be stopped.

    Args:
        argv: The command line: the program, the node's name and
              app-payload-arg, the number of the line to send.

    Returns:
        The exit status: nought once nobody is reading any more, one for a
        failure, which has been said on the error interface.
    """
    # Run the way an app_pl is run: the MOC interface handed over, the payload
    # interface not. Said at once rather than discovered as a write that goes
    # wherever the descriptor happens to go.
    if not is_handed_over(MOC_INTERFACE):
        complain(
            "lorem_pl: the MOC interface was not handed over, and this app "
            "writes payload data on it: it is run by tcspecial, which opens "
            "descriptor 1 for it as one end of a socket pair"
        )
        return 1
    if is_handed_over(PAYLOAD_INTERFACE):
        complain(
            "lorem_pl: the payload interface was handed over at descriptor 0, "
            "and this app does not support it: it is an app_pl, which reads "
            "nothing"
        )
        return 1

    if len(argv) != 3:
        complain(
            "lorem_pl: this app is run with 2 arguments, the node's name then "
            "app-payload-arg, and it was given %d: the list is fixed by the "
            "kind of app it is" % (len(argv) - 1)
        )
        return 1
    name, payload_arg = argv[1], argv[2]

    # Which line, as the node says. Refused rather than taken as the first
    # line: a node that asked for line "two" has said something nobody read.
    try:
        number = int(payload_arg.strip())
    except ValueError:
        complain(
            'lorem_pl %s: app-payload-arg is "%s", and this payload is run '
            "with the number of the line it sends, counting from one"
            % (name, payload_arg)
        )
        return 1
    if not 1 <= number <= len(PARAGRAPH):
        complain(
            "lorem_pl %s: there is no line %d: the paragraph has %d"
            % (name, number, len(PARAGRAPH))
        )
        return 1

    # The words, then what ends the line, each a packet of its own.
    time.sleep(BEFORE_THE_FIRST_WORD)
    for word in (
        [w.encode("utf-8") for w in PARAGRAPH[number - 1]] + [TERMINATOR]
    ):
        try:
            write_all(MOC_INTERFACE, packet(word))
        except BrokenPipeError:
            # Nobody is reading any more: the vehicle has finished with this
            # payload, which is an end and not a fault.
            return 0
        except OSError as e:
            complain(
                'lorem_pl %s: "%s" could not be written: %s'
                % (name, word.decode("utf-8"), e)
            )
            return 1
        time.sleep(A_WORD_EVERY)

    # Said everything, and still here: see the note at the top.
    while True:
        time.sleep(60)


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except KeyboardInterrupt:
        sys.exit(0)
