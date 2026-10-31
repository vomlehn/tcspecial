"""The ground station starts the vehicle, and the vehicle goes when it does.

``sleep`` stands in for tcspecial: what is tested is the process, and a real
tcspecial would bind the example's ports, which a test run beside the example
itself must not.
"""

import subprocess
import sys
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "python"))
sys.path.insert(0, str(HERE))

import ping_gateway  # noqa: E402


def alive(pid):
    """Whether a process of this id is still there, a zombie counting as gone.

    Gone either way it can be found gone: no entry to open, or -- reaped
    between the open and the read -- an entry that answers ESRCH, which
    reading raises as ProcessLookupError.

    Args:
        pid: The process id.

    Returns:
        ``True`` while the process is there and not a zombie.
    """
    try:
        with open(f"/proc/{pid}/stat") as stat:
            return stat.read().split()[2] != "Z"
    except (FileNotFoundError, ProcessLookupError):
        return False


class VehicleTest(unittest.TestCase):
    def test_it_is_started_and_killed(self):
        vehicle = ping_gateway.Vehicle("sleep", ["1000"])
        vehicle.start()
        pid = vehicle.process.pid
        self.assertTrue(alive(pid))

        vehicle.stop()
        self.assertIsNone(vehicle.process)
        self.assertFalse(
            alive(pid), "killed, and reaped rather than left a zombie"
        )

        # Stopping what is stopped is nothing.
        vehicle.stop()

    def test_it_dies_with_the_program_that_started_it(self):
        # A program that starts a vehicle and is then killed outright, so that
        # nothing of its own runs: the kernel kills the vehicle.
        parent = subprocess.Popen(
            [
                sys.executable,
                "-c",
                "import sys, time; sys.path[:0] = sys.argv[1:3]; "
                "import ping_gateway; "
                "v = ping_gateway.Vehicle('sleep', ['1000']); v.start(); "
                "print(v.process.pid, flush=True); time.sleep(1000)",
                str(HERE),
                str(HERE.parents[1] / "python"),
            ],
            stdout=subprocess.PIPE,
            text=True,
        )
        pid = int(parent.stdout.readline())
        self.assertTrue(alive(pid))

        parent.kill()
        parent.wait()
        deadline = time.monotonic() + 5
        while alive(pid) and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertFalse(alive(pid), "the vehicle outlived its ground station")

    def test_a_vehicle_nobody_built_is_said_so(self):
        original = ping_gateway.find_built
        ping_gateway.find_built = lambda program: None
        try:
            with self.assertRaises(ValueError) as said:
                ping_gateway.Vehicle.for_configuration("ping.xml")
        finally:
            ping_gateway.find_built = original
        self.assertIn("cargo build --bin tcspecial", str(said.exception))
        self.assertIn("--no-vehicle", str(said.exception))



class ConnectTest(unittest.TestCase):
    """The ground station sends its own CONNECT, which this test answers as
    the vehicle would, and goes on sending it until it is matched."""

    def test_connect_goes_until_the_vehicle_answers_with_ours(self):
        import socket
        import types

        from tcslibgs import fixed_packet
        from tcslibgs.configuration import CommandLink, HostPort

        vehicle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        vehicle.bind(("127.0.0.1", 0))
        vehicle.settimeout(10)
        station = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        station.bind(("127.0.0.1", 0))
        station_port = station.getsockname()[1]
        station.close()
        digest = "00112233445566778899aabbccddeeff"
        setup = types.SimpleNamespace(
            command=CommandLink(
                send=HostPort("127.0.0.1", station_port),
                recv=HostPort("127.0.0.1", vehicle.getsockname()[1]),
            ),
            digest=digest,
        )
        matched = ping_gateway.connecting(setup)
        self.assertIsNotNone(matched)
        try:
            # The first unanswered, the second answered with ours.
            vehicle.recvfrom(256)
            datagram, sender = vehicle.recvfrom(256)
            command = fixed_packet.FixedCommandKind.decode(datagram)
            self.assertEqual(bytes(command.checksum).hex(), digest)
            vehicle.sendto(
                fixed_packet.ConnectAnswer(
                    sequence=command.sequence, status=0, timestamp=0,
                    running=bytes.fromhex(digest), major=0, minor=1,
                    patch=0, newest_taken=command.sequence,
                ).to_bytes(),
                sender,
            )
            self.assertTrue(matched.wait(10), "matched")
        finally:
            vehicle.close()

    def test_no_command_link_sends_nothing(self):
        import types

        setup = types.SimpleNamespace(command=None, digest="00" * 16)
        self.assertIsNone(ping_gateway.connecting(setup))


class VerboseTest(unittest.TestCase):
    def test_status_is_said_only_when_asked_for(self):
        import contextlib
        import io

        for flag in ["-v", "--verbose"]:
            self.assertTrue(ping_gateway.arguments([flag]).verbose, flag)
        self.assertFalse(ping_gateway.arguments([]).verbose)

        was = ping_gateway.VERBOSE
        try:
            for verbose, expected in [(False, ""), (True, "listening\n")]:
                ping_gateway.VERBOSE = verbose
                said = io.StringIO()
                with contextlib.redirect_stderr(said):
                    ping_gateway.status("listening")
                self.assertEqual(said.getvalue(), expected)
        finally:
            ping_gateway.VERBOSE = was


class SpellingTest(unittest.TestCase):
    def test_the_spelling_is_xml_unless_yaml_is_asked_for_and_never_both(self):
        import contextlib
        import io

        stem = str(ping_gateway.STEM)
        self.assertEqual(
            ping_gateway.arguments([]).configuration, stem + ".xml"
        )
        self.assertEqual(
            ping_gateway.arguments(["--xml"]).configuration, stem + ".xml"
        )
        self.assertEqual(
            ping_gateway.arguments(["--yaml"]).configuration, stem + ".yaml"
        )
        self.assertEqual(
            ping_gateway.arguments(["a.yaml"]).configuration, "a.yaml"
        )
        for refused in (
            ["--yaml", "--xml"],
            ["--yaml", "a.xml"],
            ["--xml", "a.yaml"],
        ):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(
                SystemExit, msg=refused
            ):
                ping_gateway.arguments(refused)

if __name__ == "__main__":
    unittest.main()
