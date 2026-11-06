"""A configuration that answers to Yamcs, as a ground station reads it, its
APIDs, and the port-0 rule for where the ground sends commands from.

The Python mirrors of ``tcslibgs::gateway::GatewayKind::Yamcs``'s base
attribute, of ``tcslibgs::graph::telemetry::apids``, and of
``tcslibgs::types::came_from`` and ``sender_of``: the two languages must
agree about which datagrams are the ground's, and what each carries. And
the Yamcs mission database ``tcslibgs.yamcs`` generates from those APIDs.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from xml.etree import ElementTree

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import configuration, interface  # noqa: E402
from tcslibgs import yamcs as generated  # noqa: E402
from tcslibgs.configuration import HostPort, came_from  # noqa: E402


class SenderAtAnyPortTest(unittest.TestCase):
    def test_port_0_is_any_port_from_the_host(self):
        # As a ground that sends from whatever port the system gives it --
        # Yamcs does -- is written. The host is still asked about.
        self.assertTrue(came_from(HostPort("127.0.0.1", 0),
                                  ("127.0.0.1", 40000)))
        self.assertTrue(came_from(HostPort("localhost", 0),
                                  ("127.0.0.1", 1)))
        self.assertFalse(came_from(HostPort("127.0.0.1", 0),
                                   ("10.0.0.1", 40000)))
        self.assertTrue(came_from(HostPort("0.0.0.0", 0),
                                  ("10.0.0.1", 40000)))
        # And any other port is still the one port.
        self.assertFalse(came_from(HostPort("127.0.0.1", 4100),
                                   ("127.0.0.1", 40000)))

    def test_only_a_sender_may_be_written_with_port_0(self):
        self.assertEqual(HostPort.of("tcs_cmd.send", "127.0.0.1:0",
                                     any_port=True),
                         HostPort("127.0.0.1", 0))
        with self.assertRaisesRegex(ValueError, "1 to 65535"):
            HostPort.of("tcs_cmd.recv", "127.0.0.1:0")
        with self.assertRaisesRegex(ValueError, "1 to 65535"):
            HostPort.of("tcs_cmd.send", "127.0.0.1:65536", any_port=True)


class YamcsGatewayTest(unittest.TestCase):
    def test_a_yamcs_gateway_counts_its_ground_base_as_a_radio_does(self):
        self.assertEqual(configuration.GATEWAY_BASES["gateway_yamcs"],
                         "ground_base")
        self.assertNotIn("gateway_yamcs", configuration.ON_BUS)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "vehicle.xtcs"
            path.write_text(
                "<configuration>"
                "<tcspecial><gateway>yamcs</gateway>"
                f"<interface>{interface.of_this_tree().hex()}</interface>"
                "<tcs_cmd><send>127.0.0.1:0</send><recv>127.0.0.1:4000</recv>"
                "</tcs_cmd></tcspecial><gateways><type name=\"gateway_yamcs\">"
                "<gateway name=\"m\"><ground_base>127.0.0.1:9000</ground_base>"
                "<apid_base>0x200</apid_base></gateway></type></gateways>"
                "<vehicle><type name=\"ping\"><compute name=\"p\">"
                "<world-out>m.p</world-out></compute>"
                "</type></vehicle></configuration>")
            config = configuration.load(path)
            self.assertEqual(config.downlink("m", "p"),
                             HostPort("127.0.0.1", 9001))
            link = config.command_link()
            self.assertEqual(link.send, HostPort("127.0.0.1", 0))


#: A configuration that answers to Yamcs, in YAML, with a gateway_yamcs node
#: ("m", device id 0) a ping node sends to ("p", device id 3), and two
#: payloads: "n", which nothing touches (device id 1), and "o", which a
#: command binding names (device id 2).
YAMCS = """\
tcspecial:
  gateway: yamcs
  interface: "{interface}"
  beacon: {{ apid: "0x100", recv: "127.0.0.1:9100", send: "0.0.0.0:9101" }}
  payload_telemetry: {{ apid_base: "{base}", dest_base: "127.0.0.1:9200",
                       source_base: "0.0.0.0:9300" }}
  tcs_cmd: {{ apid: "0x101", recv: "127.0.0.1:4000", send: "127.0.0.1:0" }}
gateways:
  - type gateway_yamcs:
      - gateway: m
        apid_base: "0x200"
        ground_base: "127.0.0.1:9000"
        world-in: [p]
vehicle:
  - type ping:
      - compute: p
        moc-in: [m]
        world-out:
          - m.p
payloads:
  - type network:
      - payload: n
        address: "127.0.0.1"
        port: "9401"
        protocol: "udp"
      - payload: o
        address: "127.0.0.1"
        port: "9402"
        protocol: "udp"
commands:
  - {{ from: m, sets: o.setpoint }}
"""


def yamcs(directory, base="0x300", gateway="yamcs", renamed=None,
          ping_from_m=False):
    """Writes :data:`YAMCS` and reads it.

    Args:
        directory:   Where to write it.
        base:        Its ``payload_telemetry.apid_base``.
        gateway:     Its ``tcspecial.gateway``.
        renamed:     Each node to rename, and its new name: the names a
                     test gives must keep the device ids' order.
        ping_from_m: Whether "m" binds aboard and its data goes to "p", as
                     a ping goes up to a ping node.

    Returns:
        The :class:`configuration.Configuration`.
    """
    path = Path(directory) / "vehicle.ytcs"
    text = (YAMCS.format(interface=interface.of_this_tree().hex(), base=base)
            .replace("gateway: yamcs", f"gateway: {gateway}"))
    if ping_from_m:
        text = text.replace('        ground_base: "127.0.0.1:9000"\n',
                            '        ground_base: "127.0.0.1:9000"\n'
                            '        address: "127.0.0.1"\n'
                            '        port: "9500"\n'
                            "        moc-out:\n"
                            "          - p.m\n")
    for old, new in (renamed or {}).items():
        text = (text.replace(f": {old}\n", f": {new}\n")
                .replace(f"from: {old},", f"from: {new},"))
    path.write_text(text)
    return configuration.load(path)


class ApidTest(unittest.TestCase):
    """The APIDs, as ``tcslibgs::graph::telemetry::apids`` counts them."""

    def test_each_link_says_or_counts_its_apid(self):
        with tempfile.TemporaryDirectory() as directory:
            config = yamcs(directory)
            self.assertTrue(config.framed())
            self.assertEqual(config.beacon_apid(), 0x100)
            self.assertEqual(config.command_apid(), 0x101)
            self.assertEqual(config.uplink_apid("m"), 0x200)
            self.assertEqual(config.downlink_apid("m", "p"), 0x203)

    def test_an_untouched_payloads_telemetry_is_counted_as_its_port(self):
        with tempfile.TemporaryDirectory() as directory:
            config = yamcs(directory)
            # "o" is named by a command binding, so the drawing touches it.
            self.assertEqual(config.untouched_payloads(), ["n"])
            self.assertEqual(config.payload_telemetry_apid("n"), 0x301)
            self.assertEqual(config.payload_telemetry_link("n"),
                             (HostPort("0.0.0.0", 9301),
                              HostPort("127.0.0.1", 9201)))
            for touched in ("o", "p"):
                with self.assertRaisesRegex(ValueError, "nothing in the"):
                    config.payload_telemetry_apid(touched)
            with self.assertRaisesRegex(ValueError, "no node called"):
                config.payload_telemetry_apid("q")

    def test_what_is_wrong_with_an_apid_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            config = yamcs(directory, base="0x7FF")
            with self.assertRaisesRegex(ValueError, "eleven bits"):
                config.payload_telemetry_apid("n")
            config = yamcs(directory, base="0x800")
            with self.assertRaisesRegex(ValueError, "apid_base"):
                config.payload_telemetry_apid("n")
            config = yamcs(directory, base="")
            with self.assertRaisesRegex(ValueError, "not stated"):
                config.payload_telemetry_apid("n")
            config = yamcs(directory, gateway="radio")
            self.assertFalse(config.framed())
            with self.assertRaisesRegex(ValueError, "no CCSDS header"):
                config.payload_telemetry_apid("n")
            with self.assertRaisesRegex(ValueError, "no CCSDS header"):
                config.beacon_apid()

    def test_every_apid_is_listed_by_what_it_carries(self):
        with tempfile.TemporaryDirectory() as directory:
            config = yamcs(directory)
            self.assertEqual([(use.carries, use.apid, use.node, use.gateway)
                              for use in config.apids()],
                             [("beacon", 0x100, None, None),
                              ("commands", 0x101, None, None),
                              ("uplink", 0x200, "m", None),
                              ("downlink", 0x203, "p", "m"),
                              ("payload_telemetry", 0x301, "n", None)])

    def test_an_apid_used_twice_is_refused_naming_both(self):
        with tempfile.TemporaryDirectory() as directory:
            # The payload's telemetry, 0x100 plus 1, lands on tcs_cmd's.
            config = yamcs(directory, base="0x100")
            with self.assertRaisesRegex(ValueError,
                                        "tcs_cmd.apid and the APID of "
                                        '"n"\'s telemetry.* both APID '
                                        "0x101"):
                config.apids()

    def test_a_gateway_nodes_commands_may_share_its_first_downlink(self):
        with tempfile.TemporaryDirectory() as directory:
            # "a" sorts first: device id 0, its downlink the apid_base.
            config = yamcs(directory, renamed={"p": "a"})
            uses = config.apids()
            self.assertEqual([(use.carries, use.apid) for use in uses
                              if use.node in ("m", "a")],
                             [("uplink", 0x200), ("downlink", 0x200)])


class MdbTest(unittest.TestCase):
    """The Yamcs mission database's wrapper, generated from a
    configuration."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.namespace = {"x": generated.NAMESPACE}

    def find(self, root, path):
        """Everything at an ElementTree path, its names in XTCE's
        namespace."""
        return root.findall(path, self.namespace)

    def test_every_apid_gets_its_container_or_command(self):
        root = generated.mdb(yamcs(self.directory.name)).getroot()
        restricted = {}
        for container in self.find(root, ".//x:SequenceContainer"):
            for comparison in self.find(
                    container, "./x:BaseContainer/x:RestrictionCriteria"
                               "/x:Comparison[@parameterRef='CcsdsApid']"):
                restricted[container.get("name")] = int(
                    comparison.get("value"))
        self.assertEqual(restricted, {"BeaconPacket": 0x100,
                                      "AnswerPacket": 0x101,
                                      "pPacket": 0x203, "nPacket": 0x301})
        assigned = {}
        for command in self.find(root, ".//x:MetaCommand"):
            for assignment in self.find(
                    command, ".//x:ArgumentAssignment"
                             "[@argumentName='CcsdsApid']"):
                assigned[command.get("name")] = int(
                    assignment.get("argumentValue"))
        self.assertEqual(assigned, {"Connect": 0x101, "m": 0x200})
        # The data of a node with no document is its parameter, what
        # follows the header whole.
        parameter, = self.find(root, ".//x:Parameter[@name='n']")
        self.assertEqual(parameter.get("parameterTypeRef"),
                         generated.BODY_TYPE)
        self.assertEqual(self.find(root, ".//x:Parameter[@name='p']"), [])

    def test_a_ping_nodes_answer_is_laid_out_by_its_document(self):
        root = generated.mdb(yamcs(self.directory.name)).getroot()
        container, = self.find(root, ".//x:SequenceContainer"
                                     "[@name='pPacket']")
        self.assertEqual([entry.get("parameterRef") for entry
                          in self.find(container, "./x:EntryList/*")],
                         ["ReplyPrefix", "ReplyCount", "ReplySentSeconds",
                          "ReplySentNanoseconds", "ReplySpace",
                          "ReplyAnsweredSeconds", "ReplyAnsweredNanoseconds"])
        # Each parameter and type is the document's, copied once.
        count, = self.find(root, ".//x:Parameter[@name='ReplyCount']")
        self.assertEqual(count.get("parameterTypeRef"), "PingCountType")
        kind, = self.find(root, ".//x:StringParameterType"
                                "[@name='PingCountType']")
        self.assertEqual(self.find(kind, ".//x:TerminationChar")[0].text,
                         "20")
        self.assertEqual(len(self.find(root, ".//x:StringParameterType"
                                             "[@name='PingSecondsType']")),
                         1)

    def test_a_gateway_node_to_a_ping_node_takes_the_ping(self):
        config = yamcs(self.directory.name, ping_from_m=True)
        uplink, = generated.uplinks(config)
        self.assertEqual((uplink.name, uplink.command, uplink.document),
                         ("m", "Ping", interface.node_document("ping")))
        root = generated.mdb(config).getroot()
        command, = self.find(root, ".//x:MetaCommand[@name='Ping']")
        self.assertEqual(
            int(self.find(command, ".//x:ArgumentAssignment")[0]
                .get("argumentValue")), 0x200)
        self.assertEqual([argument.get("name") for argument
                          in self.find(command, "./x:ArgumentList/*")],
                         ["Count", "SentSeconds", "SentNanoseconds"])
        entries = self.find(command, "./x:CommandContainer/x:EntryList/*")
        self.assertEqual(entries[0].get("binaryValue"), b"ping #".hex()
                         .upper())
        self.assertEqual(self.find(root, ".//x:MetaCommand[@name='m']"), [])

    def test_a_gateway_node_to_other_nodes_too_sends_data_whole(self):
        config = yamcs(self.directory.name, ping_from_m=True)
        path = Path(config.path)
        path.write_text(path.read_text().replace(
            "          - p.m\n",
            "          - p.m\n          - n.m\n"))
        config = configuration.load(path)
        uplink, = generated.uplinks(config)
        self.assertEqual((uplink.command, uplink.document), ("m", None))

    def test_a_document_the_wrapper_cannot_copy_is_refused(self):
        document = interface.node_document("ping").read_text()
        cases = (
            # A type that refers to another value.
            ("<TerminationChar>20</TerminationChar>",
             "<TerminationChar>20</TerminationChar><Dummy/>",
             "Dummy, which the Yamcs wrapper does not copy"),
            # A second answer.
            ("</ContainerSet>",
             '<SequenceContainer name="More"><EntryList/>'
             "</SequenceContainer></ContainerSet>",
             "2 containers that are not abstract"),
            # A restriction that is not a constant.
            ('<Comparison parameterRef="ReplySpace" value=" "/>',
             '<Comparison parameterRef="ReplySpace" value=" " '
             'comparisonOperator="!="/>',
             "other than a parameter being equal"),
            # A reference outside the document.
            ('parameterRef="ReplyCount"/>',
             'parameterRef="/Other/ReplyCount"/>',
             "which the document does not have"))
        for old, new, says in cases:
            with self.subTest(says=says):
                path = Path(self.directory.name) / "ping.xtce"
                path.write_text(document.replace(old, new, 1))
                saved = dict(interface.NODE_DOCUMENTS)
                interface.NODE_DOCUMENTS["ping"] = path
                try:
                    with self.assertRaisesRegex(ValueError, says):
                        generated.mdb(yamcs(self.directory.name,
                                            ping_from_m=True))
                finally:
                    interface.NODE_DOCUMENTS.clear()
                    interface.NODE_DOCUMENTS.update(saved)

    def test_the_links_follow_the_configuration(self):
        config = yamcs(self.directory.name)
        self.assertEqual([(link.name, link.apid, link.ground.port)
                          for link in generated.telemetry(config)],
                         [("p", 0x203, 9003), ("n", 0x301, 9201)])
        # "m" binds nothing aboard, so the ground has nowhere to send to
        # it: its command is counted, but nothing carries it.
        self.assertEqual([stream["tcPatterns"] for stream
                          in generated.command_streams(config)],
                         [["/TcsYamcs/Connect"]])
        self.assertEqual([(link["name"], link["port"])
                          for link in generated.data_links(config)],
                         [("commands", 4000), ("beacon", 9100),
                          ("p-downlink", 9003), ("n-downlink", 9201)])

    def test_a_name_the_wrapper_has_is_refused(self):
        config = yamcs(self.directory.name, renamed={"m": "Connect"})
        with self.assertRaisesRegex(ValueError, '"Connect" gives the Yamcs '
                                                "command name"):
            generated.mdb(config)
        config = yamcs(self.directory.name, renamed={"n": "BeaconBody"})
        with self.assertRaisesRegex(ValueError, "already has"):
            generated.mdb(config)

    def test_the_worlds_ambient_is_a_local_temperature_when_asked(self):
        config = yamcs(self.directory.name)
        root = generated.mdb(config).getroot()
        self.assertEqual(self.find(root, ".//x:Parameter[@name='"
                                         f"{generated.WORLD_AMBIENT}']"), [])
        root = generated.mdb(config, world=True).getroot()
        parameter, = self.find(root, ".//x:Parameter[@name='"
                                     f"{generated.WORLD_AMBIENT}']")
        self.assertEqual(parameter.get("parameterTypeRef"),
                         generated.WORLD_TEMPERATURE_TYPE)
        properties, = self.find(parameter, "./x:ParameterProperties")
        self.assertEqual(properties.get("dataSource"), "local")
        kind, = self.find(root, ".//x:FloatParameterType[@name='"
                                f"{generated.WORLD_TEMPERATURE_TYPE}']")
        self.assertEqual([unit.text for unit in self.find(kind, ".//x:Unit")],
                         ["K"])
        # No packet carries it, so it has no encoding to read one by.
        self.assertEqual(self.find(kind, "./x:IntegerDataEncoding"), [])
        # And a node of that name is refused, told why.
        config = yamcs(self.directory.name,
                       renamed={"n": generated.WORLD_AMBIENT})
        with self.assertRaisesRegex(ValueError, "simulated world"):
            generated.mdb(config, world=True)

    def test_an_apid_used_twice_is_refused(self):
        with self.assertRaisesRegex(ValueError, "both APID"):
            generated.mdb(yamcs(self.directory.name, base="0x100"))

    def test_the_wrapper_is_well_formed_xtce(self):
        """Well-formed, and valid against the XTCE schema: the XTCE 1.2
        schema Yamcs reads, where ``$XTCE_XSD`` names it, or else XTCE
        1.3's in the tcs-xtce checkout beside this tree, the namespace
        moved to 1.3's. With the ping's command in it, copied from the
        ping node's document."""
        path = Path(self.directory.name) / "tcs-ccsds.xtce"
        generated.write_mdb(yamcs(self.directory.name, ping_from_m=True),
                            path)
        ElementTree.parse(path)
        references = (Path(__file__).resolve().parents[3] / "tcs-xtce"
                      / "references")
        schema = os.environ.get("XTCE_XSD")
        if schema is None and (references / "SpaceSystem.xsd").exists():
            schema = references / "SpaceSystem.xsd"
            text = path.read_text().replace(generated.NAMESPACE,
                                            "http://www.omg.org/spec/XTCE/"
                                            "20250214")
            path.write_text(text)
        if schema is None or shutil.which("xmllint") is None:
            self.skipTest("no XTCE schema or no xmllint to check it with")
        catalog = references / "catalog.xml"
        environment = dict(os.environ)
        if catalog.exists():
            environment["XML_CATALOG_FILES"] = str(catalog)
        ran = subprocess.run(["xmllint", "--nonet", "--noout", "--schema",
                              str(schema), str(path)],
                             capture_output=True, text=True, env=environment)
        self.assertEqual(ran.returncode, 0, ran.stderr)


class WrittenTest(unittest.TestCase):
    """The wrapper as it is written: no line wider than the template's."""

    def test_a_wide_start_tag_has_an_attribute_on_each_line(self):
        root = ElementTree.Element(generated._tag("SpaceSystem"),
                                   {"name": "TcsYamcs"})
        ElementTree.SubElement(root, generated._tag("Argument"),
                               {"name": "SentNanoseconds",
                                "argumentTypeRef": "PingNanosecondsArg",
                                "initialValue": "000000000"})
        ElementTree.SubElement(root, generated._tag("Parameter"),
                               {"name": "A", "parameterTypeRef": "B"})
        self.assertEqual(generated.serialized(root), [
            '<SpaceSystem xmlns="http://www.omg.org/spec/XTCE/20180204" '
            'name="TcsYamcs">',
            '  <Argument name="SentNanoseconds"',
            '            argumentTypeRef="PingNanosecondsArg"',
            '            initialValue="000000000"/>',
            '  <Parameter name="A" parameterTypeRef="B"/>',
            "</SpaceSystem>"])

    def test_text_and_attributes_are_escaped(self):
        root = ElementTree.Element(generated._tag("Comparison"),
                                   {"value": '<"&>'})
        root.text = "a < b & c"
        written = "\n".join(generated.serialized(root))
        self.assertEqual(ElementTree.fromstring(written).get("value"),
                         '<"&>')
        self.assertEqual(ElementTree.fromstring(written).text, "a < b & c")


if __name__ == "__main__":
    unittest.main()
