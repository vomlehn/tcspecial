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

import ping_moc  # noqa: E402


def alive(pid):
    """Whether a process of this id is still there, a zombie counting as gone."""
    try:
        with open(f"/proc/{pid}/stat") as stat:
            return stat.read().split()[2] != "Z"
    except FileNotFoundError:
        return False


class VehicleTest(unittest.TestCase):
    def test_it_is_started_and_killed(self):
        vehicle = ping_moc.Vehicle("sleep", ["1000"])
        vehicle.start()
        pid = vehicle.process.pid
        self.assertTrue(alive(pid))

        vehicle.stop()
        self.assertIsNone(vehicle.process)
        self.assertFalse(alive(pid), "killed, and reaped rather than left a zombie")

        # Stopping what is stopped is nothing.
        vehicle.stop()

    def test_it_dies_with_the_program_that_started_it(self):
        # A program that starts a vehicle and is then killed outright, so that
        # nothing of its own runs: the kernel kills the vehicle.
        parent = subprocess.Popen(
            [
                sys.executable,
                "-c",
                "import sys, time; sys.path[:0] = sys.argv[1:3]; import ping_moc; "
                "v = ping_moc.Vehicle('sleep', ['1000']); v.start(); "
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
        original = ping_moc.find_built
        ping_moc.find_built = lambda program: None
        try:
            with self.assertRaises(ValueError) as said:
                ping_moc.Vehicle.for_configuration("ping.xml")
        finally:
            ping_moc.find_built = original
        self.assertIn("cargo build --bin tcspecial", str(said.exception))
        self.assertIn("--no-vehicle", str(said.exception))


if __name__ == "__main__":
    unittest.main()
