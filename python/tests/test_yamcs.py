"""A configuration that answers to Yamcs, as a ground station reads it, and
the port-0 rule for where the ground sends commands from.

The Python mirrors of ``tcslibgs::gateway::GatewayKind::Yamcs``'s base
attribute and of ``tcslibgs::types::came_from`` and ``sender_of``: the two
languages must agree about which datagrams are the ground's.
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import configuration, interface  # noqa: E402
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
                "<payload-out><target>m</target></payload-out></compute>"
                "</type></vehicle></configuration>")
            config = configuration.load(path)
            self.assertEqual(config.downlink("m", "p"),
                             HostPort("127.0.0.1", 9001))
            link = config.command_link()
            self.assertEqual(link.send, HostPort("127.0.0.1", 0))


if __name__ == "__main__":
    unittest.main()
