"""Listening for a spacecraft's beacons.

The Python spelling of ``tcslib/src/beacon.rs``: the half of a ground
station's beacon that is not a window. The other half, the panel, is
``python/tcsui/beacon_panel.py``, as it is ``tcsui`` in Rust. Everything here
is said there first, in the same words, and ``python/tests/test_beacon.py``
holds the two to one another with the same examples the Rust tests use.

A beacon says that tcspecial is running, which build it is, which version of
a configuration it is serving and the digest of the files it read. It arrives
on its own, every ``beacon.default_interval``, over a socket of its own, and it
keeps arriving whether or not a ground station has connected -- so it is the
one thing a ground station can know about a spacecraft without having asked it
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
        """What this stage is called where a person reads it.

        Returns:
            The stage's name: ``Stage::label`` in ``tcslib::beacon``, word for
            word.
        """
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
        """The contact a stage amounts to.

        Args:
            stage: The :class:`Stage`.

        Returns:
            ``Contact.MADE`` for a stage in contact, else ``Contact.LOST``.
        """
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

        Args:
            interval: How often the vehicle beacons, in nanoseconds; nought
                      for the standard interval.

        Returns:
            The schedule, a :class:`Schedule` of four steps.
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
        """How long contact is held after a beacon.

        Returns:
            The total time, in nanoseconds, of the leading run of steps whose
            stage counts as contact, or ``FOREVER`` if one of them never ends.
        """
        held = 0
        for step in self._steps:
            if not step.stage.in_contact():
                break
            if step.holds is FOREVER:
                return FOREVER
            held += step.holds
        return held

    def at(self, elapsed):
        """What to show for a beacon received ``elapsed`` nanoseconds ago.

        Args:
            elapsed: How long ago the beacon arrived, in nanoseconds; not
                     negative.

        Returns:
            The :class:`Showing`: the step's stage, whether a blinking
            indicator is lit, and how long until either changes.
        """
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
        """What to show, given when the last beacon arrived and the time now.

        Args:
            last: When the last beacon arrived, in nanoseconds since the
                  epoch, or ``None`` if none has.
            now:  The time now, in nanoseconds since the epoch.

        Returns:
            The :class:`Showing` for the beacon's age. Nothing heard is
            ``UNHEARD``, and so is a last beacon in the future.
        """
        if last is None or now < last:
            return UNHEARD
        return self.at(now - last)

    def contact(self, elapsed):
        """Whether the spacecraft is being heard from, for a beacon this old.

        Args:
            elapsed: How long ago the last beacon arrived, in nanoseconds.

        Returns:
            ``Contact.MADE`` while the schedule still holds contact,
            ``Contact.LOST`` after.
        """
        held = self.while_in_contact()
        if held is FOREVER or elapsed < held:
            return Contact.MADE
        return Contact.LOST

    def contact_at(self, last, now):
        """Whether the spacecraft is being heard from, ``now``.

        Args:
            last: When the last beacon arrived, in nanoseconds since the
                  epoch, or ``None`` if none has.
            now:  The time now, in nanoseconds since the epoch.

        Returns:
            The :class:`Contact`; ``LOST`` if nothing was heard or the last
            beacon is in the future.
        """
        if last is None or now < last:
            return Contact.LOST
        return self.contact(now - last)


def time_of_day(since_epoch):
    """A time as every panel shows it: ``Timestamp::time_of_day``.

    Args:
        since_epoch: The time, in nanoseconds since the epoch.

    Returns:
        Hours, minutes and seconds of the day, UTC, as ``HH:MM:SS``, with no
        fractions.
    """
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
    #: Whether its restart flag was set: the vehicle has started, or taken a
    #: RESTART, and has not been matched since.
    restart_flag: bool = False
    #: What is wrong with the configuration it announces, if anything.
    differs: str = None


def _beacon_of(datagram):
    """The beacon in a datagram, as tcspecial packs it.

    Args:
        datagram: The bytes that arrived on the beacon link.

    Returns:
        A dict of the beacon's ``sequence``, ``version`` and ``digest`` (as
        text) and ``restart_flag``, or ``None`` if the datagram is not a
        beacon this can read.
    """
    try:
        beacon = Beacon.read(datagram)
    except BeaconError:
        return None
    return {
        "sequence": beacon.sequence,
        "version": version_text(beacon.version),
        "digest": digest_text(beacon.digest),
        "restart_flag": beacon.restart_flag,
    }


def hear(datagram, expected):
    """Read a datagram from the beacon link.

    Args:
        datagram: The bytes that arrived on the beacon link.
        expected: This station's own configuration digest, as thirty-two hex
                  digits, which the beacon's is compared with; ``None`` to
                  compare nothing.

    Returns:
        A :class:`Heard`: what to show, the beacon's numbers if it could be
        read, and what differs about its configuration, if anything.
    """
    beacon = _beacon_of(datagram)
    why = None
    if beacon is not None and expected is not None:
        why = differs(beacon["digest"], expected)
    message = message_of(datagram)
    return Heard(
        message=f"{why} -- {message}" if why else message,
        sequence=None if beacon is None else beacon["sequence"],
        restart_flag=(beacon is not None
                           and beacon["restart_flag"]),
        differs=why,
    )


def restarted(now):
    """What to say of a restart: the same words as the Rust's.

    Not which kind of restart it was: every start begins with the restart
    flag set, so the flag says nothing of whether the ground commanded it.

    Args:
        now: The number the beacons start again at.

    Returns:
        The sentence to say.
    """
    return f"the beacons start again at #{now}, so tcspecial has restarted"


def differs(theirs, ours):
    """Whether the configuration tcspecial announces is the one this station
    read.

    Args:
        theirs: The digest the beacon carries, as hex.
        ours:   The digest of the configuration this station read, as hex.

    Returns:
        ``None`` when the digests agree, else a sentence saying they differ.
    """
    if theirs == ours:
        return None
    return (
        f"CONFIGURATION DIFFERS: tcspecial is serving md5 {theirs}, and this "
        f"MOC read md5 {ours}"
    )


def message_of(datagram):
    """What a beacon said, as a window shows it.

    Args:
        datagram: The bytes that arrived on the beacon link.

    Returns:
        The beacon's version and digest and whether its restart flag was
        set, as one line. A datagram this cannot read is shown
        as what arrived, cut short to forty characters.
    """
    beacon = _beacon_of(datagram)
    if beacon is not None:
        said = what_it_read(beacon["version"], beacon["digest"])
        return (said + ", RESTARTED, NOT YET MATCHED"
                if beacon["restart_flag"] else said)
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
    REPEATED = "repeated"


def counted(last, now):
    """What beacon number ``now`` says about the beacons since ``last``.

    The link delivers in order, so a number that goes back is a restart, and
    the same number again is the radio's repeat: the Rust's rule.

    Args:
        last: The number of the last beacon counted, or ``None`` if there
              was none.
        now:  The number of the beacon just arrived.

    Returns:
        A pair: the :class:`Counted` kind, and how many beacons were missed
        between the two (nought unless the kind is ``MISSED``).
    """
    if last is None:
        return Counted.FIRST, 0
    if now == last:
        return Counted.REPEATED, 0
    if now < last:
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

    Args:
        link:     The beacon link from the configuration.
        schedule: The :class:`Schedule` the last beacon ages through.
        expected: This station's configuration digest, as hex; ``None`` to
                  compare nothing.
        clock:    What gives the time now, in nanoseconds since the epoch.
    """

    def __init__(self, link, schedule, expected, clock=time.time_ns):
        self._link = link
        self._schedule = schedule
        self._expected = expected
        self._clock = clock
        self._socket = None
        self._at = None
        self._said = NO_BEACON_MESSAGE
        self._count = None
        self._differed = False
        # The last stranger warned of, so that one sending steadily is warned
        # of once and not on every datagram.
        self._stranger = None

    @staticmethod
    def listening(link, schedule, expected):
        """Bind where the configuration says beacons are received.

        Args:
            link:     The beacon link from the configuration: where beacons
                      are received and where they come from.
            schedule: The :class:`Schedule` the last beacon ages through.
            expected: This station's configuration digest, as hex, to compare
                      each beacon's with; ``None`` to compare nothing.

        Returns:
            The :class:`Beacons`, with its socket bound.

        Raises:
            OSError: The socket could not be made or bound.
        """
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

        Returns:
            The :class:`Pass`: what arrived, if anything, and what to show.
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
        """Fold in one datagram from ``sender``.

        Apart from the socket so that it can be exercised without one. A
        datagram from anywhere but where the configuration says beacons come
        from is ignored, with a warning the first time.

        Args:
            datagram: The bytes that arrived.
            sender:   The (address, port) they came from.
            remarks:  Remarks already made on this pass, which this appends
                      to; ``None`` for none.

        Returns:
            The :class:`Pass` this datagram makes.
        """
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
            now = heard.sequence
            last = self._count
            kind, missed = counted(last, now)
            if kind is Counted.MISSED:
                remarks.append(
                    f"{missed} beacon{'' if missed == 1 else 's'} did not "
                    f"arrive: #{now} came after #{last}"
                )
            elif kind is Counted.RESTARTED:
                remarks.append(restarted(now))
            # A repeat is the radio's, and says nothing new.
            self._count = now

        return self.after(Arrived.BEACON, remarks, heard.message)

    def after(self, arrived, remarks, message=None, warnings=None):
        """Fold what a pass found into what is known, and say what to show.

        A pass that found nothing still has the beacon before it to show, and
        so does a pass whose socket broke.

        Args:
            arrived:  What the pass found.
            remarks:  What is worth saying about the pass, in order.
            message:  What the beacon said, for a pass that found one.
            warnings: What is wrong, to be said whether or not asked; ``None``
                      for nothing.

        Returns:
            The :class:`Pass`.
        """
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
