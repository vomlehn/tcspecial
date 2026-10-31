"""The beacon, as it goes on the link.

The Python spelling of ``tcslibgs/src/beacon.rs``. A beacon is text, and
nothing but text: its fields in order, a colon between each, so that a person
reading a packet capture or a log reads it as it is, with no decoder::

    tcspecial:1.2.3:d41d8cd98f00b204e9800998ecf8427e:42:normal:0123...cdef

which is the name, the version, the configuration's digest, the beacon's
number, ``restart`` or ``normal`` for the restart flag, and the
interface's digest, numbers in decimal and digests in lower-case hex. It is
read strictly, as the Rust reads it. A beacon carries no check of its own:
the transport under it checks it, as UDP's checksum does here.
"""

from dataclasses import dataclass

from . import interface as _interface

#: The name every beacon starts with.
BEACON_NAME = "tcspecial"
#: What the restart flag's field says when the flag is set -- the start has
#: not yet been matched, or has taken a RESTART -- and when it is clear.
RESTART_FLAG = "restart"
NORMAL_FLAG = "normal"
#: The longest a beacon is: the name, a version of three parts of up to three
#: digits, two digests of thirty-two hex digits, a number of up to ten digits
#: and the longer flag, with the five colons between them.
BEACON_BYTES = 9 + 11 + 32 + 10 + 7 + 32 + 5

_HEX = set("0123456789abcdef")


class BeaconError(ValueError):
    """Why a datagram is not a beacon."""


def _decimal(text):
    """Reads a number written in decimal digits and nothing else.

    Args:
        text: The digits.

    Returns:
        The number, or ``None`` where the text is not digits or the number
        does not fit 32 bits.
    """
    if not text or not all("0" <= c <= "9" for c in text):
        return None
    number = int(text)
    return number if number < 2 ** 32 else None


def _hex(text):
    """Reads sixteen bytes written as thirty-two lower-case hex digits.

    Args:
        text: The digits.

    Returns:
        The bytes, or ``None`` where the text is not that.
    """
    if len(text) != 32 or not set(text) <= _HEX:
        return None
    return bytes.fromhex(text)


def _version(text):
    """Reads a version written ``major.minor.patch``.

    Args:
        text: The version, each part decimal digits that fit a byte.

    Returns:
        The major, minor and patch numbers, or ``None`` where the text is
        not a version.
    """
    parts = text.split(".")
    if len(parts) != 3:
        return None
    numbers = [_decimal(part) for part in parts]
    if any(n is None or n > 255 for n in numbers):
        return None
    return tuple(numbers)


def _refused(why):
    """Says that a datagram is not a beacon.

    Args:
        why: The first thing wrong with it.

    Returns:
        The :class:`BeaconError` to raise.
    """
    return BeaconError(
        f"not a beacon ({why}): something other than tcspecial, or a "
        "tcspecial of another layout, is sending to the beacon port"
    )


@dataclass(frozen=True)
class Beacon:
    """What a beacon says."""

    #: Which beacon this is, counted by the tcspecial that sent it from one.
    sequence: int
    #: The version of the build that sent it: major, minor and patch.
    version: tuple
    #: The digest of the configuration file the sender read: sixteen bytes.
    digest: bytes
    #: The restart flag: set at every start of tcspecial and from a RESTART
    #: being taken, and clear once the start has taken a CONNECT whose
    #: configuration checksum is the one it runs.
    restart_flag: bool = False
    #: The interface the sender speaks: the digest of the XTCE documents it
    #: was built from, sixteen bytes. This tree's, where none is given.
    interface: bytes = None

    def __post_init__(self):
        if self.interface is None:
            object.__setattr__(self, "interface", _interface.of_this_tree())

    @staticmethod
    def read(datagram):
        """The beacon a datagram holds.

        A datagram that is longer than a beacon, is not text, or has a field
        out of its form is refused rather than read as far as it goes.

        Args:
            datagram: The bytes that arrived on the beacon link.

        Returns:
            The :class:`Beacon`.

        Raises:
            BeaconError: The datagram holds no beacon; the message says why.
        """
        if len(datagram) > BEACON_BYTES:
            raise BeaconError(
                f"{len(datagram)} bytes, where a beacon is at most "
                f"{BEACON_BYTES}: something other than tcspecial, or a "
                "tcspecial of another layout, is sending to the beacon port"
            )
        try:
            text = datagram.decode("utf-8")
        except UnicodeDecodeError:
            raise _refused("it is not text") from None
        fields = text.split(":")
        if len(fields) != 6:
            raise _refused(f"it has {len(fields)} fields, not six")
        name, version, digest, sequence, flag, interface = fields
        if name != BEACON_NAME:
            raise _refused('it does not start "tcspecial:"')
        version = _version(version)
        if version is None:
            raise _refused("the version is not three numbers")
        digest = _hex(digest)
        if digest is None:
            raise _refused("the configuration digest is not 32 hex digits")
        sequence = _decimal(sequence)
        if sequence is None:
            raise _refused("the number is not a number")
        if flag not in (RESTART_FLAG, NORMAL_FLAG):
            raise _refused(
                'the restart flag is neither "restart" nor "normal"')
        interface = _hex(interface)
        if interface is None:
            raise _refused("the interface digest is not 32 hex digits")
        return Beacon(
            sequence=sequence,
            version=version,
            digest=digest,
            restart_flag=flag == RESTART_FLAG,
            interface=interface,
        )

    def to_bytes(self):
        """This beacon as it goes on the link.

        Returns:
            The beacon packed as tcspecial packs it, ``BEACON_BYTES`` long.
        """
        major, minor, patch = self.version
        flag = RESTART_FLAG if self.restart_flag else NORMAL_FLAG
        return (
            f"{BEACON_NAME}:{major}.{minor}.{patch}:{self.digest.hex()}:"
            f"{self.sequence}:{flag}:{self.interface.hex()}"
        ).encode("ascii")
