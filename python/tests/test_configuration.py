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
PING = ROOT / "examples" / "ping" / "ping.xtcs"


def written(text):
    """A configuration in a file of its own.

    Args:
        text: The XML inside the ``configuration`` element.

    Returns:
        A pair: the temporary directory holding the file, which removes it
        when cleaned up, and the file's path.
    """
    directory = tempfile.TemporaryDirectory()
    path = Path(directory.name) / "vehicle.xtcs"
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
        self.assertEqual(config.uplink("gateway"), HostPort("127.0.0.1", 7600))
        # ping is the second of the two names, device id 1.
        self.assertEqual(config.device_id("ping"), 1)
        self.assertEqual(config.downlink("gateway", "ping"),
                         HostPort("127.0.0.1", 7701))

    def test_the_beacon_interval_is_read_as_the_rust_reads_one(self):
        self.assertEqual(configuration.load(PING).beacon_interval(),
                         5_000_000_000)
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

    def test_ids_tell_case_apart_and_sort_by_code_point(self):
        """Names that differ only in case are two names, and ids follow the
        names' code points, as tests/ids/case.ids says and tcslibgs's Rust
        test of the same file holds it to."""
        ids = Path(__file__).resolve().parents[2] / "tests" / "ids"
        config = configuration.load(str(ids / "case.ytcs"))
        expected = {}
        for line in (ids / "case.ids").read_text().splitlines():
            if not line.startswith("#"):
                name, id = line.split(" ")
                expected[name] = int(id)
        self.assertEqual(len(expected), 8)
        self.assertEqual({node.name: config.device_id(node.name)
                          for node in config.nodes},
                         expected)

    def test_a_device_id_is_a_nodes_place_among_the_names(self):
        # Whatever section each is in, and whatever order the file writes
        # them in: the id is the place among every node's name.
        directory, path = written(
            '<payloads><type name="i2c"><payload name="b"/><payload name="a"/>'
            '</type></payloads><gateways><type name="gateway_radio"><gateway '
            'name="m"><ground_base>127.0.0.1:9000</ground_base>'
            '<world-in>p</world-in></gateway></type></gateways>'
            '<vehicle><type name="ping"><compute name="p"><world-out>'
            'm.p</world-out></compute></type></vehicle>')
        with directory:
            config = configuration.load(path)
            self.assertEqual([config.device_id(n) for n in "abmp"],
                             [0, 1, 2, 3])
            self.assertEqual([node.name for node in config.nodes],
                             ["a", "b", "m", "p"])
            self.assertEqual(config.downlink("m", "p"),
                             HostPort("127.0.0.1", 9003))
            with self.assertRaisesRegex(ValueError, "sends nothing"):
                config.downlink("m", "a")
            with self.assertRaisesRegex(ValueError, "binds nothing aboard"):
                config.uplink("m")

    def test_every_file_the_two_readers_refuse_is_refused(self):
        # tests/refused, which tcslib_protocol::graph::grouped holds the Rust
        # reader to as well: the two refuse the same things, at the line of
        # each.
        files = sorted((ROOT / "tests" / "refused").iterdir())
        self.assertGreater(len(files), 20)
        for path in files:
            with self.subTest(path=path.name):
                with self.assertRaisesRegex(ValueError, "line"):
                    configuration.load(path)

    def test_an_included_files_nodes_are_counted_with_the_top_files(self):
        # From a directory beside the top file's, by its path from there:
        # the ids are each node's place among every file's names.
        with tempfile.TemporaryDirectory() as directory:
            here = Path(directory)
            (here / "vehicle").mkdir()
            (here / "common").mkdir()
            top = here / "vehicle" / "top.xtcs"
            top.write_text(
                '<configuration><include name="s" path="../common/zone"/>'
                '<payloads><type name="i2c"><payload name="b"/></type>'
                "</payloads></configuration>")
            (here / "common" / "zone.xtcs").write_text(
                '<configuration><payloads><type name="i2c"><payload name="a"/>'
                '<payload name="c"/></type></payloads></configuration>')
            config = configuration.load(top)
            self.assertEqual(config.included,
                             [configuration.Include("s", "../common/zone")])
            self.assertEqual([config.device_id(n) for n in "abc"],
                             [0, 1, 2])

    def test_an_include_name_says_includes_are_relative(self):
        # Nothing is expanded: a ~ or a variable is refused, saying what an
        # include is, rather than looked for as a file of that name.
        for name in ["~/zone", "$HOME/zone", "/zone"]:
            with self.subTest(name=name):
                directory, path = written(
                    f'<include name="s" path="{name}"/>')
                with directory:
                    with self.assertRaisesRegex(
                            ValueError, "relative to this file's directory"):
                        configuration.load(path)

    def test_an_include_is_refused_as_the_rust_refuses_it(self):
        def include(path, name="s"):
            return f'<include name="{name}" path="{path}"/>'

        refused = {
            include(""): "is empty",
            include("/zone"): "is an absolute path",
            include("./zone"): "part that is empty",
            include("a//zone"): "part that is empty",
            include("~/zone"): "begins with ~",
            include("$HOME/zone"): r"has a \$ in it",
            include("${COMMON}/zone"): r"has a \$ in it",
            include("zone") + include("zone", "t"): "included twice",
            include("vehicle"): "included twice",
            include("section"): "may not have a tcspecial",
            include("nests"): "may not include others",
            "<exports><world-in>a</world-in></exports>":
                "only an included file may have an exports section",
            include("zone")
            + '<payloads><type name="i2c"><payload name="a"/></type>'
            "</payloads>": 'node called "a" here and in',
            # Written as includes once were: a bare path.
            "<include>zone</include>": "is a bare path",
            '<include name="s"/>': "has no path",
            # And a subdiagram's name is its own.
            include("zone", "a.b"): "a dot or a slash",
            include("zone") + include("section", "s"): "named twice",
            include("zone", "b")
            + '<payloads><type name="i2c"><payload name="b"/></type>'
            "</payloads>": "has the name of a node",
        }
        for text, why in refused.items():
            with self.subTest(text=text):
                directory, path = written(text)
                with directory:
                    here = path.parent
                    (here / "zone.xtcs").write_text(
                        '<configuration><payloads><type name="i2c">'
                        '<payload name="a"/></type></payloads>'
                        "</configuration>")
                    (here / "section.xtcs").write_text(
                        "<configuration><tcspecial/></configuration>")
                    (here / "nests.xtcs").write_text(
                        '<configuration><include name="z" path="zone"/>'
                        "</configuration>")
                    with self.assertRaisesRegex(ValueError, why):
                        configuration.load(path)

    def test_a_yaml_include_names_its_subdiagram(self):
        # In YAML a subdiagram is written as a node is, its path under its
        # name; a bare path is refused, saying how to write it now.
        with tempfile.TemporaryDirectory() as directory:
            here = Path(directory)
            (here / "zone.ytcs").write_text("payloads: []\n")
            top = here / "top.ytcs"
            top.write_text("include:\n  - subdiagram: inside\n"
                           "    path: zone\n")
            self.assertEqual(configuration.load(top).included,
                             [configuration.Include("inside", "zone")])
            top.write_text("include:\n  - zone\n")
            with self.assertRaisesRegex(ValueError, "subdiagram: <name>"):
                configuration.load(top)

    def test_the_two_spellings_read_alike(self):
        # The YAML and the XML of one example say the same thing, so they are
        # read into the same configuration: every node, by name, with the
        # same attributes and links, and the same tcspecial section.
        for example in ["ping", "beacon", "minimal", "thermostat",
                        "simple-app"]:
            stem = ROOT / "examples" / example / example
            xml = configuration.load(stem.with_suffix(".xtcs"))
            yaml = configuration.load(stem.with_suffix(".ytcs"))
            self.assertEqual(xml.tcspecial, yaml.tcspecial, example)
            self.assertEqual(xml.nodes, yaml.nodes, example)

    def test_a_configuration_on_a_cfs_bus_says_its_message_ids(self):
        # tcspecial/tests/cfs.rs's numbers, read from either spelling.
        stem = ROOT / "examples" / "cfs" / "cfs"
        xml = configuration.load(stem.with_suffix(".xtcs"))
        yaml = configuration.load(stem.with_suffix(".ytcs"))
        self.assertEqual(xml.tcspecial, yaml.tcspecial)
        self.assertEqual(xml.nodes, yaml.nodes)
        for config in (xml, yaml):
            bus = config.bus()
            self.assertEqual(bus.sbn.peer, HostPort("127.0.0.1", 2234))
            self.assertEqual((bus.sbn.processor_id, bus.sbn.peer_processor_id,
                              bus.sbn.spacecraft_id), (2, 1, 0x42))
            self.assertEqual(bus.sbn.ground_processor_id(), 3)
            self.assertEqual(bus.sbn.release.protocol_version, 11)
            self.assertEqual((bus.cmd_msgid, bus.response_msgid,
                              bus.beacon_msgid), (0x4280, 0x4200, 0x4201))
            self.assertEqual(config.downlink_msgid("fsw", "thermometer"),
                             0x4242)
            for k, name in enumerate(["fsw", "heater", "thermometer"]):
                self.assertEqual(config.node_command_msgid(name), 0x4290 + k)
                self.assertEqual(config.node_answer_msgid(name), 0x4210 + k)
            self.assertEqual(config.telemetry_msgids(),
                             [0x4200, 0x4201, 0x4210, 0x4211, 0x4212,
                              0x4242])
            # What is a socket elsewhere is a message ID here, and is said
            # so rather than looked for.
            with self.assertRaisesRegex(ValueError, "software bus"):
                config.beacon_link()
            with self.assertRaisesRegex(ValueError, "software bus"):
                config.command_link()
            with self.assertRaisesRegex(ValueError, "downlink_msgid"):
                config.downlink("fsw", "thermometer")
        self.assertIsNone(configuration.load(PING).bus())

    def test_a_whole_number_is_read_as_the_rust_reads_one(self):
        for said in ["1000", "1_000", "0x3e8", "0X3E8", "0x3_e8", " 1000 "]:
            self.assertEqual(configuration.whole("x", said), 1000, said)
        for wrong in ["", "_", "0x", "ten", "1.5", "-1", "0b101", "48g"]:
            with self.assertRaisesRegex(ValueError, "whole number"):
                configuration.whole("x", wrong)
        with self.assertRaisesRegex(ValueError, "most"):
            configuration.whole("x", "0x10000", 0xFFFF)

    def test_a_file_that_is_neither_spelling_is_refused(self):
        with self.assertRaisesRegex(ValueError, r"\.ytcs.*\.xtcs"):
            configuration.load(ROOT / "examples" / "ping" / "ping.svg")

    def test_an_old_extension_is_refused_with_the_new_name(self):
        # Every configuration was once named .yaml or .xml: one still named
        # so is told the name to give it, rather than read.
        for old, new in [("a.yaml", "a.ytcs"), ("a.yml", "a.ytcs"),
                         ("a.xml", "a.xtcs")]:
            with self.assertRaisesRegex(ValueError, f"rename it {new}"):
                configuration.load(old)

    def test_a_datagram_is_taken_only_from_where_the_configuration_says(self):
        # types.rs's own cases.
        self.assertTrue(came_from(HostPort("127.0.0.1", 4100),
                                  ("127.0.0.1", 4100)))
        self.assertFalse(came_from(HostPort("127.0.0.1", 4100),
                                   ("127.0.0.1", 4101)))
        self.assertFalse(came_from(HostPort("127.0.0.1", 4100),
                                   ("10.0.0.1", 4100)))
        self.assertTrue(came_from(HostPort("localhost", 4100),
                                  ("127.0.0.1", 4100)))
        self.assertTrue(came_from(HostPort("0.0.0.0", 5550),
                                  ("127.0.0.1", 5550)))
        self.assertTrue(came_from(HostPort("0.0.0.0", 5550),
                                  ("10.0.0.1", 5550)))
        self.assertFalse(came_from(HostPort("0.0.0.0", 5550),
                                   ("127.0.0.1", 5551)))

    def test_a_host_and_port_is_read_as_the_rust_reads_one(self):
        self.assertEqual(HostPort.of("x", " 0.0.0.0:4000 "),
                         HostPort("0.0.0.0", 4000))
        for wrong in ["0.0.0.0", ":4000", "host:", "host:port", "host:0",
                      "host:65536"]:
            with self.subTest(wrong=wrong):
                with self.assertRaises(ValueError):
                    HostPort.of("x", wrong)


if __name__ == "__main__":
    unittest.main()


class AmbientTest(unittest.TestCase):
    """The simulated world's ambient temperature, as tcsverify reads it."""

    def test_the_worlds_ambient_is_read_beside_the_configuration(self):
        program = configuration.find_tcsverify()
        if program is None:
            self.skipTest("tcsverify is not built")
        kelvin, why = configuration.ambient(
            ROOT / "examples" / "thermostat" / "thermostat.xtcs", program)
        self.assertEqual((kelvin, why), (288.15, None))
        # A world with no ambient temperature is said, not guessed.
        kelvin, why = configuration.ambient(
            ROOT / "examples" / "minimal" / "minimal.ytcs", program)
        self.assertIsNone(kelvin)
        self.assertIn("no world", why)

    def test_no_tcsverify_is_said(self):
        kelvin, why = configuration.ambient(
            ROOT / "examples" / "thermostat" / "thermostat.xtcs",
            "/nonexistent/tcsverify")
        self.assertIsNone(kelvin)
        self.assertIn("cannot be run", why)
