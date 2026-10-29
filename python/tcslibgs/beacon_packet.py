# @generated automatically by tcs-xtce from tcslibgs/xtce/beacon.xml
# (SpaceSystem 'TcsBeacon').
# Any changes made to this file will be lost when it is regenerated;
# change the XTCE document instead and rerun `python3 -m tcs_xtce`.
# tcs-xtce: https://github.com/vomlehn/tcs-xtce
# Rust crate: https://crates.io/crates/tcs-xtce
"""Packet codecs for the ``TcsBeacon`` mission."""

from __future__ import annotations

import dataclasses
import enum
from dataclasses import dataclass
from typing import ClassVar, Optional, Union

from xtce import packet as pk


def _describe_bool(v: bool) -> str:
    return "true" if v else "false"


__all__ = ["AlarmMonitor", "Beacon", "BeaconLayoutKind"]


# The functions the application supplies for the document's custom
# algorithms, by name: set them before decoding or encoding a packet
# that needs them, or it raises NoAlgorithm.
HOOKS: dict = {}


class AlarmMonitor:
    """The alarm state of each parameter whose alarms depend on earlier
    samples, which the packets' ``monitor()`` reads and updates."""

    def __init__(self) -> None:
        pass


@dataclass(slots=True)
class Beacon(pk.Packet):
    """Sequence container ``/TcsBeacon/Beacon``.

    Fixed by its inheritance: ``Name`` = 'tcspecial ', ``Separator`` = ' ',
    ``Spare`` = 0.
    """

    NAME: ClassVar[str] = "/TcsBeacon/Beacon"
    MAX_BYTES: ClassVar[int] = 55

    major: int = 0
    """``/TcsBeacon/Major`` (/TcsBeacon/U8Type)"""
    minor: int = 0
    """``/TcsBeacon/Minor`` (/TcsBeacon/U8Type)"""
    patch: int = 0
    """``/TcsBeacon/Patch`` (/TcsBeacon/U8Type)"""
    digest: bytes = b""
    """``/TcsBeacon/Digest`` (/TcsBeacon/DigestType)"""
    sequence: int = 0
    """``/TcsBeacon/Sequence`` (/TcsBeacon/U32Type)"""
    generation: int = 0
    """``/TcsBeacon/Generation`` (/TcsBeacon/U32Type)"""
    commanded_restart: bool = False
    """``/TcsBeacon/CommandedRestart`` (/TcsBeacon/FlagType)"""
    interface: bytes = b""
    """``/TcsBeacon/Interface`` (/TcsBeacon/DigestType)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[Beacon, int]:
        """Decode a packet that starts at the first byte of ``data``; return
        it and the number of bytes it takes."""
        r = pk.BitReader(data)
        v1 = pk.read_string(r, (80, 80, b'\x00', 0, 'US-ASCII'), "Name")
        v2 = r.uint_order(8, False, "Major")
        v3 = r.uint_order(8, False, "Minor")
        v4 = r.uint_order(8, False, "Patch")
        v5 = pk.read_string(r, (8, 8, b'\x00', 0, 'US-ASCII'), "Separator")
        v6 = r.bytes(16, "Digest")
        v7 = r.uint_order(32, False, "Sequence")
        v8 = r.uint_order(32, False, "Generation")
        v9 = r.uint_order(1, False, "CommandedRestart") != 0
        v10 = r.uint_order(7, False, "Spare")
        v11 = r.bytes(16, "Interface")
        if not (v1 == "tcspecial "):
            raise pk.PacketError("Mismatch", "/TcsBeacon/Beacon",
                                 "/TcsBeacon/Name == tcspecial ")
        if not (v5 == " "):
            raise pk.PacketError("Mismatch", "/TcsBeacon/Beacon",
                                 "/TcsBeacon/Separator ==  ")
        if not (v10 == 0):
            raise pk.PacketError("Mismatch", "/TcsBeacon/Beacon",
                                 "/TcsBeacon/Spare == 0")
        return (Beacon(major=v2, minor=v3, patch=v4, digest=v6, sequence=v7,
                       generation=v8, commanded_restart=v9, interface=v11),
                r.used())

    def encode_into(self, w: pk.BitWriter) -> None:
        pk.write_string(w, (80, 80, b"\x00", 0, "US-ASCII"), "tcspecial ",
                        "Name")
        w.uint_order(8, False,
                     pk.from_signed(self.major, 8, "unsigned", "Major"))
        w.uint_order(8, False,
                     pk.from_signed(self.minor, 8, "unsigned", "Minor"))
        w.uint_order(8, False,
                     pk.from_signed(self.patch, 8, "unsigned", "Patch"))
        pk.write_string(w, (8, 8, b'\x00', 0, 'US-ASCII'), " ", "Separator")
        pk.write_binary(w, self.digest, 16, "Digest")
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        w.uint_order(32, False,
                     pk.from_signed(self.generation, 32, "unsigned",
                                    "Generation"))
        w.uint_order(1, False, 1 if self.commanded_restart else 0)
        w.uint_order(7, False, pk.from_signed(0, 7, 'unsigned', "Spare"))
        pk.write_binary(w, self.interface, 16, "Interface")

    def alarms(self) -> list:
        """(field, alarm level) for each field with an alarm; the level is None
        when the field is absent.  Alarms that depend on earlier samples count
        only this one (see ``monitor``)."""
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Like ``alarms()``, counting the earlier samples whose state ``m``
        keeps,
        for change ranges, minViolations and minConformance; ``t`` is this
        sample's time in seconds."""
        return []

    def algorithms(self) -> list:
        """(output, value) for each MathAlgorithm this packet triggers; the
        value
        is None when an input is absent."""
        return []

    def describe(self) -> str:
        """Canonical text form, identical in every generated language."""
        return "Beacon{" + ", ".join(
            ("major=" + str(self.major), "minor=" + str(self.minor),
             "patch=" + str(self.patch),
             "digest=" + pk.describe_bytes(self.digest),
             "sequence=" + str(self.sequence),
             "generation=" + str(self.generation),
             "commanded_restart=" + _describe_bool(self.commanded_restart),
             "interface=" + pk.describe_bytes(self.interface))) + "}"


class BeaconLayoutKind(pk.PacketKind):
    """Every concrete packet derived from ``/TcsBeacon/BeaconLayout``."""

    NAME: ClassVar[str] = "/TcsBeacon/BeaconLayout"
    MAX_BYTES: ClassVar[int] = 55
    MEMBERS: ClassVar[tuple] = (Beacon,)

    @staticmethod
    def decode_prefix(data) -> tuple[Union[Beacon], int]:
        """Decode the root's entries, then pick the first child (in
        document order) whose criteria match, recursively; return it and
        the number of bytes it takes."""
        r = pk.BitReader(data)
        v1 = pk.read_string(r, (80, 80, b'\x00', 0, 'US-ASCII'), "Name")
        v2 = r.uint_order(8, False, "Major")
        v3 = r.uint_order(8, False, "Minor")
        v4 = r.uint_order(8, False, "Patch")
        v5 = pk.read_string(r, (8, 8, b'\x00', 0, 'US-ASCII'), "Separator")
        v6 = r.bytes(16, "Digest")
        v7 = r.uint_order(32, False, "Sequence")
        v8 = r.uint_order(32, False, "Generation")
        v9 = r.uint_order(1, False, "CommandedRestart") != 0
        v10 = r.uint_order(7, False, "Spare")
        v11 = r.bytes(16, "Interface")
        if (v1 == "tcspecial ") and (v5 == " ") and (v10 == 0):
            return (Beacon(major=v2, minor=v3, patch=v4, digest=v6, sequence=v7,
                           generation=v8, commanded_restart=v9, interface=v11),
                    r.used())
        raise pk.PacketError("NoMatch", "/TcsBeacon/BeaconLayout")
