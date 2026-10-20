"""How the Tk windows look: one ttk theme, chosen once, for every window.

The Python counterpart of the Slint windows' style. Tk's own widgets are drawn
the way Tk drew them in the nineties -- flat grey, raised buttons, one-pixel
borders -- and ttk's themes are the standard library's way past that. ``clam``
is chosen because it is on every Tk, whatever the platform, and draws a
titled box, a default button and a field alike everywhere: a window that
looked one way on one desktop and another on the next would be two windows to
learn.
"""

import tkinter.font
from tkinter import ttk

from tcsui import fonts

#: The theme every window is drawn in.
THEME = "clam"


def use_in(window):
    """Draw this window, and every window the application opens after it, in
    :data:`THEME`. Called once, on the root window, before anything is put in
    it, as :func:`tcsui.fonts.use_in` is."""
    style = ttk.Style(window)
    style.theme_use(THEME)
    window.configure(background=background(window))
    # The kinds of text a panel holds, in the sizes the Slint panels set
    # them in, each a multiple of the one size every window is set in:
    #
    # * a heading over one part of a panel, bold;
    # * a value that changes, in the window's own face, whose digits are all
    #   one width, so that a field does not shift as they change;
    # * the figure a panel is read for, a value larger than the rest;
    # * a note about what the panel shows, grey -- and no smaller, nothing in
    #   any window being smaller than the one size.
    #
    # By the families Tk's named fonts are in: a named font's name in a font
    # description is read as a family nobody has, and quietly becomes another.
    points = fonts.POINTS
    text = tkinter.font.nametofont("TkDefaultFont", root=window).actual("family")
    style.configure("Heading.TLabel", font=(text, points, "bold"))
    # A box's title, bold, as tcsui's TitledBox sets it.
    style.configure("TLabelframe.Label", font=(text, points, "bold"))
    style.configure("Value.TLabel", font=(text, points))
    style.configure("Figure.TLabel", font=(text, round(points * 1.5)))
    style.configure("Note.TLabel", font=(text, points), foreground="#4b5563")


def background(widget):
    """The theme's background, for what Tk draws that ttk does not -- a
    canvas, the window itself -- so that it does not show as a grey patch."""
    return ttk.Style(widget).lookup("TFrame", "background")
