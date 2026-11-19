#!/usr/bin/env python3
"""The lander example's ground station: two tcspecials flying a lunar lander,
watched through Yamcs from the Earth.

The vehicles are ``lander-a``, which flies the X axis, and ``lander-b``,
which flies the Y axis: each reads its axis of the joystick in tcs-sim's
lander pane and drives that axis's thrust with it, and sends its axis's
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
  checksum until the vehicle answers with it, through the delay, and then
  turns the lander's autorestart on, so that a lander nobody flies starts
  again by itself after each crash;
* it shows what arrives in a window -- the landscape and the lander where it
  was said to be, the values, the fuel, where the joystick was held, and
  whether it has landed or crashed -- or, with ``--seconds``, prints it for
  that long with no window.

Yamcs's web interface, at http://localhost:19090 by default, shows both
instances meanwhile.

Usage, from the repository's root::

    python3 examples/lander/lander_yamcs.py
    python3 examples/lander/lander_yamcs.py --seed 7
    python3 examples/lander/lander_yamcs.py --seconds 60
    python3 examples/lander/lander_yamcs.py --describe

``--yamcs`` is the directory a Yamcs release unpacks to, holding
``bin/yamcsd``; ``$YAMCS_HOME`` says it when ``--yamcs`` does not, and
where neither does, the Yamcs release the examples are written for is
fetched the first time and kept in ``~/.cache/tcspecial``.
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
import struct
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
#: milliseconds: about as often as a screen is drawn, the lander drawn
#: between readings by dead reckoning, so that it moves smoothly.
REFRESH_MS = 16

#: The longest the lander is carried on from its last reported position by
#: dead reckoning, in seconds: past this, the link has gone quiet.
RECKON_FOR = 0.5

#: How long after it opens the window puts itself under tcs-sim's, in
#: milliseconds: long enough for it to have been shown, which it must be
#: for the request to mean anything.
LOWER_AFTER_MS = 500

#: How often the reading loop asks Yamcs, in seconds.
READ_EVERY = 0.1

#: The window's display of the landscape, in pixels for each metre: a little
#: under tcs-sim's, so that the window fits beside tcs-sim's on a screen
#: 1707 logical pixels wide, as 3840 at a scale of 2.25 is.
SCALE = 0.48

#: The joystick's square's side, which is each arm's thickness, and how far
#: each arm reaches beyond it, in pixels: as tcs-sim's lander pane draws
#: it.
SQUARE = 9.45
ARM = 105

#: How many bytes one of the sky's records is, and how they are laid out:
#: the sky's generation, how many stars it has, and the star's number, two
#: bytes each; its radius, in tenths of a pixel, and its brightness, from
#: nought to 255, a byte each; and its place across and up, in centimetres,
#: four bytes each; most significant byte first. tcsprotocol's
#: ``node::physics::lander``, "Sky", is where they are said.
STAR_RECORD = 16
STAR_LAYOUT = ">HHHBBII"

#: How wide, in pixels, the scene is that a record's radius is given for:
#: tcsprotocol's ``SKY_WIDTH``, part of the records' layout.
SKY_WIDTH = 600.0

#: How tall each end mark of the pad is, as a fraction of the display's
#: height, and the radius of the light on top of it, in metres: as tcs-sim's
#: lander pane draws them.
PAD_MARK = 1 / 50
PAD_LIGHT = 6.0

#: How long the pad's lights take to flash on and off again, in seconds.
PAD_FLASH = 1.0

#: The radius of each of the lander's lights, in pixels; and how long its
#: white lights take to flash again, and how long each flash lasts, in
#: seconds: as tcs-sim's lander pane has them.
LANDER_LIGHT = 1.5
STROBE_EVERY = 1.0
STROBE_FOR = 0.15

#: The ground's fill: grey with a little brown, as tcs-sim's lander pane
#: has it.
GROUND = "#aaa197"

#: What the window says of a crashed lander, as tcs-sim's lander pane does.
CRASHED_SAID = "On the Moon, no one can hear you CRASH!"

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
                        help="where Yamcs is unpacked (default: "
                             "$YAMCS_HOME, or else Yamcs fetched the first "
                             "time and kept)")
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
        #: The command the ground sends up through the gateway, by the
        #: payload the gateway passes it to, where the gateway binds aboard
        #: to take it and passes it on by moc-out: lander-a's to autorestart,
        #: lander-b's to restart.
        self.commands = {}
        for uplink in generated.uplinks(self.config):
            if uplink.vehicle is None:
                continue
            for target in self.config.node(uplink.name).links.get(
                    "moc-out", []):
                self.commands[target.split(".", 1)[0]] = (
                    generated.qualified(uplink.command))

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


class Sky:
    """The lander's sky, as its records come down: no stars until the first
    arrives, and each star drawn as its record arrives.

    tcs-sim sends the stars round and round, four to a packet, each record
    whole; Yamcs is read for the latest, so a packet newer than the last
    read hides the one before it, and a packet lost on the way is lost.
    Either is had on a later pass. Once every star of a generation is here,
    each packet more of it is answered with a quench, the generation sent
    up to ``sky_quench``, which stops the sending; a lost quench is answered
    again by the next packet. A new generation starts the sky afresh.

    Args:
        quench: Sends a generation up to quench it, or ``None`` to send
                nothing.
    """

    def __init__(self, quench=None):
        self._quench = quench
        #: The generation being gathered, ``None`` before any.
        self.generation = None
        #: How many stars it has.
        self.total = 0
        #: Each star had, by its number, as ``(x, y, radius, brightness)``:
        #: its place, in metres; its radius, in pixels; and its brightness,
        #: from nought to one.
        self.stars = {}
        #: The numbers of the stars had and not yet drawn.
        self._fresh = []
        #: Whether the generation changed since the window last drew.
        self._changed = False
        self._lock = threading.Lock()

    def take(self, data):
        """Takes a packet of records, quenching the sky where it is whole.

        Args:
            data: The packet's bytes: whole records, a trailing part of one
                  left alone.

        Returns:
            The generations quenched, each once, in order.
        """
        quenched = []
        for at in range(0, len(data) - STAR_RECORD + 1, STAR_RECORD):
            (generation, total, number, radius, bright, x, y) = (
                struct.unpack(STAR_LAYOUT, data[at:at + STAR_RECORD]))
            with self._lock:
                if generation != self.generation:
                    self.generation, self.total = generation, total
                    self.stars, self._fresh = {}, []
                    self._changed = True
                if number < total and number not in self.stars:
                    self.stars[number] = (x / 100, y / 100, radius / 10,
                                          bright / 255)
                    self._fresh.append(number)
                whole = len(self.stars) >= self.total
            if whole and generation not in quenched:
                quenched.append(generation)
        if self._quench is not None:
            for generation in quenched:
                self._quench(generation)
        return quenched

    def fresh(self):
        """The stars not yet drawn, and whether the sky drawn so far is to
        be cleared first, a new generation having come.

        Returns:
            ``(cleared, stars)``: whether to clear; and each star as
            :attr:`stars` holds it.
        """
        with self._lock:
            stars = [self.stars[n] for n in self._fresh]
            cleared, self._changed, self._fresh = self._changed, False, []
        return cleared, stars


class Readings:
    """What the two vehicles last said, read from Yamcs.

    Args:
        reads: Each vehicle's :class:`Yamcs` and :class:`Craft`, as
               ``(yamcs, craft)``.
    """

    def __init__(self, reads, sky=None):
        self._reads = reads
        #: The :class:`Sky`, which each new packet of the ``sky`` node's
        #: records is given to; ``None`` for none.
        self.sky = sky
        #: Each value, by the name of the node that sent it.
        self.values = {}
        #: When each value last changed, on this program's monotonic clock.
        self.changed = {}
        self._lock = threading.Lock()

    def read(self):
        """Reads each vehicle's latest values from Yamcs."""
        for yamcs, craft in self._reads:
            fresh = []
            names = list(craft.parameters.values())
            latest = yamcs.latest(names)
            now = time.monotonic()
            with self._lock:
                for node, parameter in craft.parameters.items():
                    if parameter in latest:
                        value = latest[parameter][0]
                        if self.values.get(node) != value:
                            self.changed[node] = now
                            if node == "sky" and self.sky is not None:
                                fresh.append(value)
                        self.values[node] = value
            # Outside the lock: a whole sky sends a quench.
            for value in fresh:
                self.sky.take(value)

    def estimate(self, axis, now=None):
        """Where the lander is along an axis now, by dead reckoning: its
        last reported position, carried on by its last reported velocity
        and acceleration for the time since that position arrived -- at
        most :data:`RECKON_FOR`, so that a link gone quiet does not carry it
        off -- and not at all while it stands on the pad or has crashed.

        Args:
            axis: ``"x"`` or ``"y"``.
            now:  The moment, on this program's monotonic clock; now if
                  ``None``.

        Returns:
            The position, in metres, or ``None`` before one has arrived.
        """
        where = self.number(f"to_{axis}")
        if where is None or self.state() != "Flying":
            return where
        with self._lock:
            since = self.changed.get(f"to_{axis}")
        if since is None:
            return where
        t = min(RECKON_FOR, max(0.0, (now or time.monotonic()) - since))
        speed = self.number(f"to_velocity_{axis}") or 0.0
        push = self.number(f"to_accel_{axis}") or 0.0
        return where + speed * t + 0.5 * push * t * t

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
            ``"Landed"``, :data:`CRASHED_SAID` or ``"Flying"``.
        """
        if self.said("landed"):
            return "Landed"
        if self.said("crashed"):
            return CRASHED_SAID
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
        parts.append(f"fuel {self.text('to_fuel', 4)}")
        return ", ".join(parts) + f"; {self.state()}"


#: The rows of values the window shows: a name, a unit, and the X node and
#: the Y node.
ROWS = (("Position", "m", "to_x", "to_y"),
        ("Velocity", "m/s", "to_velocity_x", "to_velocity_y"),
        ("Acceleration", "m/s²", "to_accel_x", "to_accel_y"),
        ("Joystick", "", "to_joystick_3_4_x", "to_joystick_3_4_y"))

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


def star_drawing(radius, bright, per_scene):
    """How a star sent down is drawn, holding as much light as tcs-sim's
    pane gives it: a disk of its radius, its edge soft, so that the light
    in it goes as the disk's area.

    Args:
        radius:    Its radius, in pixels of a scene :data:`SKY_WIDTH` wide,
                   as its record says.
        bright:    Its brightness, from nought to one, as its record says.
        per_scene: This window's pixels for each of that scene's.

    Returns:
        ``(disk, r, level)``: whether to draw a disk of radius ``r``
        pixels, or else a single pixel; and its grey, nought to 255. A
        star whose disk is smaller than a pixel is one pixel, dimmed by
        the part of a pixel its disk would cover.
    """
    r = radius * per_scene
    area = math.pi * r * r
    if area < 1.0:
        return False, 0.5, round(255 * bright * area)
    return True, r, round(255 * bright)


def display_scale():
    """How much larger than at 96 dots an inch the display wants this
    window drawn.

    The window is an X11 one, through XWayland where the session is
    Wayland, and is drawn in the display's own pixels, which on a scaled
    display makes it small. KDE Plasma says the scale to X11 programs as
    the resource ``Xft.dpi`` -- 216 at a scale of 2.25 -- which
    ``xrdb -query`` reads.

    Returns:
        ``Xft.dpi`` over 96; one where there is none, or no ``xrdb``.
    """
    try:
        said = subprocess.run(["xrdb", "-query"], capture_output=True,
                              text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return 1.0
    found = re.search(r"^Xft\.dpi:\s*([0-9.]+)\s*$", said, re.MULTILINE)
    if not found or float(found.group(1)) <= 0:
        return 1.0
    return float(found.group(1)) / 96


def lander_lights(lander):
    """Where the lander's lights are, from the place between its feet.

    The display shows the lander's right side, its front to the display's
    right: so the green light on the right of its cabin, its upper stage,
    faces the viewer from the cabin's middle, and the flashing white lights
    are on the cabin's top and its rear, the display's left, and on the
    front of the nozzle, halfway up it: the nozzle being the shape straight
    below the cabin's middle that reaches lowest.
    The cabin is the shape of the outline that reaches highest.

    Args:
        lander: The :class:`Lander`.

    Returns:
        The green light's place, and the white lights', each ``(x, y)`` in
        metres; ``None`` and none where the outline has no shape.
    """
    shapes = [line for line in lander.outline if len(line) > 2]
    if not shapes:
        return None, []
    cabin = max(shapes, key=lambda line: max(y for _, y in line))
    xs = [x for x, _ in cabin]
    ys = [y for _, y in cabin]
    middle, half = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
    # The nozzle's front, its right, the lander facing right, halfway up.
    below = [line for line in shapes
             if min(x for x, _ in line) <= middle <= max(x for x, _ in line)]
    nozzle = min(below, key=lambda line: min(y for _, y in line))
    front = (max(x for x, _ in nozzle),
             (min(y for _, y in nozzle) + max(y for _, y in nozzle)) / 2)
    return (middle, half), [(middle, max(ys)), front, (min(xs), half)]


class Uplink:
    """Has Yamcs send a vehicle's gateway a number, which the gateway passes
    to a payload of the vehicle's.

    Args:
        yamcs:   The Yamcs instance of that vehicle.
        command: The gateway's command, its qualified name.
        what:    What the byte does, for the status and a complaint.
    """

    def __init__(self, yamcs, command, what):
        self._yamcs = yamcs
        self._command = command
        self._what = what

    def send(self, byte, octets=1):
        """Sends a number, from a thread of its own, so that the window does
        not wait on Yamcs: a refusal is said on stderr.

        Args:
            byte:   The number, most significant byte first.
            octets: How many bytes it is sent in.

        Returns:
            The command's arguments.
        """
        args = {"Data": byte.to_bytes(octets, "big").hex(),
                "DataBits": 8 * octets}

        def send():
            """Has Yamcs send it, saying why where it will not."""
            try:
                self._yamcs.issue(self._command, args)
                status(f"{self._what} {args['Data']} sent")
            except (RuntimeError, OSError) as why:
                print(f"lander_yamcs: {self._what}: {why}", file=sys.stderr,
                      flush=True)

        threading.Thread(target=send, daemon=True).start()
        return args


class Restarter:
    """Has Yamcs send the lander a restart, up through the vehicle whose
    gateway passes it to the payload that restarts the lander.

    Each restart is a count, from one to 255 and round again, so that each
    differs from the last: the simulated payload passes a value on only
    when it changes, and the lander starts again at each new one.

    Args:
        uplink: The :class:`Uplink` to the restart payload's vehicle.
    """

    def __init__(self, uplink):
        self._uplink = uplink
        self._count = 0

    def restart(self):
        """Sends the next restart.

        Returns:
            The command's arguments.
        """
        self._count = self._count % 255 + 1
        return self._uplink.send(self._count)


class Autorestart:
    """Has Yamcs tell the lander whether to start again by itself after a
    crash, up through the vehicle whose gateway passes it to the payload
    that sets the lander's autorestart. Until it is told, it does not: it
    stays down until it is restarted. The program enables it as soon as the
    vehicles are connected, so that a lander nobody flies starts again by
    itself after each crash. A landing does not end a flight, so this says
    nothing to a landed lander.

    Args:
        uplink: The :class:`Uplink` to the autorestart payload's vehicle.
    """

    def __init__(self, uplink):
        self._uplink = uplink

    def disable(self):
        """Keeps the lander down after a crash, until it is restarted.

        Returns:
            The command's arguments.
        """
        return self._uplink.send(0)

    def enable(self):
        """Lets the lander start again by itself after a crash, once its
        pause has run.

        Returns:
            The command's arguments.
        """
        return self._uplink.send(1)


def room_of(path):
    """The width tcs-sim's window left for this program's, as it wrote it.

    Args:
        path: The file it was told to write it in.

    Returns:
        The width, in logical pixels; ``None`` where it wrote none, or
        none that reads.
    """
    try:
        return float(Path(path).read_text().strip())
    except (OSError, ValueError):
        return None


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


def run_the_window(readings, lander, seed, delay, stopped, restarter,
                   autorestart, room=None):
    """The window, and the loop that reads Yamcs for it.

    At the top left of the display, as no wider than ``room``, where it is
    given: the width tcs-sim's window, at the top right, leaves it.

    Args:
        readings:    The :class:`Readings`.
        lander:      The :class:`Lander`, whose landscape and outline are
                     drawn.
        seed:        The landscape's seed, which the window says.
        delay:       The link's delay, in seconds, which the window says.
        stopped:     Says whether tcs-sim has stopped, its window closed:
                     this window is closed then too, which ends the run.
        restarter:   The :class:`Restarter` its Restart button sends by, or
                     ``None``, for no button, where no vehicle takes one.
        autorestart: The :class:`Autorestart` its two autorestart buttons
                     send by, or ``None``, for none.
        room:        The width it may take, in logical pixels, or ``None``
                     for its own.

    Returns:
        The exit status, nought, once the window is closed.
    """
    import tkinter as tk
    from tkinter import ttk

    from tcs_ui import fonts, powered, theme

    def reading_loop():
        """Reads Yamcs, for ever."""
        while True:
            try:
                readings.read()
            except (OSError, ValueError) as why:
                status(f"reading Yamcs: {why}")
            time.sleep(READ_EVERY)

    window = tk.Tk()
    # As large as the display's scale asks: the type, in points, through
    # Tk's own scaling, before the fonts are made; everything given in
    # pixels by multiplying it by zoom.
    zoom = display_scale()
    window.tk.call("tk", "scaling", zoom * 96 / 72)
    fonts.use_in(window)
    theme.use_in(window)
    window.title("TCS Lander MOC")
    window.configure(background="#ffffff")
    window.resizable(False, False)
    # The top left: tcs-sim's window asks for the top right.
    window.geometry("+0+0")
    between = round(theme.BETWEEN * zoom)
    powered.pack_in(window, between)
    scale = SCALE * zoom
    if room is not None:
        # The landscape no wider than the room less the window's margins,
        # the rest of the window being no wider than it.
        fits = (room - 2 * theme.BETWEEN - 4) / lander.width
        scale = min(SCALE, max(fits, 0.1)) * zoom

    wide, high = lander.width * scale, lander.height * scale

    def at(x, y):
        """A point of the world, in the scene's pixels.

        Args:
            x: Across, in metres.
            y: Up, in metres.

        Returns:
            ``(x, y)`` in pixels, down from the top.
        """
        return x * scale, high - y * scale

    # A black sky with stars over grey ground, as tcs-sim's lander pane
    # draws it.
    scene = tk.Canvas(window, width=wide, height=high, background="#000000",
                      highlightthickness=1, highlightbackground="#6b7280")
    scene.pack(padx=between, pady=(between, between // 2))
    night = scene.create_rectangle(0, 0, wide, high, fill="#000000",
                                   outline="")

    def draw_the_sky():
        """Draws the stars come down since it last drew, just above the
        black, so that the ground and the lander cover them; a new
        generation clears those drawn first."""
        if readings.sky is None:
            return
        cleared, stars = readings.sky.fresh()
        if cleared:
            scene.delete("star")
        # This window's pixels for each of the scene's a record's radius is
        # given in: the landscape is drawn narrower than tcs-sim's.
        per_scene = scale * lander.width / SKY_WIDTH
        for x, y, radius, bright in stars:
            cx, cy = at(x, y)
            disk, r, level = star_drawing(radius, bright, per_scene)
            if level == 0:
                continue
            grey = f"#{level:02x}{level:02x}{level:02x}"
            draw = scene.create_oval if disk else scene.create_rectangle
            star = draw(cx - r, cy - r, cx + r, cy + r, fill=grey,
                        outline="", tags="star")
            scene.tag_raise(star, night)
    flat = [c for x, y in lander.ground for c in at(x, y)]
    scene.create_polygon(*at(0, 0), *flat, *at(lander.width, 0),
                         fill=GROUND, outline="")
    scene.create_line(*flat, fill="#3b3127", width=1.5 * zoom)
    # The pad, white with a thin black edge, and an upright mark at each
    # end, so that it is told by its shape and not by its colour.
    (px0, py0), (px1, py1) = lander.pad
    pad_lines = [(at(px0, py0), at(px1, py1))]
    mark = PAD_MARK * lander.height
    pad_lines += [(at(x, py0), at(x, py0 + mark)) for x in (px0, px1)]
    for colour, width in (("#000000", 4), ("#ffffff", 2.5)):
        for start, end in pad_lines:
            scene.create_line(*start, *end, fill=colour,
                              width=width * zoom)
    # A light on top of each mark, flashing: bright while lit and dim
    # between, so that they are seen even then.
    lights = []
    for x in (px0, px1):
        cx, cy = at(x, py0 + mark + PAD_LIGHT)
        r = PAD_LIGHT * scale
        lights.append(scene.create_oval(cx - r, cy - r, cx + r, cy + r,
                                        fill="#E69F00", outline="#000000",
                                        width=zoom))
    state = scene.create_text(8 * zoom, 6 * zoom, anchor="nw",
                              text="Flying",
                              fill="#ffffff",
                              font=("TkDefaultFont", 14, "bold"))
    drawn = []
    opened = time.monotonic()
    green, whites = lander_lights(lander)

    # Below the landscape, the values, the joystick under them, and the note
    # under that, so that the window is no wider than the landscape. In the
    # type tcs-sim's window is set in, two points under the theme's.
    ttk.Style(window).configure("Lander.TLabel",
                                font=("TkDefaultFont", fonts.POINTS - 2))
    below = ttk.Frame(window)
    below.pack(fill="x", padx=between)
    panel = ttk.Frame(below, padding=round(theme.INSIDE * zoom))
    panel.pack(side="top", anchor="w")

    def label(**options):
        """A label in the lander window's type.

        Args:
            options: What ``ttk.Label`` is given besides its parent.

        Returns:
            The label.
        """
        return ttk.Label(panel, style="Lander.TLabel", **options)

    label(text="X").grid(row=0, column=1, sticky="e")
    label(text="Y").grid(row=0, column=2, sticky="e")
    shown = []
    for number, (name, unit, x_node, y_node) in enumerate(ROWS, start=1):
        said = f"{name} ({unit})" if unit else name
        label(text=said).grid(row=number, column=0, sticky="w")
        for column, node in ((1, x_node), (2, y_node)):
            value = label(width=8, anchor="e")
            value.grid(row=number, column=column, sticky="e",
                       padx=(between // 2, 0))
            shown.append((value, node))
    fuel = label(text="Fuel --")
    fuel.grid(row=len(ROWS) + 1, column=0, sticky="w")
    note = ttk.Label(window, style="Lander.TLabel", wraplength=wide,
                     text=f"Seed {seed}; everything here is {delay:g} s "
                          "behind the lander")
    note.pack(fill="x", padx=between, pady=(0, between))
    # Commands sent up: each reaches the lander a light time later, and
    # what it did is seen here a light time after that.
    buttons = ttk.Frame(window)
    buttons.pack(anchor="w", padx=between, pady=(0, between))
    if restarter is not None:
        ttk.Button(buttons, text="Restart",
                   command=restarter.restart).pack(side="left")
    if autorestart is not None:
        for text, command in (("Disable Autorestart", autorestart.disable),
                              ("Enable Autorestart", autorestart.enable)):
            ttk.Button(buttons, text=text, command=command).pack(
                side="left", padx=(between // 2, 0))

    # The joystick, as it was sent down, as tcs-sim's draws it: the square,
    # its dead zone, and rectangles extending from its left, right and top,
    # each hatched along its outside -- there is no thrust down -- with a
    # plus across the square while it is at rest, and two lines where it is
    # held otherwise. The pointer does nothing here.
    square, arm = SQUARE * zoom, ARM * zoom
    tick, long_tick = 6 * zoom, 10.5 * zoom
    reach = square / 2 + arm
    thick = square
    stick = tk.Canvas(below, width=2 * reach,
                      height=reach + square / 2 + long_tick,
                      background="#ffffff", highlightthickness=0)
    stick.pack(side="top", anchor="center", pady=between // 2)
    stick.create_rectangle(0, reach - thick / 2, 2 * reach, reach + thick / 2,
                           fill="#e5e7eb", outline="#6b7280")
    stick.create_rectangle(reach - thick / 2, 0, reach + thick / 2,
                           reach + thick / 2, fill="#e5e7eb",
                           outline="#6b7280")
    stick.create_rectangle(arm, arm, arm + square, arm + square,
                           fill="#f3f4f6", outline="#374151")
    for n in range(1, 11):
        along = square / 2 + n * arm / 10
        size = long_tick if n % 5 == 0 else tick
        for x in (reach - along, reach + along):
            stick.create_line(x, reach - thick / 2 - size, x,
                              reach - thick / 2, fill="#374151")
            stick.create_line(x, reach + thick / 2, x,
                              reach + thick / 2 + size, fill="#374151")
        y = reach - along
        stick.create_line(reach - thick / 2 - size, y, reach - thick / 2, y,
                          fill="#374151")
        stick.create_line(reach + thick / 2, y, reach + thick / 2 + size, y,
                          fill="#374151")
    plus = [stick.create_line(arm, reach, arm + square, reach,
                              fill="#0072B2"),
            stick.create_line(reach, arm, reach, arm + square,
                              fill="#0072B2")]
    across = stick.create_line(0, 0, 0, 0, fill="#0072B2", width=2 * zoom,
                               state="hidden")
    up = stick.create_line(0, 0, 0, 0, fill="#0072B2", width=2 * zoom,
                           state="hidden")

    def take_in():
        """Shows what the loop has read, and comes back in
        :data:`REFRESH_MS`; or closes the window, tcs-sim's having been
        closed."""
        if stopped():
            window.destroy()
            return
        for value, node in shown:
            value.configure(text=readings.text(node))
        left = readings.number("to_fuel")
        fuel.configure(text="Fuel --" if left is None
                       else f"Fuel {round(left * 100)}%")
        scene.itemconfigure(state, text=readings.state())
        draw_the_sky()
        for item in drawn:
            scene.delete(item)
        drawn.clear()
        x, y = readings.estimate("x"), readings.estimate("y")
        if x is not None and y is not None:
            # As tcs-sim's lander pane draws it: its single lines --
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
                                                   width=width * zoom))
            for flat in lines:
                if len(flat) > 4:
                    drawn.append(scene.create_polygon(
                        *flat, fill="#ffffff", outline="#000000",
                        width=0.75 * zoom))
            # Its lights over it: the green steady, the white flashing --
            # bright in a flash and dim between.
            strobe = (time.monotonic() - opened) % STROBE_EVERY < STROBE_FOR
            r = LANDER_LIGHT * zoom
            flash = "#ffffff" if strobe else "#9ca3af"
            lamps = [(place, flash, "#000000") for place in whites]
            if green is not None:
                lamps.append((green, "#009E73", ""))
            for (lx, ly), fill, edge in lamps:
                cx, cy = at(x + lx, y + ly)
                # A black ring round each white one, so that it shows on
                # the white lander as well as against the sky.
                drawn.append(scene.create_oval(cx - r, cy - r, cx + r,
                                               cy + r, fill=fill,
                                               outline=edge, width=zoom))
        since = time.monotonic() - opened
        lit = since % PAD_FLASH < PAD_FLASH / 2
        for light in lights:
            scene.itemconfigure(light, fill="#E69F00" if lit else "#5c4000")
        sx = readings.number("to_joystick_3_4_x") or 0.0
        sy = readings.number("to_joystick_3_4_y") or 0.0
        # At rest, the plus; held, a line across the level rectangles and
        # one across the upright, halfway up the square for no thrust.
        resting = sx == 0 and sy == 0
        for item in plus:
            stick.itemconfigure(item, state="normal" if resting else "hidden")
        for item in (across, up):
            stick.itemconfigure(item, state="hidden" if resting else "normal")
        cx = reach + stick_offset(sx) * zoom
        cy = reach - stick_offset(max(0.0, sy)) * zoom
        stick.coords(across, cx, reach - thick / 2, cx, reach + thick / 2)
        stick.coords(up, reach - thick / 2, cy, reach + thick / 2, cy)
        window.after(REFRESH_MS, take_in)

    threading.Thread(target=reading_loop, daemon=True).start()
    window.after(REFRESH_MS, take_in)
    # Under tcs-sim's window, which the lander is flown from and which is to
    # be on top where the two overlap: this one opens after it, and so over
    # it. A request, which the window manager may refuse; the lander
    # example's KWin rule keeps it below in any case.
    window.after(LOWER_AFTER_MS, window.lower)
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
    room_file = run_dir / "room"
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
            # Told where to say how much width its window leaves, at the top
            # right, for this program's window to fit in.
            room_file.unlink(missing_ok=True)
            simulator = Child(tcs_sim, ["--sending"]
                              + simulator_args(sim_path, crafts), "tcs-sim",
                              options.verbose,
                              env=dict(os.environ,
                                       TCS_SIM_ROOM_FILE=str(room_file)))
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
        uplinks = {target: Uplink(yamcs, command, target)
                   for yamcs, craft in zip(yamcses, crafts)
                   for target, command in craft.commands.items()}
        # The sky, quenched by its generation, two bytes, once it is whole.
        quencher = uplinks.get("sky_quench")
        sky = Sky(None if quencher is None
                  else lambda generation: quencher.send(generation, 2))
        readings = Readings(list(zip(yamcses, crafts)), sky)
        restarter = (Restarter(uplinks["restart"]) if "restart" in uplinks
                     else None)
        autorestart = (Autorestart(uplinks["autorestart"])
                       if "autorestart" in uplinks else None)
        # On from the start, so that a lander nobody flies starts again by
        # itself after each crash; the window's buttons change it after.
        if autorestart is not None:
            autorestart.enable()
        if options.seconds is None:
            return run_the_window(readings, lander, seed, options.delay,
                                  stopped, restarter, autorestart,
                                  room_of(room_file))
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
