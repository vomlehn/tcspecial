# @generated automatically by tcs-xtce from tcsprotocol/xtce/ping.xtce
# (SpaceSystem 'TcsPing').
# Any changes made to this file will be lost when it is regenerated;
# change the XTCE document instead and rerun `python3 -m tcs_xtce`.
# tcs-xtce: https://github.com/vomlehn/tcs-xtce
# Rust crate: https://crates.io/crates/tcs-xtce
"""Packet codecs for the ``TcsPing`` mission."""

from __future__ import annotations

import dataclasses
import enum
from dataclasses import dataclass
from typing import ClassVar, Optional, Union

from ._xtce import packet as pk


def _describe_bool(v: bool) -> str:
    return "true" if v else "false"


__all__ = ["AlarmMonitor", "PingReply", "Ping", "PingReplyFieldsKind"]


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
class PingReply(pk.Packet):
    """Sequence container ``/TcsPing/PingReply``.

    Fixed by its inheritance: ``ReplyPrefix`` = 'ping #', ``ReplySpace`` = ' '.
    """

    NAME: ClassVar[str] = "/TcsPing/PingReply"
    MAX_BYTES: ClassVar[int] = 88

    reply_count: str = ""
    """``/TcsPing/ReplyCount`` (/TcsPing/PingCountType)"""
    reply_sent_seconds: str = ""
    """``/TcsPing/ReplySentSeconds`` (/TcsPing/PingSecondsType)"""
    reply_sent_nanoseconds: str = ""
    """``/TcsPing/ReplySentNanoseconds`` (/TcsPing/PingNanosecondsType)"""
    reply_answered_seconds: str = ""
    """``/TcsPing/ReplyAnsweredSeconds`` (/TcsPing/PingSecondsType)"""
    reply_answered_nanoseconds: str = ""
    """``/TcsPing/ReplyAnsweredNanoseconds`` (/TcsPing/PingNanosecondsType)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[PingReply, int]:
        """Decodes a ``/TcsPing/PingReply`` packet that starts at the first byte
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
        v1 = pk.read_string(r, (48, 48, b'\x00', 0, 'US-ASCII'), "ReplyPrefix")
        v2 = pk.read_string(r, (None, 168, b' ', 0, 'US-ASCII'), "ReplyCount")
        v3 = pk.read_string(r, (None, 168, b".", 0, "US-ASCII"),
                            "ReplySentSeconds")
        v4 = pk.read_string(r, (72, 72, b"\x00", 0, "US-ASCII"),
                            "ReplySentNanoseconds")
        v5 = pk.read_string(r, (8, 8, b'\x00', 0, 'US-ASCII'), "ReplySpace")
        v6 = pk.read_string(r, (None, 168, b".", 0, "US-ASCII"),
                            "ReplyAnsweredSeconds")
        v7 = pk.read_string(r, (72, 72, b"\x00", 0, "US-ASCII"),
                            "ReplyAnsweredNanoseconds")
        if not (v1 == "ping #"):
            raise pk.PacketError("Mismatch", "/TcsPing/PingReply",
                                 "/TcsPing/ReplyPrefix == ping #")
        if not (v5 == " "):
            raise pk.PacketError("Mismatch", "/TcsPing/PingReply",
                                 "/TcsPing/ReplySpace ==  ")
        return (PingReply(reply_count=v2, reply_sent_seconds=v3,
                          reply_sent_nanoseconds=v4, reply_answered_seconds=v6,
                          reply_answered_nanoseconds=v7), r.used())

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsPing/PingReply`` packet with a writer, after what
        it has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        pk.write_string(w, (48, 48, b"\x00", 0, "US-ASCII"), "ping #",
                        "ReplyPrefix")
        pk.write_string(w, (None, 168, b" ", 0, "US-ASCII"), self.reply_count,
                        "ReplyCount")
        pk.write_string(w, (None, 168, b".", 0, "US-ASCII"),
                        self.reply_sent_seconds, "ReplySentSeconds")
        pk.write_string(w, (72, 72, b"\x00", 0, "US-ASCII"),
                        self.reply_sent_nanoseconds, "ReplySentNanoseconds")
        pk.write_string(w, (8, 8, b'\x00', 0, 'US-ASCII'), " ", "ReplySpace")
        pk.write_string(w, (None, 168, b".", 0, "US-ASCII"),
                        self.reply_answered_seconds, "ReplyAnsweredSeconds")
        pk.write_string(w, (72, 72, b"\x00", 0, "US-ASCII"),
                        self.reply_answered_nanoseconds,
                        "ReplyAnsweredNanoseconds")

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsPing/PingReply`` packet's fields.
        Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsPing/PingReply`` packet's fields
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
        """Evaluates the MathAlgorithms that a ``/TcsPing/PingReply`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsPing/PingReply`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return "PingReply{" + ", ".join(
            ("reply_count=" + pk.describe_str(self.reply_count),
             "reply_sent_seconds=" + pk.describe_str(self.reply_sent_seconds),
             "reply_sent_nanoseconds="
             + pk.describe_str(self.reply_sent_nanoseconds),
             "reply_answered_seconds="
             + pk.describe_str(self.reply_answered_seconds),
             "reply_answered_nanoseconds="
             + pk.describe_str(self.reply_answered_nanoseconds))) + "}"


@dataclass(slots=True)
class Ping(pk.Packet):
    """Meta-command ``/TcsPing/Ping``.
    """

    NAME: ClassVar[str] = "/TcsPing/Ping"
    MAX_BYTES: ClassVar[int] = 57

    count: str = ""
    """``/TcsPing/Ping#Count`` (/TcsPing/PingCountArg)"""
    sent_seconds: str = ""
    """``/TcsPing/Ping#SentSeconds`` (/TcsPing/PingSecondsArg)"""
    sent_nanoseconds: str = ""
    """``/TcsPing/Ping#SentNanoseconds`` (/TcsPing/PingNanosecondsArg)"""

    @classmethod
    def decode_prefix(cls, data) -> tuple[Ping, int]:
        """Decodes a ``/TcsPing/Ping`` packet that starts at the first byte of
        ``data``.

        Args:
            data: The packet's bytes, and any after it.

        Returns:
            The packet and the number of bytes it takes.

        Raises:
            pk.PacketError: The bytes aren't such a packet, or it can't be
                decoded; its ``kind`` says why.
        """
        r = pk.BitReader(data)
        r.uint(48, "/TcsPing/Ping")
        v8 = pk.read_string(r, (None, 168, b' ', 0, 'US-ASCII'), "Count")
        v9 = pk.read_string(r, (None, 168, b'.', 0, 'US-ASCII'), "SentSeconds")
        v10 = pk.read_string(r, (72, 72, b"\x00", 0, "US-ASCII"),
                             "SentNanoseconds")
        return Ping(count=v8, sent_seconds=v9, sent_nanoseconds=v10), r.used()

    def encode_into(self, w: pk.BitWriter) -> None:
        """Encodes this ``/TcsPing/Ping`` packet with a writer, after what it
        has written.

        Args:
            w: The writer.

        Raises:
            pk.PacketError: A field's value can't be encoded; its ``kind`` says
                why.
        """
        w.uint(48, 0x70696e672023)
        pk.write_string(w, (None, 168, b" ", 0, "US-ASCII"), self.count,
                        "Count")
        pk.write_string(w, (None, 168, b".", 0, "US-ASCII"), self.sent_seconds,
                        "SentSeconds")
        pk.write_string(w, (72, 72, b"\x00", 0, "US-ASCII"),
                        self.sent_nanoseconds, "SentNanoseconds")

    def alarms(self) -> list:
        """Evaluates the alarms of this ``/TcsPing/Ping`` packet's fields.
        Alarms that depend on earlier samples count only this one (see
        ``monitor``).

        Returns:
            (field, alarm level) for each field with an alarm; the level is None
            when the field is absent.
        """
        return []

    def monitor(self, m: AlarmMonitor, t: float = 0.0) -> list:
        """Evaluates the alarms of this ``/TcsPing/Ping`` packet's fields like
        ``alarms()``, counting the earlier samples for change ranges,
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
        """Evaluates the MathAlgorithms that a ``/TcsPing/Ping`` packet
        triggers, from this one's values.

        Returns:
            (output, value) for each; the value is None when an input is absent
            or a custom algorithm's function isn't set or fails.
        """
        return []

    def describe(self) -> str:
        """Describes this ``/TcsPing/Ping`` packet.

        Returns:
            Its canonical text form, identical in every generated language.
        """
        return ("Ping{"
                + ", ".join(("count=" + pk.describe_str(self.count),
                             "sent_seconds="
                             + pk.describe_str(self.sent_seconds),
                             "sent_nanoseconds="
                             + pk.describe_str(self.sent_nanoseconds))) + "}")


class PingReplyFieldsKind(pk.PacketKind):
    """Every concrete packet derived from ``/TcsPing/PingReplyFields``."""

    NAME: ClassVar[str] = "/TcsPing/PingReplyFields"
    MAX_BYTES: ClassVar[int] = 88
    MEMBERS: ClassVar[tuple] = (PingReply,)

    @staticmethod
    def decode_prefix(data) -> tuple[Union[PingReply], int]:
        """Decodes a packet derived from ``/TcsPing/PingReplyFields`` that
        starts at the first byte of ``data``: reads the root's entries, then
        picks the first child (in document order) whose criteria match,
        recursively.

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
        v1 = pk.read_string(r, (48, 48, b'\x00', 0, 'US-ASCII'), "ReplyPrefix")
        v2 = pk.read_string(r, (None, 168, b' ', 0, 'US-ASCII'), "ReplyCount")
        v3 = pk.read_string(r, (None, 168, b".", 0, "US-ASCII"),
                            "ReplySentSeconds")
        v4 = pk.read_string(r, (72, 72, b"\x00", 0, "US-ASCII"),
                            "ReplySentNanoseconds")
        v5 = pk.read_string(r, (8, 8, b'\x00', 0, 'US-ASCII'), "ReplySpace")
        v6 = pk.read_string(r, (None, 168, b".", 0, "US-ASCII"),
                            "ReplyAnsweredSeconds")
        v7 = pk.read_string(r, (72, 72, b"\x00", 0, "US-ASCII"),
                            "ReplyAnsweredNanoseconds")
        if (v1 == "ping #") and (v5 == " "):
            return (PingReply(reply_count=v2, reply_sent_seconds=v3,
                              reply_sent_nanoseconds=v4,
                              reply_answered_seconds=v6,
                              reply_answered_nanoseconds=v7), r.used())
        raise pk.PacketError("NoMatch", "/TcsPing/PingReplyFields")
