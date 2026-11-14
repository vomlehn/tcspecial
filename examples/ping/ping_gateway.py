#!/usr/bin/env python3
"""The ping example's ground station: a window that pings the vehicle.

Two halves borrowed and one of its own, as the beacon example is put together
in Rust:

* the beacon, from ``python/tcslib_moc/beacon.py`` and
  ``python/tcslib_ui/beacon_panel.py`` -- the Python spellings of
  ``tcslib_moc::beacon`` and ``tcslib_ui`` -- so the window says whether the
  vehicle is alive before anybody presses anything;
* the ping, from ``examples/ping-common/ping_ground.py``, which sends each
  ping up to the ``gateway`` node and times the ``ping`` node's answer, and
  the window, from ``ping_window.py`` beside it: what ping-yamcs, the same
  example through Yamcs, shares with this one;
* and this program: read the configuration, start the two loops, put what
  they report in the window.

It sends a CONNECT of its own, carrying the configuration's checksum, as
soon as the vehicle is started: a vehicle sends nothing down -- the ping
node's answers included -- until it has taken one, and a station that only
pings cannot count on a commanding one's being there. It sends it again each
second no answer comes, or the answer carries another configuration's
checksum, until the vehicle answers with this one's, and says so; see
``python/tcslib_moc/listening.py``. It is sent from the configuration's
``tcs_cmd.send``, so nothing else -- tcs-moc -- may be using that address
meanwhile. ``--pings`` waits for the match before its first ping.

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

The configuration is ``examples/ping/ping.xtcs`` unless another is named:
``--yaml`` reads ``ping.ytcs`` instead, and ``--xml`` says the default. One
of the two at most, and a file named beside one has to be that spelling.
Reading XML it uses nothing outside the standard library; reading YAML it
needs PyYAML. Python 3.7 or later.
"""

import argparse
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "python"))
# What this example shares with ping-yamcs, its version through Yamcs: the
# ping link and the window.
sys.path.insert(0, str(HERE.parent / "ping-common"))

from tcslib_moc.beacon import Beacons, Schedule  # noqa: E402
from tcslib_moc.listening import Listening  # noqa: E402
from tcslib_moc.processes import Vehicle  # noqa: E402
from tcslib_protocol import configuration  # noqa: E402
from ping_ground import Pinger, report  # noqa: E402
from ping_window import run_the_window, the_one  # noqa: E402

#: The example's configuration, without its extension: ``.xtcs``, which is
#: read where nobody says otherwise, or ``.ytcs``, the two saying the same
#: thing.
STEM = HERE / "ping"

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
            f"the configuration (default: {STEM}.xtcs, or with --yaml "
            f"{STEM}.ytcs)"
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
        "--tcs-verify", help="the tcs-verify to digest the configuration with"
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
            options.configuration, options.tcs_verify
        )
        # Where the CONNECT goes from and to, where the section says:
        # ``None`` for a section that states no command link.
        said = self.config.tcspecial
        self.command = (
            self.config.command_link()
            if "tcs_cmd.send" in said and "tcs_cmd.recv" in said
            else None
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


def connecting(setup):
    """Start sending CONNECT, on a thread of its own, until the vehicle
    answers with this configuration's checksum.

    What is wrong -- a vehicle running another configuration -- is said
    whether or not ``-v`` was given; a CONNECT unanswered while the vehicle
    starts, and the match, are status.

    Args:
        setup: What the program read out of the configuration.

    Returns:
        A ``threading.Event`` set once the vehicle has answered with this
        configuration's checksum; or ``None`` where no CONNECT can be sent,
        having said why: the configuration states no command link, or could
        not be digested.

    Raises:
        OSError: ``tcs_cmd.send`` cannot be bound.
    """
    if setup.command is None:
        status("the configuration states no command link: no CONNECT is sent")
        return None
    if setup.digest is None:
        print("no CONNECT is sent, since the configuration has no digest",
              file=sys.stderr)
        return None
    ours = bytes.fromhex(setup.digest)
    listening = Listening(setup.command, ours)
    matched = threading.Event()

    def say(heard):
        """Say what one CONNECT heard: see :func:`connecting`. A standard
        error nobody reads any longer -- closed by whoever started this --
        is no reason to stop connecting, so a failure to say it is let go.

        Args:
            heard: The :class:`tcslib_moc.listening.Heard`.
        """
        try:
            if heard.running is not None and not heard.matched:
                print(heard.said(ours), file=sys.stderr, flush=True)
            else:
                status(heard.said(ours))
        except OSError:
            pass

    def connect():
        """Send CONNECT until matched, and say so."""
        listening.until_matched(say)
        listening.close()
        matched.set()

    threading.Thread(target=connect, daemon=True).start()
    return matched


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
        lines, answered = report(pinger.exchange())
        for line in lines:
            print(line, flush=True)
        answered_all = answered_all and answered
    return 0 if answered_all else 1


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
        # The CONNECT, once the vehicle is started, since nothing comes down
        # until the vehicle has taken one.
        try:
            matched = connecting(setup)
        except OSError as why:
            print(f"cannot send CONNECT: {why}", file=sys.stderr)
            return 1
        if options.pings is not None:
            if matched is not None:
                matched.wait()
            return run_without_a_window(setup, beacons, pinger, options.pings)
        return run_the_window(setup, beacons, pinger, say=status)
    finally:
        if vehicle is not None:
            vehicle.stop()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
