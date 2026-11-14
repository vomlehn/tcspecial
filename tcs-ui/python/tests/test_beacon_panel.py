"""The beacon panel, held to the Rust's own examples.

The tests of ``tcs-ui/src/lib.rs`` said again, with the same figures, on the
beacon states ``tcsmoc.beacon`` works out.
"""

import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[2] / "python"))
sys.path.insert(0, str(HERE.parent))

from tcsmoc.beacon import Schedule, Stage  # noqa: E402

MS = 1_000_000
SECOND = 1_000_000_000
AT = 1_000_000 * SECOND


class PanelTest(unittest.TestCase):
    """What the panel shows for each stage: tcs-ui's palette."""

    def test_the_indicator_ages_through_the_palette(self):
        from tcs_ui.beacon_panel import indicator, palette

        s = Schedule.standard()
        self.assertEqual(indicator(s.after(None, AT), False), palette.UNKNOWN)
        self.assertEqual(indicator(s.at(0), True), palette.WELL)
        self.assertEqual(indicator(s.at(6 * SECOND), True), palette.WELL)
        aging = s.at(20_200 * MS)
        self.assertTrue(aging.lit)
        self.assertEqual(indicator(aging, True), palette.WATCH)
        waiting = s.at(40_200 * MS)
        self.assertTrue(waiting.lit)
        self.assertEqual(indicator(waiting, True), palette.WRONG)
        # The dark half of a blink, once a beacon has arrived: white.
        self.assertEqual(indicator(s.at(20_700 * MS), True), palette.OFF)
        self.assertEqual(indicator(s.at(40_700 * MS), True), palette.OFF)
        self.assertEqual(indicator(s.at(11_500 * MS), True), palette.OFF)

    def test_once_heard_the_indicator_is_never_grey_again(self):
        from tcs_ui.beacon_panel import indicator, palette

        s = Schedule.standard()
        for ms in range(0, 60_000, 100):
            self.assertNotEqual(indicator(s.at(ms * MS), True),
                                palette.UNKNOWN, ms)
        # A last beacon a moved clock puts in the future is unheard, and white.
        in_the_future = s.after(AT, AT - SECOND)
        self.assertEqual(in_the_future.stage, Stage.UNHEARD)
        self.assertEqual(indicator(in_the_future, True), palette.OFF)

    def test_the_panel_shows_what_it_was_given(self):
        import tkinter
        from tcs_ui.beacon_panel import NOTHING_YET, BeaconPanel, Shown

        try:
            window = tkinter.Tk()
        except tkinter.TclError as why:
            self.skipTest(f"no display for Tk: {why}")
        try:
            panel = BeaconPanel(window)
            before = panel.showing()
            self.assertEqual((before.indicator, before.received,
                              before.message),
                             (NOTHING_YET.indicator, NOTHING_YET.received,
                              NOTHING_YET.message))
            shown = Shown(indicator="#22c55e", received="12:34:56",
                          message="tcspecial v0.1.0", stage="Heard")
            panel.show(shown)
            after = panel.showing()
            self.assertEqual((after.indicator, after.received, after.message),
                             (shown.indicator, shown.received, shown.message))
            # The panel shows no stage, and says so.
            self.assertEqual(after.stage, "")
        finally:
            window.destroy()

    def test_the_palette_is_tcs_uis(self):
        from tcs_ui.beacon_panel import palette

        # tcs_ui::palette, byte for byte.
        self.assertEqual(palette.UNKNOWN, "#c4c4c4")
        self.assertEqual(palette.OFF, "#ffffff")
        self.assertEqual(palette.WELL, "#22c55e")
        self.assertEqual(palette.WATCH, "#ffe100")
        self.assertEqual(palette.WRONG, "#ff0000")


if __name__ == "__main__":
    unittest.main()
