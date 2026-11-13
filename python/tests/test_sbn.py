"""SBN's wire format and the cFS packet, held to the Rust by its examples.

The same figures as the tests of ``tcslib-protocol/src/sbn.rs`` and
``tcslib-protocol/src/cfs.rs``, so that the two spellings cannot drift apart
unnoticed.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import cfs, sbn  # noqa: E402

HASH = "3b339a5e0f686cef822a5d6ea95004222ab7af33"


class SbnTest(unittest.TestCase):
    def test_a_header_round_trips(self):
        header = sbn.Header(sbn.APPLICATION, 2, 0x42)
        datagram = header.write(b"abc")
        self.assertEqual(
            datagram,
            bytes([0, 0, 0, 3, 3, 0, 0, 0, 2, 0, 0, 0, 0x42]) + b"abc",
        )
        self.assertEqual(sbn.Header.read(datagram), (header, b"abc"))
        with self.assertRaises(ValueError):
            sbn.Header.read(datagram[:-1])
        with self.assertRaises(ValueError):
            sbn.Header.read(datagram[:12])

    def test_the_ident_is_gits_id_of_the_hash(self):
        ident = sbn.ident_of(HASH)
        self.assertEqual(len(ident), sbn.IDENT_BYTES)
        self.assertEqual(ident[:5], b"$Id: ")
        self.assertEqual(ident[5:45], HASH.encode())
        self.assertEqual(ident[45:], b" $\0")
        for wrong in ["", "v7.0.1", HASH[1:], HASH[1:] + "g"]:
            with self.assertRaisesRegex(ValueError, "40 hexadecimal digits"):
                sbn.ident_of(wrong)

    def test_a_subscription_round_trips_and_its_ident_is_held_to(self):
        ident = sbn.ident_of(HASH)
        message = sbn.write_subscriptions(ident, [0x4280, 0x4290])
        self.assertEqual(len(message), 48 + 2 + 12)
        self.assertEqual(message[48:50], bytes([0, 2]))
        self.assertEqual(message[50:56], bytes([0, 0, 0x42, 0x80, 0, 0]))
        self.assertEqual(sbn.read_subscriptions(message, ident),
                         [0x4280, 0x4290])
        other = sbn.ident_of("1" * 40)
        with self.assertRaises(sbn.IdentRefused) as refused:
            sbn.read_subscriptions(message, other)
        self.assertEqual(refused.exception.carried, ident)
        with self.assertRaises(ValueError):
            sbn.read_subscriptions(message[:-1], ident)

    def test_a_release_is_one_this_library_speaks(self):
        self.assertEqual(sbn.release_of(" v7.0.1 ").protocol_version, 11)
        with self.assertRaisesRegex(ValueError, "v7.0.1"):
            sbn.release_of("v6.4.0")

    def test_a_ground_station_is_a_third_processor(self):
        self.assertEqual(sbn.ground_processor_id(2, 1), 3)
        self.assertEqual(sbn.ground_processor_id(1, 7), 8)


class CfsTest(unittest.TestCase):
    def test_a_command_is_laid_out_as_cfe_lays_one(self):
        packet = cfs.write_command(0x4280, 0, 0x42, 0x04, b"c")
        # Command, secondary header, APID 0; unsegmented; ten bytes less
        # seven; EDS version 1 and subsystem 0x42; system 0x42; function
        # code 4 and its checksum; the four spare bytes; the data.
        self.assertEqual(
            packet,
            bytes([0x18, 0x00, 0xC0, 0x00, 0x00, 0x0A, 0x08, 0x42, 0x00,
                   0x42, 0x04, 0x42, 0, 0, 0, 0]) + b"c",
        )
        whole = 0xFF
        for byte in packet:
            whole ^= byte
        self.assertEqual(whole, 0, "a right checksum sums to nought")
        self.assertTrue(cfs.is_command(0x4280))
        self.assertFalse(cfs.is_command(0x4201))

    def test_a_telemetry_packet_is_read_as_cfe_lays_one(self):
        time = bytes([0, 0, 0, 9, 0x80, 0])
        packet = (bytes([0x08, 0x42, 0xC0, 0x00, 0x00, 0x0F, 0x08, 0x42,
                         0x00, 0x42]) + time + b"293.15")
        frame = cfs.read_telemetry(packet)
        self.assertEqual((frame.msgid, frame.time, frame.data),
                         (0x4242, time, b"293.15"))
        for wrong in [packet[:-1], b"\x18" + packet[1:],
                      packet[:2] + b"\x40" + packet[3:], packet[:15]]:
            with self.assertRaises(ValueError):
                cfs.read_telemetry(wrong)


if __name__ == "__main__":
    unittest.main()
