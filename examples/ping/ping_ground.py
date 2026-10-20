"""The ground's half of a ping, with no window.

The Python spelling of ``tcslib/src/ping.rs``, which is what tcsmoc pings a
``ping`` node with: the same rules for what is kept and the same three times,
shown the same way. Here between the two halves it borrows: the ping format
(``python/tcslibgs/ping.py``, after ``tcslibgs``) and the configuration
(``python/tcslibgs/configuration.py``). A :class:`Pinger`
sends each ping up to the ``moc`` node, takes each answer the ``ping`` node
sends back down, and keeps what a window shows: the last ping sent and when,
the last answer received and when, and the three times between them.

Apart from the window, as the beacon's facts are apart from its panel, so
that a test -- or a ground station with no screen -- has all of it.

What is kept
------------
A ping sent clears the time received and the three times, because the answer
they were about is not the answer to this ping: a window that went on showing
them would be timing a ping nobody is waiting for. They come back when an
answer arrives whose count is the count of the ping last sent.

An answer to an earlier ping -- one that was slow, or whose ping was sent
again before it came back -- is still the last thing received, and is shown
as that. It changes none of the times, because it is not the answer to the
ping they are timing.

The three times
---------------
* the round trip: from the ground sending the ping to the ground receiving
  the answer. One clock, the ground's, at both ends.
* up: from the ground sending the ping to the vehicle having it.
* down: from the vehicle having it to the ground receiving the answer.

Up and down are each read across two clocks, the ground's and the vehicle's,
so each includes how far apart those clocks are: on one machine that is
nothing, and between a ground station and a spacecraft it may be more than the
link takes, so either may come out negative. Their sum is the round trip
whatever the offset, which is how to tell a clock that is out from a link that
is slow.
"""

import socket
import time
from dataclasses import dataclass

from tcslib.beacon import NO_BEACON_MESSAGE
from tcslibgs import ping

NANOS = 1_000_000_000

#: What a panel shows for a ping or an answer before there is one:
#: ``tcslib::ping::NO_MESSAGE``, the beacon panel's placeholder.
NO_MESSAGE = NO_BEACON_MESSAGE
#: What :func:`precise_time` shows for no time: ``NO_PRECISE_TIME``.
NO_PRECISE_TIME = "--:--:--.---------"
#: What :func:`interval` shows for no length of time: ``NO_INTERVAL``.
NO_INTERVAL = "--.------ ms"


@dataclass(frozen=True)
class Exchange:
    """What a window shows of the pings, at one moment."""

    #: The last ping sent, as it went, or ``None`` before one has.
    sent: str = None
    #: When it went, in nanoseconds since the epoch, by the ground's clock.
    sent_at: int = None
    #: The last thing received on the answer link, as it arrived.
    received: str = None
    #: When the answer to the last ping arrived, by the ground's clock;
    #: ``None`` from the moment a ping is sent until its answer arrives.
    received_at: int = None
    #: When the vehicle had the last ping, by the vehicle's clock.
    answered_at: int = None

    def round_trip(self):
        """Ground sent to ground received, in nanoseconds, or ``None``."""
        if self.received_at is None:
            return None
        return self.received_at - self.sent_at

    def up(self):
        """Ground sent to vehicle had, in nanoseconds, across two clocks."""
        if self.received_at is None:
            return None
        return self.answered_at - self.sent_at

    def down(self):
        """Vehicle had to ground received, in nanoseconds, across two clocks."""
        if self.received_at is None:
            return None
        return self.received_at - self.answered_at


class Pinger:
    """The ground's end of the ping link: a ping goes up to ``uplink``, which
    the ``moc`` node binds aboard, and its answer comes down to ``downlink``,
    which this binds."""

    def __init__(self, uplink, downlink, clock=time.time_ns):
        self._uplink = (uplink.address, uplink.port)
        self._clock = clock
        self._up = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._down = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._down.bind((downlink.address, downlink.port))
        self._count = 0
        self._exchange = Exchange()

    def exchange(self):
        """What there is to show now."""
        return self._exchange

    def ping(self):
        """Send the next ping, and say what it was.

        The count goes up by one each time, from one, and the time it carries
        is the ground's, now. What was received last is kept, and when its
        answer arrived is cleared.
        """
        self._count += 1
        at = self._clock()
        message = ping.message(self._count, at)
        self._up.sendto(message.encode("ascii"), self._uplink)
        self._exchange = Exchange(
            sent=message,
            sent_at=at,
            received=self._exchange.received,
        )
        return message

    def receive(self, timeout):
        """Take one datagram from the answer link, waiting no longer than
        ``timeout`` seconds, and say whether one arrived."""
        self._down.settimeout(timeout)
        try:
            datagram, _ = self._down.recvfrom(ping.REPLY_BYTES + 1)
        except socket.timeout:
            return False
        self.answered(datagram, self._clock())
        return True

    def answered(self, datagram, at):
        """Fold in a datagram that arrived at ``at``: apart from the socket so
        that it can be exercised without one."""
        text = datagram.decode("utf-8", errors="replace")
        try:
            pong = ping.read_reply(datagram)
        except ValueError as why:
            # Shown, because something arrived and an operator should see
            # what; timed, never, because it answers nothing.
            self._exchange = Exchange(
                sent=self._exchange.sent,
                sent_at=self._exchange.sent_at,
                received=f"not an answer: {why}",
                received_at=self._exchange.received_at,
                answered_at=self._exchange.answered_at,
            )
            return
        latest = pong.ping.count == self._count
        self._exchange = Exchange(
            sent=self._exchange.sent,
            sent_at=self._exchange.sent_at,
            received=text,
            received_at=at if latest else self._exchange.received_at,
            answered_at=pong.answered if latest else self._exchange.answered_at,
        )

    def close(self):
        """Stop sending and receiving."""
        self._up.close()
        self._down.close()


def precise_time(since_epoch):
    """A time of day to the nanosecond, UTC, as a ping is timed: the beacon
    panel's ``HH:MM:SS`` with the fraction a ping is about;
    :data:`NO_PRECISE_TIME` for none."""
    if since_epoch is None:
        return NO_PRECISE_TIME
    seconds, nanoseconds = divmod(since_epoch, NANOS)
    within_a_day = seconds % 86_400
    return (
        f"{within_a_day // 3600:02d}:{(within_a_day % 3600) // 60:02d}:"
        f"{within_a_day % 60:02d}.{nanoseconds:09d}"
    )


def interval(nanoseconds):
    """A length of time between two of a ping's times, in milliseconds to the
    nanosecond, with its sign: either one-way time may be negative;
    :data:`NO_INTERVAL` for none."""
    if nanoseconds is None:
        return NO_INTERVAL
    sign = "-" if nanoseconds < 0 else ""
    whole, part = divmod(abs(nanoseconds), 1_000_000)
    return f"{sign}{whole}.{part:06d} ms"
