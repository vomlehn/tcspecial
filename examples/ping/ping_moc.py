#!/usr/bin/env python3
"""The ping example's ground station: a window that pings the vehicle.

Two halves borrowed and one of its own, as the beacon example is put together
in Rust:

* the beacon, from ``python/tcslib/beacon.py`` and
  ``python/tcsui/beacon_panel.py`` -- the Python spellings of ``tcslib::beacon``
  and ``tcsui`` -- so the window says whether the vehicle is alive before
  anybody presses anything;
* the ping, from ``ping_ground.py`` beside this, which sends each ping up to
  the ``moc`` node and times the ``ping`` node's answer;
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

    python3 examples/ping/ping_moc.py [configuration.xml]
    python3 examples/ping/ping_moc.py --pings 3        # no window
    python3 examples/ping/ping_moc.py --describe       # what it read
    python3 examples/ping/ping_moc.py --no-vehicle     # tcspecial runs elsewhere

The configuration is the XML spelling, which the Python standard library can
read, and defaults to ``examples/ping/ping.xml``. It uses nothing outside the
standard library, and Python 3.7 or later.
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
from ping_ground import Pinger, interval, precise_time  # noqa: E402

#: The configuration, where nobody says otherwise.
CONFIGURATION = HERE / "ping.xml"

#: How often the window takes in what the two loops have reported.
REFRESH_MS = 50


def arguments(argv):
    parser = argparse.ArgumentParser(
        description="Ping the vehicle's ping node and time the answer, beside "
        "the beacon panel."
    )
    parser.add_argument(
        "configuration",
        nargs="?",
        default=str(CONFIGURATION),
        help="the configuration, in its XML spelling (default: %(default)s)",
    )
    parser.add_argument(
        "--moc", help="the moc node the ping goes through, where there is more than one"
    )
    parser.add_argument(
        "--node", help="the ping node to ping, where there is more than one"
    )
    parser.add_argument("--tcsverify", help="the tcsverify to digest the configuration with")
    parser.add_argument(
        "--tcspecial", help="the tcspecial to start (default: the repository's build)"
    )
    parser.add_argument(
        "--no-vehicle",
        action="store_true",
        help="start no tcspecial: one is running elsewhere, or a test is playing it",
    )
    parser.add_argument(
        "--pings", type=int, metavar="N", help="send N pings, print each, and stop: no window"
    )
    parser.add_argument(
        "--describe", action="store_true", help="print what the configuration says, and stop"
    )
    return parser.parse_args(argv)


def the_one(nodes, kind, named):
    """The node of this kind the program uses: the one named, or the only one."""
    if named is not None:
        return named
    names = [node.name for node in nodes if node.node_type == kind]
    if len(names) != 1:
        raise ValueError(
            f"the configuration has {len(names)} {kind} nodes, so say which: "
            f"--{'moc' if kind == 'moc' else 'node'} <name>"
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
    """

    def __init__(self, program, args):
        self.program = program
        self.args = list(args)
        self.process = None
        self._stopping = False

    @staticmethod
    def for_configuration(path, program=None):
        """tcspecial on this configuration: the one named, or the build's."""
        program = program or find_built("tcspecial")
        if program is None:
            raise ValueError(
                "tcspecial is not built or on the PATH: build it with "
                "`cargo build --bin tcspecial`, or say --no-vehicle"
            )
        return Vehicle(program, [str(path)])

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
        status = self.process.wait()
        if not self._stopping:
            print(f"tcspecial stopped by itself: exit status {status}", file=sys.stderr, flush=True)

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
    """What the program read out of the configuration."""

    def __init__(self, options):
        self.config = configuration.load(options.configuration)
        self.moc = the_one(self.config.nodes, "moc", options.moc)
        self.node = the_one(self.config.nodes, "ping", options.node)
        self.uplink = self.config.uplink(self.moc)
        self.downlink = self.config.downlink(self.moc, self.node)
        self.beacon = self.config.beacon_link()
        self.digest, self.why_no_digest = configuration.digest(
            options.configuration, options.tcsverify
        )

    def describe(self):
        """The facts, one per line, as the agreement test reads them."""
        return "\n".join([
            f"beacon.send {self.beacon.send}",
            f"beacon.recv {self.beacon.recv}",
            f"uplink {self.uplink}",
            f"downlink {self.downlink}",
            f"digest {self.digest}",
        ])


def run_without_a_window(setup, beacons, pinger, count):
    """Send ``count`` pings, one after another, and print each exchange.

    Each ping is given two seconds to be answered, which is long enough for
    any link this example runs over and short enough that a vehicle that never
    answers is a run that ends rather than one that hangs.
    """
    heard = threading.Event()

    def listen():
        while not heard.is_set():
            beacon_pass = beacons.next_pass()
            for remark in beacon_pass.remarks:
                print(f"beacon: {remark}", flush=True)
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
            if pinger.receive(timeout=max(left, 0.01)) and pinger.exchange().received_at:
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
    """The window, and the two loops that feed it."""
    import tkinter as tk

    from tcsui import fonts
    from tcsui.beacon_panel import BeaconPanel, Shown

    reports = queue.Queue()

    # Each loop on a thread of its own, reporting to the window rather than
    # touching it: tkinter is not to be used from any thread but its own.
    def beacon_loop():
        while True:
            beacon_pass = beacons.next_pass()
            for remark in beacon_pass.remarks:
                print(remark, file=sys.stderr, flush=True)
            reports.put(("beacon", Shown.of(beacon_pass)))

    def answer_loop():
        while True:
            try:
                if pinger.receive(timeout=0.5):
                    reports.put(("answer", None))
            except OSError:
                # The socket is closed, which is the program ending.
                return

    window = tk.Tk()
    fonts.use_in(window)
    window.title("tcspecial ping")
    window.resizable(False, False)

    beacon = BeaconPanel(window)
    beacon.pack(fill="x", padx=8, pady=(8, 4))

    panel = tk.Frame(window, highlightbackground="#808080", highlightthickness=1, padx=6, pady=6)
    panel.pack(fill="x", padx=8, pady=(4, 8))

    shown = {}

    def row(at, label):
        tk.Label(panel, text=label).grid(row=at, column=0, sticky="w")
        value = tk.Label(panel, text="", font=("TkFixedFont", fonts.POINTS), anchor="w", width=60)
        value.grid(row=at, column=1, sticky="w", padx=8)
        shown[label] = value

    def show():
        exchange = pinger.exchange()
        for label, text in [
            ("Sent:", exchange.sent or ""),
            ("Time sent:", precise_time(exchange.sent_at)),
            ("Last received:", exchange.received or ""),
            ("Time received:", precise_time(exchange.received_at)),
            ("Round trip:", interval(exchange.round_trip())),
            ("MOC to tcspecial:", interval(exchange.up())),
            ("tcspecial to MOC:", interval(exchange.down())),
        ]:
            shown[label].configure(text=text)

    def send():
        pinger.ping()
        show()

    tk.Button(panel, text="Send ping", command=send).grid(
        row=0, column=0, columnspan=2, sticky="w", pady=(0, 6)
    )
    for at, label in enumerate(
        [
            "Sent:",
            "Time sent:",
            "Last received:",
            "Time received:",
            "Round trip:",
            "MOC to tcspecial:",
            "tcspecial to MOC:",
        ],
        start=1,
    ):
        row(at, label)
    # As wide as the rows above it, measured before it is added: a note that
    # wrapped to the panel as the panel grew would widen the panel it wraps
    # to, and the window with it, for ever.
    panel.update_idletasks()
    tk.Label(
        panel,
        text="Times are UTC. The two one-way times are each read across two "
        "clocks, the MOC's and tcspecial's, so each includes how far apart "
        "they are; their sum is the round trip.",
        justify="left",
        fg="#4b5563",
        wraplength=panel.winfo_reqwidth() - 16,
    ).grid(row=8, column=0, columnspan=2, sticky="w", pady=(6, 0))

    def take_in():
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
    options = arguments(argv)
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
            f"{options.configuration} cannot be digested, so no beacon's digest is "
            f"checked: {setup.why_no_digest}",
            file=sys.stderr,
        )
    try:
        beacons = Beacons.listening(setup.beacon, Schedule.standard(), setup.digest)
        pinger = Pinger(setup.uplink, setup.downlink)
    except OSError as why:
        print(f"cannot listen: {why}", file=sys.stderr)
        return 1
    print(
        f"Listening for beacons at {setup.beacon.recv}, which {setup.beacon.send} "
        f"sends from; pinging {setup.node} through {setup.moc} at {setup.uplink}, "
        f"answers at {setup.downlink}",
        file=sys.stderr,
        flush=True,
    )

    # The vehicle, once this end is listening: a beacon sent before the ground
    # has its socket is one nobody reads.
    vehicle = None
    if not options.no_vehicle:
        try:
            vehicle = Vehicle.for_configuration(options.configuration, options.tcspecial)
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
