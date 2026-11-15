"""A consolidator's inputs, relayed to Yamcs: which sources are relayed and
under which APIDs, the Python mirror of
``tcsprotocol::graph::telemetry::relayed``; what Yamcs is given of them; and
the relay itself, ``tcsmoc::relay``, started through libtcsmoc-c.

``tests/consolidator/yamcs.ytcs`` is the configuration, which the Rust tests
of the same read too: temp and valve reach the gateway node only in pack's
datum, and label is drawn in itself as well.
"""

import socket
import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcsmoc import relay  # noqa: E402
from tcsprotocol import configuration  # noqa: E402
from tcsprotocol import yamcs as generated  # noqa: E402

#: The configuration the tests read.
PATH = (Path(__file__).resolve().parents[2] / "tests" / "consolidator"
        / "yamcs.ytcs")


class RelayedTest(unittest.TestCase):
    def setUp(self):
        self.config = configuration.load(PATH)

    def test_only_inputs_not_drawn_in_are_relayed(self):
        self.assertEqual(self.config.relayed("gateway"),
                         [("temp", "pack"), ("valve", "pack")])
        self.assertEqual(relay.gateways_to_relay(self.config), ["gateway"])

    def test_a_relayed_input_has_its_own_apid_and_port(self):
        # Each as it would be were it drawn in: the gateway node's bases
        # counted on by its device id, 3 and 4.
        used = {use.node: use for use in self.config.apids()
                if use.carries == "downlink"}
        self.assertEqual(used["temp"].apid, 0x203)
        self.assertEqual(used["temp"].relayed_by, "pack")
        self.assertEqual(used["valve"].apid, 0x204)
        self.assertIsNone(used["label"].relayed_by)
        self.assertEqual(self.config.downlink("gateway", "valve").port,
                         26704)

    def test_yamcs_is_given_the_inputs_and_never_the_datum(self):
        links = generated.telemetry(self.config)
        self.assertEqual([(link.name, link.apid) for link in links],
                         [("label", 0x201), ("temp", 0x203),
                          ("valve", 0x204)])
        ports = [link.get("port") for link in
                 generated.data_links(self.config)]
        self.assertIn(26703, ports)
        # pack's port is the relay's.
        self.assertNotIn(26702, ports)


class RelayTest(unittest.TestCase):
    def setUp(self):
        try:
            relay.library()
        except OSError:
            raise unittest.SkipTest("libtcsmoc-c is not built")

    def test_each_new_input_reaches_its_own_port(self):
        temp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.addCleanup(temp.close)
        temp.bind(("127.0.0.1", 26703))
        temp.settimeout(5)
        with relay.Relays(PATH, configuration.load(PATH)) as relays:
            self.assertEqual(len(relays), 1)
            # pack's datum, laid out by hand: the states 10 (temp new), 00
            # and 00 (valve and name none), temp's sixteen bits, valve's
            # one and the name's four-bit length, padded to a byte.
            bits = "100000" + format(0x1234, "016b") + "0" + "0000"
            bits += "0" * (-len(bits) % 8)
            datum = int(bits, 2).to_bytes(len(bits) // 8, "big")
            body = struct.pack(">I", len(datum)) + datum
            header = struct.pack(">HHH", 0x202, 0xC000, len(body) - 1)
            vehicle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            self.addCleanup(vehicle.close)
            vehicle.sendto(header + body, ("127.0.0.1", 26702))
            got = temp.recv(64)
        # APID 0x203, the first of its count, two bytes: temp's reading.
        self.assertEqual(got, bytes.fromhex("0203c00000011234"))

    def test_with_nothing_to_relay_the_library_is_not_needed(self):
        ping = (Path(__file__).resolve().parents[2] / "examples"
                / "ping-yamcs" / "ping-yamcs.ytcs")
        relays = relay.Relays(ping, configuration.load(ping))
        self.assertEqual(len(relays), 0)
        self.assertIsNone(relays._lib)


if __name__ == "__main__":
    unittest.main()
