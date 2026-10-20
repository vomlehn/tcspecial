"""The ping example's own code: what is kept as pings go and answers come,
and the window that shows it.

Run by ``tests/python.rs`` under ``cargo test``, or directly::

    python3 -m unittest discover -s examples/ping -p 'test_*.py'
"""

import os
import socket
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "python"))
sys.path.insert(0, str(HERE))

from ping_ground import Pinger, interval, precise_time  # noqa: E402
from tcslibgs import ping  # noqa: E402
from tcslibgs.configuration import HostPort  # noqa: E402

SECOND = 1_000_000_000
AT = 1_760_900_000 * SECOND


def a_free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Clock:
    """A clock a test sets."""

    def __init__(self, at):
        self.at = at

    def __call__(self):
        return self.at


class PingerTest(unittest.TestCase):
    def setUp(self):
        self.clock = Clock(AT)
        self.vehicle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.vehicle.bind(("127.0.0.1", 0))
        self.vehicle.settimeout(2)
        uplink = HostPort("127.0.0.1", self.vehicle.getsockname()[1])
        self.pinger = Pinger(uplink, HostPort("127.0.0.1", a_free_port()), clock=self.clock)

    def tearDown(self):
        self.pinger.close()
        self.vehicle.close()

    def the_vehicle_had(self):
        """What reached the vehicle."""
        return self.vehicle.recvfrom(256)[0]

    def test_a_ping_goes_up_numbered_and_timed(self):
        self.assertEqual(self.pinger.ping(), f"ping #1 {ping.time_text(AT)}")
        self.assertEqual(self.the_vehicle_had(), f"ping #1 {ping.time_text(AT)}".encode())
        self.clock.at += SECOND
        self.pinger.ping()
        self.assertEqual(ping.read_message(self.the_vehicle_had()).count, 2)
        exchange = self.pinger.exchange()
        self.assertEqual(exchange.sent_at, AT + SECOND)

    def test_the_answer_to_the_last_ping_is_timed(self):
        message = self.pinger.ping().encode()
        answered = AT + 3_000_000
        self.pinger.answered(ping.reply(message, answered), AT + 5_000_000)

        exchange = self.pinger.exchange()
        self.assertEqual(exchange.received, f"ping #1 {ping.time_text(AT)} {ping.time_text(answered)}")
        self.assertEqual(exchange.received_at, AT + 5_000_000)
        self.assertEqual(exchange.round_trip(), 5_000_000)
        self.assertEqual(exchange.up(), 3_000_000)
        self.assertEqual(exchange.down(), 2_000_000)
        self.assertEqual(exchange.up() + exchange.down(), exchange.round_trip())

    def test_a_ping_clears_the_time_received_and_keeps_what_was(self):
        first = self.pinger.ping().encode()
        self.pinger.answered(ping.reply(first, AT + 1), AT + 2)
        received = self.pinger.exchange().received

        self.clock.at = AT + SECOND
        self.pinger.ping()
        exchange = self.pinger.exchange()
        self.assertIsNone(exchange.received_at, "the answer it was about is not this ping's")
        self.assertIsNone(exchange.round_trip())
        self.assertIsNone(exchange.up())
        self.assertIsNone(exchange.down())
        self.assertEqual(exchange.received, received, "the last thing received is still that")

    def test_an_earlier_pings_answer_is_shown_and_not_timed(self):
        first = self.pinger.ping().encode()
        self.clock.at = AT + SECOND
        self.pinger.ping()
        self.pinger.answered(ping.reply(first, AT + 7), AT + SECOND + 9)

        exchange = self.pinger.exchange()
        self.assertTrue(exchange.received.startswith("ping #1 "), exchange.received)
        self.assertIsNone(exchange.received_at, "it does not answer ping #2")

    def test_what_is_not_an_answer_is_shown_and_not_timed(self):
        self.pinger.ping()
        self.pinger.answered(b"hello", AT + 1)
        exchange = self.pinger.exchange()
        self.assertTrue(exchange.received.startswith("not an answer: "), exchange.received)
        self.assertIsNone(exchange.received_at)

    def test_an_answer_arrives_on_the_downlink(self):
        message = self.pinger.ping().encode()
        port = self.pinger._down.getsockname()[1]
        self.vehicle.sendto(ping.reply(message, AT + 1), ("127.0.0.1", port))
        self.assertTrue(self.pinger.receive(timeout=2))
        self.assertIsNotNone(self.pinger.exchange().received_at)
        self.assertFalse(self.pinger.receive(timeout=0.01), "nothing more came")


class ShowingTest(unittest.TestCase):
    def test_a_time_is_shown_to_the_nanosecond_and_utc(self):
        self.assertEqual(precise_time((3600 + 120 + 3) * SECOND + 4), "01:02:03.000000004")
        self.assertEqual(precise_time(None), "--:--:--.---------")

    def test_an_interval_is_milliseconds_with_its_sign(self):
        self.assertEqual(interval(16_441_799), "16.441799 ms")
        self.assertEqual(interval(-250_000), "-0.250000 ms")
        self.assertEqual(interval(0), "0.000000 ms")
        self.assertEqual(interval(None), "--.------ ms")


@unittest.skipUnless(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"), "no display")
class WindowTest(unittest.TestCase):
    """The window, driven as a person would drive it, with no vehicle: the
    test answers the ping itself."""

    def test_pressing_ping_shows_the_ping_and_the_answer_fills_in_the_times(self):
        import tkinter as tk
        from tkinter import ttk

        import ping_moc

        vehicle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        vehicle.bind(("127.0.0.1", 0))
        vehicle.settimeout(2)
        down = a_free_port()
        pinger = Pinger(HostPort("127.0.0.1", vehicle.getsockname()[1]), HostPort("127.0.0.1", down))

        class NoBeacons:
            def next_pass(self):
                import time
                time.sleep(3600)

        shown = {}
        original = tk.Tk.mainloop

        def labels(widget):
            for child in widget.winfo_children():
                yield child
                yield from labels(child)

        def mainloop(window, n=0):
            def press():
                button = next(w for w in labels(window) if isinstance(w, ttk.Button))
                button.invoke()
                message = vehicle.recvfrom(256)[0]
                vehicle.sendto(ping.reply(message, ping.now()), ("127.0.0.1", down))
                window.after(600, read)

            def read():
                texts = [w.cget("text") for w in labels(window) if isinstance(w, ttk.Label)]
                shown["texts"] = texts
                window.destroy()

            window.after(200, press)
            original(window, n)

        tk.Tk.mainloop = mainloop
        try:
            ping_moc.run_the_window(None, NoBeacons(), pinger)
        finally:
            tk.Tk.mainloop = original
            pinger.close()
            vehicle.close()

        texts = shown["texts"]
        self.assertTrue(any(t.startswith("ping #1 ") and t.count(" ") == 2 for t in texts), texts)
        self.assertTrue(any(t.startswith("ping #1 ") and t.count(" ") == 3 for t in texts), texts)
        self.assertTrue(any(t.endswith(" ms") for t in texts), texts)


if __name__ == "__main__":
    unittest.main()
