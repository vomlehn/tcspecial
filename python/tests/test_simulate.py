"""The simulation mode read from a simulator file, and the ground's own
simulation allowed only where the run simulates."""

import tempfile
import unittest
from pathlib import Path

from tcsmoc.simulation import allow
from tcsprotocol.simulate import mode_of, run_for


class TestSimulate(unittest.TestCase):
    """tcsprotocol.simulate and tcsmoc.simulation."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.path = Path(self.dir.name)

    def write(self, name, text):
        """Writes a file in the test's directory.

        Args:
            name: Its name.
            text: What it holds.

        Returns:
            Its path.
        """
        path = self.path / name
        path.write_text(text)
        return path

    def test_the_file_states_the_mode(self):
        """Each mode is read; none stated, or blank, is none; a word that is
        no mode is refused naming the three."""
        for mode in ("none", "some", "all"):
            sim = self.write(f"{mode}-sim.yaml",
                             f"tcspecial:\n  simulate: {mode}\n")
            self.assertEqual(mode_of(sim), mode)
        self.assertEqual(mode_of(self.write("a-sim.yaml", "description: x\n")),
                         "none")
        self.assertEqual(
            mode_of(self.write("b-sim.yaml", "tcspecial:\n  simulate:\n")),
            "none")
        with self.assertRaisesRegex(ValueError, "none, some, all"):
            mode_of(self.write("c-sim.yaml", "tcspecial:\n  simulate: most\n"))
        with self.assertRaises(ValueError):
            mode_of(self.path / "missing-sim.yaml")

    def test_a_run_finds_the_file_named_or_beside(self):
        """No file is flight; the one beside the configuration is read, and
        the one named in the environment before it."""
        config = self.path / "x.ytcs"
        self.assertEqual(run_for(config, {}), ("none", None))
        beside = self.write("x-sim.yaml", "tcspecial:\n  simulate: some\n")
        self.assertEqual(run_for(config, {}), ("some", str(beside)))
        named = self.write("y-sim.yaml", "tcspecial:\n  simulate: all\n")
        self.assertEqual(run_for(config, {"PAYLOAD_SIM_YAML": str(named)}),
                         ("all", str(named)))

    def test_the_grounds_simulation_is_refused_in_flight(self):
        """Allowed in a simulated run; refused in the flight setting, naming
        the feature and the file."""
        allow("--delay=1.3", "all", "x-sim.yaml")
        allow("--delay=1.3", "some", None)
        with self.assertRaisesRegex(ValueError,
                                    "x-sim.yaml states simulate: none"):
            allow("--delay=1.3", "none", "x-sim.yaml")
        with self.assertRaisesRegex(ValueError, "no simulator file"):
            allow("--delay=1.3", "none", None)


if __name__ == "__main__":
    unittest.main()
