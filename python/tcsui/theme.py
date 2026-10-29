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

#: The window's background, and every box's: white rather than the theme's
#: grey, which is the one colour the beacon indicator keeps for "nothing heard
#: yet" and would leave an unheard indicator nearly the colour of its window.
#: The Slint windows read the same white from tcsui/ui/colors.slint.
BACKGROUND = "#ffffff"

#: A box's edge: one flat pixel, the colour tcsui's TitledBox draws in, rather
#: than the theme's raised bevel.
BORDER = "#808080"

#: A button's face, a light grey that stands off the white so the button reads
#: as a thing to press, darker under the pointer and darker again pressed, in
#: a frame darker than the face.
BUTTON = "#f1f3f5"
BUTTON_ACTIVE = "#e2e6ea"
BUTTON_PRESSED = "#d3d9df"
BUTTON_BORDER = "#9aa1a9"

#: The space inside a box, and between one box and the next or the window's
#: edge: two sizes for the whole window, so that it looks the same throughout.
INSIDE = 8
BETWEEN = 12


def use_in(window):
    """Draw this window, and every window the application opens after it, in
    :data:`THEME`.

    Called once, on the root window, before anything is put in it, as
    :func:`tcsui.fonts.use_in` is.

    Args:
        window: The root window.
    """
    style = ttk.Style(window)
    style.theme_use(THEME)
    style.configure(".", background=BACKGROUND)
    style.configure(
        "TLabelframe",
        background=BACKGROUND,
        bordercolor=BORDER,
        relief="solid",
        borderwidth=1,
    )
    style.configure("TLabelframe.Label", background=BACKGROUND)
    style.configure(
        "TButton",
        background=BUTTON,
        bordercolor=BUTTON_BORDER,
        lightcolor=BUTTON,
        darkcolor=BUTTON,
        relief="solid",
        borderwidth=1,
        padding=(12, 4),
    )
    style.map(
        "TButton",
        background=[("pressed", BUTTON_PRESSED), ("active", BUTTON_ACTIVE)],
        lightcolor=[("pressed", BUTTON_PRESSED), ("active", BUTTON_ACTIVE)],
        darkcolor=[("pressed", BUTTON_PRESSED), ("active", BUTTON_ACTIVE)],
    )
    window.configure(background=background(window))
    # The kinds of text a panel holds, all in the one size every window is
    # set in:
    #
    # * a value that changes, plain, in the window's own face, whose digits
    #   are all one width, so that a field does not shift as they change;
    # * a button, plain, as every value is;
    # * a note about what the panel shows, grey.
    #
    # By the families Tk's named fonts are in: a named font's name in a font
    # description is read as a family nobody has, and quietly becomes another.
    points = fonts.POINTS
    text = tkinter.font.nametofont("TkDefaultFont", root=window).actual(
        "family"
    )
    # A box's title, bold, as tcsui's TitledBox sets it.
    style.configure("TLabelframe.Label", font=(text, points, "bold"))
    style.configure("Value.TLabel", font=(text, points))
    style.configure("TButton", font=(text, points))
    style.configure("Note.TLabel", font=(text, points), foreground="#4b5563")


def background(widget):
    """The theme's background, for what Tk draws that ttk does not -- a
    canvas, the window itself -- so that it does not show as a grey patch."""
    return ttk.Style(widget).lookup("TFrame", "background")
