"""What a ping and its answer say, which the ground and the vehicle share.

The Python spelling of ``tcslibgs/src/node/vehicle/ping/mod.rs``, for a ground
station written in Python. Everything here is said there first; this says it
again, in the same words, and ``python/tests/test_ping.py`` holds the two to
one another with the same examples the Rust tests use.

A ping is::

    ping #<count> <sent>

and its answer is the ping with one space and the vehicle's time after it::

    ping #<count> <sent> <answered>

The count is a decimal number from one, which the ground adds one to for each
ping it sends. Each time is the number of seconds since the UNIX epoch,
written with exactly nine digits after the point, so that it says the time to
the nanosecond: ``1760900000.000000001``.

Times are whole nanoseconds since the epoch, as ``time.time_ns()`` gives them,
and never floats: a float of seconds holds about a microsecond at today's
epoch, and the format promises nanoseconds.
"""

import time
from dataclasses import dataclass

#: What every ping begins with.
PREFIX = "ping #"

#: The most digits a count can have: those of the largest 64-bit number.
COUNT_DIGITS = 20

#: The largest count and the largest whole number of seconds there are.
LARGEST = 2**64 - 1

#: The most bytes a time can take: twenty digits of seconds, the point, and
#: nine digits of nanoseconds.
TIME_BYTES = 20 + 1 + 9

#: The largest ping there is: the prefix, the largest count, a space and the
#: largest time.
MESSAGE_BYTES = len(PREFIX) + COUNT_DIGITS + 1 + TIME_BYTES

#: The largest answer there is: the largest ping, the space, and the largest
#: time the vehicle could add to it.
REPLY_BYTES = MESSAGE_BYTES + 1 + TIME_BYTES

NANOS = 1_000_000_000


@dataclass(frozen=True)
class Ping:
    """A ping, as the ground sent it."""

    #: Which ping this is, counting from one.
    count: int
    #: When the ground sent it, in nanoseconds since the UNIX epoch.
    sent: int


@dataclass(frozen=True)
class Pong:
    """An answer to a ping, as the vehicle sent it."""

    #: The ping it answers.
    ping: Ping
    #: When the vehicle had the ping, in nanoseconds since the UNIX epoch.
    answered: int


def now():
    """The time now, in nanoseconds since the UNIX epoch.

    Returns:
        The time now, in nanoseconds since the UNIX epoch.

    Raises:
        ValueError: Only for a clock set before 1970, which is a clock that
                    is wrong and that a ping exists to show.
    """
    at = time.time_ns()
    if at < 0:
        raise ValueError(
            "the clock says a time before the UNIX epoch, so no time since it "
            "can be written: set the clock"
        )
    return at


def time_text(since_epoch):
    """A time as a ping writes it: whole seconds, a point, and nine digits."""
    seconds, nanoseconds = divmod(since_epoch, NANOS)
    return f"{seconds}.{nanoseconds:09d}"


def message(count, sent):
    """The ping numbered ``count``, sent at ``sent``.

    The count starts at one, so nought is refused: an answer numbered nought
    would be an answer to a ping nobody sent.
    """
    if count == 0:
        raise ValueError("a ping's count starts at one, and this one is nought")
    return f"{PREFIX}{count} {time_text(sent)}"


def read_message(data):
    """What a ping says.

    Args:
        data: The datagram, as bytes.

    Returns:
        The :class:`Ping`: its count and when it was sent.

    Raises:
        ValueError: The datagram is not a ping; the message says why.
    """
    if len(data) > MESSAGE_BYTES:
        raise ValueError(
            f"{len(data)} bytes is longer than any ping, which is at most "
            f"{MESSAGE_BYTES}: {_said(data)}"
        )
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        raise ValueError(
            f"a ping is ASCII text, and this is not: {_said(data)}"
        ) from None
    rest = text[len(PREFIX):] if text.startswith(PREFIX) else None
    if rest is None or " " not in rest:
        raise ValueError(
            f'{_said(data)} is not a ping, which is "{PREFIX}<count> '
            f'<seconds>.<nanoseconds>"'
        )
    count, sent = rest.split(" ", 1)
    try:
        return Ping(count=_read_count(count), sent=_read_time(sent))
    except ValueError as why:
        raise ValueError(f"{_said(data)}: {why}") from None


def reply(data, answered):
    """The answer to this ping, had at ``answered``: the ping, one space, and
    the time.

    The vehicle's half of the exchange, here so that a test can play the
    vehicle with the same rule the vehicle answers by.
    """
    read_message(data)
    return data + b" " + time_text(answered).encode("ascii")


def read_reply(data):
    """What an answer to a ping says.

    Args:
        data: The datagram, as bytes: the ping, a space and the vehicle's
              time.

    Returns:
        The :class:`Pong`: the ping it answers and when the vehicle had it.

    Raises:
        ValueError: The datagram is not an answer to a ping; the message says
                    why.
    """
    if len(data) > REPLY_BYTES:
        raise ValueError(
            f"{len(data)} bytes is longer than any answer to a ping, which is "
            f"at most {REPLY_BYTES}: {_said(data)}"
        )
    # The vehicle's time is after the last space, and the ping is everything
    # before it.
    at = data.rfind(b" ")
    if at < 0:
        raise ValueError(
            f"{_said(data)} is not an answer to a ping, which is the ping, a "
            "space and a time"
        )
    ping = read_message(data[:at])
    try:
        stamp = data[at + 1:].decode("utf-8")
    except UnicodeDecodeError:
        raise ValueError(
            f"an answer to a ping is ASCII text, and this is not: {_said(data)}"
        ) from None
    try:
        answered = _read_time(stamp)
    except ValueError as why:
        raise ValueError(f"{_said(data)}: {why}") from None
    return Pong(ping=ping, answered=answered)


def _read_count(text):
    """A ping's count.

    Args:
        text: The count as written: decimal digits, from one, with no leading
              nought.

    Returns:
        The count.

    Raises:
        ValueError: ``text`` is not so written, or is larger than ``LARGEST``.
    """
    if not text or not all("0" <= c <= "9" for c in text):
        raise ValueError(f'the count "{text}" is not a whole number')
    if text.startswith("0"):
        raise ValueError(
            f'the count "{text}" starts with a nought: counts start at one and '
            "are written without leading noughts"
        )
    count = int(text)
    if count > LARGEST:
        raise ValueError(f'the count "{text}" is larger than {LARGEST}')
    return count


def _read_time(text):
    """A time: whole seconds, a point, and exactly nine digits of nanoseconds.

    Exactly nine, not up to nine: a time written to the millisecond would read
    as the same number, and the format promises nanoseconds. The whole seconds
    are held to the one spelling as well, no leading nought but in nought
    itself, so that a time has one way to be written.

    Args:
        text: The time as written.

    Returns:
        The time, in nanoseconds since the UNIX epoch.

    Raises:
        ValueError: ``text`` is not so written, or has more whole seconds
                    than ``LARGEST``.
    """
    wrong = ValueError(
        f'the time "{text}" is not seconds since the UNIX epoch with nine '
        "digits after the point, such as 1760900000.000000001"
    )
    if "." not in text:
        raise wrong
    seconds, nanoseconds = text.split(".", 1)

    def digits(part):
        return bool(part) and all("0" <= c <= "9" for c in part)

    if (
        not digits(seconds)
        or not digits(nanoseconds)
        or len(nanoseconds) != 9
        or (len(seconds) > 1 and seconds.startswith("0"))
    ):
        raise wrong
    whole = int(seconds)
    if whole > LARGEST:
        raise ValueError(f'the time "{text}" has more seconds than {LARGEST}')
    return whole * NANOS + int(nanoseconds)


def _said(data):
    """A message, for a complaint.

    Args:
        data: The datagram, as bytes.

    Returns:
        The datagram quoted, as text with anything unprintable escaped, and
        cut short past ``REPLY_BYTES`` so that one bad datagram cannot fill a
        log.
    """
    shown = data[:REPLY_BYTES]
    more = "..." if len(data) > len(shown) else ""
    text = (
        shown.decode("utf-8", errors="replace")
        .encode("unicode_escape")
        .decode("ascii")
    )
    return f'"{text}{more}"'
