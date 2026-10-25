"""The ping format, held to the Rust's own examples.

Each test here is a test of ``tcslibgs/src/node/vehicle/ping/mod.rs`` said
again, with the same messages and the same answers, so that the two spellings
of the format are held to one another.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import ping  # noqa: E402

SECOND = 1_000_000_000
SENT = 1_760_900_000 * SECOND + 1
ANSWERED = 1_760_900_000 * SECOND + 250_000_000


class PingTest(unittest.TestCase):
    def test_a_ping_is_written_as_the_format_says(self):
        self.assertEqual(ping.message(1, SENT), "ping #1 1760900000.000000001")
        self.assertEqual(ping.time_text(0), "0.000000000")
        with self.assertRaisesRegex(ValueError, "starts at one"):
            ping.message(0, SENT)

    def test_the_answer_is_the_ping_a_space_and_the_time(self):
        message = ping.message(7, SENT).encode()
        answer = ping.reply(message, ANSWERED)
        self.assertEqual(
            answer, b"ping #7 1760900000.000000001 1760900000.250000000"
        )
        self.assertEqual(
            ping.read_reply(answer),
            ping.Pong(ping=ping.Ping(count=7, sent=SENT), answered=ANSWERED),
        )

    def test_the_largest_ping_fills_the_buffer_exactly(self):
        largest = ping.LARGEST * SECOND + 999_999_999
        self.assertEqual(len(ping.time_text(largest)), ping.TIME_BYTES)
        message = ping.message(ping.LARGEST, largest)
        self.assertEqual(len(message), ping.MESSAGE_BYTES)
        answer = ping.reply(message.encode(), largest)
        self.assertEqual(len(answer), ping.REPLY_BYTES)
        self.assertEqual(ping.read_reply(answer).ping.count, ping.LARGEST)

    def test_what_is_not_a_ping_is_refused(self):
        for message, says in [
            ("", "is not a ping"),
            ("pong #1 1760900000.000000001", "is not a ping"),
            ("ping #1", "is not a ping"),
            ("ping 1 1760900000.000000001", "is not a ping"),
            ("ping #0 1760900000.000000001", "starts with a nought"),
            ("ping #01 1760900000.000000001", "starts with a nought"),
            ("ping #x 1760900000.000000001", "is not a whole number"),
            ("ping #1  1760900000.000000001", "nine digits"),
            ("ping #1 1760900000", "nine digits"),
            ("ping #1 1760900000.0001", "nine digits"),
            ("ping #1 1760900000.0000000001", "nine digits"),
            ("ping #1 1760900000.000000001\n", "nine digits"),
            ("ping #1 01760900000.000000001", "nine digits"),
            ("ping #1 -1.000000000", "nine digits"),
            ("ping #18446744073709551616 1.000000000", "larger than"),
            ("ping #1 18446744073709551616.000000000", "more seconds than"),
        ]:
            with self.subTest(message=message):
                with self.assertRaisesRegex(ValueError, says):
                    ping.read_message(message.encode())
                with self.assertRaises(ValueError):
                    ping.reply(message.encode(), ANSWERED)
        with self.assertRaisesRegex(ValueError, "ASCII"):
            ping.read_message(b"\xff" * 8)

    def test_a_message_longer_than_any_ping_is_refused(self):
        long = ("ping #1 1760900000.000000001" + " " * 4000).encode()
        with self.assertRaises(ValueError) as refused:
            ping.read_message(long)
        self.assertIn("longer than any ping", str(refused.exception))
        self.assertLess(len(str(refused.exception)), 200)

    def test_what_is_not_an_answer_is_refused(self):
        for answer in [
            "ping #1 1760900000.000000001",
            "ping #1 1760900000.000000001 now",
            "ping #1 1760900000.000000001 1760900000.25",
            "nonsense",
        ]:
            with self.subTest(answer=answer):
                with self.assertRaises(ValueError):
                    ping.read_reply(answer.encode())

    def test_the_sizes_are_the_rusts(self):
        # The same arithmetic, and the same answers: 6 + 20 + 1 + 30, and
        # that plus 1 + 30.
        self.assertEqual(ping.MESSAGE_BYTES, 57)
        self.assertEqual(ping.REPLY_BYTES, 88)


if __name__ == "__main__":
    unittest.main()
