"""memusage.py's tests: what it reads, the Yamcs server it writes for one
tcspecial and for two, and what it prints of an answer. None needs Yamcs."""

import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import memusage  # noqa: E402

PING_YAMCS = HERE.parent / "ping-yamcs" / "ping-yamcs.xtcs"


def fake_yamcs_home(directory):
    """A directory that passes for an unpacked Yamcs: its processor sample,
    which the run's processor configuration is made from.

    Args:
        directory: Where to make it.

    Returns:
        Its path.
    """
    home = Path(directory) / "yamcs"
    (home / "etc").mkdir(parents=True)
    (home / "bin").mkdir()
    (home / "bin" / "yamcsd").write_text("")
    (home / "etc" / "processor.yaml.sample").write_text(
        'realtime:\n  services:\n    - class: X\n      args:\n'
        '        stream: "tc_realtime"\n')
    return home


class WatchedTest(unittest.TestCase):
    """What is read of each configuration."""

    def test_the_example_is_read(self):
        one = memusage.Watched(HERE / "memusage.xtcs", "memusage", None)
        self.assertEqual(one.nodes.port, 18300)
        self.assertIn("instance memusage", one.describe())

    def test_a_node_link_that_refuses_yamcs_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixed.ytcs"
            path.write_text((HERE / "memusage.ytcs").read_text().replace(
                'source_base: "127.0.0.1:0"\n  # A payload',
                'source_base: "127.0.0.1:18310"\n  # A payload'))
            with self.assertRaisesRegex(ValueError, "127.0.0.1:0"):
                memusage.Watched(path, "fixed", None)

    def test_instances_are_named_apart(self):
        self.assertEqual(memusage.instance_name(Path("a/ping-yamcs.xtcs"),
                                                []), "ping_yamcs")
        self.assertEqual(memusage.instance_name(Path("b/memusage.ytcs"),
                                                ["memusage"]), "memusage_2")


class WriteTest(unittest.TestCase):
    """The Yamcs server written for one tcspecial and for two."""

    def written(self, paths):
        """Writes the server for these configurations.

        Args:
            paths: The configurations.

        Returns:
            The run directory's ``etc``, and the instances' names.
        """
        import tcsmoc.yamcs_ground as ground
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        home = fake_yamcs_home(self.directory.name)
        run = Path(self.directory.name) / "run"
        names = []
        watched = []
        for path in paths:
            name = memusage.instance_name(Path(path), names)
            names.append(name)
            watched.append(memusage.Watched(path, name, None))
        ground.write_yamcs_instances(
            [(one.config, one.instance) for one in watched], run, 18090,
            home, "test")
        return run, names

    def test_one_tcspecial_is_one_instance(self):
        run, names = self.written([HERE / "memusage.xtcs"])
        self.assertIn("  - memusage\n",
                      (run / "etc" / "yamcs.yaml").read_text())
        self.assertTrue((run / "tcs-ccsds.xtce").is_file())

    def test_two_tcspecials_are_two_instances_of_one_server(self):
        import yaml
        run, names = self.written([HERE / "memusage.xtcs", PING_YAMCS])
        self.assertEqual(names, ["memusage", "ping_yamcs"])
        server = yaml.safe_load((run / "etc" / "yamcs.yaml").read_text())
        self.assertEqual(server["instances"], names)
        for name, port in zip(names, (18300, 16300)):
            instance = yaml.safe_load(
                (run / "etc" / f"yamcs.{name}.yaml").read_text())
            mdb = instance["mdb"][-1]["args"]["file"]
            self.assertEqual(mdb, str(run / f"tcs-ccsds.{name}.xtce"))
            self.assertTrue(Path(mdb).is_file())
            links = {link["name"]: link for link in instance["dataLinks"]}
            self.assertEqual(links["node-commands"]["port"], port)


class ReportTest(unittest.TestCase):
    """What is printed of an answer."""

    def test_each_thread_is_a_line_and_its_node_is_named(self):
        one = memusage.Watched(HERE / "memusage.xtcs", "memusage", None)
        answer = {"mapped": 9 * 1024 * 1024, "resident": 7 * 1024 * 1024,
                  "locked": 0, "lock_limit": memusage.UNLIMITED,
                  "first": 0, "total": 2, "status": "Success",
                  "threads": [
                      {"role": "Main", "node": memusage.NO_NODE,
                       "stack": 8 * 1024 * 1024, "deepest": 40 * 1024},
                      {"role": "Node", "node": 2, "stack": 65536,
                       "deepest": 8192}]}
        lines = memusage.report(one, answer)
        self.assertIn("limit unlimited", lines[0])
        self.assertIn("2 threads", lines[0])
        self.assertIn("main", lines[1])
        # gateway is 0, ping1 1, ping2 2: by their names' order.
        self.assertIn("node ping2", lines[2])
        self.assertIn("12%", lines[2])


if __name__ == "__main__":
    unittest.main()
