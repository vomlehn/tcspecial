"""SBN, the Software Bus Network, as a ground station speaks it: one peer of
a cFS software bus, over SBN's UDP protocol module.

The Python spelling of ``tcslib-protocol/src/sbn.rs``, held to it by the same
examples in ``tests/test_sbn.py`` as the Rust's own tests use. See "What SBN is,
on the wire" in ``docs/tcspecial-design.rst``, which says what in it was checked
against SBN v7.0.1 and how.

A datagram is thirteen bytes of header and then the message, every number
most significant byte first::

    length 4 | type 1 | processor ID 4 | spacecraft ID 4 | the message

and a subscription, or an unsubscription, is::

    ident 48 | count 2 | count x (message ID 4 | priority 1 | reliability 1)
"""

import struct
from dataclasses import dataclass

#: The length of a datagram's header.
HEADER_BYTES = 13

#: The length of ``SBN_IDENT``.
IDENT_BYTES = 48

#: What one subscription takes in a subscription message.
SUBSCRIPTION_BYTES = 6

#: A subscription: the message IDs the sender wants.
SUBSCRIBE = 1
#: An unsubscription: message IDs it no longer wants.
UNSUBSCRIBE = 2
#: An application's message: a cFS packet, carried whole.
APPLICATION = 3
#: SBN's protocol version, one byte.
PROTOCOL = 4
#: The UDP module's heartbeat, which carries nothing.
HEARTBEAT = 0xA0
#: Its announcement, which carries nothing.
ANNOUNCE = 0xA1
#: Its disconnect, which carries nothing.
DISCONNECT = 0xA2

#: How long a peer may go without sending before it sends a heartbeat, in
#: seconds.
HEARTBEAT_AFTER = 5.0

#: How long a peer may be silent before it is disconnected, in seconds.
SILENCE = 10.0

#: How often a peer not yet connected is announced to, in seconds.
ANNOUNCE_EVERY = 1.0


@dataclass(frozen=True)
class Release:
    """An SBN release this library speaks, and its ``SBN_PROTO_VER``."""

    name: str
    protocol_version: int


#: Every release spoken: the release decides the layout of every message.
RELEASES = (Release("v7.0.1", 11),)


def release_of(said):
    """Reads ``sbn.release``.

    Args:
        said: The release, as the file wrote it.

    Returns:
        The :class:`Release`.

    Raises:
        ValueError: It is not a release spoken here, naming the ones that
                    are.
    """
    said = said.strip()
    for release in RELEASES:
        if release.name == said:
            return release
    spoken = ", ".join(r.name for r in RELEASES)
    raise ValueError(
        f'sbn.release is "{said}", which this library does not speak: the '
        f"release decides the layout of every message, and it speaks {spoken}"
    )


@dataclass(frozen=True)
class Header:
    """A datagram's header: its message type and who sent it."""

    kind: int
    processor_id: int
    spacecraft_id: int

    def write(self, message):
        """This header and then ``message``, as one datagram.

        Args:
            message: What follows the header.

        Returns:
            The datagram, as bytes.
        """
        return struct.pack(">IBII", len(message), self.kind,
                           self.processor_id, self.spacecraft_id) + message

    @staticmethod
    def read(datagram):
        """Reads a datagram's header, refusing one whose length is not what
        follows it.

        Args:
            datagram: The whole datagram.

        Returns:
            A pair: the :class:`Header`, and the message after it.

        Raises:
            ValueError: It is shorter than a header, or its length is not
                        what follows it.
        """
        if len(datagram) < HEADER_BYTES:
            raise ValueError("shorter than an SBN header")
        length, kind, processor, spacecraft = struct.unpack(
            ">IBII", datagram[:HEADER_BYTES])
        if length != len(datagram) - HEADER_BYTES:
            raise ValueError("of a length its SBN header does not say")
        return Header(kind, processor, spacecraft), datagram[HEADER_BYTES:]


def ident_of(said):
    """``SBN_IDENT`` from the hash ``sbn.ident`` states: git's ``$Id$`` of
    SBN's ``sbn_types.h``, with the C string's terminating nought.

    Args:
        said: ``sbn.ident``, as the file wrote it.

    Returns:
        The 48 bytes.

    Raises:
        ValueError: It is not 40 hexadecimal digits.
    """
    said = said.strip()
    if len(said) != 40 or any(c not in "0123456789abcdefABCDEF"
                              for c in said):
        raise ValueError(
            f'sbn.ident is "{said}", which is not the 40 hexadecimal digits '
            "of the far end's SBN_IDENT: copy the hash SBN prints in its "
            "start-up event on the cFS side"
        )
    return b"$Id: " + said.lower().encode("ascii") + b" $\0"


def write_subscriptions(ident, msgids):
    """A subscription or unsubscription message, every message ID at
    priority and reliability nought.

    Args:
        ident:  The far end's ``SBN_IDENT``, 48 bytes.
        msgids: The message IDs.

    Returns:
        The message.

    Raises:
        ValueError: There are more message IDs than a count of two bytes
                    holds.
    """
    msgids = list(msgids)
    if len(msgids) > 0xFFFF:
        raise ValueError("more message IDs than one subscription holds")
    return (ident + struct.pack(">H", len(msgids))
            + b"".join(struct.pack(">IBB", m, 0, 0) for m in msgids))


class IdentRefused(ValueError):
    """A subscription opened with another ident: the far end was built from
    another SBN.

    Args:
        carried: The 48 bytes it carried.
    """

    def __init__(self, carried):
        super().__init__("a subscription with another ident")
        self.carried = carried


def read_subscriptions(message, ident):
    """Reads a subscription or unsubscription message, checking its ident.

    Args:
        message: The message, after the datagram's header.
        ident:   The ident it must open with.

    Returns:
        Its message IDs, in the order it lists them; one wider than sixteen
        bits as ``None``, which matches nothing.

    Raises:
        IdentRefused: It opens with another ident.
        ValueError:   It is not laid out as a subscription is.
    """
    if len(message) < IDENT_BYTES + 2:
        raise ValueError("a subscription shorter than its ident and count")
    if message[:IDENT_BYTES] != ident:
        raise IdentRefused(bytes(message[:IDENT_BYTES]))
    (count,) = struct.unpack(">H", message[IDENT_BYTES:IDENT_BYTES + 2])
    entries = message[IDENT_BYTES + 2:]
    if len(entries) != count * SUBSCRIPTION_BYTES:
        raise ValueError("a subscription whose count is not how many it lists")
    out = []
    for at in range(0, len(entries), SUBSCRIPTION_BYTES):
        (msgid,) = struct.unpack(">I", entries[at:at + 4])
        out.append(msgid if msgid <= 0xFFFF else None)
    return out


def ground_processor_id(processor_id, peer_processor_id):
    """The processor ID a ground station joins the simulated cFS end as:
    one more than the larger of the vehicle's and the far end's.

    Only the simulated cFS end takes a ground station as a peer: SBN on a
    real cFS sends no peer what another published.

    Args:
        processor_id:      ``sbn.processor_id``, the vehicle's.
        peer_processor_id: ``sbn.peer_processor_id``, the far end's.

    Returns:
        The ground station's processor ID, which is neither.
    """
    return min(max(processor_id, peer_processor_id) + 1, 0xFFFFFFFF)
