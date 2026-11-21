"""Tests of the lander example's ground station, lander_yamcs.py.

None needs Yamcs, tcs-sim or a vehicle, or opens a window. The landscapes
made are held to the lander's physics by tcs-verify, where it is built.
"""

import math
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
        ground, pad = lander_yamcs.landscape(11, 1000, 600, 45)
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

    def test_the_sky_comes_down_and_is_quenched(self):
        """No stars at first; each star once, as its record arrives, with a
        part record left alone; a quench for each packet once the sky is
        whole; and a new generation starting the sky afresh."""
        import struct
        sent = []
        sky = lander_yamcs.Sky(sent.append)
        self.assertEqual(sky.fresh(), (False, []))

        def packet(generation, total, *numbers):
            """Packs a sky packet of star records.

            Args:
                generation: The sky's generation, in every record.
                total:      How many stars the whole sky has.
                *numbers:   Each record's star number, in order.

            Returns:
                The records, one after another: each star 1.5 pixels
                across and of brightness 255, its number metres across
                and 250.5 metres up.
            """
            return b"".join(
                struct.pack(lander_yamcs.STAR_LAYOUT, generation, total, n,
                            15, 255, 100 * n, 25050)
                for n in numbers)

        self.assertEqual(sky.take(packet(1, 3, 0, 1) + b"\x00\x01"), [])
        cleared, stars = sky.fresh()
        self.assertTrue(cleared)
        self.assertEqual(stars, [(0.0, 250.5, 1.5, 1.0),
                                 (1.0, 250.5, 1.5, 1.0)])
        # Again: nothing new, and not yet whole.
        self.assertEqual(sky.take(packet(1, 3, 0, 1)), [])
        self.assertEqual(sky.fresh(), (False, []))
        self.assertEqual(sent, [])
        self.assertEqual(sky.take(packet(1, 3, 2, 0)), [1])
        self.assertEqual(sky.fresh(), (False, [(2.0, 250.5, 1.5, 1.0)]))
        self.assertEqual(sky.take(packet(1, 3, 1)), [1])
        self.assertEqual(sent, [1, 1])
        # A new generation.
        self.assertEqual(sky.take(packet(2, 2, 1)), [])
        cleared, stars = sky.fresh()
        self.assertTrue(cleared)
        self.assertEqual(len(stars), 1)

    def test_a_star_holds_the_light_tcs_sim_gives_it(self):
        """Scaled from the scene its radius is given for; a disk where it
        is a pixel or more, and a single pixel, dimmed by the part of it
        the disk would cover, where it is less."""
        disk, r, level = lander_yamcs.star_drawing(2.0, 1.0, 1.5)
        self.assertEqual((disk, r, level), (True, 3.0, 255))
        # Half a scene pixel at 0.8 of a pixel each: a disk a little short
        # of half a pixel across, a fraction of a pixel's light.
        disk, r, level = lander_yamcs.star_drawing(0.5, 0.6, 0.8)
        self.assertFalse(disk)
        self.assertEqual(level, round(255 * 0.6 * math.pi * 0.16))

    def test_an_uplink_sends_a_number_in_its_bytes(self):
        """Two bytes, most significant first, for a sky's generation."""
        class Yamcs:
            """Keeps what it is asked to send."""

            def __init__(self):
                self.issued = []

            def issue(self, command, args):
                """Keeps it.

                Args:
                    command: The command.
                    args:    Its arguments.
                """
                self.issued.append((command, args))

        uplink = lander_yamcs.Uplink(Yamcs(), "/x/gateway_sky", "sky")
        self.assertEqual(uplink.send(258, 2),
                         {"Data": "0102", "DataBits": 16})
        self.assertEqual(uplink.send(7), {"Data": "07", "DataBits": 8})

    def test_the_landers_lights_are_on_its_cabin(self):
        """The green light in the cabin's middle, facing the viewer, and
        the white ones on its top and rear, the left, and on the front of
        the nozzle."""
        import yaml
        lander = lander_yamcs.Lander(lander_yamcs.lander_settings(
            yaml.safe_load(lander_yamcs.SHIPPED_SIM.read_text())))
        green, whites = lander_yamcs.lander_lights(lander)
        # The shipped cabin: from -10.5 to 10.5 across and 27 to 39 up;
        # its nozzle from -5.25 to 5.25 across and 8.25 to 13.5 up.
        self.assertEqual(green, (0.0, 33.0))
        self.assertEqual(whites,
                         [(0.0, 39.0), (5.25, 10.875), (-10.5, 33.0)])

    def test_each_restart_is_a_new_count(self):
        """Restart sends the gateway's command with a count that differs
        each time, from one to 255 and round again."""
        sent = []

        class Yamcs:
            """A Yamcs that keeps what it is asked to send."""

            def issue(self, command, args):
                """Keeps a command.

                Args:
                    command: Its name.
                    args:    Its arguments.
                """
                sent.append((command, args))

        restarter = lander_yamcs.Restarter(
            lander_yamcs.Uplink(Yamcs(), "/TcsYamcs/gateway", "restart"))
        said = [restarter.restart()["Data"] for _ in range(256)]
        self.assertEqual(said[:2], ["01", "02"])
        self.assertEqual(said[254:], ["ff", "01"])
        for _ in range(100):
            if len(sent) == 256:
                break
            time.sleep(0.01)
        self.assertEqual(sent[0], ("/TcsYamcs/gateway",
                                   {"Data": "01", "DataBits": 8}))

    def test_each_vehicle_takes_its_controls(self):
        """lander-a's gateway_sky passes the ground's command to
        sky_quench; lander-b's gateway to restart."""
        a, b = (lander_yamcs.Craft(name) for name in lander_yamcs.VEHICLES)
        self.assertEqual(a.commands,
                         {"sky_quench": "/TcsYamcs/gateway_sky"})
        self.assertEqual(b.commands, {"restart": "/TcsYamcs/gateway"})

    def test_the_lander_is_reckoned_on_between_readings(self):
        """Between readings the lander is carried on by its velocity and
        acceleration, no further than RECKON_FOR, and not once down."""
        readings = lander_yamcs.Readings([])
        readings.values.update({"to_x": 100.0, "to_velocity_x": 10.0,
                                "to_accel_x": 2.0})
        readings.changed["to_x"] = 50.0
        self.assertAlmostEqual(readings.estimate("x", now=50.25),
                               100.0 + 2.5 + 0.0625)
        far = readings.estimate("x", now=60.0)
        reach = lander_yamcs.RECKON_FOR
        self.assertAlmostEqual(far, 100.0 + 10.0 * reach + reach * reach)
        readings.values["crashed"] = b"1"
        self.assertEqual(readings.estimate("x", now=50.25), 100.0)

    def test_a_signal_says_yes_with_a_one(self):
        """The landed and crashed signals are the text 1 or 0."""
        readings = lander_yamcs.Readings([])
        self.assertEqual(readings.state(), "Flying")
        readings.values["crashed"] = b"1"
        self.assertEqual(readings.state(), lander_yamcs.CRASHED_SAID)
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
