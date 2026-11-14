# @generated automatically by tcs-xtce from tcsprotocol/xtce/beacon.xtce
# (SpaceSystem 'TcsBeacon').
# Any changes made to this file will be lost when it is regenerated;
# change the XTCE document instead and rerun `python3 -m tcs_xtce`.
# tcs-xtce: https://github.com/vomlehn/tcs-xtce
# Runtime crate: https://crates.io/crates/tcsxtce
"""Packet codecs for the ``TcsBeacon`` mission."""

from __future__ import annotations

import dataclasses
import enum
from dataclasses import dataclass
from typing import ClassVar, Optional, Union

from ._xtce import packet as pk


def _describe_bool(v: bool) -> str:
    return "true" if v else "false"


__all__ = ["AlarmMonitor", "Beacon"]


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
    """

    NAME: ClassVar[str] = "/TcsBeacon/Beacon"
    MAX_BYTES: ClassVar[int] = 106

    beacon_name: str = ""
    """``/TcsBeacon/BeaconName`` (/TcsBeacon/BeaconNameType)"""
    beacon_version: str = ""
    """``/TcsBeacon/BeaconVersion`` (/TcsBeacon/BeaconVersionType)"""
    beacon_digest: str = ""
    """``/TcsBeacon/BeaconDigest`` (/TcsBeacon/BeaconDigestType)"""
    beacon_number: str = ""
    """``/TcsBeacon/BeaconNumber`` (/TcsBeacon/BeaconNumberType)"""
    beacon_flag: str = ""
    """``/TcsBeacon/BeaconFlag`` (/TcsBeacon/BeaconFlagType)"""
    beacon_interface: str = ""
    """``/TcsBeacon/BeaconInterface`` (/TcsBeacon/BeaconInterfaceType)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[Beacon, int]:
        """Decodes a ``/TcsBeacon/Beacon`` packet that starts at the first byte
        of ``data``.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet and the number of bytes it takes.

        Raises:
            pk.PacketError: The bytes aren't such a packet, or it can't be
                decoded; its ``kind`` says why.
        """
        r = pk.BitReader(data)
        v1 = pk.read_string(r, (None, 80, b':', 0, 'UTF-8'), "BeaconName")
        v2 = pk.read_string(r, (None, 96, b':', 0, 'UTF-8'), "BeaconVersion")
        v3 = pk.read_string(r, (None, 264, b':', 0, 'UTF-8'), "BeaconDigest")
        v4 = pk.read_string(r, (None, 88, b':', 0, 'UTF-8'), "BeaconNumber")
        v5 = pk.read_string(r, (None, 64, b':', 0, 'UTF-8'), "BeaconFlag")
        v6 = pk.read_string(r, (256, 256, b"\x00", 0, "UTF-8"),
                            "BeaconInterface")
        return (Beacon(beacon_name=v1, beacon_version=v2, beacon_digest=v3,
                       beacon_number=v4, beacon_flag=v5, beacon_interface=v6),
                r.used())

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsBeacon/Beacon`` packet with a writer, after what
        it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        pk.write_string(w, (None, 80, b":", 0, "UTF-8"), self.beacon_name,
                        "BeaconName")
        pk.write_string(w, (None, 96, b":", 0, "UTF-8"), self.beacon_version,
                        "BeaconVersion")
        pk.write_string(w, (None, 264, b":", 0, "UTF-8"), self.beacon_digest,
                        "BeaconDigest")
        pk.write_string(w, (None, 88, b":", 0, "UTF-8"), self.beacon_number,
                        "BeaconNumber")
        pk.write_string(w, (None, 64, b":", 0, "UTF-8"), self.beacon_flag,
                        "BeaconFlag")
        pk.write_string(w, (256, 256, b"\x00", 0, "UTF-8"),
                        self.beacon_interface, "BeaconInterface")

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsBeacon/Beacon`` packet's fields.
        Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsBeacon/Beacon`` packet's fields
        like ``alarms()``, counting the earlier samples for change ranges,
        minViolations and minConformance.

        Args:
            m: The state of the alarms that depend on earlier samples, which the
               method reads and updates.
            t: This sample's time, in seconds.

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def algorithms(self) -> list:
        """Evaluates the MathAlgorithms that a ``/TcsBeacon/Beacon`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsBeacon/Beacon`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return "Beacon{" + ", ".join(
            ("beacon_name=" + pk.describe_str(self.beacon_name),
             "beacon_version=" + pk.describe_str(self.beacon_version),
             "beacon_digest=" + pk.describe_str(self.beacon_digest),
             "beacon_number=" + pk.describe_str(self.beacon_number),
             "beacon_flag=" + pk.describe_str(self.beacon_flag),
             "beacon_interface="
             + pk.describe_str(self.beacon_interface))) + "}"
