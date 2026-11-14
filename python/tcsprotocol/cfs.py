"""What crosses a cFS software bus: a cFS packet around a tcspecial packet.

The Python spelling of ``tcsprotocol/src/cfs.rs``, for the half a ground
station needs: a command written, and a telemetry packet read. See "What crosses
the bus" in ``docs/tcspecial-design.rst``. The layouts are cFE's, in a build
with
``MISSION_INCLUDE_CCSDSEXT_HEADER``, and its message IDs its version 2 mapping;
every number is most significant byte first::

    command    primary 6 | extended 4 | function code 1, checksum 1 |
               spare 4 | the tcspecial command
    telemetry  primary 6 | extended 4 | time 6 (seconds 4, 1/65536ths 2) |
               the answer, the beacon or the datum
"""

import struct
from dataclasses import dataclass

#: The primary and extended headers together.
BASE_BYTES = 10

#: What comes before a command's data.
COMMAND_HEADER_BYTES = BASE_BYTES + 2 + 4

#: What comes before a telemetry packet's data.
TELEMETRY_HEADER_BYTES = BASE_BYTES + 6

#: The bit of a message ID that says it is a command's.
COMMAND_BIT = 0x0080

#: The bits of a message ID taken from the primary header's APID.
APID_BITS = 0x007F

#: The EDS version cFE writes into every extended header by default.
EDS_VERSION = 1


def is_command(msgid):
    """Whether the version 2 mapping reads a message ID as a command's.

    Args:
        msgid: The message ID.

    Returns:
        ``True`` where its command bit is set.
    """
    return msgid & COMMAND_BIT != 0


def _base(msgid, sequence, total, spacecraft_id):
    """The primary and extended headers.

    Args:
        msgid:         The message ID, whose command bit says the type.
        sequence:      The sequence count, of which 14 bits are carried.
        total:         The whole packet's length, headers included.
        spacecraft_id: The extended header's system ID, cut to 16 bits.

    Returns:
        The ten bytes.
    """
    command = 0x1000 if is_command(msgid) else 0
    stream = command | 0x0800 | (msgid & APID_BITS)
    return struct.pack(
        ">HHHHH", stream, 0xC000 | (sequence & 0x3FFF), total - 7,
        (EDS_VERSION << 11) | (msgid >> 8), spacecraft_id & 0xFFFF,
    )


def _checksum(packet):
    """cFE's checksum of a whole command: 0xFF, exclusive-ored with every
    byte. A command whose checksum byte is right sums to nought.

    Args:
        packet: The whole command.

    Returns:
        The checksum, a byte.
    """
    sum_ = 0xFF
    for byte in packet:
        sum_ ^= byte
    return sum_


def write_command(msgid, sequence, spacecraft_id, function_code, data):
    """A command, its checksum made last over the rest.

    Args:
        msgid:         Its message ID.
        sequence:      Its sequence count.
        spacecraft_id: The spacecraft it is on.
        function_code: Its function code, below 128: the tcspecial
                       command's code, which is its first byte.
        data:          What follows the headers.

    Returns:
        The packet.

    Raises:
        ValueError: It is longer than a CCSDS length counts.
    """
    total = COMMAND_HEADER_BYTES + len(data)
    if total - 7 > 0xFFFF:
        raise ValueError("a command longer than a CCSDS packet")
    packet = bytearray(_base(msgid | COMMAND_BIT, sequence, total,
                             spacecraft_id))
    packet += bytes([function_code & 0x7F, 0]) + bytes(4) + bytes(data)
    packet[BASE_BYTES + 1] = _checksum(packet)
    return bytes(packet)


@dataclass(frozen=True)
class TelemetryFrame:
    """A telemetry packet read off the bus."""

    msgid: int
    #: Its time, as cFE's six bytes.
    time: bytes
    #: What follows the headers.
    data: bytes


def read_telemetry(packet):
    """Reads a telemetry packet, refusing what is not one whole cFS
    telemetry packet with a secondary header.

    Args:
        packet: The whole packet, as SBN carried it.

    Returns:
        The :class:`TelemetryFrame`.

    Raises:
        ValueError: What is wrong with it, in ``cfs.rs``'s words.
    """
    if len(packet) < TELEMETRY_HEADER_BYTES:
        raise ValueError("shorter than a cFS packet's headers")
    stream, sequence, length, subsystem, _ = struct.unpack(
        ">HHHHH", packet[:BASE_BYTES])
    if stream >> 13 != 0:
        raise ValueError("not CCSDS version 1 (version number 0)")
    if stream & 0x0800 == 0:
        raise ValueError("without the secondary header every cFS packet has")
    if stream & 0x1000 != 0:
        raise ValueError("a command where telemetry was expected")
    if sequence & 0xC000 != 0xC000:
        raise ValueError("a segment of a packet rather than a whole one")
    if length + 7 != len(packet):
        raise ValueError("of a length its primary header does not say")
    msgid = (stream & APID_BITS) | ((subsystem & 0x00FF) << 8)
    return TelemetryFrame(
        msgid=msgid,
        time=bytes(packet[BASE_BYTES:TELEMETRY_HEADER_BYTES]),
        data=bytes(packet[TELEMETRY_HEADER_BYTES:]),
    )
