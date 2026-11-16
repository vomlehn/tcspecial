"""Tests of the lander example's ground station, lander_yamcs.py.

None needs Yamcs, tcs-sim or a vehicle, or opens a window. The landscapes
made are held to the lander's physics by tcs-verify, where it is built.
"""

import socket
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import lander_yamcs  # noqa: E402
from tcsmoc.processes import find_built  # noqa: E402

#: How many seeds' landscapes are made and checked.
SEEDS = 40


class Landscape(unittest.TestCase):
    """The landscapes it makes."""

    def test_one_seed_makes_one_landscape(self):
        """The same seed twice makes the same landscape, and another seed
        another."""
        one = lander_yamcs.landscape(7, 1000, 600, 30)
        self.assertEqual(one, lander_yamcs.landscape(7, 1000, 600, 30))
        self.assertNotEqual(one, lander_yamcs.landscape(8, 1000, 600, 30))

    def test_the_ground_keeps_the_rules(self):
        """Every landscape runs edge to edge, no steeper than the steepest,
        between nought and the highest, its pad level, twice the lander's
        width, in the middle half and the lowest fifth."""
        steep = lander_yamcs.math.tan(lander_yamcs.math.radians(
            lander_yamcs.STEEPEST))
        for seed in range(SEEDS):
            ground, pad = lander_yamcs.landscape(seed, 1000, 600, 30)
            with self.subTest(seed=seed):
                self.assertEqual(ground[0][0], 0.0)
                self.assertEqual(ground[-1][0], 1000.0)
                for (x0, y0), (x1, y1) in zip(ground, ground[1:]):
                    self.assertGreater(x1, x0)
                    self.assertLessEqual(abs(y1 - y0), steep * (x1 - x0))
                for _, y in ground:
                    self.assertTrue(0 <= y <= 360, y)
                (px0, py0), (px1, py1) = pad
                self.assertEqual(py0, py1)
                self.assertAlmostEqual(px1 - px0, 60)
                self.assertTrue(250 <= (px0 + px1) / 2 <= 750)
                self.assertLessEqual(py0, 120)
                self.assertIn(pad[0], ground)
                self.assertEqual(ground.index(pad[1]),
                                 ground.index(pad[0]) + 1)

    def test_the_run_file_is_one_tcs_verify_takes(self):
        """The run's simulator file, with each seed's landscape, is one the
        lander's physics takes, with either configuration."""
        verify = find_built("tcs-verify")
        if verify is None:
            self.skipTest("tcs-verify is not built")
        with tempfile.TemporaryDirectory() as run:
            for seed in range(0, SEEDS, 4):
                path, _ = lander_yamcs.write_run(Path(run) / str(seed), seed)
                for instance in lander_yamcs.VEHICLES:
                    config = HERE.parent / instance / f"{instance}.ytcs"
                    ran = subprocess.run([str(verify), str(config),
                                          str(path)],
                                         capture_output=True, text=True)
                    with self.subTest(seed=seed, instance=instance):
                        self.assertEqual(ran.returncode, 0,
                                         ran.stdout + ran.stderr)

    def test_the_run_file_keeps_the_rest(self):
        """Only the ground and the pad are replaced: the comments, the
        lander and the payloads are as shipped, and the file says its
        seed."""
        with tempfile.TemporaryDirectory() as run:
            path, lander = lander_yamcs.write_run(Path(run), 11)
            text = path.read_text()
        self.assertIn("seed 11", text.splitlines()[1])
        shipped = lander_yamcs.SHIPPED_SIM.read_text()
        kept = [line for line in shipped.splitlines()
                if line.lstrip().startswith(("#", "- payload:",
                                             "configuration:", "outline"))]
        for line in kept:
            self.assertIn(line, text)
        ground, pad = lander_yamcs.landscape(11, 1000, 600, 30)
        self.assertEqual(lander.ground, ground)
        self.assertEqual(lander.pad, pad)


class Vehicles(unittest.TestCase):
    """The two vehicles the ground station runs."""

    def test_the_two_can_run_at_once(self):
        """tcs-conflict finds nothing the two would both take: no port, state
        file or log directory."""
        conflict = find_built("tcs-conflict")
        if conflict is None:
            self.skipTest("tcs-conflict is not built")
        paths = [str(HERE.parent / name / f"{name}.ytcs")
                 for name in lander_yamcs.VEHICLES]
        ran = subprocess.run([str(conflict), *paths], capture_output=True,
                             text=True)
        self.assertEqual(ran.returncode, 0, ran.stdout + ran.stderr)


class Relays(unittest.TestCase):
    """The relays that stand between Yamcs and each vehicle."""

    def test_each_link_is_moved_and_relayed(self):
        """A link Yamcs listens on is moved up, its relay on the vehicle's
        port; one Yamcs sends on is pointed at its relay, which passes both
        ways."""
        links = [{"name": "commands", "host": "127.0.0.1", "port": 19200},
                 {"name": "beacon", "port": 19510}]
        moved, relays = lander_yamcs.relayed(links, 10000)
        self.assertEqual(moved[0]["port"], 29200)
        self.assertEqual(moved[1]["port"], 29510)
        self.assertEqual(links[0]["port"], 19200, "left as they were")
        self.assertEqual(relays, [
            (("127.0.0.1", 29200), ("127.0.0.1", 19200), True),
            (("127.0.0.1", 19510), ("127.0.0.1", 29510), False)])

    def test_every_vehicle_link_is_relayed(self):
        """Every link of both vehicles' instances has a relay, and no two
        relays listen on one port."""
        listening = []
        for instance in lander_yamcs.VEHICLES:
            craft = lander_yamcs.Craft(instance)
            links = lander_yamcs.generated.data_links(craft.config)
            _, relays = lander_yamcs.relayed(links)
            self.assertEqual(len(relays), len(links))
            listening += [listen for listen, _, _ in relays]
        self.assertEqual(len(listening), len(set(listening)))

    def _free(self):
        """A UDP socket on a port nobody has, closed after the test.

        Returns:
            The socket, bound on the loopback.
        """
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.addCleanup(sock.close)
        sock.bind(("127.0.0.1", 0))
        sock.settimeout(3)
        return sock

    @staticmethod
    def _port():
        """A port nobody has, for a relay to listen on.

        Returns:
            The port.
        """
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
        sock.close()
        return port

    def test_one_way_holds_each_datagram(self):
        """A datagram is passed on, no sooner than the delay."""
        yamcs = self._free()
        listen = ("127.0.0.1", self._port())
        line = lander_yamcs.DelayLine(0.3)
        line.one_way(listen, yamcs.getsockname())
        line.start()
        try:
            vehicle = self._free()
            sent = time.monotonic()
            vehicle.sendto(b"down", listen)
            data, _ = yamcs.recvfrom(100)
            self.assertEqual(data, b"down")
            self.assertGreaterEqual(time.monotonic() - sent, 0.3)
        finally:
            line.stop()

    def test_both_ways_answers_whoever_asked(self):
        """A command is passed on, and the answer comes back to the socket
        that sent it, each held for the delay."""
        vehicle = self._free()
        listen = ("127.0.0.1", self._port())
        line = lander_yamcs.DelayLine(0.2)
        line.both_ways(listen, vehicle.getsockname())
        line.start()
        try:
            yamcs = self._free()
            sent = time.monotonic()
            yamcs.sendto(b"CONNECT", listen)
            data, relay = vehicle.recvfrom(100)
            self.assertEqual(data, b"CONNECT")
            vehicle.sendto(b"answer", relay)
            data, sender = yamcs.recvfrom(100)
            self.assertEqual(data, b"answer")
            self.assertEqual(sender, listen)
            self.assertGreaterEqual(time.monotonic() - sent, 0.4)
        finally:
            line.stop()


class Shown(unittest.TestCase):
    """What the window shows."""

    def test_the_joystick_dot_is_where_tcs_sim_had_it(self):
        """Nought is the middle; a full push is the arm's end."""
        self.assertEqual(lander_yamcs.stick_offset(0), 0)
        reach = lander_yamcs.SQUARE / 2 + lander_yamcs.ARM
        self.assertEqual(lander_yamcs.stick_offset(1), reach)
        self.assertEqual(lander_yamcs.stick_offset(-1), -reach)
        self.assertEqual(lander_yamcs.stick_offset(5), reach)

    def test_the_scale_is_what_x11_is_told(self):
        """Xft.dpi over 96, and one where xrdb says none or is missing."""
        from unittest import mock
        ran = mock.Mock(stdout="Xft.antialias:\t1\nXft.dpi:\t216\n")
        with mock.patch.object(lander_yamcs.subprocess, "run",
                               return_value=ran):
            self.assertEqual(lander_yamcs.display_scale(), 2.25)
        ran = mock.Mock(stdout="Xft.antialias:\t1\n")
        with mock.patch.object(lander_yamcs.subprocess, "run",
                               return_value=ran):
            self.assertEqual(lander_yamcs.display_scale(), 1.0)
        with mock.patch.object(lander_yamcs.subprocess, "run",
                               side_effect=OSError("no xrdb")):
            self.assertEqual(lander_yamcs.display_scale(), 1.0)

    def test_the_stars_are_as_bright_as_the_sky_has_them(self):
        """About three times as many at each magnitude as at the one
        brighter; disks shrinking to a pixel, then pixels dimming."""
        counts = [0] * 6
        for n in range(1000):
            m = lander_yamcs.magnitude((n + 0.5) / 1000)
            counts[min(int(m), 5)] += 1
        for fewer, more in zip(counts[2:], counts[3:]):
            self.assertTrue(2.5 < more / fewer < 3.5, counts)
        drawn = [lander_yamcs.star_class((c + 0.5) * 0.75)
                 for c in range(8)]
        self.assertEqual(drawn[0], (lander_yamcs.BRIGHTEST_RADIUS, 1.0))
        for (r0, b0), (r1, b1) in zip(drawn, drawn[1:]):
            self.assertTrue(r1 <= r0 and b1 <= b0, drawn)
            self.assertTrue(r1 < r0 or b1 < b0, drawn)
        self.assertEqual(drawn[-1][0], lander_yamcs.PIXEL_RADIUS)
        self.assertLess(drawn[-1][1], 0.5)

    def test_a_signal_says_yes_with_a_one(self):
        """The landed and crashed signals are the text 1 or 0."""
        readings = lander_yamcs.Readings([])
        self.assertEqual(readings.state(), "Flying")
        readings.values["crashed"] = b"1"
        self.assertEqual(readings.state(), "Crashed")
        readings.values["crashed"] = b"0"
        readings.values["landed"] = "1"
        self.assertEqual(readings.state(), "Landed")

    def test_points_read_back(self):
        """Points written as the simulator file states them read back."""
        points = [(0.0, 1.5), (10.0, 0.0)]
        said = lander_yamcs.said_of(points)
        self.assertEqual(said, "0,1.5 10,0 m")
        self.assertEqual(lander_yamcs.points_of(said), [points])
        with self.assertRaises(ValueError):
            lander_yamcs.points_of("0,1 2,3")


if __name__ == "__main__":
    unittest.main()
