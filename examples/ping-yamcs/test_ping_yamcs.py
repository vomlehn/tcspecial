"""Tests of ping_yamcs.py that need neither Yamcs nor a vehicle: what it
reads from the configuration, the Yamcs configuration it writes, and its ping
link driven through a Yamcs played here.

Run from the repository's root::

    python3 -m pytest examples/ping-yamcs/test_ping_yamcs.py
"""

import dataclasses
import re
import sys
import tempfile
import textwrap
import types
import unittest
from pathlib import Path
from xml.etree import ElementTree

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import ping_yamcs  # noqa: E402
from tcslibgs import configuration, ping, ping_packet  # noqa: E402
from tcslibgs import yamcs as generated  # noqa: E402


def options(**changed):
    """The options ping_yamcs reads, for the example's configuration, with
    no tcsverify asked for: the digest is not what these tests are about.

    Args:
        changed: Options to change.

    Returns:
        A namespace like the one :func:`ping_yamcs.arguments` gives.
    """
    said = dict(configuration=str(HERE / "ping-yamcs.xtcs"), tcsverify=None)
    said.update(changed)
    return types.SimpleNamespace(**said)


class SetupTest(unittest.TestCase):
    """What is read out of the example's configuration."""

    def test_the_example_says_its_apids_and_ports(self):
        setup = ping_yamcs.Setup(options())
        self.assertEqual((setup.beacon_apid, setup.cmd_apid, setup.uplink_apid,
                          setup.reply_apid), (0x100, 0x101, 0x200, 0x201))
        self.assertEqual(setup.uplink.port, 16600)
        self.assertEqual(setup.downlink.port, 16701)
        self.assertEqual(setup.beacon.recv.port, 16510)
        self.assertEqual(setup.command.port, 16200)

    def test_both_spellings_say_the_same(self):
        xml = ping_yamcs.Setup(options()).yamcs_settings(Path("/r"), 1)
        yaml = ping_yamcs.Setup(
            options(configuration=str(
                        HERE / "ping-yamcs.ytcs"))).yamcs_settings(Path("/r"),
                                                                   1)
        self.assertEqual(xml, yaml)


class WriteYamcsTest(unittest.TestCase):
    """The Yamcs configuration written from the templates."""

    def setUp(self):
        import tempfile
        self.home = Path(tempfile.mkdtemp())
        (self.home / "etc").mkdir()
        (self.home / "etc" / "processor.yaml.sample").write_text(
            'args:\n  stream: "tc_realtime"\n')
        self.run = Path(tempfile.mkdtemp()) / "run"

    def test_every_template_name_is_filled_in(self):
        setup = ping_yamcs.Setup(options())
        ping_yamcs.write_yamcs(setup.config, self.run, 16090, self.home,
                              "ping", "ping_yamcs.py")
        for written in ("etc/yamcs.yaml", "etc/yamcs.ping.yaml",
                        "tcs-ccsds.xtce", "etc/processor.yaml"):
            text = (self.run / written).read_text()
            self.assertIsNone(re.search(r"\$\w", text), written)

    def test_the_wrapper_carries_the_configurations_apids(self):
        setup = ping_yamcs.Setup(options())
        ping_yamcs.write_yamcs(setup.config, self.run, 16090, self.home,
                              "ping", "ping_yamcs.py")
        self.assertEqual(wrapper_apids(self.run / "tcs-ccsds.xtce"),
                         {("BeaconPacket", 0x100), ("AnswerPacket", 0x101),
                          ("Connect", 0x101), ("Ping", 0x200),
                          ("pingPacket", 0x201)})
        self.assertEqual(setup.ping_command, PING_COMMAND)
        self.assertEqual(setup.ping_reply, PING_REPLY)

    def test_the_instance_has_a_link_on_every_port(self):
        import yaml
        setup = ping_yamcs.Setup(options())
        ping_yamcs.write_yamcs(setup.config, self.run, 16090, self.home,
                              "ping", "ping_yamcs.py")
        instance = yaml.safe_load(
            (self.run / "etc" / "yamcs.ping.yaml").read_text())
        self.assertEqual([(link["name"], link["port"])
                          for link in instance["dataLinks"]],
                         [("commands", 16200), ("beacon", 16510),
                          ("gateway-uplink", 16600),
                          ("ping-downlink", 16701)])
        self.assertEqual([(stream["name"], stream["tcPatterns"])
                          for stream in instance["streamConfig"]["tc"]],
                         [("tc_commands", ["/TcsYamcs/Connect"]),
                          ("tc_gateway", ["/TcsYamcs/Ping"])])
        self.assertIn('streams: ["tc_commands", "tc_gateway"]',
                      (self.run / "etc" / "processor.yaml").read_text())

    def test_added_nodes_get_their_own_containers_and_links(self):
        import yaml
        with tempfile.TemporaryDirectory() as directory:
            path = with_more_nodes(Path(directory))
            setup = ping_yamcs.Setup(options(configuration=str(path)))
            ping_yamcs.write_yamcs(setup.config, self.run, 16090, self.home,
                              "ping", "ping_yamcs.py")
        # gateway 0, ping 1, pong 2, sensor 3, tank 4.
        self.assertEqual(wrapper_apids(self.run / "tcs-ccsds.xtce"),
                         {("BeaconPacket", 0x100), ("AnswerPacket", 0x101),
                          ("Connect", 0x101), ("Ping", 0x200),
                          ("pingPacket", 0x201), ("pongPacket", 0x202),
                          ("sensorPacket", 0x303), ("tankPacket", 0x304)})
        instance = yaml.safe_load(
            (self.run / "etc" / "yamcs.ping.yaml").read_text())
        self.assertEqual([(link["name"], link["port"])
                          for link in instance["dataLinks"]][3:],
                         [("ping-downlink", 16701), ("pong-downlink", 16702),
                          ("sensor-downlink", 16413),
                          ("tank-downlink", 16414)])


class WrittenMdbTest(unittest.TestCase):
    """The wrapper as ping_yamcs.py writes it: no line wider than 80
    columns, and the same XTCE as :func:`tcslibgs.yamcs.mdb` makes, for the
    example and for a copy of it with more nodes."""

    def written(self, path, directory):
        """Writes the wrapper for a configuration, and makes it again.

        Args:
            path:      The configuration.
            directory: Where to write it.

        Returns:
            The text written, and the tree made.
        """
        config = configuration.load(path)
        written = Path(directory) / "tcs-ccsds.xtce"
        generated.write_mdb(config, written)
        return written.read_text(), generated.mdb(config)

    def test_no_line_is_wider_than_80_columns(self):
        with tempfile.TemporaryDirectory() as directory:
            for path in (HERE / "ping-yamcs.xtcs",
                         with_more_nodes(Path(directory))):
                text, _ = self.written(path, directory)
                for number, line in enumerate(text.splitlines(), 1):
                    self.assertLessEqual(len(line), 80,
                                         f"{path.name}, line {number}: "
                                         f"{line}")

    def test_it_reads_as_the_xtce_it_was_made_from(self):
        with tempfile.TemporaryDirectory() as directory:
            for path in (HERE / "ping-yamcs.xtcs",
                         with_more_nodes(Path(directory))):
                text, tree = self.written(path, directory)
                parser = ElementTree.XMLParser(
                    target=ElementTree.TreeBuilder(insert_comments=True))
                parser.feed(text)
                self.assertEqual(shape(parser.close()),
                                 shape(tree.getroot()), path.name)


def shape(element):
    """What an element says, apart from the whitespace around it.

    Args:
        element: The element, or a comment.

    Returns:
        Its tag, its attributes, its text without the whitespace around
        it, and the same of each element in it, in order.
    """
    return (element.tag, dict(element.attrib),
            (element.text or "").strip(),
            [shape(inside) for inside in element])


def with_more_nodes(directory):
    """A copy of the example's configuration with a node more drawn into
    the gateway node -- "pong", a payload -- and two payloads nothing
    touches, "sensor" and "tank", which report over payload_telemetry.
    Their names sort after "gateway" and "ping", whose device ids stay 0
    and 1.

    Args:
        directory: Where to write it.

    Returns:
        The copy's path.
    """
    text = (HERE / "ping-yamcs.xtcs").read_text()
    payloads = textwrap.indent(textwrap.dedent("""\
        <payloads>
            <type name="network">
                <payload name="pong">
                    <address>127.0.0.1</address>
                    <port>16800</port>
                    <protocol>udp</protocol>
                    <payload-out><target>gateway</target></payload-out>
                </payload>
                <payload name="sensor">
                    <address>127.0.0.1</address>
                    <port>16801</port>
                    <protocol>udp</protocol>
                </payload>
                <payload name="tank">
                    <address>127.0.0.1</address>
                    <port>16802</port>
                    <protocol>udp</protocol>
                </payload>
            </type>
        </payloads>
        """), " " * 4)
    if text.count("    <tcsgui>") != 1:
        raise ValueError("the example has no one tcsgui section")
    path = directory / "with-more-nodes.xtcs"
    path.write_text(text.replace("    <tcsgui>", payloads + "    <tcsgui>"))
    return path


def wrapper_apids(path):
    """Every APID a written wrapper restricts a container to or assigns a
    command, with the container or command.

    Args:
        path: The wrapper.

    Returns:
        A set of ``(name, apid)``.
    """
    namespace = {"x": generated.NAMESPACE}
    root = ElementTree.parse(path).getroot()
    found = set()
    for container in root.iterfind(".//x:SequenceContainer", namespace):
        for comparison in container.iterfind(
                "./x:BaseContainer/x:RestrictionCriteria/x:Comparison"
                "[@parameterRef='CcsdsApid']", namespace):
            found.add((container.get("name"),
                       int(comparison.get("value"))))
    for command in root.iterfind(".//x:MetaCommand", namespace):
        for assigned in command.iterfind(
                "./x:BaseMetaCommand/x:ArgumentAssignmentList"
                "/x:ArgumentAssignment[@argumentName='CcsdsApid']",
                namespace):
            found.add((command.get("name"),
                       int(assigned.get("argumentValue"))))
    return found


#: The ping's command and its answer's parameters, as the ping node's
#: interface document names them.
PING_COMMAND = "/TcsYamcs/Ping"
PING_REPLY = ["/TcsYamcs/ReplyCount", "/TcsYamcs/ReplySentSeconds",
              "/TcsYamcs/ReplySentNanoseconds",
              "/TcsYamcs/ReplyAnsweredSeconds",
              "/TcsYamcs/ReplyAnsweredNanoseconds"]


class FakeYamcs:
    """A Yamcs that answers each ping as a vehicle would, a millisecond
    later by its reception time.

    Args:
        answer: Whether it answers at all.
    """

    def __init__(self, answer=True):
        self.answer = answer
        self.issued = []
        self.reply = None

    def issue(self, command, args):
        """Takes a command; a ping is answered with the vehicle's time, each
        field of the answer a parameter.

        Args:
            command: The command's qualified name.
            args:    Its arguments, by name.
        """
        self.issued.append((command, args))
        if command == PING_COMMAND and self.answer:
            data = ping_packet.Ping(
                count=args["Count"], sent_seconds=args["SentSeconds"],
                sent_nanoseconds=args["SentNanoseconds"]).to_bytes()
            sent = ping.read_message(data).sent
            answer = ping_packet.PingReply.decode(
                ping.reply(data, sent + 500_000))
            self.reply = ({name: getattr(answer, field) for field, name
                           in zip(ping_yamcs.REPLY_PARAMETERS, PING_REPLY)},
                          sent + 1_000_000)

    def parameters(self, names):
        """The latest answer's fields, or ``None``."""
        return self.reply if names == PING_REPLY else None


class YamcsPingerTest(unittest.TestCase):
    """The ping link through Yamcs."""

    def test_a_ping_goes_as_its_fields_and_is_timed(self):
        yamcs = FakeYamcs()
        pinger = ping_yamcs.YamcsPinger(yamcs, PING_COMMAND, PING_REPLY,
                                        clock=lambda: 10**18 + 7)
        sent = pinger.ping()
        self.assertEqual(sent, "ping #1 1000000000.000000007")
        command, args = yamcs.issued[-1]
        self.assertEqual(command, PING_COMMAND)
        self.assertEqual(args, {"Count": "1", "SentSeconds": "1000000000",
                                "SentNanoseconds": "000000007"})
        self.assertTrue(pinger.receive(0.1))
        exchange = pinger.exchange()
        self.assertEqual(exchange.received,
                         "ping #1 1000000000.000000007 1000000000.000500007")
        self.assertEqual(exchange.round_trip(), 1_000_000)
        self.assertEqual(exchange.up(), 500_000)
        self.assertEqual(exchange.down(), 500_000)

    def test_fields_that_are_not_an_answer_are_shown_as_not_one(self):
        fields = dict(zip(PING_REPLY, ["1", "1", "x" * 30, "2", "000000000"]))
        data = ping_yamcs.answer_of(fields, PING_REPLY)
        with self.assertRaises(ValueError):
            ping.read_reply(data)

    def test_the_names_are_the_wrappers(self):
        root = generated.mdb(configuration.load(
            HERE / "ping-yamcs.xtcs")).getroot()
        namespace = {"x": generated.NAMESPACE}
        command, = root.iterfind(".//x:MetaCommand[@name='Ping']", namespace)
        self.assertEqual([argument.get("name") for argument in
                          command.iterfind("./x:ArgumentList/x:Argument",
                                           namespace)],
                         list(ping_yamcs.PING_ARGUMENTS.values()))
        held = [entry.get("parameterRef") for entry in root.iterfind(
            ".//x:SequenceContainer[@name='pingPacket']/x:EntryList/*",
            namespace)]
        self.assertEqual([name for name in held
                          if name not in ("ReplyPrefix", "ReplySpace")],
                         list(ping_yamcs.REPLY_PARAMETERS.values()))
        self.assertEqual(list(ping_yamcs.REPLY_PARAMETERS),
                         [field.name for field in dataclasses.fields(
                             ping_packet.PingReply)])
        self.assertEqual(list(ping_yamcs.PING_ARGUMENTS),
                         [field.name for field in dataclasses.fields(
                             ping_packet.Ping)])

    def test_no_answer_is_no_answer(self):
        pinger = ping_yamcs.YamcsPinger(FakeYamcs(answer=False),
                                        PING_COMMAND, PING_REPLY,
                                        clock=lambda: 10**18)
        pinger.ping()
        self.assertFalse(pinger.receive(0.05))
        self.assertIsNone(pinger.exchange().received_at)


if __name__ == "__main__":
    unittest.main()
