"""The ground's end of a cFS software bus: a ground station's SBN peer.

The Python spelling of ``tcslib_moc::bus::Ground`` in ``tcslib-moc/src/bus.rs``.
A vehicle that answers to cFS has no command socket, no beacon socket and no
telemetry sockets: everything it takes and sends crosses one SBN peer as cFS
packets told apart by message ID. The far end a station joins is the cFS end as
tcssim simulates it, ``tcslib_moc::bus::SimulatedCfs``, at ``sbn.peer``; SBN on
a real cFS sends no peer what another published, so a station is a peer of the
simulated cFS end and of nothing else.

:class:`Ground` is a station's one peer of the bus, kept on a thread of its
own: it announces once a second until it is heard, then sends SBN's protocol
version and subscribes to every message ID the vehicle publishes, sends a
heartbeat after five seconds of sending nothing, disconnects after ten of
silence, and refuses a far end of another protocol version or ident. It
turns the bus back into the UDP links a station already speaks: a loopback
socket the spacecraft's commands are sent to and their answers come from,
and a loopback socket of its own for each reader of telemetry, made by
:meth:`Ground.receiving`. So :mod:`tcslib_moc.beacon` and
:mod:`tcslib_moc.listening` work on the bus unchanged.

Only the commands a Python station sends go over it: the fixed ones,
CONNECT, RESTART_ARM and RESTART, on ``tcs_cmd.cmd_msgid``, each published
with its opcode, its first byte, as its function code.

The wire and the packets are :mod:`tcslib_protocol.sbn` and
:mod:`tcslib_protocol.cfs`; nothing here states either again.
"""

import select
import socket
import sys
import threading
import time

from tcslib_protocol import cfs, sbn
from tcslib_protocol.configuration import came_from

#: The longest datagram read: the longest UDP datagram.
ROOM = 65536

#: The longest a wait lasts before the clock is looked at again, in seconds.
TICK = 0.1


class Liveness:
    """When a peer last sent and last heard, and whether it is connected:
    ``tcslib_protocol::sbn::Liveness``, asked with the time so that it can be
    tested without waiting.

    Args:
        now: The time, in seconds, by a monotonic clock.
    """

    def __init__(self, now):
        self.connected = False
        self.sent_at = now
        self.heard_at = now
        self.announced_at = None

    def heard(self, now):
        """Something arrived from the far end, which connects a peer that was
        not.

        Args:
            now: When it arrived.

        Returns:
            Whether this is what connected it.
        """
        self.heard_at = now
        was = self.connected
        self.connected = True
        return not was

    def sent(self, now):
        """Something was sent to the far end.

        Args:
            now: When it was sent.
        """
        self.sent_at = max(self.sent_at, now)

    def announced(self, now):
        """An announcement was sent.

        Args:
            now: When it was sent.
        """
        self.announced_at = now
        self.sent(now)

    def disconnect(self):
        """The peer is disconnected, and is announced to again at once."""
        self.connected = False
        self.announced_at = None

    def due(self, now):
        """What is due now.

        Args:
            now: The time.

        Returns:
            One of ``sbn.ANNOUNCE``, ``sbn.HEARTBEAT``, ``sbn.DISCONNECT``,
            or ``None`` for nothing; silence before a heartbeat, where both
            are.
        """
        if not self.connected:
            if (self.announced_at is not None
                    and now - self.announced_at < sbn.ANNOUNCE_EVERY):
                return None
            return sbn.ANNOUNCE
        if now - self.heard_at >= sbn.SILENCE:
            return sbn.DISCONNECT
        if now - self.sent_at >= sbn.HEARTBEAT_AFTER:
            return sbn.HEARTBEAT
        return None

    def until_due(self, now):
        """How long until something is next due, in seconds, nought where
        something is due already.

        Args:
            now: The time.

        Returns:
            The length of time.
        """
        if not self.connected:
            if self.announced_at is None:
                return 0.0
            return max(0.0, self.announced_at + sbn.ANNOUNCE_EVERY - now)
        silence = self.heard_at + sbn.SILENCE - now
        heartbeat = self.sent_at + sbn.HEARTBEAT_AFTER - now
        return max(0.0, min(silence, heartbeat))


def _loopback():
    """A UDP socket on loopback, at a port the system chooses."""
    held = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    held.bind(("127.0.0.1", 0))
    return held


class Ground:
    """A ground station's peer of a cFS software bus, kept on a daemon
    thread: see the module's documentation.

    Args:
        bus:      The configuration's
                  :class:`tcslib_protocol.configuration.Bus`.
        msgids:   Every message ID the vehicle publishes, which it
                  subscribes to: :meth:`Configuration.telemetry_msgids`.
        say:      Where a line about the peer's coming and going or a
                  refusal is written: standard error by default.

    Raises:
        OSError: A socket cannot be made or bound.
    """

    def __init__(self, bus, msgids, say=None):
        self._bus = bus
        self._msgids = list(msgids)
        self._say = say or (lambda line: print(line, file=sys.stderr))
        self._peer = (bus.sbn.peer.address, bus.sbn.peer.port)
        self._us = (bus.sbn.ground_processor_id(), bus.sbn.spacecraft_id)
        self._socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._socket.bind(("0.0.0.0", 0))
        self._spacecraft = _loopback()
        self._delivering = _loopback()
        self._readers = {}
        self._lock = threading.Lock()
        self._client = None
        self._sequence = 0
        self._verified = threading.Event()
        self._stop = threading.Event()
        self._liveness = Liveness(time.monotonic())
        self._thread = threading.Thread(target=self._run, daemon=True,
                                        name="sbn-ground")
        self._thread.start()

    def spacecraft(self):
        """Where the spacecraft's commands are sent, and its answers come
        from, as ``(address, port)``."""
        return self._spacecraft.getsockname()

    def delivering(self):
        """Where what a reader asked for comes from, as ``(address,
        port)``."""
        return self._delivering.getsockname()

    def is_verified(self):
        """Whether the far end is connected and has said its protocol
        version is the release's."""
        return self._verified.is_set()

    def wait_verified(self, timeout):
        """Waits for the far end to be connected and verified.

        Args:
            timeout: The longest to wait, in seconds.

        Returns:
            Whether it is.
        """
        return self._verified.wait(timeout)

    def receiving(self, msgid):
        """A socket of the reader's own, to which what the vehicle publishes
        under ``msgid`` is sent -- what follows the cFS packet's headers,
        whole -- from :meth:`delivering`.

        Args:
            msgid: The message ID the reader wants.

        Returns:
            The socket, on loopback.

        Raises:
            OSError: It cannot be bound.
        """
        held = _loopback()
        with self._lock:
            self._readers.setdefault(msgid, []).append(held.getsockname())
        return held

    def close(self):
        """Stops the peer, telling the far end it is going, and closes its
        sockets."""
        self._stop.set()
        self._thread.join()
        self._send(sbn.DISCONNECT)
        for held in (self._socket, self._spacecraft, self._delivering):
            held.close()

    def _run(self):
        """Keeps the peer until told to stop."""
        while not self._stop.is_set():
            self._tick(time.monotonic())
            wait = min(self._liveness.until_due(time.monotonic()), TICK)
            try:
                ready, _, _ = select.select(
                    [self._socket, self._spacecraft], [], [], wait)
            except (OSError, ValueError):
                return
            if self._socket in ready:
                self._from_the_bus(time.monotonic())
            if self._spacecraft in ready:
                self._command()

    def _tick(self, now):
        """Does what the clock says is due.

        Args:
            now: The time.
        """
        due = self._liveness.due(now)
        if due == sbn.ANNOUNCE:
            self._send(sbn.ANNOUNCE)
            self._liveness.announced(now)
        elif due == sbn.HEARTBEAT:
            self._send(sbn.HEARTBEAT)
        elif due == sbn.DISCONNECT:
            self._say(
                f"tcslib_moc: the SBN peer at {self._bus.sbn.peer} has been "
                f"silent for {sbn.SILENCE:g} s: disconnected, and announcing "
                "to it again"
            )
            self._disconnect()

    def _from_the_bus(self, now):
        """Takes what the far end sent.

        Args:
            now: When it arrived.
        """
        try:
            datagram, sender = self._socket.recvfrom(ROOM)
        except OSError:
            return
        if not came_from(self._bus.sbn.peer, sender):
            return
        try:
            header, message = sbn.Header.read(datagram)
        except ValueError:
            return
        if (header.processor_id != self._bus.sbn.peer_processor_id
                or header.spacecraft_id != self._bus.sbn.spacecraft_id):
            return
        release = self._bus.sbn.release
        if self._liveness.heard(now):
            self._say(
                f"tcslib_moc: the SBN peer at {self._bus.sbn.peer} is "
                f"connected, as processor {self._us[0]} of spacecraft "
                f"{self._us[1]}"
            )
            self._send(sbn.PROTOCOL, bytes([release.protocol_version]))
            self._send(sbn.SUBSCRIBE, sbn.write_subscriptions(
                self._bus.sbn.ident, self._msgids))
        if header.kind == sbn.PROTOCOL:
            if message == bytes([release.protocol_version]):
                self._verified.set()
            else:
                self._say(
                    f"tcslib_moc: the SBN peer at {self._bus.sbn.peer} says "
                    f"its protocol version is {list(message)}, and SBN "
                    f"{release.name} speaks {release.protocol_version}: "
                    "refused"
                )
                self._refuse()
        elif header.kind in (sbn.SUBSCRIBE, sbn.UNSUBSCRIBE):
            try:
                sbn.read_subscriptions(message, self._bus.sbn.ident)
            except sbn.IdentRefused as refused:
                carried = refused.carried.rstrip(b"\0").decode(
                    "ascii", "replace")
                expected = self._bus.sbn.ident.rstrip(b"\0").decode("ascii")
                self._say(
                    f"tcslib_moc: the SBN peer at {self._bus.sbn.peer} "
                    f'subscribed with the ident "{carried}", and sbn.ident '
                    f'says "{expected}": refused'
                )
                self._refuse()
            except ValueError:
                pass
        elif header.kind == sbn.APPLICATION:
            self._telemetry(message)
        elif header.kind == sbn.DISCONNECT:
            self._disconnect()

    def _telemetry(self, packet):
        """Sends a telemetry packet's data on to whoever it is for.

        Args:
            packet: The cFS packet, after SBN's header.
        """
        if not self._verified.is_set():
            return
        try:
            frame = cfs.read_telemetry(packet)
        except ValueError:
            return
        # A send that fails is a reader that has gone, which is the
        # reader's to notice; the bus goes on.
        try:
            if frame.msgid == self._bus.response_msgid:
                if self._client is not None:
                    self._spacecraft.sendto(frame.data, self._client)
                return
            with self._lock:
                readers = list(self._readers.get(frame.msgid, ()))
            for to in readers:
                self._delivering.sendto(frame.data, to)
        except OSError:
            pass

    def _command(self):
        """Publishes a command a station sent to the spacecraft's link."""
        try:
            datagram, sender = self._spacecraft.recvfrom(ROOM)
        except OSError:
            return
        self._client = sender
        if not self._verified.is_set():
            self._say(
                "tcslib_moc: a command was not sent: the SBN peer at "
                f"{self._bus.sbn.peer} is not connected"
            )
            return
        if not datagram:
            return
        packet = cfs.write_command(
            self._bus.cmd_msgid, self._sequence, self._bus.sbn.spacecraft_id,
            datagram[0], datagram,
        )
        self._sequence = (self._sequence + 1) % 0x4000
        self._send(sbn.APPLICATION, packet)

    def _refuse(self):
        """Refuses the far end: disconnects, and then tells it so, so that
        nothing is taken as verified once the far end has heard it."""
        self._disconnect()
        self._send(sbn.DISCONNECT)

    def _disconnect(self):
        """Disconnects on this side, and announces again."""
        self._verified.clear()
        self._liveness.disconnect()

    def _send(self, kind, message=b""):
        """Sends a message of ``kind`` to the far end.

        Args:
            kind:    The SBN message type.
            message: What follows SBN's header.
        """
        datagram = sbn.Header(kind, *self._us).write(message)
        try:
            self._socket.sendto(datagram, self._peer)
        except OSError:
            return
        self._liveness.sent(time.monotonic())


#: The peers this process has started, by the bus each is a peer of.
_GROUNDS = {}
_GROUNDS_LOCK = threading.Lock()


def ground_of(config):
    """This process's peer of the bus a configuration answers over, started
    the first time it is asked for: one peer for each bus, as
    ``Ground::of`` keeps.

    Args:
        config: The :class:`tcslib_protocol.configuration.Configuration`.

    Returns:
        The :class:`Ground`, or ``None`` for a configuration that does not
        answer over a bus.

    Raises:
        ValueError: The bus or its message IDs do not read.
        OSError:    A socket cannot be bound.
    """
    bus = config.bus()
    if bus is None:
        return None
    with _GROUNDS_LOCK:
        ground = _GROUNDS.get(bus)
        if ground is None:
            ground = Ground(bus, config.telemetry_msgids())
            _GROUNDS[bus] = ground
        return ground
