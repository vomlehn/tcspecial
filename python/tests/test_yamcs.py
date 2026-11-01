"""A configuration that answers to Yamcs, as a ground station reads it, its
APIDs, and the port-0 rule for where the ground sends commands from.

The Python mirrors of ``tcslibgs::gateway::GatewayKind::Yamcs``'s base
attribute, of ``tcslibgs::graph::telemetry::apids``, and of
``tcslibgs::types::came_from`` and ``sender_of``: the two languages must
agree about which datagrams are the ground's, and what each carries.
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
vehicle:
  - type ping:
      - compute: p
        payload-out:
          - target: m
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


def yamcs(directory, base="0x300", gateway="yamcs"):
    """Writes :data:`YAMCS` and reads it.

    Args:
        directory: Where to write it.
        base:      Its ``payload_telemetry.apid_base``.
        gateway:   Its ``tcspecial.gateway``.

    Returns:
        The :class:`configuration.Configuration`.
    """
    path = Path(directory) / "vehicle.ytcs"
    path.write_text(YAMCS.format(interface=interface.of_this_tree().hex(),
                                 base=base)
                    .replace("gateway: yamcs", f"gateway: {gateway}"))
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


if __name__ == "__main__":
    unittest.main()
