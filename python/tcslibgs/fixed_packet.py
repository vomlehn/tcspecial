# @generated automatically by tcs-xtce from tcslibgs/xtce/fixed.xml
# (SpaceSystem 'TcsFixed').
# Any changes made to this file will be lost when it is regenerated;
# change the XTCE document instead and rerun `python3 -m tcs_xtce`.
# tcs-xtce: https://github.com/vomlehn/tcs-xtce
# Rust crate: https://crates.io/crates/tcs-xtce
"""Packet codecs for the ``TcsFixed`` mission."""

from __future__ import annotations

import dataclasses
import enum
from dataclasses import dataclass
from typing import ClassVar, Optional, Union

from ._xtce import packet as pk


def _describe_bool(v: bool) -> str:
    return "true" if v else "false"


__all__ = ["StatusType", "AlarmMonitor", "RestartArmAnswer", "RestartAnswer",
           "ConnectAnswer", "Beacon", "RestartArm", "Restart", "Connect",
           "FixedAnswerKind", "FixedCommandKind"]


class StatusType(enum.IntEnum):
    """Enumeration ``/TcsFixed/StatusType``.

    Decoding keeps values the enumeration does not list (see
    :func:`tcs_xtce.packet.enum_value`)."""

    SUCCESS = 0
    FAILURE = 1
    INVALID_COMMAND = 2
    INVALID_PARAMETER = 3
    NOT_ARMED = 4
    NOT_FOUND = 5
    STALE = 9
    UNMATCHED = 10

    @property
    def label(self) -> Optional[str]:
        """The label of this value, if the enumeration lists it."""
        return pk.find_label(int(self), _LABELS_StatusType)

    @staticmethod
    def label_of(value: int) -> Optional[str]:
        """Names a value of enumeration ``/TcsFixed/StatusType``.

        Args:
            value: The value, which needn't be a member: a value in a label's
                   range, or one the enumeration doesn't list.

        Returns:
            The value's label, or None if the enumeration doesn't list it.
        """
        return pk.find_label(int(value), _LABELS_StatusType)


_LABELS_StatusType = (("Success", 0, 0), ("Failure", 1, 1),
                      ("InvalidCommand", 2, 2), ("InvalidParameter", 3, 3),
                      ("NotArmed", 4, 4), ("NotFound", 5, 5), ("Stale", 9, 9),
                      ("Unmatched", 10, 10))


def _DESCRIBE_StatusType(v) -> str:
    return pk.describe_enum(v, _LABELS_StatusType)


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
class RestartArmAnswer(pk.Packet):
    """Sequence container ``/TcsFixed/RestartArmAnswer``.

    Fixed by its inheritance: ``Opcode`` = 2.
    """

    NAME: ClassVar[str] = "/TcsFixed/RestartArmAnswer"
    MAX_BYTES: ClassVar[int] = 14

    sequence: int = 0
    """``/TcsFixed/Sequence`` (/TcsFixed/U32Type)"""
    status: StatusType = pk.enum_value(StatusType, 0)
    """``/TcsFixed/Status`` (/TcsFixed/StatusType)"""
    timestamp: int = 0
    """``/TcsFixed/Timestamp`` (/TcsFixed/U64Type)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[RestartArmAnswer, int]:
        """Decodes a ``/TcsFixed/RestartArmAnswer`` packet that starts at the
        first byte of ``data``.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet and the number of bytes it takes.

        Raises:
            pk.PacketError: The bytes aren't such a packet, or it can't be
                decoded; its ``kind`` says why.
        """
        r = pk.BitReader(data)
        v1 = r.uint_order(8, False, "Opcode")
        v2 = r.uint_order(32, False, "Sequence")
        v3 = pk.enum_value(StatusType, r.uint_order(8, False, "Status"))
        v4 = r.uint_order(64, False, "Timestamp")
        if not (v1 == 2):
            raise pk.PacketError("Mismatch", "/TcsFixed/RestartArmAnswer",
                                 "/TcsFixed/Opcode == 2")
        return RestartArmAnswer(sequence=v2, status=v3, timestamp=v4), r.used()

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/RestartArmAnswer`` packet with a writer,
        after what it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint_order(8, False, pk.from_signed(2, 8, 'unsigned', "Opcode"))
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        w.uint_order(8, False,
                     pk.from_signed(int(self.status), 8, "unsigned", "Status"))
        w.uint_order(64, False,
                     pk.from_signed(self.timestamp, 64, "unsigned",
                                    "Timestamp"))

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsFixed/RestartArmAnswer`` packet's
        fields. Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/RestartArmAnswer`` packet's
        fields like ``alarms()``, counting the earlier samples for change
        ranges, minViolations and minConformance.

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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/RestartArmAnswer``
        packet triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/RestartArmAnswer`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("RestartArmAnswer{"
                + ", ".join(("sequence=" + str(self.sequence),
                             "status=" + _DESCRIBE_StatusType(self.status),
                             "timestamp=" + str(self.timestamp))) + "}")


@dataclass(slots=True)
class RestartAnswer(pk.Packet):
    """Sequence container ``/TcsFixed/RestartAnswer``.

    Fixed by its inheritance: ``Opcode`` = 3.
    """

    NAME: ClassVar[str] = "/TcsFixed/RestartAnswer"
    MAX_BYTES: ClassVar[int] = 14

    sequence: int = 0
    """``/TcsFixed/Sequence`` (/TcsFixed/U32Type)"""
    status: StatusType = pk.enum_value(StatusType, 0)
    """``/TcsFixed/Status`` (/TcsFixed/StatusType)"""
    timestamp: int = 0
    """``/TcsFixed/Timestamp`` (/TcsFixed/U64Type)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[RestartAnswer, int]:
        """Decodes a ``/TcsFixed/RestartAnswer`` packet that starts at the first
        byte of ``data``.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet and the number of bytes it takes.

        Raises:
            pk.PacketError: The bytes aren't such a packet, or it can't be
                decoded; its ``kind`` says why.
        """
        r = pk.BitReader(data)
        v1 = r.uint_order(8, False, "Opcode")
        v2 = r.uint_order(32, False, "Sequence")
        v3 = pk.enum_value(StatusType, r.uint_order(8, False, "Status"))
        v4 = r.uint_order(64, False, "Timestamp")
        if not (v1 == 3):
            raise pk.PacketError("Mismatch", "/TcsFixed/RestartAnswer",
                                 "/TcsFixed/Opcode == 3")
        return RestartAnswer(sequence=v2, status=v3, timestamp=v4), r.used()

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/RestartAnswer`` packet with a writer, after
        what it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint_order(8, False, pk.from_signed(3, 8, 'unsigned', "Opcode"))
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        w.uint_order(8, False,
                     pk.from_signed(int(self.status), 8, "unsigned", "Status"))
        w.uint_order(64, False,
                     pk.from_signed(self.timestamp, 64, "unsigned",
                                    "Timestamp"))

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsFixed/RestartAnswer`` packet's
        fields. Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/RestartAnswer`` packet's
        fields like ``alarms()``, counting the earlier samples for change
        ranges, minViolations and minConformance.

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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/RestartAnswer``
        packet triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/RestartAnswer`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("RestartAnswer{"
                + ", ".join(("sequence=" + str(self.sequence),
                             "status=" + _DESCRIBE_StatusType(self.status),
                             "timestamp=" + str(self.timestamp))) + "}")


@dataclass(slots=True)
class ConnectAnswer(pk.Packet):
    """Sequence container ``/TcsFixed/ConnectAnswer``.

    Fixed by its inheritance: ``Opcode`` = 4.
    """

    NAME: ClassVar[str] = "/TcsFixed/ConnectAnswer"
    MAX_BYTES: ClassVar[int] = 37

    sequence: int = 0
    """``/TcsFixed/Sequence`` (/TcsFixed/U32Type)"""
    status: StatusType = pk.enum_value(StatusType, 0)
    """``/TcsFixed/Status`` (/TcsFixed/StatusType)"""
    timestamp: int = 0
    """``/TcsFixed/Timestamp`` (/TcsFixed/U64Type)"""
    running: bytes = b""
    """``/TcsFixed/Running`` (/TcsFixed/ChecksumType)"""
    major: int = 0
    """``/TcsFixed/Major`` (/TcsFixed/U8Type)"""
    minor: int = 0
    """``/TcsFixed/Minor`` (/TcsFixed/U8Type)"""
    patch: int = 0
    """``/TcsFixed/Patch`` (/TcsFixed/U8Type)"""
    newest_taken: int = 0
    """``/TcsFixed/NewestTaken`` (/TcsFixed/U32Type)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[ConnectAnswer, int]:
        """Decodes a ``/TcsFixed/ConnectAnswer`` packet that starts at the first
        byte of ``data``.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet and the number of bytes it takes.

        Raises:
            pk.PacketError: The bytes aren't such a packet, or it can't be
                decoded; its ``kind`` says why.
        """
        r = pk.BitReader(data)
        v1 = r.uint_order(8, False, "Opcode")
        v2 = r.uint_order(32, False, "Sequence")
        v3 = pk.enum_value(StatusType, r.uint_order(8, False, "Status"))
        v4 = r.uint_order(64, False, "Timestamp")
        if not (v1 == 4):
            raise pk.PacketError("Mismatch", "/TcsFixed/ConnectAnswer",
                                 "/TcsFixed/Opcode == 4")
        v5 = r.bytes(16, "Running")
        v6 = r.uint_order(8, False, "Major")
        v7 = r.uint_order(8, False, "Minor")
        v8 = r.uint_order(8, False, "Patch")
        v9 = r.uint_order(32, False, "NewestTaken")
        return (ConnectAnswer(sequence=v2, status=v3, timestamp=v4, running=v5,
                              major=v6, minor=v7, patch=v8, newest_taken=v9),
                r.used())

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/ConnectAnswer`` packet with a writer, after
        what it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint_order(8, False, pk.from_signed(4, 8, 'unsigned', "Opcode"))
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        w.uint_order(8, False,
                     pk.from_signed(int(self.status), 8, "unsigned", "Status"))
        w.uint_order(64, False,
                     pk.from_signed(self.timestamp, 64, "unsigned",
                                    "Timestamp"))
        pk.write_binary(w, self.running, 16, "Running")
        w.uint_order(8, False,
                     pk.from_signed(self.major, 8, "unsigned", "Major"))
        w.uint_order(8, False,
                     pk.from_signed(self.minor, 8, "unsigned", "Minor"))
        w.uint_order(8, False,
                     pk.from_signed(self.patch, 8, "unsigned", "Patch"))
        w.uint_order(32, False,
                     pk.from_signed(self.newest_taken, 32, "unsigned",
                                    "NewestTaken"))

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsFixed/ConnectAnswer`` packet's
        fields. Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/ConnectAnswer`` packet's
        fields like ``alarms()``, counting the earlier samples for change
        ranges, minViolations and minConformance.

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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/ConnectAnswer``
        packet triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/ConnectAnswer`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("ConnectAnswer{"
                + ", ".join(("sequence=" + str(self.sequence),
                             "status=" + _DESCRIBE_StatusType(self.status),
                             "timestamp=" + str(self.timestamp),
                             "running=" + pk.describe_bytes(self.running),
                             "major=" + str(self.major),
                             "minor=" + str(self.minor),
                             "patch=" + str(self.patch),
                             "newest_taken=" + str(self.newest_taken))) + "}")


@dataclass(slots=True)
class Beacon(pk.Packet):
    """Sequence container ``/TcsFixed/Beacon``.
    """

    NAME: ClassVar[str] = "/TcsFixed/Beacon"
    MAX_BYTES: ClassVar[int] = 106

    beacon_name: str = ""
    """``/TcsFixed/BeaconName`` (/TcsFixed/BeaconNameType)"""
    beacon_version: str = ""
    """``/TcsFixed/BeaconVersion`` (/TcsFixed/BeaconVersionType)"""
    beacon_digest: str = ""
    """``/TcsFixed/BeaconDigest`` (/TcsFixed/BeaconDigestType)"""
    beacon_number: str = ""
    """``/TcsFixed/BeaconNumber`` (/TcsFixed/BeaconNumberType)"""
    beacon_flag: str = ""
    """``/TcsFixed/BeaconFlag`` (/TcsFixed/BeaconFlagType)"""
    beacon_interface: str = ""
    """``/TcsFixed/BeaconInterface`` (/TcsFixed/BeaconInterfaceType)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[Beacon, int]:
        """Decodes a ``/TcsFixed/Beacon`` packet that starts at the first byte
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
        v10 = pk.read_string(r, (None, 80, b':', 0, 'UTF-8'), "BeaconName")
        v11 = pk.read_string(r, (None, 96, b':', 0, 'UTF-8'), "BeaconVersion")
        v12 = pk.read_string(r, (None, 264, b':', 0, 'UTF-8'), "BeaconDigest")
        v13 = pk.read_string(r, (None, 88, b':', 0, 'UTF-8'), "BeaconNumber")
        v14 = pk.read_string(r, (None, 64, b':', 0, 'UTF-8'), "BeaconFlag")
        v15 = pk.read_string(r, (256, 256, b"\x00", 0, "UTF-8"),
                             "BeaconInterface")
        return (Beacon(beacon_name=v10, beacon_version=v11, beacon_digest=v12,
                       beacon_number=v13, beacon_flag=v14,
                       beacon_interface=v15), r.used())

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/Beacon`` packet with a writer, after what it
        has written.

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
        """Evaluates the alarms of this ``/TcsFixed/Beacon`` packet's fields.
        Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/Beacon`` packet's fields
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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/Beacon`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/Beacon`` packet.

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


@dataclass(slots=True)
class RestartArm(pk.Packet):
    """Meta-command ``/TcsFixed/RestartArm``.

    Fixed by its inheritance: ``Opcode`` = 2.
    """

    NAME: ClassVar[str] = "/TcsFixed/RestartArm"
    MAX_BYTES: ClassVar[int] = 29

    sequence: int = 0
    """``/TcsFixed/FixedCommand#Sequence`` (/TcsFixed/U32Arg)"""
    arm_key: int = 0
    """``/TcsFixed/RestartArm#ArmKey`` (/TcsFixed/U64Arg)"""
    target: bytes = b""
    """``/TcsFixed/RestartArm#Target`` (/TcsFixed/ChecksumArg)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[RestartArm, int]:
        """Decodes a ``/TcsFixed/RestartArm`` packet that starts at the first
        byte of ``data``.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet and the number of bytes it takes.

        Raises:
            pk.PacketError: The bytes aren't such a packet, or it can't be
                decoded; its ``kind`` says why.
        """
        r = pk.BitReader(data)
        v16 = r.uint_order(8, False, "Opcode")
        v17 = r.uint_order(32, False, "Sequence")
        if not (v16 == 2):
            raise pk.PacketError("Mismatch", "/TcsFixed/RestartArm",
                                 "Opcode = 2")
        v18 = r.uint_order(64, False, "ArmKey")
        v19 = r.bytes(16, "Target")
        return RestartArm(sequence=v17, arm_key=v18, target=v19), r.used()

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/RestartArm`` packet with a writer, after
        what it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint_order(8, False, pk.from_signed(2, 8, 'unsigned', "Opcode"))
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        w.uint_order(64, False,
                     pk.from_signed(self.arm_key, 64, "unsigned", "ArmKey"))
        pk.write_binary(w, self.target, 16, "Target")

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsFixed/RestartArm`` packet's
        fields. Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/RestartArm`` packet's fields
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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/RestartArm`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/RestartArm`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("RestartArm{"
                + ", ".join(("sequence=" + str(self.sequence),
                             "arm_key=" + str(self.arm_key),
                             "target=" + pk.describe_bytes(self.target))) + "}")


@dataclass(slots=True)
class Restart(pk.Packet):
    """Meta-command ``/TcsFixed/Restart``.

    Fixed by its inheritance: ``Opcode`` = 3.
    """

    NAME: ClassVar[str] = "/TcsFixed/Restart"
    MAX_BYTES: ClassVar[int] = 29

    sequence: int = 0
    """``/TcsFixed/FixedCommand#Sequence`` (/TcsFixed/U32Arg)"""
    arm_key: int = 0
    """``/TcsFixed/Restart#ArmKey`` (/TcsFixed/U64Arg)"""
    target: bytes = b""
    """``/TcsFixed/Restart#Target`` (/TcsFixed/ChecksumArg)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[Restart, int]:
        """Decodes a ``/TcsFixed/Restart`` packet that starts at the first byte
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
        v16 = r.uint_order(8, False, "Opcode")
        v17 = r.uint_order(32, False, "Sequence")
        if not (v16 == 3):
            raise pk.PacketError("Mismatch", "/TcsFixed/Restart", "Opcode = 3")
        v20 = r.uint_order(64, False, "ArmKey")
        v21 = r.bytes(16, "Target")
        return Restart(sequence=v17, arm_key=v20, target=v21), r.used()

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/Restart`` packet with a writer, after what
        it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint_order(8, False, pk.from_signed(3, 8, 'unsigned', "Opcode"))
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        w.uint_order(64, False,
                     pk.from_signed(self.arm_key, 64, "unsigned", "ArmKey"))
        pk.write_binary(w, self.target, 16, "Target")

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsFixed/Restart`` packet's fields.
        Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/Restart`` packet's fields
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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/Restart`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/Restart`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("Restart{"
                + ", ".join(("sequence=" + str(self.sequence),
                             "arm_key=" + str(self.arm_key),
                             "target=" + pk.describe_bytes(self.target))) + "}")


@dataclass(slots=True)
class Connect(pk.Packet):
    """Meta-command ``/TcsFixed/Connect``.

    Fixed by its inheritance: ``Opcode`` = 4.
    """

    NAME: ClassVar[str] = "/TcsFixed/Connect"
    MAX_BYTES: ClassVar[int] = 21

    sequence: int = 0
    """``/TcsFixed/FixedCommand#Sequence`` (/TcsFixed/U32Arg)"""
    checksum: bytes = b""
    """``/TcsFixed/Connect#Checksum`` (/TcsFixed/ChecksumArg)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[Connect, int]:
        """Decodes a ``/TcsFixed/Connect`` packet that starts at the first byte
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
        v16 = r.uint_order(8, False, "Opcode")
        v17 = r.uint_order(32, False, "Sequence")
        if not (v16 == 4):
            raise pk.PacketError("Mismatch", "/TcsFixed/Connect", "Opcode = 4")
        v22 = r.bytes(16, "Checksum")
        return Connect(sequence=v17, checksum=v22), r.used()

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsFixed/Connect`` packet with a writer, after what
        it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint_order(8, False, pk.from_signed(4, 8, 'unsigned', "Opcode"))
        w.uint_order(32, False,
                     pk.from_signed(self.sequence, 32, "unsigned", "Sequence"))
        pk.write_binary(w, self.checksum, 16, "Checksum")

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsFixed/Connect`` packet's fields.
        Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsFixed/Connect`` packet's fields
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
        """Evaluates the MathAlgorithms that a ``/TcsFixed/Connect`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsFixed/Connect`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("Connect{"
                + ", ".join(("sequence=" + str(self.sequence),
                             "checksum=" + pk.describe_bytes(self.checksum)))
                + "}")


class FixedAnswerKind(pk.PacketKind):
    """Every concrete packet derived from ``/TcsFixed/FixedAnswer``."""

    NAME: ClassVar[str] = "/TcsFixed/FixedAnswer"
    MAX_BYTES: ClassVar[int] = 37
    MEMBERS: ClassVar[tuple] = (RestartArmAnswer, RestartAnswer, ConnectAnswer,)

    @staticmethod
    def decode_prefix(data) -> tuple[Union[RestartArmAnswer, RestartAnswer,
                                           ConnectAnswer], int]:
        """Decodes a packet derived from ``/TcsFixed/FixedAnswer`` that starts
        at the first byte of ``data``: reads the root's entries, then picks the
        first child (in document order) whose criteria match, recursively.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet, an instance of the member that matched, and the number
            of bytes it takes.

        Raises:
            pk.PacketError: ``NoMatch`` if no member matches, or another
                ``kind`` if the packet can't be decoded.
        """
        r = pk.BitReader(data)
        v1 = r.uint_order(8, False, "Opcode")
        v2 = r.uint_order(32, False, "Sequence")
        v3 = pk.enum_value(StatusType, r.uint_order(8, False, "Status"))
        v4 = r.uint_order(64, False, "Timestamp")
        if (v1 == 2):
            return (RestartArmAnswer(sequence=v2, status=v3, timestamp=v4),
                    r.used())
        if (v1 == 3):
            return RestartAnswer(sequence=v2, status=v3, timestamp=v4), r.used()
        if (v1 == 4):
            v5 = r.bytes(16, "Running")
            v6 = r.uint_order(8, False, "Major")
            v7 = r.uint_order(8, False, "Minor")
            v8 = r.uint_order(8, False, "Patch")
            v9 = r.uint_order(32, False, "NewestTaken")
            return (ConnectAnswer(sequence=v2, status=v3, timestamp=v4,
                                  running=v5, major=v6, minor=v7, patch=v8,
                                  newest_taken=v9), r.used())
        raise pk.PacketError("NoMatch", "/TcsFixed/FixedAnswer")


class FixedCommandKind(pk.PacketKind):
    """Every concrete packet derived from ``/TcsFixed/FixedCommand``."""

    NAME: ClassVar[str] = "/TcsFixed/FixedCommand"
    MAX_BYTES: ClassVar[int] = 29
    MEMBERS: ClassVar[tuple] = (RestartArm, Restart, Connect,)

    @staticmethod
    def decode_prefix(data) -> tuple[Union[RestartArm, Restart, Connect], int]:
        """Decodes a packet derived from ``/TcsFixed/FixedCommand`` that starts
        at the first byte of ``data``: reads the root's entries, then picks the
        first child (in document order) whose criteria match, recursively.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet, an instance of the member that matched, and the number
            of bytes it takes.

        Raises:
            pk.PacketError: ``NoMatch`` if no member matches, or another
                ``kind`` if the packet can't be decoded.
        """
        r = pk.BitReader(data)
        v16 = r.uint_order(8, False, "Opcode")
        v17 = r.uint_order(32, False, "Sequence")
        if (v16 == 2):
            v18 = r.uint_order(64, False, "ArmKey")
            v19 = r.bytes(16, "Target")
            return RestartArm(sequence=v17, arm_key=v18, target=v19), r.used()
        if (v16 == 3):
            v20 = r.uint_order(64, False, "ArmKey")
            v21 = r.bytes(16, "Target")
            return Restart(sequence=v17, arm_key=v20, target=v21), r.used()
        if (v16 == 4):
            v22 = r.bytes(16, "Checksum")
            return Connect(sequence=v17, checksum=v22), r.used()
        raise pk.PacketError("NoMatch", "/TcsFixed/FixedCommand")
