"""Tk's named fonts are set in tcsui's one default size."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcsui import fonts  # noqa: E402


class FontsTest(unittest.TestCase):
    def test_every_named_font_is_the_default_size(self):
        import tkinter
        import tkinter.font

        try:
            window = tkinter.Tk()
        except tkinter.TclError as why:
            self.skipTest(f"no display for Tk: {why}")
        try:
            fonts.use_in(window)
            for name in tkinter.font.names(window):
                font = tkinter.font.Font(root=window, name=name, exists=True)
                self.assertEqual(font.actual("size"), fonts.POINTS, name)
        finally:
            window.destroy()


if __name__ == "__main__":
    unittest.main()
