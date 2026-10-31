"""A ground station that only listens, and the CONNECT it sends all the same.

The Python spelling of ``tcslib/src/listening.rs``. A vehicle sends nothing
down but its beacons and the answers to its commands until it takes a
CONNECT carrying the checksum of the configuration it runs, so a station
that only reads the vehicle's data receives nothing unless somebody has sent
one. :class:`Listening` sends CONNECT with its configuration's checksum on
the spacecraft command link, from ``tcs_cmd.send`` to ``tcs_cmd.recv``, and
again each time no answer comes in time or the answer carries another
checksum, until the vehicle answers with this one. The CONNECT and its
answer are packed and unpacked by :mod:`tcslibgs.fixed`, from the XTCE of
the fixed commands.

It commands nothing else: a CONNECT changes nothing aboard but the beacons,
the downlink and which commands are taken, so a repeat is safe.
"""

import socket
import time

from tcslibgs import fixed
from tcslibgs.configuration import came_from

#: How long each CONNECT waits for its answer before it is sent again.
WAIT = 1.0


class Heard:
    """What one CONNECT heard back.

    Args:
        matched: Whether the vehicle answered with this station's checksum.
        running: The checksum the vehicle answered with, sixteen bytes, or
                 ``None`` where no answer came.
        version: The vehicle's build, as ``major.minor.patch``, or ``None``.
    """

    def __init__(self, matched, running=None, version=None):
        self.matched = matched
        self.running = running
        self.version = version

    def said(self, ours):
        """A sentence saying what was heard, for the station to print.

        Args:
            ours: The station's checksum, sixteen bytes.

        Returns:
            The sentence, in the Rust's words.
        """
        if self.matched:
            return (
                f"matched: tcspecial v{self.version} answered CONNECT with "
                f"this configuration's checksum, {ours.hex()}, and sends its "
                "data from now"
            )
        if self.running is not None:
            return (
                f"tcspecial runs configuration {self.running.hex()}, not this "
                f"one, {ours.hex()}: it sends nothing this station could "
                "read; CONNECT goes again"
            )
        return "no answer to CONNECT (none came in time): it goes again"


class Listening:
    """The spacecraft command link of a station that only listens.

    Args:
        link: The configuration's command link: ``send``, where this station
              binds, and ``recv``, where the vehicle takes commands.
        ours: The checksum of the configuration this station read, sixteen
              bytes.

    Raises:
        OSError: ``tcs_cmd.send`` cannot be bound: something else holds it.
    """

    def __init__(self, link, ours):
        self.link = link
        self.ours = ours
        self.sequence = 1
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.bind((link.send.address, link.send.port))

    def close(self):
        """Close the link."""
        self.socket.close()

    def connect(self, wait=WAIT):
        """Send one CONNECT, and wait up to ``wait`` seconds for its answer.

        An answer is taken only from where the vehicle takes commands, and
        only as the answer to this CONNECT, by its sequence; anything else
        is dropped.

        Args:
            wait: How long to wait, in seconds.

        Returns:
            The :class:`Heard`.
        """
        sequence = self.sequence
        self.sequence = (self.sequence + 1) % 2**32
        to = (self.link.recv.address, self.link.recv.port)
        self.socket.sendto(fixed.connect(sequence, self.ours), to)
        deadline = time.monotonic() + wait
        while True:
            left = deadline - time.monotonic()
            if left <= 0:
                return Heard(False)
            self.socket.settimeout(left)
            try:
                datagram, sender = self.socket.recvfrom(65535)
            except socket.timeout:
                return Heard(False)
            if not came_from(self.link.recv, sender):
                continue
            answer = fixed.answer(datagram)
            if (
                answer is None
                or not hasattr(answer, "running")
                or answer.sequence != sequence
            ):
                continue
            if answer.status != fixed.SUCCESS:
                return Heard(False)
            running = bytes(answer.running)
            version = f"{answer.major}.{answer.minor}.{answer.patch}"
            return Heard(running == self.ours, running, version)

    def until_matched(self, say, wait=WAIT):
        """Send CONNECT, and again until the vehicle answers with this
        station's checksum.

        Args:
            say:  Called with each :class:`Heard`, the match last.
            wait: How long each waits for its answer, in seconds.

        Returns:
            The :class:`Heard` of the match.
        """
        while True:
            heard = self.connect(wait)
            say(heard)
            if heard.matched:
                return heard
