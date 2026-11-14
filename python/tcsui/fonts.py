"""The type every window is set in, where its text names no size of its own.

The Python spelling of ``tcsui/ui/fonts.slint``: the same figure, in points,
for the Tk windows. Text that states a size keeps it; this is the size of
everything that does not -- Tk's named fonts, which a label, a button and a
field are drawn in unless told otherwise.
"""

import tkinter.font

#: The default type size, in points: ``Fonts.window`` in ``fonts.slint``.
POINTS = 12


def use_in(window):
    """Set every one of Tk's named fonts in this window to :data:`POINTS`.

    Called once, on the root window, before anything is put in it: the named
    fonts are the application's, so every window it opens after that has
    them too. A positive size is points to Tk, as it is to Slint.

    Args:
        window: The root window.
    """
    for name in tkinter.font.names(window):
        tkinter.font.Font(root=window, name=name, exists=True).configure(
            size=POINTS
        )
