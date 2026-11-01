#!/usr/bin/env python3
"""The ping example with Yamcs as the ground station.

The vehicle is the ping example's -- a ``ping`` node that answers each ping
with its time on the end -- and so is what is timed: the round trip, and the
two halves of it, up and down. What differs is the ground end of the link:
Yamcs (https://yamcs.org), which speaks CCSDS packets, so the configuration's
gateway is ``yamcs``. Every packet to and from the ground then carries a CCSDS
primary header, which tcspecial adds as each goes and takes off as each
arrives: the beacon, CONNECT and its answer, the ping and its answer.

This program puts the two together:

* it writes a Yamcs configuration from the tcspecial one -- the ports, and the
  APIDs Yamcs tells the packets apart by -- into a run directory, with a
  mission database of tcspecial's own XTCE (``tcslibgs/xtce``) and a wrapper,
  ``yamcs/tcs-ccsds.xtce.in``, that puts the header around it;
* it starts Yamcs on it, and tcspecial on the configuration, each killed when
  this program exits;
* it has Yamcs send a CONNECT carrying the configuration's checksum until the
  vehicle answers with it -- the vehicle sends nothing down, ping answers
  included, until it has -- and then sends pings through Yamcs and reads the
  answers back from it, over Yamcs's REST API.

Yamcs's web interface, at http://localhost:16090 by default, shows every
packet and parameter meanwhile: the beacon, the answers, and the pings.

Usage, from the repository's root, with Yamcs unpacked somewhere::

    python3 examples/ping-yamcs/ping_yamcs.py --yamcs ~/yamcs-5.14.0
    python3 examples/ping-yamcs/ping_yamcs.py --pings 3   # and stop
    python3 examples/ping-yamcs/ping_yamcs.py --describe  # what it read

``--yamcs`` is the directory a Yamcs release unpacks to, holding
``bin/yamcsd``; ``$YAMCS_HOME`` says it when ``--yamcs`` does not. With
neither ``--pings`` nor ``--describe`` it pings once a second until it is
stopped. ``--no-vehicle`` leaves tcspecial to somebody else, and
``--no-yamcs`` uses a Yamcs already running on the run directory's
configuration.
"""

import argparse
import base64
import datetime
import json
import os
import shutil
import signal
import string
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "python"))
# The ping example's halves, which this one shares: its vehicle, and the
# ping link's bookkeeping.
sys.path.insert(0, str(HERE.parent / "ping"))

from tcslibgs import configuration, ping  # noqa: E402
from tcslibgs.configuration import whole  # noqa: E402
from ping_gateway import Vehicle, the_one  # noqa: E402
from ping_ground import Exchange, Pinger, interval  # noqa: E402

#: Where Yamcs's web interface and REST API are, unless --http-port says.
HTTP_PORT = 16090

#: The Yamcs instance this example runs.
INSTANCE = "ping"

#: The largest APID, eleven bits.
MAX_APID = 0x7FF

#: The wrapper's qualified names, as Yamcs knows them.
CONNECT = "/TcsYamcs/Connect"
PING_TEXT = "/TcsYamcs/PingText"
PING_REPLY = "/TcsYamcs/PingReply"
RUNNING = "/TcsFixed/Running"
BEACON_FIELDS = ("Name", "Version", "Number", "Flag")


def arguments(argv):
    """Reads the command line.

    Args:
        argv: The arguments, without the program's name.

    Returns:
        The options, as :mod:`argparse` gives them.
    """
    parser = argparse.ArgumentParser(description="Ping a tcspecial vehicle "
                                                 "through Yamcs.")
    parser.add_argument(
        "configuration", nargs="?", default=str(HERE / "ping-yamcs.xtcs"),
        help="the tcspecial configuration (default: ping-yamcs.xtcs)")
    parser.add_argument("--yamcs", default=os.environ.get("YAMCS_HOME"),
                        help="where Yamcs is unpacked (default: $YAMCS_HOME)")
    parser.add_argument("--run-dir", default="/tmp/ping-yamcs",
                        help="where Yamcs's configuration and data are written "
                             "(default: /tmp/ping-yamcs)")
    parser.add_argument("--http-port", type=int, default=HTTP_PORT,
                        help=f"Yamcs's web port (default: {HTTP_PORT})")
    parser.add_argument("--pings", type=int, metavar="N",
                        help="send N pings, print each exchange, and stop")
    parser.add_argument("--describe", action="store_true",
                        help="print what was read and the Yamcs settings, and "
                             "stop")
    parser.add_argument("--no-vehicle", action="store_true",
                        help="do not start tcspecial")
    parser.add_argument("--no-yamcs", action="store_true",
                        help="do not start Yamcs; use one already running")
    parser.add_argument("--tcspecial", help="the tcspecial program to run")
    parser.add_argument("--tcsverify",
                        help="the tcsverify program that gives the "
                             "configuration's checksum")
    parser.add_argument("-v", "--verbose", action="store_true",
                        help="say what it is doing")
    return parser.parse_args(argv)


#: Whether to print status messages: ``-v`` or ``--verbose``.
VERBOSE = False


def status(text):
    """Say ``text`` on stderr, if asked to with ``-v``."""
    if VERBOSE:
        print(text, file=sys.stderr, flush=True)


def apid(attribute, said):
    """Reads an APID the configuration states.

    Args:
        attribute: Its name, for the complaint.
        said:      The value as the file writes it, decimal or 0x hex.

    Returns:
        The APID.

    Raises:
        ValueError: It is missing, not a number, or past eleven bits.
    """
    if said is None:
        raise ValueError(f"{attribute} is not stated, and a yamcs "
                         "configuration needs it")
    return whole(attribute, said, MAX_APID)


class Setup:
    """What the program read out of the configuration, and the Yamcs
    settings that follow from it.

    Args:
        options: The command line's options, from :func:`arguments`.

    Raises:
        ValueError: The configuration is wrong for this program.
        OSError: The configuration cannot be read.
    """

    def __init__(self, options):
        self.path = Path(options.configuration).resolve()
        self.config = configuration.load(self.path)
        said = self.config.tcspecial
        if said.get("gateway") != "yamcs":
            raise ValueError(f"{self.path}: its gateway is "
                             f"{said.get('gateway')!r}, and this program is "
                             "Yamcs's: say gateway: yamcs")
        self.gateway = the_one(self.config.nodes, "gateway_yamcs", None)
        self.node = the_one(self.config.nodes, "ping", None)
        self.uplink = self.config.uplink(self.gateway)
        self.downlink = self.config.downlink(self.gateway, self.node)
        self.beacon = self.config.beacon_link()
        self.command = configuration.HostPort.of("tcs_cmd.recv",
                                                 said.get("tcs_cmd.recv"))
        self.beacon_apid = apid("beacon.apid", said.get("beacon.apid"))
        self.cmd_apid = apid("tcs_cmd.apid", said.get("tcs_cmd.apid"))
        gateway = self.config.node(self.gateway)
        self.uplink_apid = apid(f'"{self.gateway}" apid_base',
                                gateway.attributes.get("apid_base"))
        self.reply_apid = self.uplink_apid + self.config.device_id(self.node)
        if self.reply_apid > MAX_APID:
            raise ValueError(f"{self.path}: the ping node's APID, apid_base "
                             f"plus its device id, is {self.reply_apid:#x}, "
                             f"past {MAX_APID:#x}")
        self.digest, self.why_no_digest = configuration.digest(
            self.path, options.tcsverify)

    def yamcs_settings(self, run_dir, http_port):
        """The names the Yamcs templates are filled in with.

        Args:
            run_dir:   Where Yamcs's configuration and data go.
            http_port: Yamcs's web port.

        Returns:
            A dictionary of each template name and its value.
        """
        return {
            "http_port": str(http_port),
            "data_dir": str(run_dir / "data"),
            "run_dir": str(run_dir),
            "xtce_dir": str(ROOT / "tcslibgs" / "xtce"),
            # Yamcs reaches tcspecial's command link where the vehicle takes
            # commands; a vehicle that takes them on every interface is
            # reached here.
            "cmd_host": local(self.command.address),
            "cmd_port": str(self.command.port),
            "uplink_host": local(self.uplink.address),
            "uplink_port": str(self.uplink.port),
            "downlink_port": str(self.downlink.port),
            "beacon_port": str(self.beacon.recv.port),
            "beacon_apid": str(self.beacon_apid),
            "cmd_apid": str(self.cmd_apid),
            "uplink_apid": str(self.uplink_apid),
            "reply_apid": str(self.reply_apid)}

    def describe(self, run_dir, http_port):
        """The facts, one per line.

        Args:
            run_dir:   Where Yamcs's configuration and data go.
            http_port: Yamcs's web port.

        Returns:
            Each Yamcs setting as ``name value``, then the digest.
        """
        settings = self.yamcs_settings(run_dir, http_port)
        lines = [f"{name} {value}" for name, value in settings.items()]
        return "\n".join(lines + [f"digest {self.digest}"])


def local(address):
    """The address to reach a socket at from this host: itself, or this
    host for one bound on every interface."""
    return "127.0.0.1" if address in ("0.0.0.0", "::", "") else address


def write_yamcs(setup, run_dir, http_port, yamcs_home):
    """Writes Yamcs's configuration for this tcspecial configuration.

    Args:
        setup:      What was read out of the configuration.
        run_dir:    Where to write it: ``etc/``, the wrapper XTCE, and an
                    empty ``data/``. Made afresh each run.
        http_port:  Yamcs's web port.
        yamcs_home: Where Yamcs is unpacked, whose sample processor
                    configuration is used as it is.

    Raises:
        OSError: A file cannot be written, or Yamcs's sample is missing.
    """
    if run_dir.exists():
        shutil.rmtree(run_dir)
    (run_dir / "etc").mkdir(parents=True)
    (run_dir / "data").mkdir()
    settings = setup.yamcs_settings(run_dir, http_port)
    templates = HERE / "yamcs"
    for template, written in (("yamcs.yaml.in", "etc/yamcs.yaml"),
                              ("yamcs.ping.yaml.in", "etc/yamcs.ping.yaml"),
                              ("tcs-ccsds.xtce.in", "tcs-ccsds.xtce")):
        text = (templates / template).read_text()
        (run_dir
         / written).write_text(string.Template(text).substitute(settings))
    # Yamcs's own processor configuration, but releasing commands to both
    # of this instance's command streams, which route them: its sample sends
    # every command to one.
    sample = (Path(yamcs_home) / "etc" / "processor.yaml.sample").read_text()
    one_stream = 'stream: "tc_realtime"'
    if sample.count(one_stream) != 1:
        raise OSError(f"{yamcs_home}/etc/processor.yaml.sample does not "
                      f"release commands with {one_stream}, as this "
                      "program expects of Yamcs 5.14")
    (run_dir / "etc" / "processor.yaml").write_text(
        sample.replace(one_stream,
                       'streams: ["tc_commands", "tc_uplink"]'))


class Yamcs:
    """Yamcs, run by this program on the run directory's configuration and
    killed when it exits, and its REST API.

    Args:
        home:      Where Yamcs is unpacked.
        run_dir:   The run directory, which :func:`write_yamcs` wrote.
        http_port: Its web port.
    """

    def __init__(self, home, run_dir, http_port):
        self.home = Path(home) if home else None
        self.run_dir = run_dir
        self.base = f"http://localhost:{http_port}/api"
        self.process = None

    def start(self):
        """Start Yamcs, told to die with this program, and wait until its
        instance runs.

        Raises:
            RuntimeError: It stops, or its instance does not run within a
                          minute.
        """
        program = self.home / "bin" / "yamcsd"
        log = open(self.run_dir / "yamcs.log", "w")
        self.process = subprocess.Popen(
            [str(program), "--etc-dir", str(self.run_dir / "etc"), "--data-dir",
             str(self.run_dir / "data"), "--cache-dir",
             str(self.run_dir / "cache")], stdout=log, stderr=subprocess.STDOUT,
            start_new_session=True)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError("Yamcs stopped: see "
                                   f"{self.run_dir / 'yamcs.log'}")
            if self.running():
                return
            time.sleep(0.5)
        raise RuntimeError(f"Yamcs's instance {INSTANCE} did not start: see "
                           f"{self.run_dir / 'yamcs.log'}")

    def running(self):
        """Whether the instance is running."""
        try:
            said = self._get(f"/instances/{INSTANCE}")
        except (OSError, ValueError):
            return False
        return said.get("state") == "RUNNING"

    def stop(self):
        """Stop Yamcs, and wait for it to be gone."""
        if self.process is None or self.process.poll() is not None:
            return
        os.killpg(self.process.pid, signal.SIGTERM)
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(self.process.pid, signal.SIGKILL)
            self.process.wait()

    def issue(self, command, args):
        """Has Yamcs send a command.

        Args:
            command: The command's qualified name.
            args:    Its arguments, by name.

        Raises:
            RuntimeError: Yamcs refuses it.
        """
        body = json.dumps({"args": args}).encode()
        url = (f"{self.base}/processors/{INSTANCE}/realtime/commands{command}")
        request = urllib.request.Request(
            url, data=body, method="POST",
            headers={"Content-Type": "application/json"})
        try:
            urllib.request.urlopen(request, timeout=5).read()
        except urllib.error.HTTPError as why:
            raise RuntimeError(f"Yamcs refused {command}: "
                               f"{why.read().decode(errors='replace')}")

    def parameter(self, name):
        """A parameter's latest value, and when Yamcs received it.

        Args:
            name: The parameter's qualified name.

        Returns:
            ``(value, received)`` -- the engineering value as Python has it
            (bytes for a binary one), and when Yamcs acquired it, in
            nanoseconds since the epoch -- or ``None`` before any has
            arrived.
        """
        try:
            said = self._get(f"/processors/{INSTANCE}/realtime/parameters"
                             f"{name}")
        except (OSError, ValueError):
            return None
        value = said.get("engValue")
        if not value:
            return None
        return eng_value(value), reception_ns(said.get("acquisitionTime"))

    def _get(self, path):
        """A REST API reply, read as JSON."""
        with urllib.request.urlopen(self.base + path, timeout=5) as reply:
            return json.loads(reply.read())


def eng_value(value):
    """A Yamcs value as Python has it: the field its type names."""
    kind = value.get("type", "")
    if kind == "BINARY":
        return base64.b64decode(value.get("binaryValue", ""))
    for field in ("stringValue", "uint32Value", "uint64Value", "sint32Value",
                  "sint64Value", "floatValue", "doubleValue", "booleanValue"):
        if field in value:
            return value[field]
    return None


def reception_ns(text):
    """A Yamcs time, as ``2026-11-01T07:46:06.828Z``, in nanoseconds since
    the epoch; nought for none."""
    if not text:
        return 0
    when = datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))
    whole_seconds = int(when.replace(microsecond=0).timestamp())
    return whole_seconds * ping.NANOS + when.microsecond * 1000


class YamcsPinger(Pinger):
    """The ping link through Yamcs: each ping is a PingText command, and
    each answer the PingReply parameter.

    The ping example's :class:`ping_ground.Pinger`, keeping what it keeps
    and timing what it times; only how a ping goes and an answer arrives
    differ. The time an answer arrived is when Yamcs received it, to the
    millisecond, which is as finely as Yamcs says.

    Args:
        yamcs: The Yamcs to send through.
        clock: What gives the time now, in nanoseconds since the epoch.
    """

    def __init__(self, yamcs, clock=time.time_ns):
        self._yamcs = yamcs
        self._clock = clock
        self._count = 0
        self._exchange = Exchange()
        latest = yamcs.parameter(PING_REPLY)
        self._seen = latest[1] if latest else 0

    def ping(self):
        """Send the next ping through Yamcs, and say what it was.

        Returns:
            The ping sent, as text.
        """
        self._count += 1
        at = self._clock()
        message = ping.message(self._count, at)
        self._yamcs.issue(PING_TEXT,
                          {"Text": message, "TextBits": 8 * len(message)})
        self._exchange = Exchange(sent=message, sent_at=at,
                                  received=self._exchange.received)
        return message

    def receive(self, timeout):
        """Wait for Yamcs to receive an answer it had not before.

        Args:
            timeout: How long to wait, in seconds.

        Returns:
            Whether one arrived.
        """
        deadline = time.monotonic() + timeout
        while True:
            latest = self._yamcs.parameter(PING_REPLY)
            if latest is not None and latest[1] > self._seen:
                text, self._seen = latest
                self.answered(text.encode("utf-8"), self._seen)
                return True
            if time.monotonic() >= deadline:
                return False
            time.sleep(0.02)

    def close(self):
        """Nothing to close: Yamcs holds the sockets."""


def connect(setup, yamcs, attempts=30):
    """Has Yamcs send CONNECT, carrying the configuration's checksum, once a
    second until the vehicle answers that it is running it.

    Args:
        setup:    What was read out of the configuration.
        yamcs:    The Yamcs to send through.
        attempts: How many to send before giving up.

    Returns:
        Whether the vehicle answered with the checksum.
    """
    want = bytes.fromhex(setup.digest)
    for sequence in range(1, attempts + 1):
        yamcs.issue(CONNECT, {"Sequence": sequence, "Checksum": setup.digest})
        status(f"CONNECT {sequence} sent")
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            running = yamcs.parameter(RUNNING)
            if running is not None and running[0] == want:
                return True
            time.sleep(0.05)
    return False


def beacon_said(yamcs):
    """What the latest beacon says, as text, or ``None`` before one."""
    fields = []
    for field in BEACON_FIELDS:
        value = yamcs.parameter(f"/TcsBeacon/Beacon{field}")
        if value is None:
            return None
        fields.append(f"{field.lower()} {value[0]}")
    return ", ".join(fields)


def ping_and_print(pinger, count):
    """Send ``count`` pings, one a second, and print each exchange.

    Args:
        pinger: The ping link.
        count:  How many, or ``None`` for until stopped.

    Returns:
        The exit status: nought if every ping was answered, one if not.
    """
    answered_all = True
    sent = 0
    while count is None or sent < count:
        sent += 1
        print(f"sent: {pinger.ping()}", flush=True)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            if (pinger.receive(max(deadline - time.monotonic(), 0.01))
                    and pinger.exchange().received_at):
                break
        exchange = pinger.exchange()
        if exchange.received_at is None:
            print("received: nothing", flush=True)
            answered_all = False
        else:
            print(f"received: {exchange.received}", flush=True)
            print(f"times: round trip {interval(exchange.round_trip())}, up "
                  f"{interval(exchange.up())}, down "
                  f"{interval(exchange.down())}", flush=True)
        if count is None:
            time.sleep(1)
    return 0 if answered_all else 1


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
    try:
        setup = Setup(options)
    except (ValueError, OSError) as why:
        print(f"ping_yamcs: {why}", file=sys.stderr)
        return 2
    if options.describe:
        print(setup.describe(run_dir, options.http_port))
        return 0
    if setup.digest is None:
        print(f"ping_yamcs: no checksum for CONNECT: {setup.why_no_digest}",
              file=sys.stderr)
        return 2
    yamcs = Yamcs(options.yamcs, run_dir, options.http_port)
    vehicle = None
    try:
        if not options.no_yamcs:
            if (not options.yamcs
                    or not (Path(options.yamcs) / "bin" / "yamcsd").exists()):
                print("ping_yamcs: say where Yamcs is, with --yamcs or "
                      "$YAMCS_HOME: the directory holding bin/yamcsd",
                      file=sys.stderr)
                return 2
            write_yamcs(setup, run_dir, options.http_port, options.yamcs)
            status(f"starting Yamcs on {run_dir}")
            yamcs.start()
            status(f"Yamcs is at http://localhost:{options.http_port}")
        if not options.no_vehicle:
            vehicle = Vehicle.for_configuration(setup.path, options.tcspecial,
                                                options.verbose)
            vehicle.start()
        if not connect(setup, yamcs):
            print("ping_yamcs: the vehicle never answered CONNECT with this "
                  "configuration's checksum", file=sys.stderr)
            return 1
        print("connected: the vehicle is running this configuration",
              flush=True)
        heard = beacon_said(yamcs)
        if heard is not None:
            print(f"beacon: {heard}", flush=True)
        return ping_and_print(YamcsPinger(yamcs), options.pings)
    except (RuntimeError, ValueError, OSError) as why:
        print(f"ping_yamcs: {why}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 0
    finally:
        if vehicle is not None:
            vehicle.stop()
        if not options.no_yamcs:
            yamcs.stop()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
