"""Waiting for the simulator before tcspecial is started.

The Python spelling of ``tcslib/src/simulator.rs``: :func:`ask` asks once and
gives what was said, or ``None`` where nothing answered, and :func:`wait`
waits until the simulator is ready for this run, raising :class:`NotReady`
with what it last heard where it is not. The messages are
``tcslibgs/ready.py``.
"""

import socket
import time

from tcslibgs.ready import ANSWER_BYTES, decode_answer, request

#: How long one request is given before it is sent again, in seconds.
RESEND_EVERY = 0.2


class NotReady(Exception):
    """What waiting came to, where it was not a simulator ready for this run.

    ``kind`` is one of ``silent``, ``starting``, ``failed`` and ``differs``,
    the four being different faults with different remedies.
    """

    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind


def ask(at, patience):
    """Ask the simulator at ``at``, a (host, port) pair, once, giving it
    ``patience`` seconds: its answer, or ``None`` where nothing answered."""
    family = socket.AF_INET6 if ":" in at[0] else socket.AF_INET
    with socket.socket(family, socket.SOCK_DGRAM) as sock:
        started = time.monotonic()
        sequence = 0
        while True:
            left = patience - (time.monotonic() - started)
            if left <= 0:
                return None
            try:
                sock.sendto(request(sequence), at)
            except OSError:
                pass
            until = time.monotonic() + min(left, RESEND_EVERY)
            while True:
                remaining = until - time.monotonic()
                if remaining <= 0:
                    break
                sock.settimeout(remaining)
                try:
                    data, came = sock.recvfrom(ANSWER_BYTES + 1)
                except socket.timeout:
                    break
                except OSError:
                    # Refused: waited out as silence is.
                    time.sleep(max(0.0, until - time.monotonic()))
                    break
                answer = decode_answer(data)
                if came[:2] == at[:2] and answer and answer.sequence <= sequence:
                    return answer
            sequence += 1


def wait(at, ours, patience):
    """Wait up to ``patience`` seconds for the simulator at ``at`` to be ready
    for the run ``ours``, a ``SetUpFor``, describes."""
    started = time.monotonic()
    heard = False
    while True:
        left = patience - (time.monotonic() - started)
        if left <= 0:
            waited = time.monotonic() - started
            if heard:
                raise NotReady(
                    "starting",
                    f"the simulator at {at[0]}:{at[1]} was still making its stand-ins "
                    f"after {waited:.1f} s: look at what tcssim says, or give it longer",
                )
            raise NotReady(
                "silent",
                f"nothing answered at {at[0]}:{at[1]} in {waited:.1f} s: tcssim is "
                "not running, or tcssim.ready in the simulator file is not where it is",
            )
        answer = ask(at, left)
        if answer is None:
            continue
        heard = True
        differences = ours.differences(answer.set_up_for)
        if differences:
            raise NotReady(
                "differs",
                f"the simulator at {at[0]}:{at[1]} is set up for another run, and so "
                f"is not used -- {'; '.join(differences)}: stop it, or start this run "
                "on what it read",
            )
        if answer.state == "ready":
            return
        if answer.state == "failed":
            raise NotReady(
                "failed",
                f"the simulator at {at[0]}:{at[1]} could not stand in: {answer.reason}",
            )
        time.sleep(min(RESEND_EVERY, left))
