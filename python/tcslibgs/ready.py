"""Whether the simulator is ready: the messages on tcssim's readiness socket.

The Python spelling of ``tcslibgs/src/ready.rs``, in the same words: a harness
sends a request carrying a sequence number to the address the simulator file
states as ``tcssim.ready``, and tcssim answers with the same number, its state
-- starting, ready or failed -- what it is set up for, and why it failed where
it did. Both messages are fixed in size and big-endian.
"""

import struct
from dataclasses import dataclass

REQUEST_MAGIC = b"TSRQ"
ANSWER_MAGIC = b"TSRA"

#: How long a request is: the magic and a sequence number.
REQUEST_BYTES = 8
#: The most of a failure's reason an answer carries.
REASON_BYTES = 240
#: How long an answer is.
ANSWER_BYTES = 4 + 4 + 1 + 1 + 16 + 16 + 2 + REASON_BYTES

#: The states, by the byte that carries them.
STATES = ("starting", "ready", "failed")
#: The simulation modes, by the byte that carries them.
MODES = ("none", "some", "all")


def request(sequence):
    """A request, as it goes on the wire."""
    return REQUEST_MAGIC + struct.pack(">I", sequence)


@dataclass(frozen=True)
class SetUpFor:
    """What a simulator is set up for, which a harness compares with its own.

    ``configuration`` and ``simulator`` are the two MD5 digests, as 16 bytes;
    ``mode`` is the ``--simulate`` mode's word.
    """

    configuration: bytes
    simulator: bytes
    mode: str

    def differences(self, theirs):
        """Each way this differs from ``theirs``, the simulator's, named with
        what each side has: empty where the two agree."""
        differ = []
        if self.configuration != theirs.configuration:
            differ.append(
                f"the configuration: md5 {self.configuration.hex()} here, "
                f"md5 {theirs.configuration.hex()} in the simulator"
            )
        if self.simulator != theirs.simulator:
            differ.append(
                f"the simulator file: md5 {self.simulator.hex()} here, "
                f"md5 {theirs.simulator.hex()} in the simulator"
            )
        if self.mode != theirs.mode:
            differ.append(
                f"the mode: --simulate={self.mode} here, "
                f"--simulate={theirs.mode} in the simulator"
            )
        return differ


@dataclass(frozen=True)
class Answer:
    sequence: int
    state: str
    set_up_for: SetUpFor
    reason: str


def encode_answer(answer):
    """An answer, as tcssim writes it: for a test playing the simulator."""
    reason = answer.reason.encode()[:REASON_BYTES]
    while True:
        try:
            reason.decode()
            break
        except UnicodeDecodeError:
            reason = reason[:-1]
    head = ANSWER_MAGIC + struct.pack(
        ">IBB",
        answer.sequence,
        STATES.index(answer.state),
        MODES.index(answer.set_up_for.mode),
    )
    body = answer.set_up_for.configuration + answer.set_up_for.simulator
    tail = struct.pack(">H", len(reason)) + reason
    return (head + body + tail).ljust(ANSWER_BYTES, b"\0")


def decode_answer(data):
    """An answer, or ``None`` for a datagram that is not one."""
    if len(data) != ANSWER_BYTES or data[:4] != ANSWER_MAGIC:
        return None
    sequence, state, mode = struct.unpack(">IBB", data[4:10])
    (length,) = struct.unpack(">H", data[42:44])
    if state >= len(STATES) or mode >= len(MODES) or length > REASON_BYTES:
        return None
    try:
        reason = data[44 : 44 + length].decode()
    except UnicodeDecodeError:
        return None
    return Answer(
        sequence=sequence,
        state=STATES[state],
        set_up_for=SetUpFor(data[10:26], data[26:42], MODES[mode]),
        reason=reason,
    )
