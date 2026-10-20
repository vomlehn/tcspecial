"""The beacon's facts and its panel, held to the Rust's own examples.

The tests of ``tcslib/src/beacon.rs`` and ``tcsui/src/lib.rs`` said again,
with the same figures. The beacons are JSON as tcspecial sends it, captured
from a running vehicle; ``examples/ping/tests/pings.rs`` sends this program
beacons the Rust serializes, so a change to that JSON is caught at both ends.
"""

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslib import beacon  # noqa: E402
from tcslib.beacon import Blink, Contact, Counted, Schedule, Stage, Step  # noqa: E402
from tcslibgs.configuration import HostPort  # noqa: E402

MS = 1_000_000
SECOND = 1_000_000_000
AT = 1_000_000 * SECOND


def a_beacon(sequence=2, digest=None, version=(0, 1, 0)):
    """A beacon, as tcspecial sends one."""
    digest = digest or [36, 195, 155, 114, 7, 4, 241, 185, 121, 87, 153, 67, 108, 53, 167, 176]
    return json.dumps({
        "Beacon": {
            "header": {
                "sequence": sequence,
                "tm_type": "Beacon",
                "status": "Success",
                "timestamp": {"micros": 1792436939903024},
            },
            "version": dict(zip(("major", "minor", "patch"), version)),
            "digest": digest,
        }
    }).encode()


def schedule():
    return Schedule([
        Step(holds=4 * SECOND, stage=Stage.HEARD),
        Step(holds=beacon.FOREVER, stage=Stage.SILENT),
    ])


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
            Step(holds=4 * SECOND, stage=Stage.HEARD, blink=Blink(on=SECOND, off=SECOND)),
            Step(holds=6 * SECOND, stage=Stage.SILENT),
            Step(holds=beacon.FOREVER, stage=Stage.HEARD),
        ])
        self.assertEqual(s.while_in_contact(), 8 * SECOND)
        self.assertEqual(s.contact_at(AT, AT + 7_999 * MS), Contact.MADE)
        self.assertEqual(s.contact_at(AT, AT + 8_001 * MS), Contact.LOST)
        self.assertEqual(s.contact_at(AT, AT + 20 * SECOND), Contact.LOST)

    def test_a_blinking_stage_says_when_it_next_changes(self):
        s = Schedule([
            Step(holds=2 * SECOND, stage=Stage.HEARD),
            Step(holds=beacon.FOREVER, stage=Stage.SILENT, blink=Blink(on=500 * MS, off=500 * MS)),
        ])
        fresh = s.at(0)
        self.assertEqual((fresh.stage, fresh.lit, fresh.until), (Stage.HEARD, True, 2 * SECOND))
        lit = s.at(2_100 * MS)
        self.assertEqual((lit.stage, lit.lit, lit.until), (Stage.SILENT, True, 400 * MS))
        dark = s.at(2_600 * MS)
        self.assertEqual((dark.lit, dark.until), (False, 400 * MS))
        old = s.at(3_600 * SECOND)
        self.assertEqual(old.stage, Stage.SILENT)
        self.assertIsNotNone(old.until)

    def test_the_standard_schedule_holds_contact_through_one_missed_beacon(self):
        s = Schedule.standard()
        self.assertEqual(s.at(0).stage, Stage.HEARD)
        self.assertEqual(s.at(6 * SECOND).stage, Stage.HEARD)
        self.assertEqual(s.while_in_contact(), 20 * SECOND)
        self.assertEqual(s.at(19 * SECOND).stage, Stage.HEARD)
        self.assertEqual(s.at(21 * SECOND).stage, Stage.AGING)
        self.assertEqual(s.at(39 * SECOND).stage, Stage.AGING)
        self.assertEqual(s.at(41 * SECOND).stage, Stage.SILENT)
        self.assertEqual(s.at(86_400 * SECOND).stage, Stage.SILENT)


    def test_a_schedule_scales_with_the_beacon_interval(self):
        def holds(schedule):
            return [step.holds for step in schedule.steps()]

        def blinks(schedule):
            return [step.blink for step in schedule.steps()]

        standard = Schedule.standard()
        ten = Schedule.for_interval(10 * SECOND)
        self.assertEqual(holds(Schedule.for_interval(5 * SECOND)), holds(standard))
        self.assertEqual(holds(ten), [20 * SECOND, 20 * SECOND, 40 * SECOND, None])
        self.assertEqual(blinks(ten), blinks(standard))
        self.assertEqual(ten.while_in_contact(), 40 * SECOND)
        self.assertEqual(ten.at(39 * SECOND).stage, Stage.HEARD)
        self.assertEqual(ten.at(41 * SECOND).stage, Stage.AGING)
        self.assertEqual(ten.at(81 * SECOND).stage, Stage.SILENT)
        self.assertEqual(Schedule.for_interval(250 * MS).steps()[0].holds, 500 * MS)
        self.assertEqual(holds(Schedule.for_interval(0)), holds(standard))


    def test_the_stage_is_the_number_of_beacons_missed(self):
        def stage_for(count):
            if count >= beacon.SILENT_AFTER_MISSED:
                return Stage.SILENT
            if count >= beacon.AGING_AFTER_MISSED:
                return Stage.AGING
            return Stage.HEARD

        # tcslib::beacon's thresholds, figure for figure.
        self.assertEqual(
            (beacon.BLINKING_AFTER_MISSED, beacon.AGING_AFTER_MISSED, beacon.SILENT_AFTER_MISSED),
            (2, 4, 8),
        )
        for interval in [250 * MS, 5 * SECOND, 60 * SECOND]:
            s = Schedule.for_interval(interval)
            for count in range(12):
                for elapsed in [interval * count, interval * count + interval // 2,
                                interval * (count + 1) - 1]:
                    self.assertEqual(s.at(elapsed).stage,
                                     stage_for(beacon.missed(elapsed, interval)),
                                     (elapsed, interval))
        self.assertEqual(beacon.missed(9 * SECOND, 5 * SECOND), 1)
        self.assertEqual(beacon.missed(10 * SECOND, 5 * SECOND), 2)
        self.assertEqual(beacon.missed(10 * SECOND, 0), 0)


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
        self.assertTrue(heard.message.startswith("CONFIGURATION DIFFERS"), heard.message)
        self.assertIsNone(beacon.hear(datagram, theirs).differs)
        self.assertIsNone(beacon.hear(datagram, None).differs)

    def test_a_beacon_is_shown_by_what_it_said(self):
        datagram = a_beacon(sequence=3, digest=[0xAB] * 16, version=(1, 2, 3))
        want = f"tcspecial v1.2.3 md5 {'ab' * 16}"
        self.assertEqual(beacon.message_of(datagram), want)
        self.assertEqual(beacon.hear(datagram, None).sequence, 3)
        self.assertIsNone(beacon.hear(b"hello", None).sequence)

        shown = beacon.message_of(b"hello")
        self.assertIn("unreadable", shown)
        self.assertIn("hello", shown)
        shown = beacon.message_of(b"x" * 200)
        self.assertTrue(shown.endswith("..."), shown)
        self.assertLess(len(shown), 60)

        ping = json.dumps({"Ping": {"header": {"sequence": 1}}}).encode()
        self.assertIn("not a beacon", beacon.message_of(ping))

    def test_a_beacons_number_says_what_happened_before_it(self):
        self.assertEqual(beacon.counted(None, 7), (Counted.FIRST, 0))
        self.assertEqual(beacon.counted(7, 8), (Counted.NEXT, 0))
        self.assertEqual(beacon.counted(7, 10), (Counted.MISSED, 2))
        self.assertEqual(beacon.counted(7, 7), (Counted.RESTARTED, 0))
        self.assertEqual(beacon.counted(7, 1), (Counted.RESTARTED, 0))

    def test_a_beacon_is_shown_by_when_it_arrived(self):
        self.assertEqual(beacon.received_at(None), "--:--:--")
        self.assertEqual(beacon.received_at((3600 + 2 * 60 + 3) * SECOND + 999), "01:02:03")


class LoopTest(unittest.TestCase):
    """What a pass keeps and says, exercised without a socket."""

    def setUp(self):
        self.now = AT
        link = beacon_link()
        self.beacons = beacon.Beacons(link, Schedule.standard(), "24c39b72" + "0" * 24,
                                      clock=lambda: self.now)

    def test_every_pass_says_when_the_last_beacon_arrived(self):
        first = self.beacons.heard(a_beacon(sequence=1), ("127.0.0.1", 7500))
        self.assertEqual(first.arrived, beacon.Arrived.BEACON)
        self.assertEqual(first.at, AT)
        self.assertEqual(first.contact, Contact.MADE)

        self.now = AT + 25 * SECOND
        later = self.beacons.after(beacon.Arrived.NOTHING, [])
        self.assertEqual(later.at, AT, "a pass that found nothing keeps the beacon before it")
        self.assertEqual(later.said, first.said)
        self.assertEqual(later.showing.stage, Stage.AGING)
        self.assertEqual(later.contact, Contact.LOST)

        broken = self.beacons.after(beacon.Arrived.BROKEN, [])
        self.assertEqual(broken.showing.stage, Stage.UNHEARD)
        self.assertEqual(broken.at, AT)

    def test_missed_beacons_a_restart_and_a_differing_configuration_are_remarked_on(self):
        sender = ("127.0.0.1", 7500)
        first = self.beacons.heard(a_beacon(sequence=1), sender)
        self.assertTrue(any("CONFIGURATION DIFFERS" in r for r in first.remarks), first.remarks)
        again = self.beacons.heard(a_beacon(sequence=4), sender)
        self.assertIn("2 beacons did not arrive: #4 came after #1", again.remarks)
        self.assertFalse(any("DIFFERS" in r for r in again.remarks), "said once, when it starts")
        restart = self.beacons.heard(a_beacon(sequence=1), sender)
        self.assertTrue(any("start again at #1" in r for r in restart.remarks), restart.remarks)

    def test_a_datagram_from_the_wrong_place_is_not_a_beacon(self):
        wrong = self.beacons.heard(a_beacon(), ("127.0.0.1", 9999))
        self.assertEqual(wrong.arrived, beacon.Arrived.NOTHING)
        self.assertIsNone(wrong.at)
        # A warning, said whether or not the console was asked to talk.
        self.assertTrue(any("ignored" in r for r in wrong.warnings), wrong.warnings)
        self.assertEqual(wrong.remarks, [])
        # The same stranger is not warned of again; another one is.
        again = self.beacons.heard(a_beacon(), ("127.0.0.1", 9999))
        self.assertEqual(again.warnings, [])
        other = self.beacons.heard(a_beacon(), ("127.0.0.1", 9998))
        self.assertTrue(any("127.0.0.1:9998" in r for r in other.warnings), other.warnings)


def beacon_link():
    from tcslibgs.configuration import BeaconLink
    return BeaconLink(send=HostPort("0.0.0.0", 7500), recv=HostPort("127.0.0.1", 7510))


class PanelTest(unittest.TestCase):
    """What the panel shows for each stage: tcsui's palette."""

    def test_the_light_ages_through_the_palette(self):
        from tcsui.beacon_panel import indicator, palette

        s = Schedule.standard()
        self.assertEqual(indicator(s.after(None, AT), False), palette.UNKNOWN)
        self.assertEqual(indicator(s.at(0), True), palette.WELL)
        self.assertEqual(indicator(s.at(6 * SECOND), True), palette.WELL)
        aging = s.at(20_200 * MS)
        self.assertTrue(aging.lit)
        self.assertEqual(indicator(aging, True), palette.WATCH)
        silent = s.at(40_200 * MS)
        self.assertTrue(silent.lit)
        self.assertEqual(indicator(silent, True), palette.WRONG)
        # The dark half of a blink, once a beacon has arrived: white.
        self.assertEqual(indicator(s.at(20_700 * MS), True), palette.OFF)
        self.assertEqual(indicator(s.at(40_700 * MS), True), palette.OFF)
        self.assertEqual(indicator(s.at(11_500 * MS), True), palette.OFF)

    def test_once_heard_the_light_is_never_grey_again(self):
        from tcsui.beacon_panel import indicator, palette

        s = Schedule.standard()
        for ms in range(0, 60_000, 100):
            self.assertNotEqual(indicator(s.at(ms * MS), True), palette.UNKNOWN, ms)
        # A last beacon a moved clock puts in the future is unheard, and white.
        in_the_future = s.after(AT, AT - SECOND)
        self.assertEqual(in_the_future.stage, Stage.UNHEARD)
        self.assertEqual(indicator(in_the_future, True), palette.OFF)

    def test_the_palette_is_tcsuis(self):
        from tcsui.beacon_panel import palette

        # tcsui::palette, byte for byte.
        self.assertEqual(palette.UNKNOWN, "#c4c4c4")
        self.assertEqual(palette.OFF, "#ffffff")
        self.assertEqual(palette.WELL, "#00ff00")
        self.assertEqual(palette.WATCH, "#ffe100")
        self.assertEqual(palette.WRONG, "#ff0000")


if __name__ == "__main__":
    unittest.main()
