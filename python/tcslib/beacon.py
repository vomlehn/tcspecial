"""Listening for a spacecraft's beacons.

The Python spelling of ``tcslib/src/beacon.rs``: the half of a ground
station's beacon that is not a window. The other half, the panel, is
``python/tcsui/beacon_panel.py``, as it is ``tcsui`` in Rust. Everything here
is said there first, in the same words, and ``python/tests/test_beacon.py``
holds the two to one another with the same examples the Rust tests use.

A beacon says that tcspecial is running, which build it is, which version of
a configuration it is serving and the digest of the files it read. It arrives
on its own, every ``beacon.interval``, over a socket of its own, and it keeps
arriving whether or not a ground station has connected -- so it is the one
thing a ground station can know about a spacecraft without having asked it
anything.

Staleness
---------
A beacon is not a request and is not answered, so the only thing that ever
happens is that one fails to arrive. How long ago the last one was is
therefore the whole of the state, and what a ground station shows is how that
age has grown, through four stages: Unheard, Heard, Aging and Waiting -- see
:meth:`Stage.label`. :class:`Schedule` is those stages and how long each
lasts, with no colours in it -- a stage is what is true, and which colour says
so is the window's business.

The stages also decide how long to wait on the socket. A station whose
indicator blinks has to wake up to blink it even when nothing arrives, so
``Showing.until`` is how long until the next change and is what the read
timeout is set to.

Times are whole nanoseconds, as ``time.time_ns()`` gives them, as the Rust's
are ``Duration`` and ``SystemTime``: a float would make the stage boundaries
depend on rounding.
"""

import enum
import socket
import time
from dataclasses import dataclass, field

from tcslibgs.beacon import Beacon, BeaconError
from tcslibgs.configuration import came_from

#: A step that never ends, which is how the standard schedule's last one holds
#: for ever: Rust's ``Duration::MAX``.
FOREVER = None

MS = 1_000_000
SECOND = 1_000_000_000

#: The beacon interval the standard schedule is for, in nanoseconds: see
#: :meth:`Schedule.for_interval`.
STANDARD_INTERVAL = 5 * SECOND

#: How many beacons in a row have gone missing when the steady green indicator
#: starts to blink, when it turns yellow, and when it turns red:
#: ``BLINKING_AFTER_MISSED``, ``AGING_AFTER_MISSED`` and
#: ``WAITING_AFTER_MISSED`` in ``tcslib::beacon``.
BLINKING_AFTER_MISSED = 2
AGING_AFTER_MISSED = 4
WAITING_AFTER_MISSED = 8


def missed(elapsed, interval):
    """How many beacons have gone missing since the last one arrived,
    ``elapsed`` nanoseconds ago, for a vehicle that beacons every ``interval``:
    one for each interval passed in full. Nought for an interval of nought."""
    if not interval:
        return 0
    return elapsed // interval

#: What the message line says before any beacon has arrived.
#:
#: Said rather than left empty, because an empty line next to a label reads as
#: a line the program has not filled in rather than one the spacecraft has not.
NO_BEACON_MESSAGE = "<none>"

#: What a time says before there is one: ``tcslibgs::NO_TRANSFER_TIME``.
NO_TRANSFER_TIME = "--:--:--"


class Stage(enum.Enum):
    """How stale the last beacon is.

    Named by what is true of the spacecraft rather than by what the window
    does about it.
    """

    #: No beacon has arrived, or the last one is in the future -- a clock that
    #: moved, which is no reading at all.
    UNHEARD = "unheard"
    #: One arrived recently enough to count: the spacecraft is being heard
    #: from, one lost datagram allowed for.
    HEARD = "heard"
    #: Late enough that contact is not claimed, and not so late that it is
    #: ``WAITING``.
    AGING = "aging"
    #: Long enough that nothing is being heard from the spacecraft: the ground
    #: is waiting for it to be heard from again.
    WAITING = "waiting"

    def in_contact(self):
        """Whether the spacecraft is being heard from at this stage: HEARD and
        nothing else."""
        return self is Stage.HEARD

    def label(self):
        """What this stage is called where a person reads it: ``Stage::label``
        in ``tcslib::beacon``, word for word."""
        return {
            Stage.UNHEARD: "Unheard",
            Stage.HEARD: "Heard",
            Stage.AGING: "Aging",
            Stage.WAITING: "Waiting",
        }[self]


class Contact(enum.Enum):
    """Whether the spacecraft is still being heard from."""

    MADE = "made"
    LOST = "lost"

    @staticmethod
    def at(stage):
        """The contact a stage amounts to."""
        return Contact.MADE if stage.in_contact() else Contact.LOST


@dataclass(frozen=True)
class Blink:
    """How long an indicator is on and off while it blinks, in nanoseconds."""

    on: int
    off: int


@dataclass(frozen=True)
class Step:
    """One stage of staleness and how long it lasts."""

    #: How long after the step before it this one holds, in nanoseconds;
    #: ``FOREVER`` for a step that never ends.
    holds: int
    #: What is true of the spacecraft while it does.
    stage: Stage
    #: Whether what shows it blinks, and how fast. ``None`` is steady.
    blink: Blink = None


@dataclass(frozen=True)
class Showing:
    """What to show, for an age."""

    #: How stale the last beacon is.
    stage: Stage
    #: Whether an indicator showing it is lit now.
    lit: bool
    #: How long until this changes by itself, in nanoseconds, where it will.
    #: ``None`` is a stage that will not change without a beacon.
    until: int = None


#: Nothing heard, and nothing to wait for but a beacon.
UNHEARD = Showing(stage=Stage.UNHEARD, lit=True, until=None)


class Schedule:
    """The stages a beacon's age passes through, in order."""

    def __init__(self, steps):
        self._steps = list(steps)

    @staticmethod
    def standard():
        """The schedule for a vehicle that beacons every five seconds:
        :meth:`for_interval` of :data:`STANDARD_INTERVAL`."""
        return Schedule.for_interval(STANDARD_INTERVAL)

    @staticmethod
    def for_interval(interval):
        """What tcsmoc shows, and what the beacon and ping examples show, for
        a vehicle that beacons every ``interval`` nanoseconds.

        Counted in beacons missed -- see :func:`missed` -- because that is
        what the indicator says: nought or one missed is steady green, two or
        three blinking green, four to seven blinking yellow, eight or more
        blinking red. For a five-second beacon that is ten seconds steady,
        ten more blinking and twenty aging. Every blink is half a
        second on and half a second off, whatever the interval. An interval of
        nought gets the standard schedule. The last step never ends, and
        blinks, because an indicator that has stopped changing is one an
        operator stops reading.
        """
        if not interval:
            interval = STANDARD_INTERVAL

        def missing(start, end):
            return interval * (end - start)

        blink = Blink(on=500 * MS, off=500 * MS)
        return Schedule([
            Step(holds=missing(0, BLINKING_AFTER_MISSED), stage=Stage.HEARD),
            Step(
                holds=missing(BLINKING_AFTER_MISSED, AGING_AFTER_MISSED),
                stage=Stage.HEARD,
                blink=blink,
            ),
            Step(
                holds=missing(AGING_AFTER_MISSED, WAITING_AFTER_MISSED),
                stage=Stage.AGING,
                blink=blink,
            ),
            Step(holds=FOREVER, stage=Stage.WAITING, blink=blink),
        ])

    def steps(self):
        """The steps, for a window that colours them."""
        return list(self._steps)

    def while_in_contact(self):
        """How long contact is held after a beacon: the leading run of steps
        whose stage counts as contact."""
        held = 0
        for step in self._steps:
            if not step.stage.in_contact():
                break
            if step.holds is FOREVER:
                return FOREVER
            held += step.holds
        return held

    def at(self, elapsed):
        """What to show for a beacon received ``elapsed`` nanoseconds ago."""
        start = 0
        for step in self._steps:
            ends = FOREVER if step.holds is FOREVER else start + step.holds
            if ends is not FOREVER and elapsed >= ends:
                start = ends
                continue

            left = None if ends is FOREVER else ends - elapsed
            if step.blink is None:
                return Showing(stage=step.stage, lit=True, until=left)

            # Where in the blink this age falls.
            cycle = step.blink.on + step.blink.off
            if cycle == 0:
                return Showing(stage=step.stage, lit=True, until=left)
            into = (elapsed - start) % cycle
            if into < step.blink.on:
                lit, until = True, step.blink.on - into
            else:
                lit, until = False, cycle - into
            return Showing(
                stage=step.stage,
                lit=lit,
                until=until if left is None else min(until, left),
            )
        return UNHEARD

    def after(self, last, now):
        """What to show, given when the last beacon arrived and the time now,
        both in nanoseconds since the epoch. Nothing heard is UNHEARD, and so
        is a last beacon in the future."""
        if last is None or now < last:
            return UNHEARD
        return self.at(now - last)

    def contact(self, elapsed):
        """Whether the spacecraft is being heard from, for a beacon this old."""
        held = self.while_in_contact()
        if held is FOREVER or elapsed < held:
            return Contact.MADE
        return Contact.LOST

    def contact_at(self, last, now):
        """Whether the spacecraft is being heard from, ``now``."""
        if last is None or now < last:
            return Contact.LOST
        return self.contact(now - last)


def time_of_day(since_epoch):
    """A time as every panel shows it: hours, minutes and seconds of the day,
    UTC, and no fractions -- ``Timestamp::time_of_day``."""
    within_a_day = (since_epoch // SECOND) % 86_400
    return (
        f"{within_a_day // 3600:02d}:{(within_a_day % 3600) // 60:02d}:"
        f"{within_a_day % 60:02d}"
    )


def received_at(at):
    """When a beacon arrived, as a window shows it: the time it arrived at the
    ground station, not a time the spacecraft put in it."""
    return NO_TRANSFER_TIME if at is None else time_of_day(at)


def version_text(version):
    """A version as it goes on the link, three decimal parts: ``version`` is
    major, minor and patch."""
    return ".".join(str(part) for part in version)


def digest_text(digest):
    """A digest as it is written: the hex of its sixteen bytes."""
    return "".join(f"{byte:02x}" for byte in digest)


def what_it_read(version, digest):
    """What one end of the link is running and reading, as one line: the
    version of the software and the digest of the files it read."""
    return f"tcspecial v{version} md5 {digest}"


@dataclass(frozen=True)
class Heard:
    """What a datagram on the beacon link turned out to be."""

    #: What a window shows for it.
    message: str
    #: The beacon's own number, if it was a beacon this can read.
    sequence: int = None
    #: What is wrong with the configuration it announces, if anything.
    differs: str = None


def _beacon_of(datagram):
    """The beacon in a datagram, as tcspecial packs it, or ``None``."""
    try:
        beacon = Beacon.read(datagram)
    except BeaconError:
        return None
    return {
        "sequence": beacon.sequence,
        "version": version_text(beacon.version),
        "digest": digest_text(beacon.digest),
    }


def hear(datagram, expected):
    """Read a datagram from the beacon link, comparing the digest it carries
    with ``expected``, this station's own, as thirty-two hex digits."""
    beacon = _beacon_of(datagram)
    why = None
    if beacon is not None and expected is not None:
        why = differs(beacon["digest"], expected)
    message = message_of(datagram)
    return Heard(
        message=f"{why} -- {message}" if why else message,
        sequence=None if beacon is None else beacon["sequence"],
        differs=why,
    )


def differs(theirs, ours):
    """Whether the configuration tcspecial announces is the one this station
    read: ``None`` when the digests agree."""
    if theirs == ours:
        return None
    return (
        f"CONFIGURATION DIFFERS: tcspecial is serving md5 {theirs}, and this "
        f"MOC read md5 {ours}"
    )


def message_of(datagram):
    """What a beacon said, as a window shows it. A datagram this cannot read
    is shown as what arrived, cut short."""
    beacon = _beacon_of(datagram)
    if beacon is not None:
        return what_it_read(beacon["version"], beacon["digest"])
    text = datagram.decode("utf-8", errors="replace")
    head = text[:40]
    return (
        f"unreadable: {head}..."
        if len(head) < len(text)
        else f"unreadable: {head}"
    )


class Counted(enum.Enum):
    """What a beacon's number says about the ones before it."""

    FIRST = "first"
    NEXT = "next"
    MISSED = "missed"
    RESTARTED = "restarted"


def counted(last, now):
    """What ``now`` says about the beacons between it and ``last``: the kind,
    and how many were missed."""
    if last is None:
        return Counted.FIRST, 0
    if now <= last:
        return Counted.RESTARTED, 0
    if now == last + 1:
        return Counted.NEXT, 0
    return Counted.MISSED, now - last - 1


class Arrived(enum.Enum):
    """What a pass of the loop found."""

    BEACON = "beacon"
    NOTHING = "nothing"
    BROKEN = "broken"


@dataclass
class Pass:
    """What one pass of the loop leaves to be shown and said."""

    #: What this pass found.
    arrived: Arrived
    #: How stale the last beacon is now, and whether an indicator is lit.
    showing: Showing
    #: When the last beacon arrived, if one ever has, in nanoseconds.
    at: int
    #: What that beacon said.
    said: str
    #: Whether the spacecraft is being heard from.
    contact: Contact
    #: What is worth saying on a console about this pass, in order: news,
    #: which a console says when asked to talk.
    remarks: list = field(default_factory=list)
    #: What is wrong, which a console says whether or not it was asked to
    #: talk: a beacon from somewhere beacons do not come from.
    warnings: list = field(default_factory=list)


class Beacons:
    """A ground station's beacon socket and what it has heard.

    The loop is the caller's: a window runs it on a thread of its own and a
    program with no window runs it where it stands. One pass waits no longer
    than the next change of what is shown.
    """

    def __init__(self, link, schedule, expected, clock=time.time_ns):
        self._link = link
        self._schedule = schedule
        self._expected = expected
        self._clock = clock
        self._socket = None
        self._at = None
        self._said = NO_BEACON_MESSAGE
        self._sequence = None
        self._differed = False
        # The last stranger warned of, so that one sending steadily is warned
        # of once and not on every datagram.
        self._stranger = None

    @staticmethod
    def listening(link, schedule, expected):
        """Bind where the configuration says beacons are received."""
        beacons = Beacons(link, schedule, expected)
        beacons._socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        beacons._socket.bind((link.recv.address, link.recv.port))
        return beacons

    def link(self):
        """Where this is listening and where it expects beacons from."""
        return self._link

    def schedule(self):
        """The stages it ages the last beacon through."""
        return self._schedule

    def next_pass(self):
        """Wait for a beacon, or for the moment what is shown would change.

        One pass of the loop: Rust's ``Beacons::pass``, which Python cannot
        call ``pass``.
        """
        waiting = self._schedule.after(self._at, self._clock())
        try:
            self._socket.settimeout(
                None if waiting.until is None else waiting.until / SECOND
            )
        except OSError:
            return self.after(Arrived.BROKEN, [])

        remarks = []
        try:
            datagram, sender = self._socket.recvfrom(65535)
        except socket.timeout:
            return self.after(Arrived.NOTHING, remarks)
        except OSError:
            return self.after(Arrived.BROKEN, remarks)
        return self.heard(datagram, sender, remarks)

    def heard(self, datagram, sender, remarks=None):
        """Fold in one datagram from ``sender``: apart from the socket so that
        it can be exercised without one."""
        remarks = [] if remarks is None else remarks
        # Only from where the configuration says beacons come from.
        if not came_from(self._link.send, sender):
            warnings = []
            if self._stranger != sender:
                warnings.append(
                    f"a beacon came from {sender[0]}:{sender[1]} and not from "
                    f"{self._link.send}, which is where the configuration says "
                    "they come from -- ignored"
                )
                self._stranger = sender
            return self.after(Arrived.NOTHING, remarks, warnings=warnings)

        heard = hear(datagram, self._expected)
        # A configuration that differs is remarked on when it starts and when
        # it stops, and not on every beacon in between.
        if (heard.differs is not None) != self._differed:
            remarks.append(
                heard.differs
                if heard.differs is not None
                else (
                    "tcspecial is serving the configuration this MOC read "
                    "again"
                )
            )
            self._differed = heard.differs is not None

        if heard.sequence is not None:
            kind, missed = counted(self._sequence, heard.sequence)
            if kind is Counted.MISSED:
                remarks.append(
                    f"{missed} beacon{'' if missed == 1 else 's'} did not "
                    f"arrive: #{heard.sequence} came after #{self._sequence}"
                )
            elif kind is Counted.RESTARTED:
                remarks.append(
                    f"the beacons start again at #{heard.sequence}, so "
                    "tcspecial has restarted"
                )
            self._sequence = heard.sequence

        return self.after(Arrived.BEACON, remarks, heard.message)

    def after(self, arrived, remarks, message=None, warnings=None):
        """Fold what a pass found into what is known, and say what to show. A
        pass that found nothing still has the beacon before it to show, and so
        does a pass whose socket broke."""
        now = self._clock()
        if arrived is Arrived.BEACON:
            self._at = now
            self._said = message
        showing = (
            UNHEARD
            if arrived is Arrived.BROKEN
            else self._schedule.after(self._at, now)
        )
        return Pass(
            arrived=arrived,
            showing=showing,
            at=self._at,
            said=self._said,
            contact=self._schedule.contact_at(self._at, now),
            remarks=remarks,
            warnings=[] if warnings is None else warnings,
        )

    def close(self):
        """Stop listening."""
        if self._socket is not None:
            self._socket.close()
