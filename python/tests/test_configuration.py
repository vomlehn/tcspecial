"""What a ground station reads out of a configuration.

``examples/ping/tests/agrees.rs`` holds these answers to the Rust's for the
shipped example; these say how they are worked out, on configurations made
to show it.
"""

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import configuration  # noqa: E402
from tcslibgs.configuration import HostPort, came_from  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
PING = ROOT / "examples" / "ping" / "ping.xml"


def written(text):
    """A configuration in a file of its own."""
    directory = tempfile.TemporaryDirectory()
    path = Path(directory.name) / "vehicle.xml"
    path.write_text(f"<configuration>{text}</configuration>")
    return directory, path


class FindBuiltTest(unittest.TestCase):
    def test_a_build_sent_elsewhere_is_found_there(self):
        # A program in a target directory of its own, which is where a
        # CARGO_TARGET_DIR build puts it and no other is looked for.
        with tempfile.TemporaryDirectory() as target:
            program = Path(target) / "debug" / "tcs-find-built-test"
            program.parent.mkdir()
            program.write_text("")
            original = os.environ.get("CARGO_TARGET_DIR")
            os.environ["CARGO_TARGET_DIR"] = target
            try:
                found = configuration.find_built("tcs-find-built-test")
            finally:
                if original is None:
                    del os.environ["CARGO_TARGET_DIR"]
                else:
                    os.environ["CARGO_TARGET_DIR"] = original
            self.assertEqual(found, str(program))

    def test_a_program_built_nowhere_is_said_so(self):
        self.assertIsNone(configuration.find_built("tcs-built-nowhere"))


class ConfigurationTest(unittest.TestCase):
    def test_the_shipped_example_says_where_everything_is(self):
        config = configuration.load(PING)
        link = config.beacon_link()
        self.assertEqual(link.send, HostPort("0.0.0.0", 7500))
        self.assertEqual(link.recv, HostPort("127.0.0.1", 7510))
        self.assertEqual(config.uplink("moc"), HostPort("127.0.0.1", 7600))
        # ping is the second node written, device id 1.
        self.assertEqual(config.device_id("ping"), 1)
        self.assertEqual(config.downlink("moc", "ping"), HostPort("127.0.0.1", 7701))

    def test_the_beacon_interval_is_read_as_the_rust_reads_one(self):
        self.assertEqual(configuration.load(PING).beacon_interval(), 5_000_000_000)
        parse = configuration.parse_interval
        self.assertEqual(parse("500 ms"), 500_000_000)
        self.assertEqual(parse("1.5 s"), parse("1_500 ms"))
        self.assertEqual(parse("0.5 d"), 12 * 3_600 * 1_000_000_000)
        self.assertEqual(parse(" 2 min "), 120_000_000_000)
        self.assertEqual(parse("1.0000004 s"), 1_000_000_000)
        for refused in ["5s", "5", "5 m", "5 mon", "-5 s", "1e3 s", "0 s",
                        "0.0000001 s", ". s", "1.2.3 s", "5 s extra", "5 µs"]:
            with self.assertRaises(ValueError, msg=refused):
                parse(refused)

    def test_a_device_id_counts_every_node_in_the_order_written(self):
        # Sections interleave in XML, and the id is the position among all of
        # them, not within a section.
        directory, path = written(
            "<payloads><name>a</name><type>i2c</type></payloads>"
            "<moc><name>m</name><type>moc</type><ground_base>127.0.0.1:9000</ground_base></moc>"
            "<vehicle><name>p</name><type>ping</type>"
            "<payload-out><target>m</target></payload-out></vehicle>"
            "<payloads><name>b</name><type>i2c</type></payloads>"
        )
        with directory:
            config = configuration.load(path)
            self.assertEqual([config.device_id(n) for n in "ampb"], [0, 1, 2, 3])
            self.assertEqual(config.downlink("m", "p"), HostPort("127.0.0.1", 9002))
            with self.assertRaisesRegex(ValueError, "sends nothing"):
                config.downlink("m", "a")
            with self.assertRaisesRegex(ValueError, "binds nothing aboard"):
                config.uplink("m")

    def test_an_include_is_refused_rather_than_counted_wrong(self):
        directory, path = written("<include>more</include>")
        with directory:
            with self.assertRaisesRegex(ValueError, "includes other files"):
                configuration.load(path)

    def test_the_two_spellings_read_alike(self):
        # The YAML and the XML of one example say the same thing, so they are
        # read into the same configuration: every node in the same order, with
        # the same attributes and links, and the same tcspecial section.
        for example in ["ping", "beacon", "minimal", "thermostat", "simple-app"]:
            stem = ROOT / "examples" / example / example
            xml = configuration.load(stem.with_suffix(".xml"))
            yaml = configuration.load(stem.with_suffix(".yaml"))
            self.assertEqual(xml.tcspecial, yaml.tcspecial, example)
            self.assertEqual(xml.nodes, yaml.nodes, example)

    def test_a_file_that_is_neither_spelling_is_refused(self):
        with self.assertRaisesRegex(ValueError, ".yaml, .yml or .xml"):
            configuration.load(ROOT / "examples" / "ping" / "ping.svg")

    def test_a_datagram_is_taken_only_from_where_the_configuration_says(self):
        # types.rs's own cases.
        self.assertTrue(came_from(HostPort("127.0.0.1", 4100), ("127.0.0.1", 4100)))
        self.assertFalse(came_from(HostPort("127.0.0.1", 4100), ("127.0.0.1", 4101)))
        self.assertFalse(came_from(HostPort("127.0.0.1", 4100), ("10.0.0.1", 4100)))
        self.assertTrue(came_from(HostPort("localhost", 4100), ("127.0.0.1", 4100)))
        self.assertTrue(came_from(HostPort("0.0.0.0", 5550), ("127.0.0.1", 5550)))
        self.assertTrue(came_from(HostPort("0.0.0.0", 5550), ("10.0.0.1", 5550)))
        self.assertFalse(came_from(HostPort("0.0.0.0", 5550), ("127.0.0.1", 5551)))

    def test_a_host_and_port_is_read_as_the_rust_reads_one(self):
        self.assertEqual(HostPort.of("x", " 0.0.0.0:4000 "), HostPort("0.0.0.0", 4000))
        for wrong in ["0.0.0.0", ":4000", "host:", "host:port", "host:0", "host:65536"]:
            with self.subTest(wrong=wrong):
                with self.assertRaises(ValueError):
                    HostPort.of("x", wrong)


if __name__ == "__main__":
    unittest.main()
