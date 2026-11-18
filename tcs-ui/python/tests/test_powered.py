"""The powered-by line is at the top of a Tk window, as the Slint one is."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "python"))

from tcs_ui import powered, theme  # noqa: E402

#: tcs-ui/ui/powered.slint, which says the same thing to the Slint windows.
SLINT = Path(__file__).resolve().parents[2] / "ui" / "powered.slint"


class PoweredTest(unittest.TestCase):
    def test_the_line_is_first_and_says_what_powers_the_window(self):
        """It is packed before anything else, in the words and size set."""
        import tkinter
        import tkinter.font
        from tkinter import ttk

        try:
            window = tkinter.Tk()
        except tkinter.TclError as why:
            self.skipTest(f"no display for Tk: {why}")
        try:
            theme.use_in(window)
            line = powered.pack_in(window, theme.BETWEEN)
            ttk.Label(window, text="below").pack()
            self.assertIs(window.pack_slaves()[0], line)
            self.assertEqual(line.cget("text"), "Powered by TCSpecial")
            font = tkinter.font.Font(root=window, font=line.cget("font"))
            self.assertEqual(font.actual("size"), powered.POINTS)
        finally:
            window.destroy()

    def test_the_slint_line_says_the_same(self):
        """The words, size and grey are the Slint component's too."""
        markup = SLINT.read_text()
        self.assertIn(f'text: "{powered.TEXT}";', markup)
        self.assertIn(f"font-size: {powered.POINTS}pt;", markup)
        self.assertIn(f"color: {powered.COLOR};", markup)


if __name__ == "__main__":
    unittest.main()
