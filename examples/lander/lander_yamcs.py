#!/usr/bin/env python3
"""The lander example's ground station: two tcspecials flying a lunar lander,
watched through Yamcs from the Earth.

The vehicles are ``lander-a``, which flies the X axis, and ``lander-b``,
which flies the Y axis: each reads its axis of the joystick in tcs-sim's
lander window and drives that axis's thrust with it, and sends its axis's
position, velocity and acceleration down through Yamcs. One tcs-sim
simulates the payloads of both, in one world, from the simulator file they
share. See docs/lander-design.rst.

This program puts the pieces together:

* it makes a landscape at random from a seed -- ``--seed``, or one it
  chooses and says -- keeping the rules the lander's physics holds a
  landscape to, and writes it into the run's copy of the simulator file,
  which tcs-sim and this program's window both read;
* it writes a Yamcs server with an instance for each vehicle, with
  ``tcsmoc/yamcs_ground.py``, each instance's links moved to ports of their
  own so that a relay can stand between it and its vehicle;
* it starts a relay on each link that holds every datagram ``--delay``
  seconds, 1.3 by default, the time light takes between the Moon and the
  Earth, both ways: telemetry down, and commands and their answers;
* it starts Yamcs, tcs-sim with both configurations, and both tcspecials,
  each killed when this program exits, however it exits;
* it has Yamcs send each vehicle a CONNECT carrying its configuration's
  checksum until the vehicle answers with it, through the delay;
* it shows what arrives in a window -- the landscape and the lander where it
  was said to be, the values, the fuel, where the joystick was held, and
  whether it has landed or crashed -- or, with ``--seconds``, prints it for
  that long with no window.

Yamcs's web interface, at http://localhost:19090 by default, shows both
instances meanwhile.

Usage, from the repository's root, with Yamcs unpacked somewhere::

    python3 examples/lander/lander_yamcs.py --yamcs ~/yamcs
    python3 examples/lander/lander_yamcs.py --seed 7
    python3 examples/lander/lander_yamcs.py --seconds 60
    python3 examples/lander/lander_yamcs.py --describe

``--yamcs`` is the directory a Yamcs release unpacks to, holding
``bin/yamcsd``; ``$YAMCS_HOME`` says it when ``--yamcs`` does not.
``--no-vehicle``, ``--no-simulator`` and ``--no-yamcs`` leave the
tcspecials, tcs-sim and Yamcs to somebody else. tcs-sim draws the lander's
window, where the lander is flown, so it wants a display.
"""

import argparse
import heapq
import itertools
import math
import os
import random
import re
import select
import socket
import subprocess
import sys
import textwrap
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "python"))
sys.path.insert(0, str(ROOT / "tcs-ui" / "python"))

from tcsmoc.processes import Child, Vehicle, built  # noqa: E402
from tcsmoc.yamcs_ground import (  # noqa: E402
    Yamcs, connect, write_yamcs_instances, yamcs_home)
from tcsprotocol import configuration  # noqa: E402
from tcsprotocol import yamcs as generated  # noqa: E402

#: The two vehicles, each by the name of its Yamcs instance, which is its
#: configuration's: X first, then Y.
VEHICLES = ("lander-a", "lander-b")

#: The simulator file the two share, of which each run writes a copy with
#: its own landscape.
SHIPPED_SIM = HERE.parent / "lander-common" / "lander-sim.yaml"

#: Where Yamcs's web interface and REST API are, unless --http-port says.
HTTP_PORT = 19090

#: How long a datagram takes between the Moon and the Earth, in seconds,
#: unless --delay says: light takes about 1.28.
DELAY = 1.3

#: How far above its own port each link Yamcs is given is moved, so that the
#: relay can have the port the vehicle sends to or is sent from.
SHIFT = 10000

#: The run's simulation mode, given to tcs-sim and both tcspecials.
SIMULATE = "all"

#: How long tcs-sim is given to say it is ready, in seconds.
SIMULATOR_PATIENCE = 60

#: The landscape's rules, as the lander's physics holds a landscape to them
#: in tcsprotocol/src/node/physics/lander.rs: the steepest slope, in
#: degrees; the highest ground and the highest pad, as fractions of the
#: display's height; and how many times the lander's width the pad is.
STEEPEST = 70.0
HIGHEST_GROUND = 0.6
HIGHEST_PAD = 0.2
PAD_TO_LANDER = 2.0

#: The steepest slope this program makes, in degrees: short of
#: :data:`STEEPEST`, so that rounding a point never makes a slope too steep.
STEEPEST_MADE = 62.0

#: The narrowest and widest a stretch of ground between two points is made,
#: as fractions of the display's width.
NARROWEST = 0.04
WIDEST = 0.12

#: The lowest a pad is made, as a fraction of the display's height: above
#: the bottom edge, so that the ground under the lander shows.
LOWEST_PAD = 0.02

#: What a value is shown as before one has arrived.
NOTHING_YET = "--"

#: How often the window takes in what the reading loop has read, in
#: milliseconds.
REFRESH_MS = 100

#: How often the reading loop asks Yamcs, in seconds.
READ_EVERY = 0.1

#: The window's display of the landscape, in pixels for each metre: half of
#: tcs-sim's.
SCALE = 0.5

#: The joystick's square's side, which is each arm's thickness, and how far
#: each arm reaches beyond it, in pixels: as tcs-sim's lander window draws
#: it.
SQUARE = 33.6
ARM = 56

#: How many stars the sky has.
STARS = 140

#: How long each group of stars takes to brighten and dim again, in
#: seconds: a group each, as tcs-sim's lander window has them.
TWINKLE_PERIODS = (1.3, 1.9, 2.6, 3.4)

#: How far above the ground a star is at least, as a fraction of the
#: display's height.
STAR_CLEARANCE = 0.02

#: The ground's fill: grey with a little brown, as tcs-sim's lander window
#: has it.
GROUND = "#aaa197"

#: Whether to print status messages: ``-v`` or ``--verbose``.
VERBOSE = False


def status(text):
    """Say ``text`` on stderr, if asked to with ``-v``.

    Args:
        text: What to say.
    """
    if VERBOSE:
        print(text, file=sys.stderr, flush=True)


def arguments(argv):
    """Reads the command line.

    Args:
        argv: The arguments, without the program's name.

    Returns:
        The options, as :mod:`argparse` gives them.
    """
    parser = argparse.ArgumentParser(description="Fly the lander example's "
                                                 "two tcspecials, watched "
                                                 "through Yamcs.")
    parser.add_argument("--yamcs", default=os.environ.get("YAMCS_HOME"),
                        help="where Yamcs is unpacked (default: $YAMCS_HOME)")
    parser.add_argument("--run-dir", default="/tmp/lander",
                        help="where the run's simulator file and Yamcs's "
                             "configuration and data are written (default: "
                             "/tmp/lander)")
    parser.add_argument("--http-port", type=int, default=HTTP_PORT,
                        help=f"Yamcs's web port (default: {HTTP_PORT})")
    parser.add_argument("--seed", type=int,
                        help="the landscape's seed (default: one chosen "
                             "at random, and said)")
    parser.add_argument("--delay", type=float, default=DELAY, metavar="S",
                        help="how long each datagram is held between a "
                             f"vehicle and Yamcs, both ways (default: "
                             f"{DELAY})")
    parser.add_argument("--seconds", type=float, metavar="N",
                        help="print what arrives for N seconds, and stop, "
                             "with no window")
    parser.add_argument("--every", type=float, default=2.0, metavar="S",
                        help="with --seconds, print a line each S seconds "
                             "(default: 2)")
    parser.add_argument("--describe", action="store_true",
                        help="print the landscape, the relays and the "
                             "parameters read, and stop")
    parser.add_argument("--no-vehicle", action="store_true",
                        help="do not start the tcspecials")
    parser.add_argument("--no-memlock", action="store_true",
                        help="run the tcspecials without locking their "
                             "memory into RAM")
    parser.add_argument("--no-simulator", action="store_true",
                        help="do not start tcs-sim")
    parser.add_argument("--no-yamcs", action="store_true",
                        help="do not start Yamcs or the relays; use ones "
                             "already running")
    parser.add_argument("--tcspecial", help="the tcspecial program to run")
    parser.add_argument("--tcs-sim", help="the tcs-sim program to run")
    parser.add_argument("-v", "--verbose", action="store_true",
                        help="say what it is doing")
    options = parser.parse_args(argv)
    if options.every <= 0:
        parser.error("--every is a number of seconds above nought")
    if options.delay < 0:
        parser.error("--delay is a number of seconds, nought or more")
    return options


def points_of(said):
    """The lines of a list of points as the simulator file states them.

    Args:
        said: The points, ``"x,y x,y ... m"``, lines parted by ``|``.

    Returns:
        Each line, a list of ``(x, y)`` in metres.

    Raises:
        ValueError: It is not in metres, or a point does not read.
    """
    said = " ".join(said.split())
    if not said.endswith(" m"):
        raise ValueError(f"{said!r} is not in metres: end it with m")
    lines = []
    for line in said[:-2].split("|"):
        points = []
        for point in line.split():
            x, y = point.split(",")
            points.append((float(x), float(y)))
        lines.append(points)
    return lines


def said_of(points):
    """A list of points as the simulator file states them.

    Args:
        points: Each ``(x, y)``, in metres.

    Returns:
        The points, ``"x,y x,y ... m"``.
    """
    return " ".join(f"{x:g},{y:g}" for x, y in points) + " m"


class Lander:
    """The lander, its display and its landscape, as a simulator file
    states them.

    Args:
        settings: The lander component's settings, as the file has them.

    Raises:
        ValueError: A setting the program needs is missing or does not
                    read.
    """

    def __init__(self, settings):
        try:
            #: The display's width and height, in metres.
            self.width = float(settings["width"].split()[0])
            self.height = float(settings["height"].split()[0])
            #: The ground, each point ``(x, y)``.
            self.ground = points_of(settings["ground"])[0]
            #: The pad's two ends.
            self.pad = points_of(settings["pad"])[0]
            #: The lander's outline, each line a list of points about
            #: where it is, its feet at nought.
            self.outline = points_of(settings["outline"])
        except (KeyError, IndexError, AttributeError) as why:
            raise ValueError(f"the lander's settings: {why}")

    def lander_width(self):
        """The lander's width, across its outline.

        Returns:
            The width, in metres.
        """
        xs = [x for line in self.outline for x, _ in line]
        return max(xs) - min(xs)


def lander_settings(sim):
    """The lander component's settings, in a simulator file as read.

    Args:
        sim: The file, as PyYAML reads it.

    Returns:
        The settings, a dictionary.

    Raises:
        ValueError: The file has no lander, or more than one.
    """
    components = (sim.get("world") or {}).get("physics", {}).get(
        "components", [])
    found = [entry for group in components
             for entry in group.get("type lander", [])]
    if len(found) != 1:
        raise ValueError(f"the simulator file has {len(found)} landers, "
                         "and this program flies one")
    return found[0]


def landscape(seed, width, height, lander_width):
    """A landscape made at random from a seed, keeping the rules the
    lander's physics holds a landscape to.

    The pad first, level, twice the lander's width, its middle within the
    middle half of the width and its height within the lowest fifth; then
    the ground out from each of its ends to the edges, a stretch at a time,
    each toward a height chosen between nought and :data:`HIGHEST_GROUND`
    of the height, and no steeper than :data:`STEEPEST_MADE`.

    Args:
        seed:         The seed: one seed, one landscape.
        width:        The display's width, in metres.
        height:       The display's height, in metres.
        lander_width: The lander's width, in metres.

    Returns:
        The ground, a list of ``(x, y)`` from the left edge to the right,
        and the pad's two ends, which are two of its points one after the
        other.
    """
    rng = random.Random(seed)
    pad_width = PAD_TO_LANDER * lander_width
    middle = rng.uniform(width / 4, 3 * width / 4)
    left = round(middle - pad_width / 2, 1)
    right = left + pad_width
    level = round(rng.uniform(LOWEST_PAD * height, HIGHEST_PAD * height), 1)
    pad = [(left, level), (right, level)]
    steep = math.tan(math.radians(STEEPEST_MADE))
    top = HIGHEST_GROUND * height
    narrowest, widest = NARROWEST * width, WIDEST * width

    def out_to(x, y, edge):
        """The ground from a point out to an edge.

        Args:
            x:    Where it starts, across.
            y:    Where it starts, up.
            edge: The edge's x, nought or the width.

        Returns:
            The points after the first, to the one at the edge.
        """
        made = []
        while x != edge:
            step = rng.uniform(narrowest, widest)
            # The last stretch reaches the edge, and is no narrower than
            # the narrowest.
            if abs(edge - x) - step < narrowest:
                after = edge
            else:
                after = round(x + math.copysign(step, edge - x), 1)
            # Toward a height anywhere from nought to the highest, as far
            # as the slope allows.
            most = steep * abs(after - x)
            aim = rng.uniform(0.0, top)
            x, y = after, round(min(y + most, max(y - most, aim)), 1)
            made.append((x, y))
        return made

    leftward = out_to(left, level, 0.0)
    rightward = out_to(right, level, width)
    ground = list(reversed(leftward)) + pad + rightward
    return ground, pad


def run_sim_text(shipped, seed, ground, pad):
    """The run's simulator file: the shipped one, its landscape replaced.

    The ground and the pad are replaced where the file states them, and
    everything else, its comments too, is left as it is.

    Args:
        shipped: The shipped file's text.
        seed:    The landscape's seed, which the file says.
        ground:  The ground's points.
        pad:     The pad's ends.

    Returns:
        The run's file's text.

    Raises:
        ValueError: The file does not state the ground and the pad once
                    each.
    """
    def replaced(text, key, value):
        """The text with one setting's quoted value replaced.

        Args:
            text:  The file's text.
            key:   The setting.
            value: Its new value.

        Returns:
            The text.
        """
        pattern = re.compile(rf'^(\s*){key}: "[^"]*"', re.MULTILINE)
        found = pattern.findall(text)
        if len(found) != 1:
            raise ValueError(f"the simulator file states {key} "
                             f"{len(found)} times, and it is replaced once")
        indent = found[0]
        lines = textwrap.wrap(f'{key}: "{value}"', 78 - len(indent),
                              subsequent_indent="  ",
                              break_on_hyphens=False)
        new = "\n".join(indent + line for line in lines)
        return pattern.sub(lambda _: new, text)

    text = replaced(shipped, "ground", said_of(ground))
    text = replaced(text, "pad", said_of(pad))
    return (f"# Written by lander_yamcs.py from {SHIPPED_SIM.name}, its "
            f"landscape made\n# from seed {seed}.\n\n" + text)


def relayed(links, shift=SHIFT):
    """A Yamcs instance's links moved for relays to stand between it and the
    vehicle, and the relays.

    A link Yamcs only listens on -- telemetry, the beacon -- is moved
    ``shift`` above its port, and its relay listens on the port the vehicle
    sends to and passes each datagram on to Yamcs. A link Yamcs sends on --
    commands, whose answers come back to the socket that sent them -- is
    pointed at a port ``shift`` above the vehicle's, where its relay
    listens, passing each command on to the vehicle and each answer back.

    Args:
        links: The links, as :func:`tcsprotocol.yamcs.data_links` gives
               them. Left as they are.
        shift: How far each port is moved.

    Returns:
        The moved links, and each relay as ``(listen, forward, both_ways)``,
        each address a ``(host, port)``.
    """
    moved, relays = [], []
    for link in links:
        link = dict(link)
        port = link["port"]
        if "host" in link:
            vehicle = (link["host"], port)
            link["host"] = "127.0.0.1"
            link["port"] = port + shift
            relays.append((("127.0.0.1", port + shift), vehicle, True))
        else:
            link["port"] = port + shift
            relays.append((("127.0.0.1", port), ("127.0.0.1", port + shift),
                           False))
        moved.append(link)
    return moved, relays


class DelayLine:
    """Relays that hold each datagram a while before passing it on, all
    served by one thread.

    Args:
        delay: How long each datagram is held, in seconds.
    """

    def __init__(self, delay):
        self.delay = delay
        # Each socket, and what it does with what arrives at it: a function
        # of the datagram's sender giving the socket to send it on and where
        # to, or None where it has nowhere to go yet.
        self._routes = {}
        self._due = []
        self._order = itertools.count()
        self._stopping = threading.Event()
        self._thread = None

    @staticmethod
    def _bound(address):
        """A UDP socket bound to an address.

        Args:
            address: ``(host, port)``.

        Returns:
            The socket.

        Raises:
            OSError: The address cannot be had, saying which it is.
        """
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            sock.bind(address)
        except OSError as why:
            sock.close()
            raise OSError(f"a relay cannot listen at "
                          f"{address[0]}:{address[1]}: {why}")
        return sock

    def one_way(self, listen, forward):
        """Adds a relay passing what arrives at one address on to another.

        Args:
            listen:  Where it listens, ``(host, port)``.
            forward: Where it passes each datagram on to.
        """
        near = self._bound(listen)
        self._routes[near] = lambda _sender: (near, forward)

    def both_ways(self, listen, forward):
        """Adds a relay passing what arrives at one address on to another,
        from a socket of its own, and the answers back to whoever last sent.

        Args:
            listen:  Where it listens, ``(host, port)``.
            forward: Where it passes each datagram on to.
        """
        near = self._bound(listen)
        far = self._bound(("127.0.0.1", 0))
        asker = []

        def from_near(sender):
            """Where a datagram from the near side goes.

            Args:
                sender: Who sent it, who is answered.

            Returns:
                The far socket and ``forward``.
            """
            asker[:] = [sender]
            return far, forward

        self._routes[near] = from_near
        self._routes[far] = lambda _sender: (near, asker[0]) if asker \
            else None

    def start(self):
        """Starts passing datagrams on."""
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self):
        """Stops, and closes every socket."""
        self._stopping.set()
        if self._thread is not None:
            self._thread.join(timeout=2)
        for sock in self._routes:
            sock.close()

    def _run(self):
        """Receives, holds and sends, until stopped."""
        sockets = list(self._routes)
        while not self._stopping.is_set():
            now = time.monotonic()
            while self._due and self._due[0][0] <= now:
                _, _, sock, to, data = heapq.heappop(self._due)
                try:
                    sock.sendto(data, to)
                except OSError as why:
                    status(f"relay: cannot send to {to}: {why}")
            wait = 0.05
            if self._due:
                wait = min(wait, max(0.0, self._due[0][0] - now))
            readable, _, _ = select.select(sockets, [], [], wait)
            for sock in readable:
                try:
                    data, sender = sock.recvfrom(65536)
                except OSError:
                    continue
                route = self._routes[sock](sender)
                if route is None:
                    continue
                out, to = route
                heapq.heappush(self._due, (time.monotonic() + self.delay,
                                           next(self._order), out, to,
                                           data))


class Craft:
    """One of the vehicles, as the program reads it.

    Args:
        instance: Its Yamcs instance's name, which is its configuration's.

    Raises:
        ValueError: Its configuration is wrong for this program.
        OSError: Its configuration cannot be read.
    """

    def __init__(self, instance):
        self.instance = instance
        self.path = HERE.parent / instance / f"{instance}.ytcs"
        self.config = configuration.load(self.path)
        if self.config.tcspecial.get("gateway") != "yamcs":
            raise ValueError(f"{self.path}: its gateway is not yamcs")
        #: Each node drawn into the gateway, by name, and its parameter's
        #: qualified name.
        self.parameters = {link.source: generated.qualified(link.name)
                           for link in generated.telemetry(self.config)}
        self.digest, self.why_no_digest = configuration.digest(self.path)

    def log_directory(self):
        """Where the vehicle logs its telemetry.

        Returns:
            The directory, or ``None`` where it says none.
        """
        said = self.config.tcspecial.get("log.directory")
        return Path(said) if said else None


def write_run(run_dir, seed):
    """Writes the run's simulator file, with a landscape made from the
    seed.

    Args:
        run_dir: Where it is written, made if it is not there.
        seed:    The landscape's seed.

    Returns:
        The file's path, and the :class:`Lander` it states.

    Raises:
        ValueError: The shipped file is not what this program expects.
        OSError: A file cannot be read or written, or PyYAML is not
                 installed.
    """
    try:
        import yaml
    except ImportError:
        raise OSError("PyYAML is not installed, and lander_yamcs.py reads "
                      "the simulator file with it: pip install PyYAML")
    shipped_text = SHIPPED_SIM.read_text()
    shipped = Lander(lander_settings(yaml.safe_load(shipped_text)))
    ground, pad = landscape(seed, shipped.width, shipped.height,
                            shipped.lander_width())
    text = run_sim_text(shipped_text, seed, ground, pad)
    run_dir.mkdir(parents=True, exist_ok=True)
    path = run_dir / SHIPPED_SIM.name
    path.write_text(text)
    return path, Lander(lander_settings(yaml.safe_load(text)))


def write_yamcs(crafts, yamcs_dir, http_port, home):
    """Writes Yamcs's server, an instance for each vehicle, its links moved
    for the relays.

    Args:
        crafts:    The :class:`Craft` of each vehicle.
        yamcs_dir: Where it is written, made afresh.
        http_port: Yamcs's web port.
        home:      Where Yamcs is unpacked.

    Returns:
        Each relay, as :func:`relayed` gives them, for every instance.

    Raises:
        As :func:`tcsmoc.yamcs_ground.write_yamcs_instances`.
    """
    import yaml
    write_yamcs_instances([(craft.config, craft.instance)
                           for craft in crafts], yamcs_dir, http_port, home,
                          "lander_yamcs.py")
    relays = []
    for craft in crafts:
        path = yamcs_dir / "etc" / f"yamcs.{craft.instance}.yaml"
        text = path.read_text()
        header = "".join(line + "\n" for line in text.splitlines()
                         if line.startswith("#"))
        said = yaml.safe_load(text)
        said["dataLinks"], theirs = relayed(said["dataLinks"])
        relays += theirs
        path.write_text(header + "# Its links moved for lander_yamcs.py's "
                        "relays.\n" + yaml.safe_dump(said, sort_keys=False))
    return relays


def simulator_args(sim_path, crafts):
    """tcs-sim's arguments for this run, but for what it is asked.

    Args:
        sim_path: The run's simulator file.
        crafts:   The :class:`Craft` of each vehicle.

    Returns:
        The arguments: the mode, the simulator file, and each
        configuration, in order.
    """
    return ([f"--simulate={SIMULATE}", f"--simulator={sim_path}"]
            + [str(craft.path) for craft in crafts])


def simulator_ready(program, sim_path, crafts, patience):
    """Waits for tcs-sim to say it is ready for this run.

    Args:
        program:  The tcs-sim program.
        sim_path: The run's simulator file.
        crafts:   The :class:`Craft` of each vehicle.
        patience: How long to wait, in seconds.

    Raises:
        RuntimeError: It is not ready in time, or is set up for another run.
    """
    asked = subprocess.run(
        [str(program), f"--wait={patience}"]
        + simulator_args(sim_path, crafts), capture_output=True, text=True)
    if asked.returncode != 0:
        raise RuntimeError("tcs-sim is not ready for this run: "
                           f"{asked.stderr.strip() or asked.stdout.strip()}")


class Readings:
    """What the two vehicles last said, read from Yamcs.

    Args:
        reads: Each vehicle's :class:`Yamcs` and :class:`Craft`, as
               ``(yamcs, craft)``.
    """

    def __init__(self, reads):
        self._reads = reads
        #: Each value, by the name of the node that sent it.
        self.values = {}
        self._lock = threading.Lock()

    def read(self):
        """Reads each vehicle's latest values from Yamcs."""
        for yamcs, craft in self._reads:
            names = list(craft.parameters.values())
            latest = yamcs.latest(names)
            with self._lock:
                for node, parameter in craft.parameters.items():
                    if parameter in latest:
                        self.values[node] = latest[parameter][0]

    def number(self, node):
        """A node's latest value as a number.

        Args:
            node: The node's name.

        Returns:
            The value, or ``None`` before one has arrived.
        """
        with self._lock:
            value = self.values.get(node)
        return value if isinstance(value, (int, float)) else None

    def said(self, node):
        """Whether a signal's latest value says yes: the text ``1``.

        Args:
            node: The signal's name.

        Returns:
            True where it said ``1``; false where it said anything else or
            nothing yet.
        """
        with self._lock:
            value = self.values.get(node)
        if isinstance(value, (bytes, bytearray)):
            value = bytes(value).decode("ascii", "replace")
        return str(value).strip() == "1"

    def state(self):
        """What the lander is doing, as the values sent down say.

        Returns:
            ``"Landed"``, ``"Crashed"`` or ``"Flying"``.
        """
        if self.said("landed"):
            return "Landed"
        if self.said("crashed"):
            return "Crashed"
        return "Flying"

    def text(self, node, places=2):
        """A node's latest value, as the program shows it.

        Args:
            node:   The node's name.
            places: How many places after the point.

        Returns:
            The value, or :data:`NOTHING_YET`.
        """
        value = self.number(node)
        return NOTHING_YET if value is None else f"{value:.{places}f}"

    def arrived(self, nodes):
        """Whether every one of some nodes has said something.

        Args:
            nodes: The nodes' names.

        Returns:
            True once each has a value.
        """
        with self._lock:
            return all(node in self.values for node in nodes)

    def line(self):
        """Every value, on one line.

        Returns:
            Each as ``name value``, then the state.
        """
        parts = [f"{node} {self.text(node)}" for node in ROWS_FLAT]
        parts.append(f"fuel {self.text('fuel', 4)}")
        return ", ".join(parts) + f"; {self.state()}"


#: The rows of values the window shows: a name, a unit, and the X node and
#: the Y node.
ROWS = (("Position", "m", "x", "y"),
        ("Velocity", "m/s", "velocity_x", "velocity_y"),
        ("Acceleration", "m/s²", "acceleration_x", "acceleration_y"),
        ("Joystick", "", "joystick_x", "joystick_y"))

#: Every node of :data:`ROWS`, in order.
ROWS_FLAT = tuple(node for _, _, x, y in ROWS for node in (x, y))


def ground_at(lander, x):
    """The ground's height at a place across the display.

    Args:
        lander: The :class:`Lander`.
        x:      The place, in metres.

    Returns:
        The height, in metres, along the line between the ground's points
        either side.
    """
    for (x0, y0), (x1, y1) in zip(lander.ground, lander.ground[1:]):
        if x <= x1:
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0)
    return lander.ground[-1][1]


def star_places(lander, seed):
    """The stars in the sky, each somewhere above the ground.

    Args:
        lander: The :class:`Lander`, whose ground they are above.
        seed:   The landscape's seed, so that one landscape has one sky.

    Returns:
        Each star as ``(x, y, group)``, in metres, ``group`` the twinkling
        group it is in.
    """
    rng = random.Random(seed)
    places = []
    for number in range(STARS):
        x = rng.uniform(0, lander.width)
        lowest = ground_at(lander, x) + STAR_CLEARANCE * lander.height
        if lowest >= lander.height:
            continue
        y = rng.uniform(lowest, lander.height)
        places.append((x, y, number % len(TWINKLE_PERIODS)))
    return places


def twinkle(seconds):
    """How bright each group of stars is at a moment.

    Args:
        seconds: The time since the window opened, in seconds.

    Returns:
        Each group's brightness, from a fifth to one.
    """
    return [0.6 + 0.4 * math.sin(math.tau * seconds / period + number)
            for number, period in enumerate(TWINKLE_PERIODS)]


def stick_offset(axis):
    """Where the joystick's dot is drawn for an axis's value, from the
    square's middle: as tcs-sim's window has it, nought anywhere in the
    square, and the arm's length out along it.

    Args:
        axis: The axis's value, minus one to one.

    Returns:
        The offset, in pixels.
    """
    if axis == 0:
        return 0.0
    along = SQUARE / 2 + min(1.0, abs(axis)) * ARM
    return math.copysign(along, axis)


def watch_and_print(readings, seconds, every, stopped):
    """Prints what arrives for a while, with no window.

    Args:
        readings: The :class:`Readings`.
        seconds:  How long.
        every:    How often to print every value, in seconds.
        stopped:  Says whether tcs-sim has stopped, its window closed,
                  which stops this too.

    Returns:
        The exit status: nought where every value said something, one where
        one never did.
    """
    deadline = time.monotonic() + seconds
    next_line = time.monotonic()
    was = None
    while time.monotonic() < deadline and not stopped():
        readings.read()
        state = readings.state()
        if state != was:
            print(f"state: {state}", flush=True)
            was = state
        if time.monotonic() >= next_line:
            print(f"read: {readings.line()}", flush=True)
            next_line += every
        time.sleep(READ_EVERY)
    if readings.arrived(ROWS_FLAT):
        return 0
    missing = [node for node in ROWS_FLAT if readings.number(node) is None]
    print(f"lander_yamcs: nothing arrived from {', '.join(missing)}",
          file=sys.stderr)
    return 1


def run_the_window(readings, lander, seed, delay, stopped):
    """The window, and the loop that reads Yamcs for it.

    Args:
        readings: The :class:`Readings`.
        lander:   The :class:`Lander`, whose landscape and outline are
                  drawn.
        seed:     The landscape's seed, which the window says.
        delay:    The link's delay, in seconds, which the window says.
        stopped:  Says whether tcs-sim has stopped, its window closed:
                  this window is closed then too, which ends the run.

    Returns:
        The exit status, nought, once the window is closed.
    """
    import tkinter as tk
    from tkinter import ttk

    from tcs_ui import fonts, theme

    def reading_loop():
        """Reads Yamcs, for ever."""
        while True:
            try:
                readings.read()
            except (OSError, ValueError) as why:
                status(f"reading Yamcs: {why}")
            time.sleep(READ_EVERY)

    window = tk.Tk()
    fonts.use_in(window)
    theme.use_in(window)
    window.title("TCS Lander MOC")
    window.configure(background="#ffffff")
    window.resizable(False, False)
    between = theme.BETWEEN

    wide, high = lander.width * SCALE, lander.height * SCALE

    def at(x, y):
        """A point of the world, in the scene's pixels.

        Args:
            x: Across, in metres.
            y: Up, in metres.

        Returns:
            ``(x, y)`` in pixels, down from the top.
        """
        return x * SCALE, high - y * SCALE

    # A black sky with twinkling stars over grey ground, as tcs-sim's lander
    # window draws it.
    scene = tk.Canvas(window, width=wide, height=high, background="#000000",
                      highlightthickness=1, highlightbackground="#6b7280")
    scene.pack(padx=between, pady=(between, between // 2))
    scene.create_rectangle(0, 0, wide, high, fill="#000000", outline="")
    stars = [[] for _ in TWINKLE_PERIODS]
    for x, y, group in star_places(lander, seed):
        cx, cy = at(x, y)
        stars[group].append(scene.create_rectangle(
            cx - 0.5, cy - 0.5, cx + 0.5, cy + 0.5, fill="#ffffff",
            outline=""))
    flat = [c for x, y in lander.ground for c in at(x, y)]
    scene.create_polygon(*at(0, 0), *flat, *at(lander.width, 0),
                         fill=GROUND, outline="")
    scene.create_line(*flat, fill="#3b3127", width=1.5)
    # The pad, white with a thin black edge, and an upright mark at each
    # end, so that it is told by its shape and not by its colour.
    (px0, py0), (px1, py1) = lander.pad
    pad_lines = [(at(px0, py0), at(px1, py1))]
    pad_lines += [(at(x, py0), at(x, py0 + 8)) for x in (px0, px1)]
    for colour, width in (("#000000", 4), ("#ffffff", 2.5)):
        for start, end in pad_lines:
            scene.create_line(*start, *end, fill=colour, width=width)
    state = scene.create_text(8, 6, anchor="nw", text="Flying",
                              fill="#ffffff",
                              font=("TkDefaultFont", 14, "bold"))
    drawn = []
    opened = time.monotonic()

    panel = ttk.Frame(window, padding=theme.INSIDE)
    panel.pack(fill="x", padx=between)
    ttk.Label(panel, text="X").grid(row=0, column=1, sticky="e")
    ttk.Label(panel, text="Y").grid(row=0, column=2, sticky="e")
    shown = []
    for number, (name, unit, x_node, y_node) in enumerate(ROWS, start=1):
        label = f"{name} ({unit})" if unit else name
        ttk.Label(panel, text=label).grid(row=number, column=0, sticky="w")
        for column, node in ((1, x_node), (2, y_node)):
            value = ttk.Label(panel, width=10, anchor="e")
            value.grid(row=number, column=column, sticky="e",
                       padx=(between, 0))
            shown.append((value, node))
    fuel = ttk.Label(panel, text="Fuel --")
    fuel.grid(row=len(ROWS) + 1, column=0, sticky="w")
    note = ttk.Label(panel, text=f"Seed {seed}; everything here is "
                                 f"{delay:g} s behind the lander")
    note.grid(row=len(ROWS) + 2, column=0, columnspan=3, sticky="w",
              pady=(between // 2, 0))

    # The joystick, as it was held: the pointer does nothing here.
    reach = SQUARE / 2 + ARM
    thick = SQUARE
    stick = tk.Canvas(window, width=2 * reach, height=2 * reach,
                      background="#ffffff", highlightthickness=0)
    stick.pack(pady=(between // 2, between))
    stick.create_rectangle(0, reach - thick / 2, 2 * reach, reach + thick / 2,
                           fill="#e5e7eb", outline="#6b7280")
    stick.create_rectangle(reach - thick / 2, 0, reach + thick / 2, 2 * reach,
                           fill="#e5e7eb", outline="#6b7280")
    stick.create_rectangle(ARM, ARM, ARM + SQUARE, ARM + SQUARE,
                           fill="#f3f4f6", outline="#374151", width=2)
    dot = stick.create_oval(reach - 7, reach - 7, reach + 7, reach + 7,
                            fill="#0072B2", outline="#000000")

    def take_in():
        """Shows what the loop has read, and comes back in
        :data:`REFRESH_MS`; or closes the window, tcs-sim's having been
        closed."""
        if stopped():
            window.destroy()
            return
        for value, node in shown:
            value.configure(text=readings.text(node))
        left = readings.number("fuel")
        fuel.configure(text="Fuel --" if left is None
                       else f"Fuel {round(left * 100)}%")
        scene.itemconfigure(state, text=readings.state())
        for item in drawn:
            scene.delete(item)
        drawn.clear()
        x, y = readings.number("x"), readings.number("y")
        if x is not None and y is not None:
            # As tcs-sim's lander window draws it: its single lines --
            # struts, legs, the antenna -- first, each a black line a little
            # wider than the white one over it; then its shapes over their
            # ends, filled white, each with its own thin black line, so that
            # where one part meets another shows.
            lines = [[c for lx, ly in line for c in at(x + lx, y + ly)]
                     for line in lander.outline]
            single = [flat for flat in lines if len(flat) == 4]
            for colour, width in (("#000000", 2.5), ("#ffffff", 1.5)):
                for flat in single:
                    drawn.append(scene.create_line(*flat, fill=colour,
                                                   width=width))
            for flat in lines:
                if len(flat) > 4:
                    drawn.append(scene.create_polygon(
                        *flat, fill="#ffffff", outline="#000000",
                        width=0.75))
        for group, bright in zip(stars, twinkle(time.monotonic() - opened)):
            level = round(255 * bright)
            for star in group:
                scene.itemconfigure(star, fill=f"#{level:02x}{level:02x}"
                                               f"{level:02x}")
        sx = readings.number("joystick_x") or 0.0
        sy = readings.number("joystick_y") or 0.0
        cx, cy = reach + stick_offset(sx), reach - stick_offset(sy)
        stick.coords(dot, cx - 7, cy - 7, cx + 7, cy + 7)
        window.after(REFRESH_MS, take_in)

    threading.Thread(target=reading_loop, daemon=True).start()
    window.after(REFRESH_MS, take_in)
    window.mainloop()
    return 0


def describe(seed, lander, crafts, http_port, delay):
    """The facts, one per line: the landscape, the relays and the
    parameters read.

    Args:
        seed:      The landscape's seed.
        lander:    The :class:`Lander`, with the run's landscape.
        crafts:    The :class:`Craft` of each vehicle.
        http_port: Yamcs's web port.
        delay:     The link's delay, in seconds.

    Returns:
        The lines.
    """
    lines = [f"seed {seed}", f"ground {said_of(lander.ground)}",
             f"pad {said_of(lander.pad)}", f"http_port {http_port}",
             f"delay {delay:g}"]
    for craft in crafts:
        _, relays = relayed(generated.data_links(craft.config))
        for listen, forward, both in relays:
            way = "both" if both else "one"
            lines.append(f"relay {craft.instance} {listen[0]}:{listen[1]} "
                         f"{forward[0]}:{forward[1]} {way}")
        for node, parameter in sorted(craft.parameters.items()):
            lines.append(f"parameter {craft.instance} {node} {parameter}")
    return "\n".join(lines)


def main(argv):
    """Runs the example.

    Args:
        argv: The command line, without the program's name.

    Returns:
        The exit status.
    """
    global VERBOSE
    options = arguments(argv)
    VERBOSE = options.verbose
    run_dir = Path(options.run_dir).resolve()
    seed = options.seed
    if seed is None:
        seed = random.SystemRandom().randrange(2 ** 31)
    try:
        crafts = [Craft(instance) for instance in VEHICLES]
        sim_path, lander = write_run(run_dir, seed)
    except (ValueError, OSError) as why:
        print(f"lander_yamcs: {why}", file=sys.stderr)
        return 2
    if options.describe:
        print(describe(seed, lander, crafts, options.http_port,
                       options.delay))
        return 0
    print(f"seed {seed}: the landscape is in {sim_path}", flush=True)
    for craft in crafts:
        if craft.digest is None:
            print(f"lander_yamcs: no checksum for {craft.instance}'s "
                  f"CONNECT: {craft.why_no_digest}", file=sys.stderr)
            return 2
    yamcs_dir = run_dir / "yamcs"
    first = Yamcs(options.yamcs, yamcs_dir, options.http_port, VEHICLES[0])
    yamcses = [first] + [first.instance_of(instance)
                         for instance in VEHICLES[1:]]
    line = DelayLine(options.delay)
    started = []
    simulator = None

    def stopped():
        """Whether the tcs-sim this program started has stopped, its window
        closed.

        Returns:
            True once it has exited; false while it runs, or where this
            program did not start it.
        """
        return (simulator is not None
                and simulator.process.poll() is not None)

    try:
        if not options.no_yamcs:
            home = yamcs_home(options.yamcs)
            relays = write_yamcs(crafts, yamcs_dir, options.http_port, home)
            for listen, forward, both in relays:
                (line.both_ways if both else line.one_way)(listen, forward)
            line.start()
            status(f"{len(relays)} relays, each holding {options.delay:g} s")
            status(f"starting Yamcs on {yamcs_dir}")
            first.start()
            status(f"Yamcs is at http://localhost:{options.http_port}")
        if not options.no_simulator:
            tcs_sim = options.tcs_sim or built("tcs-sim", "say --no-simulator")
            simulator = Child(tcs_sim, ["--sending"]
                              + simulator_args(sim_path, crafts), "tcs-sim",
                              options.verbose)
            simulator.start()
            started.append(simulator)
            status("waiting for tcs-sim to make its simulated devices")
            simulator_ready(tcs_sim, sim_path, crafts, SIMULATOR_PATIENCE)
        if not options.no_vehicle:
            for craft in crafts:
                # tcspecial makes no log directory of its own.
                logs = craft.log_directory()
                if logs is not None:
                    logs.mkdir(parents=True, exist_ok=True)
                vehicle = Vehicle.for_configuration(
                    craft.path, options.tcspecial, options.verbose, SIMULATE,
                    memlock=not options.no_memlock)
                vehicle.start()
                started.append(vehicle)
        for yamcs, craft in zip(yamcses, crafts):
            if not connect(yamcs, craft.digest, say=status):
                print(f"lander_yamcs: {craft.instance} never answered "
                      "CONNECT with its configuration's checksum",
                      file=sys.stderr)
                return 1
            print(f"connected: {craft.instance} is running its "
                  "configuration", flush=True)
        readings = Readings(list(zip(yamcses, crafts)))
        if options.seconds is None:
            return run_the_window(readings, lander, seed, options.delay,
                                  stopped)
        return watch_and_print(readings, options.seconds, options.every,
                               stopped)
    except (RuntimeError, ValueError, OSError) as why:
        print(f"lander_yamcs: {why}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 0
    finally:
        for child in reversed(started):
            child.stop()
        if not options.no_yamcs:
            first.stop()
            line.stop()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
