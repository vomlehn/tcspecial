"""The count a string or bytes is sent the ground after, as
``tcsprotocol::link_type``'s tests hold the Rust to: the same bytes, both
ways."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcsprotocol.count import counted, uncounted  # noqa: E402


class CountTest(unittest.TestCase):
    def test_a_count_is_written_and_read_back(self):
        self.assertEqual(counted(b"ping", xtce=True), b"\0\0\0\x04ping")
        self.assertEqual(uncounted(b"\0\0\0\x04ping", xtce=True), b"ping")
        self.assertEqual(counted(b"ping"), b"\x04ping")
        self.assertEqual(uncounted(b"\x04ping"), b"ping")
        long = bytes(300)
        # 300 is 0b10_0101100: 44 with the top bit set, then 2.
        self.assertEqual(counted(long)[:2], b"\xac\x02")
        self.assertEqual(uncounted(counted(long)), long)
        self.assertEqual(counted(b""), b"\0")
        self.assertEqual(uncounted(b"\0"), b"")

    def test_a_count_that_is_wrong_is_refused(self):
        for packet, xtce in ((b"\0\0\0\x05ping", True), (b"\0\0", True),
                             (b"\x05ping", False), (b"\x84\x00ping", False),
                             (b"\x80", False), (b"", False)):
            with self.subTest(packet=packet), self.assertRaises(ValueError):
                uncounted(packet, xtce)


if __name__ == "__main__":
    unittest.main()
