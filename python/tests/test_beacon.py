"""The beacon's facts, held to the Rust's own examples.

The tests of ``tcsmoc/src/beacon.rs`` said again, with the same figures. The beacons are written here as text, field by
field, rather than by the code under test; ``examples/ping/tests/pings.rs``
sends this program beacons the Rust writes, so a change to the layout is caught
at both ends.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcsmoc import beacon  # noqa: E402
from tcsmoc.beacon import (  # noqa: E402
    Blink,
    Contact,
    Counted,
    Schedule,
    Stage,
    Step,
)
from tcsprotocol import interface  # noqa: E402
from tcsprotocol.beacon import (BEACON_BYTES, Beacon,  # noqa: E402
                                    BeaconError)
from tcsprotocol.configuration import HostPort  # noqa: E402

MS = 1_000_000
SECOND = 1_000_000_000
AT = 1_000_000 * SECOND


#: The link digest the tests' beacons carry: the Rust's tests' too.
LINKS = bytes([0x5A] * 16)


def a_beacon(sequence=2, digest=None, version=(0, 1, 0), flagged=False,
             interface_digest=None, links=LINKS):
    """A beacon, as tcspecial sends one.

    The name, the version, the digest, the number, the restart flag, the
    link digest and the interface's digest, as text, a colon between each.

    Args:
        sequence:         The beacon's number.
        digest:           The configuration's digest, sixteen byte values;
                          ``None`` for a fixed one.
        version:          The version, major, minor and patch.
        flagged:          Whether the restart flag is set.
        interface_digest: The interface's digest, sixteen bytes; ``None`` for
                          this tree's.
        links:            The link digest, sixteen bytes.

    Returns:
        The beacon's bytes.
    """
    digest = digest or [36, 195, 155, 114, 7, 4, 241, 185,
                        121, 87, 153, 67, 108, 53, 167, 176]
    interface_digest = interface_digest or interface.of_this_tree()
    major, minor, patch = version
    flag = "restart" if flagged else "normal"
    return (f"tcspecial:{major}.{minor}.{patch}:{bytes(digest).hex()}:"
            f"{sequence}:{flag}:{links.hex()}:{bytes(interface_digest).hex()}"
            ).encode("ascii")


def schedule():
    """A short schedule for the tests: four seconds heard, then waiting.

    Returns:
        The :class:`Schedule`.
    """
    return Schedule([
        Step(holds=4 * SECOND, stage=Stage.HEARD),
        Step(holds=beacon.FOREVER, stage=Stage.WAITING),
    ])


class StageTest(unittest.TestCase):
    def test_each_stage_is_called_what_it_is(self):
        # Stage::label in tcsmoc::beacon, word for word.
        self.assertEqual(
            [stage.label() for stage in Stage],
            ["Unheard", "Heard", "Aging", "Waiting"],
        )


class ScheduleTest(unittest.TestCase):
    def test_contact_is_held_while_the_stage_is_one_of_contact(self):
        s = schedule()
        self.assertEqual(s.contact_at(AT, AT), Contact.MADE)
        self.assertEqual(s.contact_at(AT, AT + 3_999 * MS), Contact.MADE)
        self.assertEqual(s.contact_at(AT, AT + 4_001 * MS), Contact.LOST)
        self.assertEqual(s.contact_at(None, AT), Contact.LOST)
        self.assertEqual(s.contact_at(AT + 60 * SECOND, AT), Contact.LOST)

    def test_a_blinking_stage_of_contact_is_still_contact(self):
        s = Schedule([
            Step(holds=4 * SECOND, stage=Stage.HEARD),
            Step(holds=4 * SECOND, stage=Stage.HEARD,
                 blink=Blink(on=SECOND, off=SECOND)),
            Step(holds=6 * SECOND, stage=Stage.WAITING),
            Step(holds=beacon.FOREVER, stage=Stage.HEARD),
        ])
        self.assertEqual(s.while_in_contact(), 8 * SECOND)
        self.assertEqual(s.contact_at(AT, AT + 7_999 * MS), Contact.MADE)
        self.assertEqual(s.contact_at(AT, AT + 8_001 * MS), Contact.LOST)
        self.assertEqual(s.contact_at(AT, AT + 20 * SECOND), Contact.LOST)

    def test_a_blinking_stage_says_when_it_next_changes(self):
        s = Schedule([
            Step(holds=2 * SECOND, stage=Stage.HEARD),
            Step(holds=beacon.FOREVER, stage=Stage.WAITING,
                 blink=Blink(on=500 * MS, off=500 * MS)),
        ])
        fresh = s.at(0)
        self.assertEqual((fresh.stage, fresh.lit, fresh.until),
                         (Stage.HEARD, True, 2 * SECOND))
        lit = s.at(2_100 * MS)
        self.assertEqual((lit.stage, lit.lit, lit.until),
                         (Stage.WAITING, True, 400 * MS))
        dark = s.at(2_600 * MS)
        self.assertEqual((dark.lit, dark.until), (False, 400 * MS))
        old = s.at(3_600 * SECOND)
        self.assertEqual(old.stage, Stage.WAITING)
        self.assertIsNotNone(old.until)

    def test_the_standard_schedule_holds_contact_through_one_missed_beacon(
            self):
        s = Schedule.standard()
        self.assertEqual(s.at(0).stage, Stage.HEARD)
        self.assertEqual(s.at(6 * SECOND).stage, Stage.HEARD)
        self.assertEqual(s.while_in_contact(), 20 * SECOND)
        self.assertEqual(s.at(19 * SECOND).stage, Stage.HEARD)
        self.assertEqual(s.at(21 * SECOND).stage, Stage.AGING)
        self.assertEqual(s.at(39 * SECOND).stage, Stage.AGING)
        self.assertEqual(s.at(41 * SECOND).stage, Stage.WAITING)
        self.assertEqual(s.at(86_400 * SECOND).stage, Stage.WAITING)


    def test_a_schedule_scales_with_the_beacon_interval(self):
        def holds(schedule):
            return [step.holds for step in schedule.steps()]

        def blinks(schedule):
            return [step.blink for step in schedule.steps()]

        standard = Schedule.standard()
        ten = Schedule.for_interval(10 * SECOND)
        self.assertEqual(holds(Schedule.for_interval(5 * SECOND)),
                         holds(standard))
        self.assertEqual(holds(ten),
                         [20 * SECOND, 20 * SECOND, 40 * SECOND, None])
        self.assertEqual(blinks(ten), blinks(standard))
        self.assertEqual(ten.while_in_contact(), 40 * SECOND)
        self.assertEqual(ten.at(39 * SECOND).stage, Stage.HEARD)
        self.assertEqual(ten.at(41 * SECOND).stage, Stage.AGING)
        self.assertEqual(ten.at(81 * SECOND).stage, Stage.WAITING)
        self.assertEqual(Schedule.for_interval(250 * MS).steps()[0].holds,
                         500 * MS)
        self.assertEqual(holds(Schedule.for_interval(0)), holds(standard))


    def test_the_stage_is_the_number_of_beacons_missed(self):
        def stage_for(count):
            """The stage for a number of beacons missed, by the thresholds.

            Args:
                count: How many beacons in a row have gone missing.

            Returns:
                The :class:`Stage`.
            """
            if count >= beacon.WAITING_AFTER_MISSED:
                return Stage.WAITING
            if count >= beacon.AGING_AFTER_MISSED:
                return Stage.AGING
            return Stage.HEARD

        # tcsmoc::beacon's thresholds, figure for figure.
        self.assertEqual(
            (beacon.BLINKING_AFTER_MISSED, beacon.AGING_AFTER_MISSED,
             beacon.WAITING_AFTER_MISSED),
            (2, 4, 8),
        )
        for interval in [250 * MS, 5 * SECOND, 60 * SECOND]:
            s = Schedule.for_interval(interval)
            for count in range(12):
                for elapsed in [interval * count,
                                interval * count + interval // 2,
                                interval * (count + 1) - 1]:
                    self.assertEqual(
                        s.at(elapsed).stage,
                        stage_for(beacon.missed(elapsed, interval)),
                        (elapsed, interval))
        self.assertEqual(beacon.missed(9 * SECOND, 5 * SECOND), 1)
        self.assertEqual(beacon.missed(10 * SECOND, 5 * SECOND), 2)
        self.assertEqual(beacon.missed(10 * SECOND, 0), 0)


class LayoutTest(unittest.TestCase):
    """tcsprotocol/beacon.py, held to the layout and to the Rust's own
    example."""

    def test_a_beacon_is_its_fields_as_text_a_colon_between_each(self):
        digest = bytes([0xD4, 0x1D, 0x8C, 0xD9, 0x8F, 0x00, 0xB2, 0x04,
                        0xE9, 0x80, 0x09, 0x98, 0xEC, 0xF8, 0x42, 0x7E])
        packed = Beacon(sequence=0x01020304, version=(1, 2, 3),
                        digest=digest, restart_flag=True,
                        links=LINKS).to_bytes()
        self.assertEqual(packed, a_beacon(sequence=0x01020304,
                                          digest=list(digest),
                                          version=(1, 2, 3),
                                          flagged=True))
        self.assertEqual(
            packed,
            b"tcspecial:1.2.3:d41d8cd98f00b204e9800998ecf8427e:16909060:"
            b"restart:5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a:"
            + interface.of_this_tree().hex().encode())
        longest = Beacon(sequence=2 ** 32 - 1, version=(255, 255, 255),
                         digest=digest, restart_flag=True).to_bytes()
        self.assertEqual(len(longest), BEACON_BYTES)
        self.assertEqual(BEACON_BYTES, 139)

    def test_this_trees_interface_is_the_one_the_rust_was_built_with(self):
        # The constant `make interface-digest` wrote for the Rust, read from
        # its source: one digest, computed by two runtimes, must agree.
        source = (Path(__file__).resolve().parents[2] / "tcsprotocol"
                  / "src" / "interface_digest.rs").read_text()
        rust = bytes(int(byte, 16)
                     for byte in __import__("re").findall(r"0x([0-9a-f]{2})",
                                                          source))
        self.assertEqual(interface.of_this_tree(), rust)

    def test_the_beacon_is_what_the_hand_written_packer_wrote(self):
        # What the hand-written packer that came before the generated code
        # wrote for these beacons, with the link digest the beacon has
        # carried since 2026-11-15 before the interface's, which
        # tcsprotocol/src/beacon.rs holds the Rust to as well.
        digest = bytes.fromhex("d41d8cd98f00b204e9800998ecf8427e")
        face = bytes([0xAB] * 16)
        for beacon, written in [
            (Beacon(0x01020304, (1, 2, 3), digest, True, face, LINKS),
             b"tcspecial:1.2.3:d41d8cd98f00b204e9800998ecf8427e:16909060:"
             b"restart:5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a:"
             b"abababababababababababababababab"),
            (Beacon(1, (0, 1, 0), digest, False, face, LINKS),
             b"tcspecial:0.1.0:d41d8cd98f00b204e9800998ecf8427e:1:normal:"
             b"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a:"
             b"abababababababababababababababab"),
            (Beacon(2 ** 32 - 1, (255, 255, 255), bytes(16), True, face,
                    bytes([0xFF] * 16)),
             b"tcspecial:255.255.255:00000000000000000000000000000000:"
             b"4294967295:restart:ffffffffffffffffffffffffffffffff:"
             b"abababababababababababababababab"),
        ]:
            self.assertEqual(beacon.to_bytes(), written)
            self.assertEqual(Beacon.read(written), beacon)
        self.assertEqual(len(
            Beacon(2 ** 32 - 1, (255, 255, 255), bytes(16), True, face)
            .to_bytes()), BEACON_BYTES)

    def test_a_beacon_reads_back_as_itself(self):
        sent = Beacon(sequence=7, version=(1, 2, 3), digest=bytes(range(16)),
                      restart_flag=True)
        self.assertEqual(Beacon.read(sent.to_bytes()), sent)
        with self.assertRaises(BeaconError):
            Beacon.read(sent.to_bytes() + b"\0")


class HearingTest(unittest.TestCase):
    def test_a_beacon_announcing_another_configuration_says_so(self):
        ours, theirs = "ab" * 16, "cd" * 16
        self.assertIsNone(beacon.differs(ours, ours))
        said = beacon.differs(theirs, ours)
        self.assertIn(theirs, said)
        self.assertIn(ours, said)

        datagram = a_beacon(digest=[0xCD] * 16)
        heard = beacon.hear(datagram, ours)
        self.assertIsNotNone(heard.differs)
        self.assertTrue(heard.message.startswith("CONFIGURATION DIFFERS"),
                        heard.message)
        self.assertIsNone(beacon.hear(datagram, theirs).differs)
        self.assertIsNone(beacon.hear(datagram, None).differs)

    def test_a_beacon_is_shown_by_what_it_said(self):
        datagram = a_beacon(sequence=3, digest=[0xAB] * 16, version=(1, 2, 3))
        want = f"tcspecial v1.2.3 md5 {'ab' * 16}"
        self.assertEqual(beacon.message_of(datagram), want)
        flagged = a_beacon(sequence=3, digest=[0xAB] * 16,
                           version=(1, 2, 3), flagged=True)
        self.assertEqual(
            beacon.message_of(flagged),
            f"tcspecial v1.2.3 md5 {'ab' * 16}, RESTARTED, NOT YET MATCHED")
        heard = beacon.hear(flagged, None)
        self.assertTrue(heard.restart_flag)
        self.assertFalse(beacon.hear(datagram, None).restart_flag)
        self.assertEqual(beacon.hear(datagram, None).sequence, 3)
        self.assertIsNone(beacon.hear(b"hello", None).sequence)

        shown = beacon.message_of(b"hello")
        self.assertIn("unreadable", shown)
        self.assertIn("hello", shown)
        shown = beacon.message_of(b"x" * 200)
        self.assertTrue(shown.endswith("..."), shown)
        self.assertLess(len(shown), 60)

        # Something that is not a beacon is shown as what arrived, and the
        # start of it is enough to name it.
        stray = b"Ping from somewhere that is not tcspecial"
        shown = beacon.message_of(stray)
        self.assertIn("unreadable", shown)
        self.assertIn("Ping", shown)

    def test_a_beacon_of_another_length_or_without_its_name_is_not_one(self):
        good = a_beacon(sequence=3)
        self.assertEqual(beacon.hear(good, None).sequence, 3)
        self.assertIsNone(beacon.hear(good + b"\0", None).sequence)
        self.assertIsNone(beacon.hear(good[:-1], None).sequence)
        self.assertIsNone(
            beacon.hear(b"T" + good[1:], None).sequence)
        # Each of the Rust's refusals, said again.
        md5, face = "d4" * 16, interface.of_this_tree().hex()
        links = LINKS.hex()
        for bad in (
            f"tcspecial:1.2:{md5}:1:normal:{links}:{face}",
            f"tcspecial:1.2.256:{md5}:1:normal:{links}:{face}",
            f"tcspecial:1.2.3:{md5.upper()}:1:normal:{links}:{face}",
            f"tcspecial:1.2.3:{md5}:+1:normal:{links}:{face}",
            f"tcspecial:1.2.3:{md5}:4294967296:normal:{links}:{face}",
            f"tcspecial:1.2.3:{md5}:1:yes:{links}:{face}",
            f"tcspecial:1.2.3:{md5}:1:normal:{links[1:]}:{face}",
            f"tcspecial:1.2.3:{md5}:1:normal:{links.upper()}:{face}",
            # A beacon of the six fields before the link digest.
            f"tcspecial:1.2.3:{md5}:1:normal:{face}",
            f"tcspecial:1.2.3:{md5}:1:normal:{links}",
            f"tcspecial:1.2.3:{md5}:1:normal:{links}:{face}:",
        ):
            self.assertIsNone(beacon.hear(bad.encode(), None).sequence, bad)

    def test_a_beacon_that_worked_its_links_out_otherwise_says_so(self):
        # As tcsmoc::beacon's test of the same says it.
        ours = "6b" * 16
        datagram = a_beacon(links=bytes([0x7C] * 16))
        heard = beacon.hear(datagram, None, ours)
        self.assertTrue(heard.differs.startswith("LINKS DIFFER"),
                        heard.differs)
        self.assertIn("7c" * 16, heard.differs)
        self.assertIn(ours, heard.differs)
        self.assertEqual(beacon.heading(heard.differs),
                         "tcspecial worked the configuration out differently")
        self.assertIsNone(beacon.hear(datagram, None, "7c" * 16).differs)
        self.assertIsNone(beacon.hear(datagram, None).differs)

        # Another interface is said whatever the station has to compare.
        other = a_beacon(interface_digest=bytes([0x11] * 16))
        heard = beacon.hear(other, None)
        self.assertTrue(heard.differs.startswith("INTERFACE DIFFERS"),
                        heard.differs)
        self.assertIn("11" * 16, heard.differs)

    def test_a_beacons_number_says_what_happened_before_it(self):
        self.assertEqual(beacon.counted(None, 7), (Counted.FIRST, 0))
        self.assertEqual(beacon.counted(7, 8), (Counted.NEXT, 0))
        self.assertEqual(beacon.counted(7, 10), (Counted.MISSED, 2))
        self.assertEqual(beacon.counted(7, 7), (Counted.REPEATED, 0))
        self.assertEqual(beacon.counted(7, 6), (Counted.RESTARTED, 0))
        self.assertEqual(beacon.counted(7, 1), (Counted.RESTARTED, 0))

    def test_a_beacon_is_shown_by_when_it_arrived(self):
        self.assertEqual(beacon.received_at(None), "--:--:--")
        self.assertEqual(
            beacon.received_at((3600 + 2 * 60 + 3) * SECOND + 999),
            "01:02:03")


class LoopTest(unittest.TestCase):
    """What a pass keeps and says, exercised without a socket."""

    def setUp(self):
        self.now = AT
        link = beacon_link()
        self.beacons = beacon.Beacons(link, Schedule.standard(),
                                      "24c39b72" + "0" * 24,
                                      clock=lambda: self.now)

    def test_every_pass_says_when_the_last_beacon_arrived(self):
        first = self.beacons.heard(a_beacon(sequence=1), ("127.0.0.1", 7500))
        self.assertEqual(first.arrived, beacon.Arrived.BEACON)
        self.assertEqual(first.at, AT)
        self.assertEqual(first.contact, Contact.MADE)

        self.now = AT + 25 * SECOND
        later = self.beacons.after(beacon.Arrived.NOTHING, [])
        self.assertEqual(
            later.at, AT,
            "a pass that found nothing keeps the beacon before it")
        self.assertEqual(later.said, first.said)
        self.assertEqual(later.showing.stage, Stage.AGING)
        self.assertEqual(later.contact, Contact.LOST)

        broken = self.beacons.after(beacon.Arrived.BROKEN, [])
        self.assertEqual(broken.showing.stage, Stage.UNHEARD)
        self.assertEqual(broken.at, AT)

    def test_remarks_on_missed_beacons_a_restart_and_a_differing_config(self):
        sender = ("127.0.0.1", 7500)
        first = self.beacons.heard(a_beacon(sequence=1), sender)
        self.assertTrue(
            any("CONFIGURATION DIFFERS" in r for r in first.remarks),
            first.remarks)
        again = self.beacons.heard(a_beacon(sequence=4), sender)
        self.assertIn("2 beacons did not arrive: #4 came after #1",
                      again.remarks)
        self.assertFalse(any("DIFFERS" in r for r in again.remarks),
                         "said once, when it starts")
        repeat = self.beacons.heard(a_beacon(sequence=4), sender)
        self.assertEqual(repeat.remarks, [], "the radio's repeat")
        started = self.beacons.heard(
            a_beacon(sequence=1, flagged=True), sender)
        self.assertIn("the beacons start again at #1, so tcspecial has "
                      "restarted", started.remarks)
        self.beacons.heard(a_beacon(sequence=3), sender)
        # A start whose #1 was lost: the number still went back.
        crashed = self.beacons.heard(a_beacon(sequence=2), sender)
        self.assertIn("the beacons start again at #2, so tcspecial has "
                      "restarted", crashed.remarks)

    def test_a_datagram_from_the_wrong_place_is_not_a_beacon(self):
        wrong = self.beacons.heard(a_beacon(), ("127.0.0.1", 9999))
        self.assertEqual(wrong.arrived, beacon.Arrived.NOTHING)
        self.assertIsNone(wrong.at)
        # A warning, said whether or not the console was asked to talk.
        self.assertTrue(any("ignored" in r for r in wrong.warnings),
                        wrong.warnings)
        self.assertEqual(wrong.remarks, [])
        # The same stranger is not warned of again; another one is.
        again = self.beacons.heard(a_beacon(), ("127.0.0.1", 9999))
        self.assertEqual(again.warnings, [])
        other = self.beacons.heard(a_beacon(), ("127.0.0.1", 9998))
        self.assertTrue(any("127.0.0.1:9998" in r for r in other.warnings),
                        other.warnings)


def beacon_link():
    from tcsprotocol.configuration import BeaconLink
    return BeaconLink(send=HostPort("0.0.0.0", 7500),
                      recv=HostPort("127.0.0.1", 7510))


if __name__ == "__main__":
    unittest.main()
