"""The beacon panel, and what to show on it.

The Python spelling of ``tcsui``: ``tcsui/src/lib.rs``, which says what colour
the indicator is and what the two lines say, and ``tcsui/ui/beacon.slint``,
which is the panel itself. The panel is drawn with tkinter, which is the
toolkit the Python standard library has.

The other half of ``python/tcslib/beacon.py``. That one listens and says what
is true of the spacecraft; this one says what that looks like. They are apart
for the reason the Rust halves are: a ground station with no window wants the
facts with no toolkit loaded, and two stations that do have windows want the
same panel rather than two that have drifted -- the same indicator in the same
place, the same labels in the same words.

Nothing is computed in the panel. :func:`shown` works out the three things it
shows, from a pass of the beacon loop, and :meth:`BeaconPanel.show` puts them
on it.
"""

import tkinter as tk
import tkinter.font
from tkinter import ttk
from dataclasses import dataclass

from tcslib.beacon import (
    NO_BEACON_MESSAGE,
    NO_TRANSFER_TIME,
    Stage,
    received_at,
)
from tcsui import theme


class palette:
    """The colours an indicator is drawn in: ``tcsui::palette``, complete."""

    #: Nothing is known: no beacon has arrived, or the station has stopped
    #: listening.
    UNKNOWN = "#c4c4c4"
    #: An indicator that has shown a colour and is not showing one now: the
    #: dark half of a blink, once a beacon has arrived. White and not grey, so
    #: that grey means only that nothing has been heard yet.
    OFF = "#ffffff"
    #: All is well: the spacecraft is being heard from.
    WELL = "#22c55e"
    #: Worth looking at. A golden yellow rather than pure yellow, which against
    #: the white of a blink's dark half could hardly be seen blinking.
    WATCH = "#ffe100"
    #: Wrong.
    WRONG = "#ff0000"


def indicator(showing, heard):
    """What an indicator shows for a stage of staleness, ``heard`` saying
    whether a beacon has ever arrived.

    Green while the spacecraft is being heard from, yellow once it is late
    enough that contact is not claimed, red once nothing is being heard, and
    grey before anything has arrived. Grey is for that and nothing else: once
    a beacon has arrived the indicator has shown a colour, and wherever it
    shows none after that it is white -- the dark half of a blink, a blinking
    indicator being one colour going dark and back, and a last beacon a moved
    clock puts in the future. ``heard`` is a pass's ``at`` having a time in
    it, which it keeps for good once a beacon arrives.
    """
    dark = palette.OFF if heard else palette.UNKNOWN
    if not showing.lit:
        return dark
    return {
        Stage.UNHEARD: dark,
        Stage.HEARD: palette.WELL,
        Stage.AGING: palette.WATCH,
        Stage.WAITING: palette.WRONG,
    }[showing.stage]


@dataclass(frozen=True)
class Shown:
    """What the panel's three properties should be, after a pass: all three
    from one reading of one beacon."""

    #: The indicator's colour.
    indicator: str
    #: When the last beacon arrived, as a time of day, or the placeholder.
    received: str
    #: What that beacon said.
    message: str
    #: Which stage the indicator is at, in words: ``Stage.label``.
    stage: str

    @staticmethod
    def of(beacon_pass):
        """What to show for a pass of the beacon loop."""
        return Shown(
            indicator=indicator(
                beacon_pass.showing, beacon_pass.at is not None
            ),
            received=received_at(beacon_pass.at),
            message=beacon_pass.said,
            stage=beacon_pass.showing.stage.label(),
        )


#: What the panel shows before the first pass: ``beacon.slint``'s defaults.
NOTHING_YET = Shown(
    indicator="#c0c0c0",
    received=NO_TRANSFER_TIME,
    message=NO_BEACON_MESSAGE,
    stage=Stage.UNHEARD.label(),
)


class BeaconPanel(ttk.LabelFrame):
    """An indicator, when the last beacon arrived, and what it said.

    Laid out as ``BeaconPanel`` in ``beacon.slint``: a box titled ``Beacon``,
    because the beacon has a socket of its own and keeps arriving after a
    disconnect, and three rows -- the indicator, then the time, then what that
    beacon said under it, the two reading as one message received then and
    saying this. Drawn in ttk, so that it takes the window's theme: see
    :mod:`tcsui.theme`.
    """

    def __init__(self, parent, **options):
        super().__init__(parent, text="Beacon", padding=theme.INSIDE, **options)
        self.columnconfigure(1, weight=1)

        ttk.Label(self, text="Status:").grid(
            row=0, column=0, sticky="w", pady=2
        )
        # As tall as the line beside it, measured from the type rather than
        # stated in pixels, as the Slint indicator is sized from Fonts.window.
        size = round(
            tkinter.font.nametofont("TkDefaultFont").metrics("linespace") * 1.1
        )
        self._indicator = tk.Canvas(
            self,
            width=size,
            height=size,
            highlightthickness=0,
            background=theme.background(self),
        )
        self._oval = self._indicator.create_oval(
            1,
            1,
            size - 1,
            size - 1,
            fill=NOTHING_YET.indicator,
            outline="#808080",
        )
        self._indicator.grid(
            row=0, column=1, sticky="w", padx=(theme.BETWEEN, 0), pady=2
        )

        ttk.Label(self, text="Last rcvd:").grid(
            row=1, column=0, sticky="w", pady=2
        )
        self._received = ttk.Label(self, text=NOTHING_YET.received)
        self._received.grid(
            row=1, column=1, sticky="w", padx=(theme.BETWEEN, 0), pady=2
        )

        ttk.Label(self, text="Last msg:").grid(
            row=2, column=0, sticky="w", pady=2
        )
        self._message = ttk.Label(self, text=NOTHING_YET.message)
        self._message.grid(
            row=2, column=1, sticky="w", padx=(theme.BETWEEN, 0), pady=2
        )

    def show(self, shown):
        """Put what a pass of the beacon loop reported on the panel: all three
        at once, from the one reading."""
        self._indicator.itemconfigure(self._oval, fill=shown.indicator)
        self._received.configure(text=shown.received)
        self._message.configure(text=shown.message)

    def showing(self):
        """What the panel shows now, for a test to read back."""
        return Shown(
            indicator=self._indicator.itemcget(self._oval, "fill"),
            received=self._received.cget("text"),
            message=self._message.cget("text"),
        )
