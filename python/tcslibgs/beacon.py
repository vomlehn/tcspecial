"""The beacon, as it goes on the link.

The Python spelling of ``tcslibgs/src/beacon.rs``. The layout is stated once,
in XTCE, in ``tcslibgs/xtce/beacon.xml``, and packed and unpacked here by the
code tcs-xtce generates from it, ``beacon_packet.py``, as the Rust packs and
unpacks it with the Rust tcs-xtce generates from the same file::

    "tcspecial " major minor patch " " digest[16] sequence[4] generation[4]
    restarting:1 spare:7 interface[16]

Fifty-five bytes, every one of them, the numbers most significant byte
first. A beacon carries no CRC of its own: the transport under it checks it,
as UDP's checksum does here.
"""

import sys
from dataclasses import dataclass
from pathlib import Path

# tcs-xtce's Python runtime, which beacon_packet.py imports as ``xtce``. It is
# vendored in this tree until tcs-xtce is published: see
# vendor/tcs-xtce/README.md. Found from here, so that a ground station need not
# be run with a path of its own.
_XTCE = Path(__file__).resolve().parents[2] / "vendor" / "tcs-xtce" / "python"
if str(_XTCE) not in sys.path:
    sys.path.insert(0, str(_XTCE))

from xtce import packet as _pk  # noqa: E402

from . import beacon_packet as _packet  # noqa: E402
from . import interface as _interface  # noqa: E402

#: How long a beacon is: every one is exactly this long.
BEACON_BYTES = _packet.Beacon.MAX_BYTES


class BeaconError(ValueError):
    """Why a datagram is not a beacon."""


@dataclass(frozen=True)
class Beacon:
    """What a beacon says."""

    #: Which beacon this is, counted by the tcspecial that sent it from one.
    sequence: int
    #: The version of the build that sent it: major, minor and patch.
    version: tuple
    #: The digest of the configuration file the sender read: sixteen bytes.
    digest: bytes
    #: How many times tcspecial has started on this vehicle, kept in its
    #: state file. A change of it, either way, is a restart.
    generation: int = 1
    #: Whether a restart has been initiated and the sender is about to end.
    restarting: bool = False
    #: The interface the sender speaks: the digest of the XTCE documents it
    #: was built from, sixteen bytes. This tree's, where none is given.
    interface: bytes = None

    def __post_init__(self):
        if self.interface is None:
            object.__setattr__(self, "interface", _interface.of_this_tree())

    @staticmethod
    def read(datagram):
        """The beacon a datagram holds, or ``BeaconError`` saying why it holds
        none. A datagram of any length but a beacon's is refused rather than
        read as far as a beacon goes."""
        if len(datagram) != BEACON_BYTES:
            raise BeaconError(
                f"{len(datagram)} bytes, where a beacon is {BEACON_BYTES}: "
                "something other than tcspecial, or a tcspecial of another "
                "layout, is sending to the beacon port"
            )
        try:
            packet = _packet.Beacon.decode(datagram)
        except _pk.PacketError as why:
            raise BeaconError(
                f"not a beacon ({why}): something other than tcspecial is "
                "sending to the beacon port"
            ) from why
        return Beacon(
            sequence=packet.sequence,
            version=(packet.major, packet.minor, packet.patch),
            digest=bytes(packet.digest),
            generation=packet.generation,
            restarting=bool(packet.restarting),
            interface=bytes(packet.interface),
        )

    def to_bytes(self):
        """This beacon as it goes on the link."""
        major, minor, patch = self.version
        return bytes(
            _packet.Beacon(
                major=major,
                minor=minor,
                patch=patch,
                digest=self.digest,
                sequence=self.sequence,
                generation=self.generation,
                restarting=self.restarting,
                interface=self.interface,
            ).to_bytes()
        )
