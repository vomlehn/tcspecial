#!/usr/bin/env python3
"""A stand-in for the deck camera's software, for stages 9 and 10 of
"Building a Vehicle" (docs/building-a-vehicle.rst).

It is not a camera. A mission's camera comes with software of its own, and
an ``app_pl`` node is how tcspecial runs such software unchanged; this
program is there so that the stages run on a desk with no camera, and it is
as small as an app can be:

* It is run as ``camera.py <node name> <app-payload-arg>``, the argument
  being how many seconds apart its frames are.
* An ``app_pl`` writes its readings on descriptor 1, one end of a socket
  pair tcspecial made, and has no descriptor 0.
* What goes wrong is written on descriptor 2.
* Every interface carries packets: four bytes of length in this machine's
  own byte order, then that many bytes.

Each frame is one packet of FRAME_BYTES: the text ``frame`` and the frame's
number, then a pattern that changes with the number, where a real camera's
would be its image, compressed. The packet is made once, at the start, and
each frame rewrites it in place, as a flight program's loop reuses the
buffers its start made rather than making new ones.

It does not finish. A program that exited would end its interface, and the
vehicle would report every pass that nothing more will come from it, so it
goes on until tcspecial stops it.
"""

import os
import stat
import struct
import sys
import time

# The descriptors an app is run with.
PAYLOAD_INTERFACE = 0
GATEWAY_INTERFACE = 1
ERROR_INTERFACE = 2

# How many bytes a frame is: small, so that a desk run's link is not filled
# with made-up pictures.
FRAME_BYTES = 64

# Where in a frame its number is written, as text, after "frame ".
NUMBER_AT = 6
NUMBER_DIGITS = 8


def packet(data):
    """Makes one packet: four bytes of length, native byte order, then data.

    Args:
        data: The packet's bytes.

    Returns:
        The packet, as bytes.
    """
    return struct.pack("=I", len(data)) + data


def write_all(fd, data):
    """Writes every byte, however many writes that takes.

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
    """Says what went wrong, as a packet on the error interface.

    Unframed text would be read as a packet's length, so it is framed; where
    the error interface is not open there is nowhere to say it.

    Args:
        message: What went wrong, as text.
    """
    try:
        write_all(ERROR_INTERFACE, packet(message.encode("utf-8")))
    except OSError:
        pass


def is_handed_over(fd):
    """Says whether a descriptor is an interface tcspecial handed over.

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
    """Sends a frame every app-payload-arg seconds until stopped.

    Args:
        argv: The command line: the program, the node's name and
              app-payload-arg, the seconds between frames.

    Returns:
        The exit status: nought once nobody is reading any more, one for a
        failure, which has been said on the error interface.
    """
    if not is_handed_over(GATEWAY_INTERFACE) or is_handed_over(
            PAYLOAD_INTERFACE):
        complain("camera: run as an app_pl by tcspecial, which hands over "
                 "descriptor 1 and not descriptor 0")
        return 1
    if len(argv) != 3:
        complain("camera: run with 2 arguments, the node's name then "
                 "app-payload-arg, and given %d" % (len(argv) - 1))
        return 1
    name, payload_arg = argv[1], argv[2]
    try:
        every = float(payload_arg.strip())
    except ValueError:
        every = 0.0
    if every <= 0.0:
        complain('camera %s: app-payload-arg is "%s", and this app is run '
                 "with the seconds between frames, more than nought"
                 % (name, payload_arg))
        return 1

    # The one packet, rewritten for each frame.
    frame = bytearray(packet(bytes(FRAME_BYTES)))
    body = memoryview(frame)[4:]
    body[:NUMBER_AT] = b"frame "
    number = 0
    while True:
        text = b"%0*d" % (NUMBER_DIGITS, number % 10 ** NUMBER_DIGITS)
        body[NUMBER_AT:NUMBER_AT + NUMBER_DIGITS] = text
        for i in range(NUMBER_AT + NUMBER_DIGITS, FRAME_BYTES):
            body[i] = (i + number) & 0xFF
        try:
            write_all(GATEWAY_INTERFACE, frame)
        except BrokenPipeError:
            return 0
        except OSError as e:
            complain("camera %s: a frame could not be sent: %s" % (name, e))
            return 1
        number += 1
        time.sleep(every)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
