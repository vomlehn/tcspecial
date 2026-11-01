"""Tests of ping_yamcs.py that need neither Yamcs nor a vehicle: what it
reads from the configuration, the Yamcs configuration it writes, and its ping
link driven through a Yamcs played here.

Run from the repository's root::

    python3 -m pytest examples/ping-yamcs/test_ping_yamcs.py
"""

import re
import sys
import types
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import ping_yamcs  # noqa: E402
from tcslibgs import ping  # noqa: E402


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
        ping_yamcs.write_yamcs(setup, self.run, 16090, self.home)
        for written in ("etc/yamcs.yaml", "etc/yamcs.ping.yaml",
                        "tcs-ccsds.xtce", "etc/processor.yaml"):
            text = (self.run / written).read_text()
            self.assertIsNone(re.search(r"\$\w", text), written)

    def test_the_wrapper_carries_the_configurations_apids(self):
        setup = ping_yamcs.Setup(options())
        ping_yamcs.write_yamcs(setup, self.run, 16090, self.home)
        wrapper = (self.run / "tcs-ccsds.xtce").read_text()
        for apid in (0x100, 0x101, 0x200, 0x201):
            self.assertIn(f'"{apid}"', wrapper)


class YamcsValueTest(unittest.TestCase):
    """Reading what Yamcs's REST API says."""

    def test_reception_times_are_read_to_the_millisecond(self):
        self.assertEqual(ping_yamcs.reception_ns("1970-01-01T00:00:01.250Z"),
                         1_250_000_000)
        self.assertEqual(ping_yamcs.reception_ns(None), 0)

    def test_values_are_read_by_their_type(self):
        self.assertEqual(
            ping_yamcs.eng_value({"type": "BINARY", "binaryValue": "AAE="}),
            b"\x00\x01")
        self.assertEqual(ping_yamcs.eng_value({"type": "STRING",
                                               "stringValue": "x"}), "x")
        self.assertEqual(ping_yamcs.eng_value({"type": "UINT32",
                                               "uint32Value": 4}), 4)


class FakeYamcs:
    """A Yamcs that answers each PingText as a vehicle would, a millisecond
    later by its reception time.

    Args:
        answer: Whether it answers at all.
    """

    def __init__(self, answer=True):
        self.answer = answer
        self.issued = []
        self.reply = None

    def issue(self, command, args):
        """Takes a command; a ping is answered with the vehicle's time."""
        self.issued.append((command, args))
        if command == ping_yamcs.PING_TEXT and self.answer:
            sent = ping.read_message(args["Text"].encode()).sent
            text = ping.reply(args["Text"].encode(), sent + 500_000)
            self.reply = (text.decode(), sent + 1_000_000)

    def parameter(self, name):
        """The latest answer, or ``None``."""
        return self.reply if name == ping_yamcs.PING_REPLY else None


class YamcsPingerTest(unittest.TestCase):
    """The ping link through Yamcs."""

    def test_a_ping_goes_as_text_with_its_length_and_is_timed(self):
        yamcs = FakeYamcs()
        pinger = ping_yamcs.YamcsPinger(yamcs, clock=lambda: 10**18)
        sent = pinger.ping()
        command, args = yamcs.issued[-1]
        self.assertEqual(command, ping_yamcs.PING_TEXT)
        self.assertEqual(args, {"Text": sent, "TextBits": 8 * len(sent)})
        self.assertTrue(pinger.receive(0.1))
        exchange = pinger.exchange()
        self.assertEqual(exchange.round_trip(), 1_000_000)
        self.assertEqual(exchange.up(), 500_000)
        self.assertEqual(exchange.down(), 500_000)

    def test_no_answer_is_no_answer(self):
        pinger = ping_yamcs.YamcsPinger(FakeYamcs(answer=False),
                                        clock=lambda: 10**18)
        pinger.ping()
        self.assertFalse(pinger.receive(0.05))
        self.assertIsNone(pinger.exchange().received_at)


if __name__ == "__main__":
    unittest.main()
