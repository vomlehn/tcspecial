"""The line "Powered by TCSpecial", small and centred, atop a Tk window.

The Python spelling of ``tcs-ui/ui/powered.slint``: the same words, in the
same size and grey, so that every window an example shows, Slint or Tk, says
where it comes from in the same way.
"""

import tkinter.font
from tkinter import ttk

#: What the line says.
TEXT = "Powered by TCSpecial"

#: Its size, in points: smaller than :data:`tcs_ui.fonts.POINTS`, as it says
#: where the window comes from rather than anything about the vehicle.
POINTS = 9

#: Its colour: the grey of a note, ``Note.TLabel`` in :mod:`tcs_ui.theme`.
COLOR = "#4b5563"


def pack_in(window, between):
    """Put the line at the top of a window, centred across it.

    Called before anything else is packed in the window, after
    :func:`tcs_ui.theme.use_in`, so that it is first, above everything.

    Args:
        window:  The window, laid out with ``pack``.
        between: The space above the line, in pixels: the window's
                 :data:`tcs_ui.theme.BETWEEN`, scaled as the window scales
                 it. The next box's own space above it keeps it apart from
                 the line.

    Returns:
        The label the line is.
    """
    family = tkinter.font.nametofont("TkDefaultFont", root=window).actual(
        "family"
    )
    line = ttk.Label(window, text=TEXT, font=(family, POINTS),
                     foreground=COLOR, anchor="center")
    line.pack(side="top", fill="x", pady=(between // 2, 0))
    return line
