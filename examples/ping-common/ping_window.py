"""The ping example's window: the beacon panel above the ping panel.

Shared by the ping example's two ground stations, ``examples/ping/
ping_gateway.py``, which talks to the vehicle itself, and ``examples/
ping-yamcs/ping_yamcs.py``, which talks to it through Yamcs: each hands it a
beacon link and a ping link, and the window shows what they report. Here in
``examples/ping-common``, which holds what the two share and is not an
example itself.
"""

import queue
import sys
import threading

from ping_ground import NO_MESSAGE, interval, precise_time

#: How often the window takes in what the two loops have reported.
REFRESH_MS = 50


def the_one(nodes, kind, named):
    """The node of this kind the program uses: the one named, or the only
    one.

    Args:
        nodes: The configuration's nodes.
        kind:  The node type wanted, as ``gateway_radio`` or ``ping``.
        named: The node named on the command line, or ``None``.

    Returns:
        The node's name.

    Raises:
        ValueError: None was named and there is not exactly one of the kind.
    """
    if named is not None:
        return named
    names = [node.name for node in nodes if node.node_type == kind]
    if len(names) != 1:
        raise ValueError(
            f"the configuration has {len(names)} {kind} nodes, so say which: "
            f"--{'gateway' if kind == 'gateway_radio' else 'node'} <name>"
        )
    return names[0]



def _quiet(_text):
    """Says nothing: what a remark is told when nobody asked for them.

    Args:
        _text: The remark.
    """


def run_the_window(setup, beacons, pinger, title="tcspecial ping",
                   say=_quiet):
    """The window, and the two loops that feed it.

    Args:
        setup:   What the program read out of the configuration.
        beacons: The beacon socket, which a loop of its own listens to: a
                 :class:`tcslib.beacon.Beacons`, or anything with its
                 ``next_pass``.
        pinger:  The ping link, whose answers a loop of its own receives.
        title:   The window's title.
        say:     What a beacon pass's remarks are told to: the program's
                 status, said with ``-v``. Its warnings are said whatever.

    Returns:
        The exit status, nought, once the window is closed.
    """
    import tkinter as tk

    from tkinter import ttk

    from tcsui import fonts, theme
    from tcsui.beacon_panel import BeaconPanel, Shown

    reports = queue.Queue()

    # Each loop on a thread of its own, reporting to the window rather than
    # touching it: tkinter is not to be used from any thread but its own.
    def beacon_loop():
        """Report each pass of the beacon loop to the window, for ever."""
        while True:
            beacon_pass = beacons.next_pass()
            for remark in beacon_pass.remarks:
                say(remark)
            # What is wrong is said whether or not -v was given.
            for warning in beacon_pass.warnings:
                print(warning, file=sys.stderr, flush=True)
            reports.put(("beacon", Shown.of(beacon_pass)))

    def answer_loop():
        """Report each answer to the window, until the socket closes."""
        while True:
            try:
                if pinger.receive(timeout=0.5):
                    reports.put(("answer", None))
            except OSError:
                # The socket is closed, which is the program ending.
                return

    window = tk.Tk()
    fonts.use_in(window)
    theme.use_in(window)
    window.title(title)
    window.resizable(False, False)

    # Two boxes, the beacon's and the ping's, with one size of space between
    # them and round them: see tcsui.theme.
    between = theme.BETWEEN
    beacon = BeaconPanel(window)
    beacon.pack(fill="x", padx=between, pady=(between, between // 2))

    # The rows of PingPanel in tcsui/ui/ping.slint, which tcsmoc shows, laid
    # out as the beacon box is: a label on the left of each row at its own
    # width and every value starting at one place after them, all left
    # justified, so that the two boxes read alike. Under them a line about the
    # times, and under that the button, centred. Titled with the node, as
    # tcsmoc's is, so that the box says what it is pinging.
    panel = ttk.LabelFrame(
        window,
        text=f"Pinging {setup.node}" if setup else "Ping",
        padding=theme.INSIDE,
    )
    panel.pack(fill="x", padx=between, pady=(between // 2, between))

    def row(number, label):
        ttk.Label(panel, text=label).grid(
            row=number, column=0, sticky="w", pady=2
        )

    def value(number, column, width=None):
        """Put a label for a value on the ping panel.

        Args:
            number: The row it goes on.
            column: The column it goes in.
            width:  Its width in characters, or ``None`` to fit its text.

        Returns:
            The label.
        """
        label = ttk.Label(panel, style="Value.TLabel", width=width)
        label.grid(
            row=number, column=column, sticky="w", padx=(between, 0), pady=2
        )
        return label

    # The message column is as wide as an answer will be, so that the window
    # is the width of what it will show and does not grow when the first
    # answer arrives.
    widest = len("ping #000000 0000000000.000000000 0000000000.000000000")

    # Sent and received, each a message and when, on one row: the two read as
    # one thing, sent then and saying this.
    row(0, "Sent:")
    sent = value(0, 1, widest)
    sent_time = value(0, 2)
    row(1, "Received:")
    received = value(1, 1, widest)
    received_time = value(1, 2)

    # The round trip is what a ping is for, so it comes first; the two halves
    # are under it, because each is read across two clocks and means less
    # alone. All three in the one plain size every value is in.
    row(2, "Round trip:")
    round_trip = value(2, 1)
    row(3, "MOC to tcspecial:")
    up = value(3, 1)
    row(4, "tcspecial to MOC:")
    down = value(4, 1)

    def show():
        """Show the ping link's latest exchange on the panel."""
        exchange = pinger.exchange()
        sent.configure(text=exchange.sent or NO_MESSAGE)
        sent_time.configure(text=precise_time(exchange.sent_at))
        received.configure(text=exchange.received or NO_MESSAGE)
        received_time.configure(text=precise_time(exchange.received_at))
        round_trip.configure(text=interval(exchange.round_trip()))
        up.configure(text=interval(exchange.up()))
        down.configure(text=interval(exchange.down()))

    def send(_event=None):
        pinger.ping()
        show()

    # Which clock the times are in, on a line of its own under them.
    ttk.Label(
        panel,
        text="Times are UTC.",
        style="Note.TLabel",
    ).grid(row=5, column=0, columnspan=3, sticky="w", pady=(theme.INSIDE, 0))
    # The default button, which Enter or Space presses wherever the focus
    # is, centred under everything else, as the beacon example's is. It never
    # takes the focus itself, so that Space on it is not a second ping.
    ttk.Button(
        panel,
        text="Send ping",
        command=send,
        default="active",
        takefocus=False,
    ).grid(row=6, column=0, columnspan=3, pady=(theme.INSIDE, 0))
    for key in ("<Return>", "<KP_Enter>", "<space>"):
        window.bind(key, send)

    # The label columns of the two boxes as wide as each other, the wider of
    # the two, so that every value in the window starts at one place and not
    # one place in each box.
    labels = [
        label
        for label in beacon.grid_slaves(column=0) + panel.grid_slaves(column=0)
        if int(label.grid_info()["columnspan"]) == 1
    ]
    label_width = max(label.winfo_reqwidth() for label in labels)
    for box in (beacon, panel):
        box.columnconfigure(0, minsize=label_width)

    show()

    def take_in():
        """Show what the loops have reported, and come back in
        ``REFRESH_MS``."""
        while True:
            try:
                kind, report = reports.get_nowait()
            except queue.Empty:
                break
            if kind == "beacon":
                beacon.show(report)
            else:
                show()
        window.after(REFRESH_MS, take_in)

    threading.Thread(target=beacon_loop, daemon=True).start()
    threading.Thread(target=answer_loop, daemon=True).start()
    window.after(REFRESH_MS, take_in)
    window.mainloop()
    return 0
