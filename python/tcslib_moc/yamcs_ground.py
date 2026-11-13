"""A ground station that reaches tcspecial through Yamcs.

What the examples that put Yamcs (https://yamcs.org) between their ground
station and the vehicle share -- ping-yamcs and thermostat-yamcs:

* :func:`write_yamcs` writes a Yamcs server and instance for a tcspecial
  configuration whose gateway is ``yamcs``, from the templates in
  ``yamcs-templates`` beside this, with the links, the command streams and
  the mission database :mod:`tcslib_protocol.yamcs` works out of the
  configuration;
* :class:`Yamcs` starts it, told to die with the ground station, and talks
  to it over its REST API: commands issued, parameters read;
* :func:`connect` has it send CONNECT, carrying the configuration's
  checksum, until the vehicle answers with it, as it sends nothing down
  until it has;
* :class:`YamcsBeacons` reads each beacon Yamcs receives as
  :class:`tcslib_moc.beacon.Beacons` reads one from its own socket.

Only the standard library is used, but for PyYAML, with which the
instance's configuration is written.
"""

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

from tcslib_moc.beacon import Arrived, Beacons
from tcslib_moc.processes import die_with_this_program
from tcslib_protocol import yamcs as generated

#: The templates of Yamcs's server and instance configurations.
TEMPLATES = Path(__file__).resolve().parent / "yamcs-templates"

#: tcspecial's own XTCE, which the instance's mission database reads as it
#: is: the crate's ``tcslib-protocol/xtce``, linked beside
#:mod:`tcslib_protocol`.
XTCE_DIR = Path(generated.__file__).resolve().parent / "xtce"

#: The wrapper's qualified names, as Yamcs knows them.
CONNECT = generated.qualified(generated.CONNECT)
RUNNING = "/TcsFixed/Running"
BEACON_BODY = generated.qualified("BeaconBody")

#: How long a pass of the beacon loop waits for a beacon, in seconds: often
#: enough that the indicator changes stage when it should, to the eye.
BEACON_POLL = 0.25

#: Nanoseconds in a second.
NANOS = 1_000_000_000

#: The command stream Yamcs's sample processor configuration releases every
#: command to, which the instance's own streams replace.
ONE_STREAM = 'stream: "tc_realtime"'


def _quiet(_text):
    """Says nothing: what a status message is told when nobody asked.

    Args:
        _text: The message.
    """


def command_settings(config):
    """What a configuration says of the links every Yamcs station uses:
    where Yamcs reaches tcspecial's command link and its beacon, and their
    APIDs.

    Args:
        config: The :class:`tcslib_protocol.configuration.Configuration`.

    Returns:
        A dictionary of each setting's name and its value, as text: the
        names a program's ``--describe`` prints them by.

    Raises:
        ValueError: The configuration does not answer to Yamcs, or a link
                    or an APID does not read.
    """
    command = config.command_link().recv
    return {"cmd_host": generated.local(command.address),
            "cmd_port": str(command.port),
            "beacon_port": str(config.beacon_link().recv.port),
            "beacon_apid": str(config.beacon_apid()),
            "cmd_apid": str(config.command_apid())}


def yamcs_home(said):
    """Where Yamcs is unpacked, checked.

    Args:
        said: The directory a Yamcs release unpacks to, as ``--yamcs`` or
              ``$YAMCS_HOME`` gives it, or ``None``.

    Returns:
        Its path.

    Raises:
        ValueError: None is given, or it holds no ``bin/yamcsd``.
    """
    if not said or not (Path(said) / "bin" / "yamcsd").exists():
        raise ValueError("say where Yamcs is, with --yamcs or $YAMCS_HOME: "
                         "the directory holding bin/yamcsd")
    return Path(said)


def write_yamcs(config, run_dir, http_port, home, instance, writer,
                world=False):
    """Writes Yamcs's configuration for a tcspecial configuration.

    The server's and the instance's from :data:`TEMPLATES`, with the instance's
    links and command streams added, and the mission database's wrapper, each
    worked out of the configuration by :mod:`tcslib_protocol.yamcs`.

    Args:
        config:    The :class:`tcslib_protocol.configuration.Configuration`.
        run_dir:   Where to write it: ``etc/``, the wrapper XTCE, and an
                   empty ``data/``. Made afresh each run.
        http_port: Yamcs's web port.
        home:      Where Yamcs is unpacked, whose sample processor
                   configuration is used, releasing commands to the
                   instance's streams.
        instance:  The instance's name.
        writer:    The program writing it, which each file says.
        world:     Whether the mission database has the simulated world's
                   ambient temperature: see :func:`tcslib_protocol.yamcs.mdb`.

    Raises:
        ValueError: The configuration does not give Yamcs what it needs: an
                    APID that does not read or is used twice, or a node
                    name XTCE does not allow.
        OSError: A file cannot be written, Yamcs's sample is missing or not
                 Yamcs 5.14's, or PyYAML, which writes the instance's
                 configuration, is not installed.
    """
    try:
        import yaml
    except ImportError:
        raise OSError(f"PyYAML is not installed, and {writer} writes "
                      "Yamcs's configuration with it: pip install PyYAML")
    run_dir = Path(run_dir)
    if run_dir.exists():
        shutil.rmtree(run_dir)
    (run_dir / "etc").mkdir(parents=True)
    (run_dir / "data").mkdir()
    settings = {"http_port": str(http_port), "data_dir": str(run_dir / "data"),
                "run_dir": str(run_dir), "xtce_dir": str(XTCE_DIR),
                "instance": instance, "writer": writer}

    def filled(template):
        """A template, its names filled in from ``settings``.

        Args:
            template: The template's name in :data:`TEMPLATES`.

        Returns:
            Its text, filled in.
        """
        text = (TEMPLATES / template).read_text()
        return string.Template(text).substitute(settings)

    (run_dir / "etc" / "yamcs.yaml").write_text(filled("yamcs.yaml.in"))
    said = yaml.safe_load(filled("instance.yaml.in"))
    said["dataLinks"] = generated.data_links(config)
    streams = generated.command_streams(config)
    said["streamConfig"]["tc"] = streams
    (run_dir / "etc" / f"yamcs.{instance}.yaml").write_text(
        f"# Written by {writer} from\n"
        "# python/tcslib_moc/yamcs-templates/instance.yaml.in and\n"
        f"# {config.path}, its links and command streams\n"
        "# by tcslib_protocol/yamcs.py: edit those, not this.\n"
        + yaml.safe_dump(said, sort_keys=False))
    generated.write_mdb(config, run_dir / "tcs-ccsds.xtce", world=world)
    sample = (Path(home) / "etc" / "processor.yaml.sample").read_text()
    if sample.count(ONE_STREAM) != 1:
        raise OSError(f"{home}/etc/processor.yaml.sample does not release "
                      f"commands with {ONE_STREAM}, as {writer} expects of "
                      "Yamcs 5.14")
    names = ", ".join(f'"{stream["name"]}"' for stream in streams)
    (run_dir / "etc" / "processor.yaml").write_text(
        sample.replace(ONE_STREAM, f"streams: [{names}]"))


class Yamcs:
    """Yamcs, run by this program on the run directory's configuration and
    killed when it exits, and its REST API.

    Args:
        home:      Where Yamcs is unpacked.
        run_dir:   The run directory, which :func:`write_yamcs` wrote.
        http_port: Its web port.
        instance:  The instance this program runs.
    """

    def __init__(self, home, run_dir, http_port, instance):
        self.home = Path(home) if home else None
        self.run_dir = Path(run_dir)
        self.instance = instance
        self.base = f"http://localhost:{http_port}/api"
        self.process = None

    def start(self):
        """Start Yamcs, told to die with this program, and wait until its
        instance runs.

        In a session of its own, so that a Ctrl-C at the terminal is this
        program's to stop it by and not Java's, which would leave it no
        Yamcs to stop. Told to die with this program all the same: a
        program that is killed runs no ``finally``, and Yamcs would go on
        holding the example's ports. ``yamcsd`` ends by becoming Java, so
        what the kernel is told holds for Java.

        Raises:
            RuntimeError: It stops, or its instance does not run within a
                          minute.
        """
        program = self.home / "bin" / "yamcsd"
        log = open(self.run_dir / "yamcs.log", "w")
        parent = os.getpid()
        self.process = subprocess.Popen(
            [str(program), "--etc-dir", str(self.run_dir / "etc"),
             "--data-dir", str(self.run_dir / "data"), "--cache-dir",
             str(self.run_dir / "cache")],
            stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
            preexec_fn=lambda: die_with_this_program(parent))
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError("Yamcs stopped: see "
                                   f"{self.run_dir / 'yamcs.log'}")
            if self.running():
                return
            time.sleep(0.5)
        raise RuntimeError(f"Yamcs's instance {self.instance} did not start: "
                           f"see {self.run_dir / 'yamcs.log'}")

    def running(self):
        """Says whether the instance is running.

        Returns:
            Whether Yamcs answers that it is, false if it does not answer.
        """
        try:
            said = self._get(f"/instances/{self.instance}")
        except (OSError, ValueError):
            return False
        return said.get("state") == "RUNNING"

    def stop(self):
        """Stop Yamcs -- its whole process group, Java and all -- and wait
        for it to be gone."""
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
        url = (f"{self.base}/processors/{self.instance}/realtime/commands"
               f"{command}")
        request = urllib.request.Request(
            url, data=body, method="POST",
            headers={"Content-Type": "application/json"})
        try:
            urllib.request.urlopen(request, timeout=5).read()
        except urllib.error.HTTPError as why:
            raise RuntimeError(f"Yamcs refused {command}: "
                               f"{why.read().decode(errors='replace')}")

    def set_value(self, name, value):
        """Sets a local parameter, one no packet carries, to a number.

        Args:
            name:  The parameter's qualified name.
            value: The number.

        Raises:
            RuntimeError: Yamcs refuses it, or does not answer.
        """
        body = json.dumps({"type": "DOUBLE", "doubleValue": value}).encode()
        request = urllib.request.Request(
            f"{self.base}/processors/{self.instance}/realtime/parameters"
            f"{name}", data=body, method="PUT",
            headers={"Content-Type": "application/json"})
        try:
            urllib.request.urlopen(request, timeout=5).read()
        except urllib.error.HTTPError as why:
            raise RuntimeError(f"Yamcs refused {name} = {value}: "
                               f"{why.read().decode(errors='replace')}")
        except OSError as why:
            raise RuntimeError(f"Yamcs did not take {name} = {value}: {why}")

    def parameters(self, names):
        """Several parameters' latest values, each from one packet, and when
        Yamcs received them.

        Args:
            names: The parameters' qualified names.

        Returns:
            ``(values, received)`` -- each value, by name, as
            :meth:`parameter` gives it, and when Yamcs acquired them, in
            nanoseconds since the epoch -- or ``None`` before all of them
            have arrived, or while what Yamcs has of them is not from one
            packet.
        """
        said = self._latest(names)
        if said is None:
            return None
        values, times = {}, set()
        for name, (value, received) in said.items():
            values[name] = value
            times.add(received)
        if len(values) != len(names) or len(times) != 1:
            return None
        return values, times.pop()

    def latest(self, names):
        """Several parameters' latest values, each as it last arrived,
        whichever packets they came in.

        Args:
            names: The parameters' qualified names.

        Returns:
            Each one that has arrived, by name, as ``(value, received)``, as
            :meth:`parameter` gives it; an empty dictionary where Yamcs does
            not answer.
        """
        return self._latest(names) or {}

    def _latest(self, names):
        """Several parameters' latest values, as Yamcs's batch read gives
        them.

        Args:
            names: The parameters' qualified names.

        Returns:
            Each one that has arrived, by name, as ``(value, received)``; or
            ``None`` where Yamcs does not answer.
        """
        body = json.dumps({"id": [{"name": name} for name in names],
                           "fromCache": True}).encode()
        request = urllib.request.Request(
            f"{self.base}/processors/{self.instance}/realtime"
            "/parameters:batchGet",
            data=body, method="POST",
            headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=5) as reply:
                said = json.loads(reply.read())
        except (OSError, ValueError):
            return None
        found = {}
        for value in said.get("value", []):
            name = value.get("id", {}).get("name")
            if name in names and value.get("engValue"):
                found[name] = (eng_value(value["engValue"]),
                               reception_ns(value.get("acquisitionTime")))
        return found

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
            said = self._get(f"/processors/{self.instance}/realtime"
                             f"/parameters{name}")
        except (OSError, ValueError):
            return None
        value = said.get("engValue")
        if not value:
            return None
        return eng_value(value), reception_ns(said.get("acquisitionTime"))

    def units(self, name):
        """The units the mission database gives a parameter.

        Args:
            name: The parameter's qualified name.

        Returns:
            Its units, as Yamcs states them -- ``["K"]`` -- empty for a
            parameter with none, or where Yamcs does not answer.
        """
        try:
            said = self._get(f"/mdb/{self.instance}/parameters{name}")
        except (OSError, ValueError):
            return []
        return [unit.get("unit") for unit in said.get("type", {}).get(
            "unitSet", [])]

    def _get(self, path):
        """A REST API reply, read as JSON.

        Args:
            path: The path after ``/api``.

        Returns:
            The reply.

        Raises:
            OSError: Yamcs does not answer, or answers with an error.
            ValueError: The reply is not JSON.
        """
        with urllib.request.urlopen(self.base + path, timeout=5) as reply:
            return json.loads(reply.read())


def eng_value(value):
    """Reads a Yamcs value as Python has it.

    Args:
        value: The value as the REST API gives it, a dictionary of its type
               and the field that type names.

    Returns:
        Bytes for a binary value, else the field's value; ``None`` for a
        type it does not know.
    """
    kind = value.get("type", "")
    if kind == "BINARY":
        return base64.b64decode(value.get("binaryValue", ""))
    for field in ("stringValue", "uint32Value", "uint64Value", "sint32Value",
                  "sint64Value", "floatValue", "doubleValue", "booleanValue"):
        if field in value:
            said = value[field]
            # The REST API writes a 64-bit integer as text, JSON's numbers
            # being doubles.
            if field.endswith("64Value") and isinstance(said, str):
                return int(said)
            return said
    return None


def reception_ns(text):
    """Reads a Yamcs time.

    Args:
        text: The time as Yamcs writes it, ``2026-11-01T07:46:06.828Z``, or
              ``None``.

    Returns:
        Nanoseconds since the epoch; nought for none.
    """
    if not text:
        return 0
    when = datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))
    whole_seconds = int(when.replace(microsecond=0).timestamp())
    return whole_seconds * NANOS + when.microsecond * 1000


def connect(yamcs, digest, attempts=30, say=_quiet):
    """Has Yamcs send CONNECT, carrying the configuration's checksum, once a
    second until the vehicle answers that it is running it.

    Args:
        yamcs:    The :class:`Yamcs` to send through.
        digest:   The configuration's checksum, as hex.
        attempts: How many to send before giving up.
        say:      What each CONNECT sent is told to, as status.

    Returns:
        Whether the vehicle answered with the checksum.
    """
    want = bytes.fromhex(digest)
    for sequence in range(1, attempts + 1):
        yamcs.issue(CONNECT, {"Sequence": sequence, "Checksum": digest})
        say(f"CONNECT {sequence} sent")
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            running = yamcs.parameter(RUNNING)
            if running is not None and running[0] == want:
                return True
            time.sleep(0.05)
    return False


class YamcsBeacons:
    """The beacon link through Yamcs: each beacon as Yamcs received it, read
    by :class:`tcslib_moc.beacon.Beacons`.

    Yamcs holds the beacon socket, so what arrives is read back from it as
    the BeaconBody parameter, the beacon's bytes whole, and handed to a
    ``Beacons`` with no socket of its own. That one checks it as it checks
    a beacon from its socket -- its checksum against the configuration's,
    its number against the one before -- and ages it through the same
    stages, so a beacon panel says here what it says with no Yamcs between.
    A pass that finds nothing new is one :data:`BEACON_POLL` long.

    Args:
        yamcs:    The Yamcs the beacons arrive at.
        link:     The beacon link from the configuration.
        schedule: The :class:`tcslib_moc.beacon.Schedule` a beacon ages through.
        expected: The configuration's checksum, as hex; ``None`` to compare
                  nothing.
        interval: How often the vehicle beacons, in nanoseconds.
        clock:    What gives the time now, in nanoseconds since the epoch.
    """

    def __init__(self, yamcs, link, schedule, expected, interval,
                 clock=time.time_ns):
        self._yamcs = yamcs
        self._beacons = Beacons(link, schedule, expected)
        # Yamcs says nothing of where a beacon came from, and the beacon
        # link to it is its own, so each is taken as from where the
        # configuration says beacons are sent from.
        self._sender = (link.send.address, link.send.port)
        # The beacon Yamcs has already is shown at once if it is this
        # vehicle's latest, as it is when it came while the CONNECT was
        # being answered; one older than a beacon's interval is from before
        # this program, a Yamcs left running, and is not news.
        latest = yamcs.parameter(BEACON_BODY)
        stale = latest is not None and clock() - latest[1] > interval
        self._seen = latest[1] if stale else 0

    def next_pass(self):
        """Wait for Yamcs to receive a beacon, or for :data:`BEACON_POLL`.

        Returns:
            The :class:`tcslib_moc.beacon.Pass`: what arrived, if anything, and
            what to show.
        """
        deadline = time.monotonic() + BEACON_POLL
        while True:
            latest = self._yamcs.parameter(BEACON_BODY)
            if latest is not None and latest[1] > self._seen:
                body, self._seen = latest
                return self._beacons.heard(body, self._sender)
            if time.monotonic() >= deadline:
                return self._beacons.after(Arrived.NOTHING, [])
            time.sleep(0.05)


def print_beacon_pass(beacon_pass):
    """Prints what a pass of the beacon link says: its remarks and what
    arrived on stdout, its warnings on stderr.

    Args:
        beacon_pass: The :class:`tcslib_moc.beacon.Pass`.

    Returns:
        Whether a beacon arrived in it.
    """
    for remark in beacon_pass.remarks:
        print(f"beacon: {remark}", flush=True)
    for warning in beacon_pass.warnings:
        print(f"beacon: {warning}", file=sys.stderr, flush=True)
    if beacon_pass.arrived is not Arrived.BEACON:
        return False
    print(f"beacon: {beacon_pass.said}", flush=True)
    return True


def first_beacon(beacons, timeout=12):
    """Print what the beacon link says until the first beacon arrives.

    Args:
        beacons: The beacon link.
        timeout: How long to wait for it, in seconds: two beacons and more
                 at the examples' interval.

    Returns:
        Whether one arrived.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if print_beacon_pass(beacons.next_pass()):
            return True
    print("beacon: none arrived", file=sys.stderr, flush=True)
    return False
