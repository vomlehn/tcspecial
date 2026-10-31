"""A station that only listens sends its own CONNECT until it is matched.

This test is the vehicle: it takes each CONNECT where the configuration says
the vehicle takes commands, leaves the first unanswered, answers the second
with another configuration's checksum and the third with the station's own.
"""

import socket
import sys
import threading
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslib.listening import Listening  # noqa: E402
from tcslibgs import fixed_packet  # noqa: E402
from tcslibgs.configuration import CommandLink, HostPort  # noqa: E402


def a_socket():
    """A UDP socket on a loopback port nothing holds."""
    held = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    held.bind(("127.0.0.1", 0))
    return held


class ListeningTest(unittest.TestCase):
    def test_connect_goes_again_until_the_vehicle_answers_with_ours(self):
        vehicle = a_socket()
        vehicle.settimeout(10)
        free = a_socket()
        station_port = free.getsockname()[1]
        free.close()
        link = CommandLink(
            send=HostPort("127.0.0.1", station_port),
            recv=HostPort("127.0.0.1", vehicle.getsockname()[1]),
        )
        ours = bytes(range(16))
        asked = []

        def answer():
            """Be the vehicle: take three CONNECTs, answering the second
            with another checksum and the third with ours."""
            for running in (None, bytes(16), ours):
                datagram, sender = vehicle.recvfrom(256)
                command = fixed_packet.FixedCommandKind.decode(datagram)
                asked.append(bytes(command.checksum))
                if running is None:
                    continue
                reply = fixed_packet.ConnectAnswer(
                    sequence=command.sequence, status=0, timestamp=0,
                    running=running, major=0, minor=1, patch=0,
                    newest_taken=command.sequence,
                )
                vehicle.sendto(reply.to_bytes(), sender)

        vehicle_thread = threading.Thread(target=answer)
        vehicle_thread.start()
        listening = Listening(link, ours)
        heard = []
        try:
            matched = listening.until_matched(heard.append, wait=0.3)
        finally:
            listening.close()
            vehicle_thread.join()
            vehicle.close()
        self.assertTrue(matched.matched)
        self.assertEqual(matched.version, "0.1.0")
        self.assertEqual(asked, [ours, ours, ours], "it carries ours")
        self.assertEqual([h.matched for h in heard], [False, False, True])
        self.assertIsNone(heard[0].running, "no answer")
        self.assertEqual(heard[1].running, bytes(16), "another checksum")
        self.assertIn("goes again", heard[1].said(ours))
        self.assertIn("matched", heard[2].said(ours))


if __name__ == "__main__":
    unittest.main()
