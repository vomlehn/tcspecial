#!/usr/bin/env python3
"""The ping example's ground station: a window that pings the vehicle.

Two halves borrowed and one of its own, as the beacon example is put together
in Rust:

* the beacon, from ``python/tcslib/beacon.py`` and
  ``python/tcsui/beacon_panel.py`` -- the Python spellings of ``tcslib::beacon``
  and ``tcsui`` -- so the window says whether the vehicle is alive before
  anybody presses anything;
* the ping, from ``ping_ground.py`` beside this, which sends each ping up to
  the ``gateway`` node and times the ``ping`` node's answer;
* and this program: read the configuration, start the two loops, put what
  they report in the window.

It starts the vehicle as well: tcspecial, from the repository's build, on the
same configuration, killed when this program exits -- however it exits, the
kernel being told to see to it where this program is killed or crashes. So a
vehicle is running exactly while its ground station is. ``--no-vehicle``
leaves that to somebody else, for a vehicle started in another terminal or
played by a test.

The window shows the last ping sent and when, the last answer received and
when, and the three times between them -- the round trip, and the two halves
of it, up and down. Press Ping to send one.

Usage, from the repository's root::

    python3 examples/ping/ping_gateway.py [--yaml | --xml] [configuration]
    python3 examples/ping/ping_gateway.py --pings 3       # no window
    python3 examples/ping/ping_gateway.py --describe      # what it read
    python3 examples/ping/ping_gateway.py --no-vehicle    # tcspecial elsewhere
    python3 examples/ping/ping_gateway.py -v              # say what it is doing

Quiet unless ``-v`` or ``--verbose`` is given: the console then says where it
is listening, each remark a beacon pass makes in the window's mode -- beacons
missed, a vehicle that restarted, a configuration that differs -- and a
tcspecial that stopped by itself. Errors and warnings are said either way, and
so is what ``--pings`` and ``--describe`` print, which is what they are for.

The configuration is ``examples/ping/ping.xml`` unless another is named:
``--yaml`` reads ``ping.yaml`` instead, and ``--xml`` says the default. One
of the two at most, and a file named beside one has to be that spelling.
Reading XML it uses nothing outside the standard library; reading YAML it
needs PyYAML. Python 3.7 or later.
"""

import argparse
import ctypes
import os
import queue
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "python"))
sys.path.insert(0, str(HERE))

from tcslib.beacon import Beacons, Schedule  # noqa: E402
from tcslibgs import configuration  # noqa: E402
from tcslibgs.configuration import find_built  # noqa: E402
from ping_ground import NO_MESSAGE, Pinger, interval, precise_time  # noqa: E402

#: The example's configuration, without its extension: ``.xml``, which is
#: read where nobody says otherwise, or ``.yaml``, the two saying the same
#: thing.
STEM = HERE / "ping"

#: How often the window takes in what the two loops have reported.
REFRESH_MS = 50


def arguments(argv):
    """Read the command line, and choose the configuration it means.

    Args:
        argv: The command line's arguments, without the program's name.

    Returns:
        The options, as ``argparse`` gives them, with ``configuration`` the
        path of the file to read.

    Raises:
        SystemExit: The command line is wrong; argparse has said why.
    """
    parser = argparse.ArgumentParser(
        description="Ping the vehicle's ping node and time the answer, beside "
        "the beacon panel."
    )
    parser.add_argument(
        "configuration",
        nargs="?",
        help=(
            f"the configuration (default: {STEM}.xml, or with --yaml "
            f"{STEM}.yaml)"
        ),
    )
    # Which of the example's two files: one, or neither, and never both.
    spelling = parser.add_mutually_exclusive_group()
    spelling.add_argument(
        "--yaml", dest="spelling", action="store_const", const="yaml",
        help="read the YAML spelling of the configuration",
    )
    spelling.add_argument(
        "--xml", dest="spelling", action="store_const", const="xml",
        help="read the XML spelling of the configuration (the default)",
    )
    parser.add_argument(
        "--gateway",
        help=(
            "the gateway node the ping goes through, where there is more "
            "than one"
        ),
    )
    parser.add_argument(
        "--node", help="the ping node to ping, where there is more than one"
    )
    parser.add_argument(
        "--tcsverify", help="the tcsverify to digest the configuration with"
    )
    parser.add_argument(
        "--tcspecial",
        help="the tcspecial to start (default: the repository's build)",
    )
    parser.add_argument(
        "--no-vehicle",
        action="store_true",
        help=(
            "start no tcspecial: one is running elsewhere, or a test is "
            "playing it"
        ),
    )
    parser.add_argument(
        "--pings",
        type=int,
        metavar="N",
        help="send N pings, print each, and stop: no window",
    )
    parser.add_argument(
        "--describe",
        action="store_true",
        help="print what the configuration says, and stop",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="print status messages on the console",
    )
    options = parser.parse_args(argv)
    try:
        options.configuration = configuration.choose(
            options.configuration, STEM, "xml", options.spelling
        )
    except ValueError as why:
        parser.error(str(why))
    return options


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


#: prctl's option for the signal a process is sent when its parent dies.
PR_SET_PDEATHSIG = 1


def die_with_this_program(parent):
    """In the child, before tcspecial runs: be killed when ``parent`` dies.

    A program that exits normally kills its vehicle itself; one that is killed
    or crashes runs nothing at all, and the vehicle would go on beaconing with
    no ground station. So the kernel is asked to send SIGKILL when this
    program goes -- and if it has gone already, between the fork and here, the
    child goes now rather than never being told.

    Args:
        parent: This program's process id, taken before the fork.
    """
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(PR_SET_PDEATHSIG, signal.SIGKILL, 0, 0, 0) != 0:
        os._exit(127)
    if os.getppid() != parent:
        os._exit(1)


class Vehicle:
    """tcspecial, started by this program and killed when it exits.

    Run itself and not through ``cargo run``: the kill is ``kill()``, and
    killing cargo would leave the tcspecial it started running, holding the
    example's ports, with nothing left that knows it is there.

    Args:
        program: The tcspecial program to run.
        args:    Its arguments.
    """

    def __init__(self, program, args):
        self.program = program
        self.args = list(args)
        self.process = None
        self._stopping = False

    @staticmethod
    def for_configuration(path, program=None, verbose=False):
        """tcspecial on this configuration: the one named, or the build's.

        Told ``--quiet`` unless ``verbose``, so that the console is this
        program's: tcspecial's startup summary is for somebody watching the
        vehicle start, and what stops it starting it says either way.

        Args:
            path:    The configuration file.
            program: The tcspecial to run, or ``None`` for the build's.
            verbose: Whether to let tcspecial print its startup summary.

        Returns:
            The :class:`Vehicle`, not yet started.

        Raises:
            ValueError: No program was named and none is built or on the
                        PATH.
        """
        program = program or find_built("tcspecial")
        if program is None:
            raise ValueError(
                "tcspecial is not built or on the PATH: build it with "
                "`cargo build --bin tcspecial`, or say --no-vehicle"
            )
        return Vehicle(program, ([] if verbose else ["--quiet"]) + [str(path)])

    def start(self):
        """Start it, told to die with this program, and say if it stops by
        itself."""
        parent = os.getpid()
        self.process = subprocess.Popen(
            [self.program, *self.args],
            preexec_fn=lambda: die_with_this_program(parent),
        )
        threading.Thread(target=self._watch, daemon=True).start()

    def _watch(self):
        """Wait for tcspecial to exit, and say so if nobody stopped it."""
        code = self.process.wait()
        if not self._stopping:
            status(f"tcspecial stopped by itself: exit status {code}")

    def stop(self):
        """Kill it, and wait for it to be gone, so that it is reaped and its
        ports are free."""
        if self.process is None:
            return
        self._stopping = True
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait()
        self.process = None


class Setup:
    """What the program read out of the configuration.

    Args:
        options: The command line's options, from :func:`arguments`.

    Raises:
        ValueError: The configuration is wrong for this program.
        OSError: The configuration cannot be read.
    """

    def __init__(self, options):
        self.config = configuration.load(options.configuration)
        self.gateway = the_one(
            self.config.nodes, "gateway_radio", options.gateway
        )
        self.node = the_one(self.config.nodes, "ping", options.node)
        self.uplink = self.config.uplink(self.gateway)
        self.downlink = self.config.downlink(self.gateway, self.node)
        self.beacon = self.config.beacon_link()
        self.beacon_interval = self.config.beacon_interval()
        self.digest, self.why_no_digest = configuration.digest(
            options.configuration, options.tcsverify
        )

    def describe(self):
        """The facts, one per line, as the agreement test reads them.

        Returns:
            The beacon link, its interval, the uplink, the downlink and the
            digest, each on a line of its own as ``name value``.
        """
        return "\n".join([
            f"beacon.send {self.beacon.send}",
            f"beacon.recv {self.beacon.recv}",
            f"beacon.default_interval {self.beacon_interval}",
            f"uplink {self.uplink}",
            f"downlink {self.downlink}",
            f"digest {self.digest}",
        ])


#: Whether to print status messages: ``-v`` or ``--verbose``.
VERBOSE = False


def status(text):
    """Say ``text`` on stderr, if asked to with ``-v``: what the program is
    doing, as against what has gone wrong, which is said whatever."""
    if VERBOSE:
        print(text, file=sys.stderr, flush=True)


def run_without_a_window(setup, beacons, pinger, count):
    """Send ``count`` pings, one after another, and print each exchange.

    Each ping is given two seconds to be answered, which is long enough for
    any link this example runs over and short enough that a vehicle that never
    answers is a run that ends rather than one that hangs.

    Args:
        setup:   What the program read out of the configuration.
        beacons: The beacon socket, listened to on a thread of its own.
        pinger:  The ping link.
        count:   How many pings to send.

    Returns:
        The exit status: nought if every ping was answered, one if not.
    """
    heard = threading.Event()

    def listen():
        """Print what the beacon link says until the first beacon arrives."""
        while not heard.is_set():
            beacon_pass = beacons.next_pass()
            for remark in beacon_pass.remarks:
                print(f"beacon: {remark}", flush=True)
            for warning in beacon_pass.warnings:
                print(f"beacon: {warning}", file=sys.stderr, flush=True)
            if beacon_pass.at is not None:
                print(f"beacon: {beacon_pass.said}", flush=True)
                heard.set()

    threading.Thread(target=listen, daemon=True).start()
    heard.wait(timeout=12)

    answered_all = True
    for _ in range(count):
        print(f"sent: {pinger.ping()}", flush=True)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            left = deadline - time.monotonic()
            if (
                pinger.receive(timeout=max(left, 0.01))
                and pinger.exchange().received_at
            ):
                break
        exchange = pinger.exchange()
        if exchange.received_at is None:
            print("received: nothing", flush=True)
            answered_all = False
            continue
        print(f"received: {exchange.received}", flush=True)
        print(
            f"times: round trip {interval(exchange.round_trip())}, up "
            f"{interval(exchange.up())}, down {interval(exchange.down())}",
            flush=True,
        )
    return 0 if answered_all else 1


def run_the_window(setup, beacons, pinger):
    """The window, and the two loops that feed it.

    Args:
        setup:   What the program read out of the configuration.
        beacons: The beacon socket, which a loop of its own listens to.
        pinger:  The ping link, whose answers a loop of its own receives.

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
                status(remark)
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
    window.title("tcspecial ping")
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


def main(argv):
    """Run the example: read the configuration, listen, start the vehicle,
    and ping it, with a window or without.

    Args:
        argv: The command line's arguments, without the program's name.

    Returns:
        The exit status: nought for success, one for a failure, which has
        been said on stderr.
    """
    global VERBOSE
    options = arguments(argv)
    VERBOSE = options.verbose
    try:
        setup = Setup(options)
    except (OSError, ValueError) as why:
        print(f"{options.configuration}: {why}", file=sys.stderr)
        return 1

    if options.describe:
        print(setup.describe())
        return 0

    if setup.digest is None:
        print(
            f"{options.configuration} cannot be digested, so no beacon's "
            f"digest is checked: {setup.why_no_digest}",
            file=sys.stderr,
        )
    try:
        beacons = Beacons.listening(
            setup.beacon,
            Schedule.for_interval(setup.beacon_interval),
            setup.digest,
        )
        pinger = Pinger(setup.uplink, setup.downlink)
    except OSError as why:
        print(f"cannot listen: {why}", file=sys.stderr)
        return 1
    status(
        f"Listening for beacons at {setup.beacon.recv}, which "
        f"{setup.beacon.send} sends from; pinging {setup.node} through "
        f"{setup.gateway} at {setup.uplink}, answers at {setup.downlink}"
    )

    # The vehicle, once this end is listening: a beacon sent before the ground
    # has its socket is one nobody reads.
    vehicle = None
    if not options.no_vehicle:
        try:
            vehicle = Vehicle.for_configuration(
                options.configuration,
                options.tcspecial,
                verbose=options.verbose,
            )
            vehicle.start()
        except (OSError, ValueError) as why:
            print(f"the vehicle cannot be started: {why}", file=sys.stderr)
            return 1
    try:
        if options.pings is not None:
            return run_without_a_window(setup, beacons, pinger, options.pings)
        return run_the_window(setup, beacons, pinger)
    finally:
        if vehicle is not None:
            vehicle.stop()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
