"""The three fixed commands, as they go on the link, in Python.

CONNECT, RESTART_ARM and RESTART are taken at any time, whatever either end
is running, and their layouts never change: they are stated in
``tcslibgs/xtce/fixed.xml``, apart from every other command, and packed and
unpacked here by the code tcs-xtce generates from it,
:mod:`tcslibgs.fixed_packet`. The Python spelling of the fixed half of
``tcslibgs/src/link.rs``: a datagram is taken as an answer only if its length
is exactly what its opcode's layout gives, as the Rust takes it.

What a ground station in Python needs of them is CONNECT, which a station
that only listens sends too: a vehicle sends nothing down but its beacons and
answers until a CONNECT carrying its configuration's checksum.
"""

from . import fixed_packet

#: CONNECT's opcode, which its answer carries back.
CONNECT = 0x04

#: The status a successful answer carries.
SUCCESS = 0


def connect(sequence, checksum):
    """A CONNECT, as it goes on the link.

    Args:
        sequence: The command's sequence number, 0 to 2**32 - 1.
        checksum: The checksum of the configuration this station read: the
                  sixteen bytes, as ``bytes.fromhex`` gives them from what
                  tcsverify prints.

    Returns:
        The datagram: opcode, sequence and checksum, 21 bytes.
    """
    return fixed_packet.Connect(sequence=sequence, checksum=checksum).to_bytes()


def answer(datagram):
    """The fixed answer a datagram holds, or ``None`` for one that is not
    exactly one.

    Args:
        datagram: The datagram received.

    Returns:
        The generated answer -- :class:`fixed_packet.ConnectAnswer`,
        ``RestartArmAnswer`` or ``RestartAnswer`` -- or ``None`` where the
        datagram is not one of the three, or is longer or shorter than its
        opcode's layout.
    """
    try:
        read, used = fixed_packet.FixedAnswerKind.decode_prefix(datagram)
    except ValueError:
        return None
    return read if used == len(datagram) else None
