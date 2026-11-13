"""A station's peer of a cFS software bus, against a far end played here.

The test is the simulated cFS end -- a UDP socket speaking SBN's wire
format, as ``tcslibgs.sbn`` and ``tcslibgs.cfs`` state it -- with the
vehicle behind it: it answers the station's announcement, takes its
subscriptions, publishes a beacon and answers a CONNECT, as
``tcslib-moc/src/bus.rs``'s tests play the peers of the Rust.
"""

import socket
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslib import beacon, bus  # noqa: E402
from tcslib.beacon import Arrived, Schedule  # noqa: E402
from tcslib.bus import Liveness  # noqa: E402
from tcslib.listening import Listening, command_link_of  # noqa: E402
from tcslibgs import cfs, configuration, fixed_packet, sbn  # noqa: E402
from tcslibgs.beacon import Beacon  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
HASH = "3b339a5e0f686cef822a5d6ea95004222ab7af33"

#: A beacon, as the vehicle sends one.
BEACON = Beacon(sequence=1, version=(0, 1, 0), digest=bytes(16),
                restart_flag=True).to_bytes()


def a_socket():
    """A UDP socket on a loopback port nothing holds.

    Returns:
        The socket, whose reads give up after a tenth of a second.
    """
    held = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    held.bind(("127.0.0.1", 0))
    held.settimeout(0.1)
    return held


def example(peer_port):
    """The cfs example, its far end moved to ``peer_port``.

    Args:
        peer_port: Where the played far end is bound.

    Returns:
        A pair: the directory it is in, and its configuration.
    """
    text = (ROOT / "examples" / "cfs" / "cfs.ytcs").read_text()
    text = text.replace("127.0.0.1:2234", f"127.0.0.1:{peer_port}")
    directory = tempfile.TemporaryDirectory()
    path = Path(directory.name) / "cfs.ytcs"
    path.write_text(text)
    return directory, configuration.load(path)


class FarEnd:
    """The far end of the bus, played by the test: processor 1 of
    spacecraft 0x42.

    Args:
        hash_: The hash its subscriptions' ident is made from.
    """

    def __init__(self, hash_=HASH):
        self.socket = a_socket()
        self.ident = sbn.ident_of(hash_)
        self.station = None

    def say(self, kind, message=b""):
        """Sends the station a message of ``kind``."""
        self.socket.sendto(sbn.Header(kind, 1, 0x42).write(message),
                           self.station)

    def until(self, kind, within=5.0):
        """Waits for the station to send a message of ``kind``, passing over
        any other, and learns where it is.

        Args:
            kind:   The SBN message type waited for.
            within: How long to wait, in seconds.

        Returns:
            The message, after SBN's header, or ``None`` where none came.
        """
        deadline = time.monotonic() + within
        while time.monotonic() < deadline:
            try:
                datagram, self.station = self.socket.recvfrom(65536)
            except socket.timeout:
                continue
            header, message = sbn.Header.read(datagram)
            assert (header.processor_id, header.spacecraft_id) == (3, 0x42)
            if header.kind == kind:
                return message
        return None

    def join(self):
        """Hears the station's announcement and answers it as SBN does.

        Returns:
            The message IDs it subscribed to.
        """
        assert self.until(sbn.ANNOUNCE) == b""
        self.say(sbn.PROTOCOL, bytes([11]))
        assert self.until(sbn.PROTOCOL) == bytes([11])
        subscribed = self.until(sbn.SUBSCRIBE)
        self.say(sbn.SUBSCRIBE,
                 sbn.write_subscriptions(self.ident, [0x4280]))
        return sbn.read_subscriptions(subscribed, sbn.ident_of(HASH))


class LivenessTest(unittest.TestCase):
    def test_a_peer_announces_heartbeats_and_disconnects_on_time(self):
        # sbn.rs's own times.
        peer = Liveness(0.0)
        self.assertEqual(peer.due(0.0), sbn.ANNOUNCE)
        peer.announced(0.0)
        self.assertIsNone(peer.due(0.5))
        self.assertAlmostEqual(peer.until_due(0.5), 0.5)
        self.assertEqual(peer.due(1.0), sbn.ANNOUNCE)
        peer.announced(1.0)
        self.assertTrue(peer.heard(1.5))
        self.assertFalse(peer.heard(1.5))
        peer.sent(1.5)
        self.assertIsNone(peer.due(6.4))
        self.assertEqual(peer.due(6.5), sbn.HEARTBEAT)
        peer.sent(6.5)
        peer.sent(11.0)
        self.assertIsNone(peer.due(11.4))
        self.assertEqual(peer.due(11.5), sbn.DISCONNECT)
        peer.disconnect()
        self.assertEqual(peer.due(11.5), sbn.ANNOUNCE)


class GroundTest(unittest.TestCase):
    def test_a_station_hears_beacons_and_connects_over_the_bus(self):
        far = FarEnd()
        self.addCleanup(far.socket.close)
        directory, config = example(far.socket.getsockname()[1])
        said = []
        with directory:
            ground = bus.Ground(config.bus(), config.telemetry_msgids(),
                                say=said.append)
            try:
                self.assertEqual(far.join(), config.telemetry_msgids())
                self.assertTrue(ground.wait_verified(5))

                # The beacon, through the beacon watch as a radio's is.
                watch = beacon.Beacons.on(
                    ground.receiving(0x4201),
                    configuration.BeaconLink(
                        send=configuration.HostPort(*ground.delivering()),
                        recv=configuration.HostPort("127.0.0.1", 0)),
                    Schedule.standard(), None)
                far.say(sbn.APPLICATION, telemetry(0x4201, BEACON))
                arrived = watch.next_pass()
                watch.close()
                self.assertEqual(arrived.arrived, Arrived.BEACON, arrived)

                # A CONNECT, published under tcs_cmd.cmd_msgid with its
                # opcode as its function code, and its answer back.
                ours = bytes(range(16))
                link = configuration.CommandLink(
                    send=configuration.HostPort("127.0.0.1", 0),
                    recv=configuration.HostPort(*ground.spacecraft()))
                listening = Listening(link, ours)
                answered = threading.Thread(target=self.answer,
                                            args=(far, ours))
                answered.start()
                try:
                    heard = listening.connect(wait=5)
                finally:
                    answered.join()
                    listening.close()
                self.assertTrue(heard.matched, heard.said(ours))
            finally:
                ground.close()
        self.assertTrue(any("connected" in line for line in said), said)

    def answer(self, far, ours):
        """Be the vehicle behind the far end: take a CONNECT and answer it
        with ``ours``.

        Args:
            far:  The far end the CONNECT arrives at.
            ours: The checksum the answer carries.
        """
        packet = far.until(sbn.APPLICATION)
        msgid, code, data = command_of(packet)
        assert (msgid, code) == (0x4280, 0x04), (hex(msgid), code)
        command = fixed_packet.FixedCommandKind.decode(data)
        reply = fixed_packet.ConnectAnswer(
            sequence=command.sequence, status=0, timestamp=0, running=ours,
            major=0, minor=1, patch=0, newest_taken=command.sequence,
        )
        far.say(sbn.APPLICATION, telemetry(0x4200, reply.to_bytes()))

    def test_a_far_end_of_another_ident_is_refused(self):
        far = FarEnd("1" * 40)
        self.addCleanup(far.socket.close)
        directory, config = example(far.socket.getsockname()[1])
        said = []
        with directory:
            ground = bus.Ground(config.bus(), config.telemetry_msgids(),
                                say=said.append)
            try:
                far.join()
                self.assertIsNotNone(far.until(sbn.DISCONNECT))
                self.assertFalse(ground.is_verified())
            finally:
                ground.close()
        self.assertTrue(any("refused" in line for line in said), said)

    def test_the_station_finds_the_bus_from_the_configuration(self):
        far = FarEnd()
        self.addCleanup(far.socket.close)
        directory, config = example(far.socket.getsockname()[1])
        with directory:
            ground = bus.ground_of(config)
            self.assertIs(bus.ground_of(config), ground, "one for each bus")
            link = command_link_of(config)
            self.assertEqual((link.recv.address, link.recv.port),
                             ground.spacecraft())
            watch = beacon.open(config, Schedule.standard(), None)
            self.assertEqual(
                (watch.link().send.address, watch.link().send.port),
                ground.delivering())
            watch.close()
        self.assertIsNone(bus.ground_of(configuration.load(
            ROOT / "examples" / "ping" / "ping.xtcs")))


def telemetry(msgid, data):
    """A telemetry packet as the vehicle publishes one, laid out by hand as
    ``tcslibgs/src/cfs.rs``'s ``write_telemetry`` lays it.

    Args:
        msgid: Its message ID.
        data:  What it carries.

    Returns:
        The packet.
    """
    total = cfs.TELEMETRY_HEADER_BYTES + len(data)
    head = cfs._base(msgid, 0, total, 0x42)
    return head + bytes([0, 0, 0, 1, 0, 0]) + data


def command_of(packet):
    """A command packet's message ID, function code and data, read as
    ``tcslibgs/src/cfs.rs``'s ``read_command`` reads them.

    Args:
        packet: The packet.

    Returns:
        The triple.
    """
    stream = int.from_bytes(packet[0:2], "big")
    subsystem = int.from_bytes(packet[6:8], "big")
    msgid = (stream & 0x7F) | ((subsystem & 0xFF) << 8) | 0x80
    whole = 0xFF
    for byte in packet:
        whole ^= byte
    assert whole == 0, "the checksum is right"
    return msgid, packet[10] & 0x7F, packet[cfs.COMMAND_HEADER_BYTES:]


if __name__ == "__main__":
    unittest.main()
