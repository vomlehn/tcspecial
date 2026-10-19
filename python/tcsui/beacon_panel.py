"""The beacon panel, and what to show on it.

The Python spelling of ``tcsui``: ``tcsui/src/lib.rs``, which says what colour
the light is and what the two lines say, and ``tcsui/ui/beacon.slint``, which
is the panel itself. The panel is drawn with tkinter, which is the toolkit the
Python standard library has.

The other half of ``python/tcslib/beacon.py``. That one listens and says what
is true of the spacecraft; this one says what that looks like. They are apart
for the reason the Rust halves are: a ground station with no window wants the
facts with no toolkit loaded, and two stations that do have windows want the
same panel rather than two that have drifted -- the same light in the same
place, the same labels in the same words.

Nothing is computed in the panel. :func:`shown` works out the three things it
shows, from a pass of the beacon loop, and :meth:`BeaconPanel.show` puts them
on it.
"""

import tkinter as tk
from dataclasses import dataclass

from tcslib.beacon import NO_BEACON_MESSAGE, NO_TRANSFER_TIME, Stage, received_at


class palette:
    """The colours a light is drawn in: ``tcsui::palette``, complete."""

    #: Nothing is known: no beacon has arrived, or the station has stopped
    #: listening.
    UNKNOWN = "#c4c4c4"
    #: All is well: the spacecraft is being heard from.
    WELL = "#00ff00"
    #: Worth looking at.
    WATCH = "#ffff00"
    #: Wrong.
    WRONG = "#ff0000"


def indicator(showing):
    """What a light shows for a stage of staleness.

    Green while the spacecraft is being heard from, yellow once it is late
    enough that contact is not claimed, red once nothing is being heard, and
    grey before anything has arrived. The dark half of a blink is grey too: a
    blinking light is one colour going dark and back.
    """
    if not showing.lit:
        return palette.UNKNOWN
    return {
        Stage.UNHEARD: palette.UNKNOWN,
        Stage.HEARD: palette.WELL,
        Stage.AGEING: palette.WATCH,
        Stage.SILENT: palette.WRONG,
    }[showing.stage]


@dataclass(frozen=True)
class Shown:
    """What the panel's three properties should be, after a pass: all three
    from one reading of one beacon."""

    #: The light's colour.
    indicator: str
    #: When the last beacon arrived, as a time of day, or the placeholder.
    received: str
    #: What that beacon said.
    message: str

    @staticmethod
    def of(beacon_pass):
        """What to show for a pass of the beacon loop."""
        return Shown(
            indicator=indicator(beacon_pass.showing),
            received=received_at(beacon_pass.at),
            message=beacon_pass.said,
        )


#: What the panel shows before the first pass: ``beacon.slint``'s defaults.
NOTHING_YET = Shown(indicator="#c0c0c0", received=NO_TRANSFER_TIME, message=NO_BEACON_MESSAGE)


class BeaconPanel(tk.Frame):
    """A light, when the last beacon arrived, and what it said.

    Laid out as ``BeaconPanel`` in ``beacon.slint``: a border, because the
    beacon has a socket of its own and keeps arriving after a disconnect, and
    three rows -- the light, then the time, then what that beacon said under
    it, the two reading as one message received then and saying this.
    """

    #: The light's size, as the Slint panel draws it.
    LIGHT = 20

    def __init__(self, parent, **options):
        super().__init__(
            parent,
            highlightbackground="#808080",
            highlightthickness=1,
            padx=6,
            pady=6,
            **options,
        )
        small = ("TkDefaultFont", 9)

        tk.Label(self, text="Beacon: ").grid(row=0, column=0, sticky="w")
        self._light = tk.Canvas(
            self, width=self.LIGHT, height=self.LIGHT, highlightthickness=0
        )
        self._lamp = self._light.create_oval(
            1, 1, self.LIGHT - 1, self.LIGHT - 1, fill=NOTHING_YET.indicator, outline="#808080"
        )
        self._light.grid(row=0, column=1, sticky="w", padx=8)

        tk.Label(self, text="Last rcvd: ").grid(row=1, column=0, sticky="w")
        self._received = tk.Label(self, text=NOTHING_YET.received, font=small)
        self._received.grid(row=1, column=1, sticky="w", padx=8)

        tk.Label(self, text="Last msg: ").grid(row=2, column=0, sticky="w")
        self._message = tk.Label(self, text=NOTHING_YET.message, font=small)
        self._message.grid(row=2, column=1, sticky="w", padx=8)

    def show(self, shown):
        """Put what a pass of the beacon loop reported on the panel: all three
        at once, from the one reading."""
        self._light.itemconfigure(self._lamp, fill=shown.indicator)
        self._received.configure(text=shown.received)
        self._message.configure(text=shown.message)

    def showing(self):
        """What the panel shows now, for a test to read back."""
        return Shown(
            indicator=self._light.itemcget(self._lamp, "fill"),
            received=self._received.cget("text"),
            message=self._message.cget("text"),
        )
