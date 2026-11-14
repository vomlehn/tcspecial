"""A ground station that reaches tcspecial through Yamcs, with no Yamcs: what
it writes for Yamcs, how it reads Yamcs's REST API, the beacon through a
Yamcs played here, and that Yamcs dies with it.

``tcsmoc.yamcs_ground``, which ping-yamcs and thermostat-yamcs share; their
own tests are beside them.
"""

import os
import re
import signal
import socket
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
from pathlib import Path

PYTHON = Path(__file__).resolve().parents[1]
ROOT = PYTHON.parent
sys.path.insert(0, str(PYTHON))

from tcsmoc import yamcs_ground  # noqa: E402
from tcsmoc.beacon import Arrived, Schedule  # noqa: E402
from tcsprotocol import configuration  # noqa: E402
from tcsprotocol.beacon import Beacon  # noqa: E402


class WriteYamcsTest(unittest.TestCase):
    """The Yamcs configuration written from the templates, for each example
    that runs Yamcs."""

    def setUp(self):
        self.home = Path(tempfile.mkdtemp())
        (self.home / "etc").mkdir()
        (self.home / "etc" / "processor.yaml.sample").write_text(
            'args:\n  stream: "tc_realtime"\n')
        self.run = Path(tempfile.mkdtemp()) / "run"

    def test_every_template_name_is_filled_in(self):
        for example, instance in (("ping-yamcs", "ping"),
                                  ("thermostat-yamcs", "thermostat")):
            config = configuration.load(
                ROOT / "examples" / example / f"{example}.xtcs")
            yamcs_ground.write_yamcs(config, self.run, 1, self.home,
                                     instance, "a_test.py")
            for written in ("etc/yamcs.yaml", f"etc/yamcs.{instance}.yaml",
                            "tcs-ccsds.xtce", "etc/processor.yaml"):
                text = (self.run / written).read_text()
                self.assertIsNone(re.search(r"\$\w", text), written)
            self.assertIn(f"  - {instance}\n",
                          (self.run / "etc" / "yamcs.yaml").read_text())

    def test_a_sample_that_is_not_5_14s_is_refused(self):
        (self.home / "etc" / "processor.yaml.sample").write_text("args: {}\n")
        config = configuration.load(
            ROOT / "examples" / "ping-yamcs" / "ping-yamcs.xtcs")
        with self.assertRaisesRegex(OSError, "Yamcs 5.14"):
            yamcs_ground.write_yamcs(config, self.run, 1, self.home, "ping",
                                     "a_test.py")

    def test_where_yamcs_is_must_be_said(self):
        with self.assertRaisesRegex(ValueError, "bin/yamcsd"):
            yamcs_ground.yamcs_home(None)
        with self.assertRaisesRegex(ValueError, "bin/yamcsd"):
            yamcs_ground.yamcs_home(str(self.home))
        (self.home / "bin").mkdir()
        (self.home / "bin" / "yamcsd").touch()
        self.assertEqual(yamcs_ground.yamcs_home(str(self.home)), self.home)


class ConnectYamcs:
    """A Yamcs whose vehicle answers the second CONNECT with ours."""

    def __init__(self, digest):
        self.digest = digest
        self.issued = []

    def issue(self, command, args):
        """Takes a command."""
        self.issued.append((command, args))

    def parameter(self, name):
        """What the vehicle says it runs, once it has had two CONNECTs."""
        if name == yamcs_ground.RUNNING and len(self.issued) >= 2:
            return bytes.fromhex(self.digest), 1
        return None


class ConnectTest(unittest.TestCase):
    def test_connect_goes_until_the_vehicle_answers_with_ours(self):
        yamcs = ConnectYamcs(DIGEST)
        said = []
        self.assertTrue(yamcs_ground.connect(yamcs, DIGEST, say=said.append))
        self.assertEqual([args["Sequence"] for _, args in yamcs.issued],
                         [1, 2])
        self.assertEqual(yamcs.issued[0],
                         (yamcs_ground.CONNECT,
                          {"Sequence": 1, "Checksum": DIGEST}))
        self.assertEqual(said, ["CONNECT 1 sent", "CONNECT 2 sent"])

    def test_a_vehicle_running_another_is_not_connected(self):
        yamcs = ConnectYamcs("ff" * 16)
        self.assertFalse(yamcs_ground.connect(yamcs, DIGEST, attempts=2))


class YamcsValueTest(unittest.TestCase):
    """Reading what Yamcs's REST API says."""

    def test_reception_times_are_read_to_the_millisecond(self):
        self.assertEqual(yamcs_ground.reception_ns("1970-01-01T00:00:01.250Z"),
                         1_250_000_000)
        self.assertEqual(yamcs_ground.reception_ns(None), 0)

    def test_values_are_read_by_their_type(self):
        self.assertEqual(
            yamcs_ground.eng_value({"type": "BINARY", "binaryValue": "AAE="}),
            b"\x00\x01")
        self.assertEqual(yamcs_ground.eng_value({"type": "STRING",
                                                 "stringValue": "x"}), "x")
        self.assertEqual(yamcs_ground.eng_value({"type": "UINT32",
                                                 "uint32Value": 4}), 4)
        # A value in its unit, and a decision, as the typed parameters of
        # thermostat-yamcs arrive.
        self.assertEqual(yamcs_ground.eng_value({"type": "DOUBLE",
                                                 "doubleValue": 293.15}),
                         293.15)
        self.assertIs(yamcs_ground.eng_value({"type": "BOOLEAN",
                                              "booleanValue": True}), True)
        self.assertEqual(yamcs_ground.eng_value({"type": "SINT64",
                                                 "sint64Value": "-3"}), -3)
        self.assertIsNone(yamcs_ground.eng_value({"type": "AGGREGATE"}))


#: The examples' beacon interval, five seconds, in nanoseconds.
INTERVAL = 5_000_000_000

#: A checksum, as the configuration's would be.
DIGEST = "00112233445566778899aabbccddeeff"


def beacon(sequence, digest=DIGEST):
    """A beacon's bytes, as tcspecial sends them and Yamcs holds them.

    Args:
        sequence: Its number.
        digest:   The checksum of the configuration it announces, as hex.

    Returns:
        The bytes.
    """
    return Beacon(sequence=sequence, version=(0, 1, 0),
                  digest=bytes.fromhex(digest)).to_bytes()


class BeaconYamcs:
    """A Yamcs that holds one beacon, received when the test says."""

    def __init__(self):
        self.body = None

    def parameter(self, name):
        """The beacon and when it arrived, or ``None``."""
        return self.body if name == yamcs_ground.BEACON_BODY else None


class YamcsBeaconsTest(unittest.TestCase):
    """The beacon link through Yamcs, read as a beacon from a socket is
    read."""

    def setUp(self):
        self.link = configuration.BeaconLink(
            send=configuration.HostPort("127.0.0.1", 16550),
            recv=configuration.HostPort("127.0.0.1", 16650))
        self.yamcs = BeaconYamcs()
        self.now = 10**18

    def beacons(self):
        """The beacon link through this test's Yamcs, at this test's time.

        Returns:
            The :class:`tcsmoc.yamcs_ground.YamcsBeacons`.
        """
        return yamcs_ground.YamcsBeacons(self.yamcs, self.link,
                                       Schedule.for_interval(INTERVAL),
                                       DIGEST, INTERVAL,
                                       clock=lambda: self.now)

    def test_each_new_beacon_is_heard_and_checked(self):
        beacons = self.beacons()
        self.assertIsNone(beacons.next_pass().at)
        self.yamcs.body = (beacon(1), self.now)
        first = beacons.next_pass()
        self.assertIs(first.arrived, Arrived.BEACON)
        self.assertIn("tcspecial v0.1.0", first.said)
        # The same one again is not another beacon.
        self.assertIs(beacons.next_pass().arrived, Arrived.NOTHING)
        # A skipped number is remarked on, as from the beacon socket.
        self.yamcs.body = (beacon(3), self.now + 1)
        third = beacons.next_pass()
        self.assertIs(third.arrived, Arrived.BEACON)
        self.assertTrue(any("did not arrive" in remark
                            for remark in third.remarks), third.remarks)

    def test_a_beacon_for_another_configuration_is_said_to_differ(self):
        beacons = self.beacons()
        self.yamcs.body = (beacon(1, "ff" * 16), self.now)
        heard = beacons.next_pass()
        self.assertTrue(heard.remarks, "a differing checksum is remarked on")

    def test_the_latest_beacon_is_shown_at_once_and_an_old_one_is_not(self):
        self.yamcs.body = (beacon(7), self.now - INTERVAL // 2)
        self.assertIsNotNone(self.beacons().next_pass().at)
        self.yamcs.body = (beacon(7), self.now - 2 * INTERVAL)
        self.assertIsNone(self.beacons().next_pass().at)


class YamcsDiesWithThisProgramTest(unittest.TestCase):
    """A ground station that is killed, and runs no ``finally``, takes its
    Yamcs with it."""

    def test_yamcs_is_killed_when_this_program_is(self):
        with tempfile.TemporaryDirectory() as home:
            home = Path(home)
            (home / "bin").mkdir()
            pid_file = home / "pid"
            # A yamcsd that ends by becoming something else, as the real
            # one ends by becoming Java.
            yamcsd = home / "bin" / "yamcsd"
            yamcsd.write_text(textwrap.dedent(f"""\
                #!/bin/sh
                echo $$ > {pid_file}
                exec sleep 60
                """))
            yamcsd.chmod(0o755)
            with socket.socket() as unused:
                unused.bind(("127.0.0.1", 0))
                port = unused.getsockname()[1]
            program = subprocess.Popen([
                sys.executable, "-c",
                "import sys; from pathlib import Path; "
                f"sys.path.insert(0, {str(PYTHON)!r}); "
                "from tcsmoc.yamcs_ground import Yamcs; "
                f"Yamcs({str(home)!r}, Path({str(home)!r}), {port}, "
                "'ping').start()"])
            try:
                deadline = time.monotonic() + 10
                while not pid_file.exists() or not pid_file.read_text():
                    self.assertLess(time.monotonic(), deadline,
                                    "the fake yamcsd never started")
                    time.sleep(0.05)
                yamcs = int(pid_file.read_text())
                program.send_signal(signal.SIGKILL)
                program.wait()
                deadline = time.monotonic() + 5
                while alive(yamcs):
                    if time.monotonic() > deadline:
                        os.kill(yamcs, signal.SIGKILL)
                        self.fail("Yamcs outlived the program that ran it")
                    time.sleep(0.05)
            finally:
                if program.poll() is None:
                    program.kill()
                    program.wait()


def alive(pid):
    """Whether a process is running: not gone, and not a zombie waiting to
    be reaped, which has stopped all the same.

    Args:
        pid: The process's id.

    Returns:
        Whether it runs.
    """
    try:
        state = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1]
    except OSError:
        return False
    return state.split()[0] != "Z"


if __name__ == "__main__":
    unittest.main()
