"""Waiting for the simulator, held to ``tcslib/src/simulator.rs``'s tests.

The same cases: a simulator that becomes ready is waited for, a failure and a
difference are said at once, and silence and starting are told apart. And the
messages are those ``tcslibgs/src/ready.rs`` writes.
"""

import socket
import sys
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslib import simulator  # noqa: E402
from tcslibgs import ready  # noqa: E402


def set_up(mode):
    return ready.SetUpFor(bytes([3] * 16), bytes([4] * 16), mode)


def playing(mode, state):
    """A simulator answering with ``state[0]``, until the test ends.

    Args:
        mode:  The mode, as ``--simulate`` names it, it says it is set up for.
        state: A one-element list holding the state it answers with, which the
               test may change while it runs.

    Returns:
        A pair: the (host, port) it listens at, and the event that stops it.
    """
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    sock.settimeout(0.1)
    stop = threading.Event()

    def serve():
        """Answer every request on the socket until ``stop`` is set, then
        close it."""
        while not stop.is_set():
            try:
                data, came = sock.recvfrom(64)
            except socket.timeout:
                continue
            if (
                len(data) != ready.REQUEST_BYTES
                or data[:4] != ready.REQUEST_MAGIC
            ):
                continue
            sequence = int.from_bytes(data[4:8], "big")
            answer = ready.Answer(
                sequence, state[0], set_up(mode), "temp_3: no pty"
            )
            sock.sendto(ready.encode_answer(answer), came)
        sock.close()

    threading.Thread(target=serve, daemon=True).start()
    return sock.getsockname(), stop


class SimulatorTest(unittest.TestCase):
    def test_an_answer_comes_back_as_it_went(self):
        answer = ready.Answer(
            0xDEADBEEF, "failed", set_up("all"), "temp_3: taken"
        )
        self.assertEqual(
            ready.decode_answer(ready.encode_answer(answer)), answer
        )
        self.assertIsNone(ready.decode_answer(b"TSRA"))

    def test_a_ready_simulator_set_up_for_this_run_is_waited_for(self):
        state = ["starting"]
        at, stop = playing("all", state)
        threading.Timer(0.3, lambda: state.__setitem__(0, "ready")).start()
        try:
            simulator.wait(at, set_up("all"), 5)
        finally:
            stop.set()

    def test_a_failure_and_a_difference_are_said_at_once(self):
        at, stop = playing("all", ["failed"])
        try:
            with self.assertRaises(simulator.NotReady) as raised:
                simulator.wait(at, set_up("all"), 5)
            self.assertEqual(raised.exception.kind, "failed")
            self.assertIn("temp_3: no pty", str(raised.exception))
        finally:
            stop.set()
        at, stop = playing("some", ["ready"])
        try:
            with self.assertRaises(simulator.NotReady) as raised:
                simulator.wait(at, set_up("all"), 5)
            self.assertEqual(raised.exception.kind, "differs")
        finally:
            stop.set()

    def test_silence_and_starting_are_told_apart(self):
        at, stop = playing("all", ["starting"])
        try:
            with self.assertRaises(simulator.NotReady) as raised:
                simulator.wait(at, set_up("all"), 0.5)
            self.assertEqual(raised.exception.kind, "starting")
        finally:
            stop.set()
        nobody = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        nobody.bind(("127.0.0.1", 0))
        at = nobody.getsockname()
        nobody.close()
        started = time.monotonic()
        with self.assertRaises(simulator.NotReady) as raised:
            simulator.wait(at, set_up("all"), 0.5)
        self.assertEqual(raised.exception.kind, "silent")
        self.assertLess(time.monotonic() - started, 2)


if __name__ == "__main__":
    unittest.main()
