"""A ground station starts a program, and the program goes when it does.

``sleep`` takes the place of tcspecial: what is tested is the process, and a
real tcspecial would bind an example's ports, which a test run beside the
example itself must not.
"""

import subprocess
import sys
import time
import unittest
from pathlib import Path

PYTHON = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PYTHON))

from tcsmoc import processes  # noqa: E402


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
        vehicle = processes.Vehicle("sleep", ["1000"])
        vehicle.start()
        pid = vehicle.process.pid
        self.assertTrue(alive(pid))
        self.assertFalse(vehicle.stopped())

        vehicle.stop()
        self.assertIsNone(vehicle.process)
        self.assertFalse(alive(pid),
                         "killed, and reaped rather than left a zombie")

        # Stopping what is stopped is nothing.
        vehicle.stop()

    def test_it_dies_with_the_program_that_started_it(self):
        # A program that starts a vehicle and is then killed outright, so
        # that nothing of its own runs: the kernel kills the vehicle.
        parent = subprocess.Popen(
            [sys.executable, "-c",
             "import sys, time; sys.path[:0] = sys.argv[1:2]; "
             "from tcsmoc import processes; "
             "v = processes.Vehicle('sleep', ['1000']); v.start(); "
             "print(v.process.pid, flush=True); time.sleep(1000)",
             str(PYTHON)],
            stdout=subprocess.PIPE, text=True)
        pid = int(parent.stdout.readline())
        self.assertTrue(alive(pid))

        parent.kill()
        parent.wait()
        deadline = time.monotonic() + 5
        while alive(pid) and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertFalse(alive(pid), "the vehicle outlived its ground station")

    def test_a_vehicle_nobody_built_is_said_so(self):
        original = processes.find_built
        processes.find_built = lambda program: None
        try:
            with self.assertRaises(ValueError) as said:
                processes.Vehicle.for_configuration("ping.xtcs")
        finally:
            processes.find_built = original
        self.assertIn("cargo build --bin tcspecial", str(said.exception))
        self.assertIn("--no-vehicle", str(said.exception))

    def test_it_is_told_its_mode_and_to_be_quiet(self):
        vehicle = processes.Vehicle.for_configuration("a.xtcs", "tcspecial",
                                                      simulate="all")
        self.assertEqual(vehicle.args,
                         ["--quiet", "--simulate=all", "a.xtcs"])
        loud = processes.Vehicle.for_configuration("a.xtcs", "tcspecial",
                                                   verbose=True)
        self.assertEqual(loud.args, ["a.xtcs"])


class ChildTest(unittest.TestCase):
    def test_one_that_stops_by_itself_is_seen_to_have(self):
        child = processes.Child("true", [])
        child.start()
        child.process.wait()
        self.assertTrue(child.stopped())
        child.stop()
        self.assertFalse(child.stopped(), "nothing started is not stopped")


if __name__ == "__main__":
    unittest.main()
