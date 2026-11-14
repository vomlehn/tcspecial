"""The ground station's own CONNECT, its ``-v`` and the spelling it reads.

That it starts the vehicle, and the vehicle goes when it does, is
``tcsmoc.processes``'s, and tested in ``python/tests/test_processes.py``.
"""

import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import ping_gateway  # noqa: E402


class ConnectTest(unittest.TestCase):
    """The ground station sends its own CONNECT, which this test answers as
    the vehicle would, and goes on sending it until it is matched."""

    def test_connect_goes_until_the_vehicle_answers_with_ours(self):
        import socket
        import types

        from tcsprotocol import fixed_packet
        from tcsprotocol.configuration import CommandLink, HostPort

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
            ping_gateway.arguments([]).configuration, stem + ".xtcs"
        )
        self.assertEqual(
            ping_gateway.arguments(["--xml"]).configuration, stem + ".xtcs"
        )
        self.assertEqual(
            ping_gateway.arguments(["--yaml"]).configuration, stem + ".ytcs"
        )
        self.assertEqual(
            ping_gateway.arguments(["a.ytcs"]).configuration, "a.ytcs"
        )
        for refused in (
            ["--yaml", "--xml"],
            ["--yaml", "a.xtcs"],
            ["--xml", "a.ytcs"],
        ):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(
                SystemExit, msg=refused
            ):
                ping_gateway.arguments(refused)

if __name__ == "__main__":
    unittest.main()
