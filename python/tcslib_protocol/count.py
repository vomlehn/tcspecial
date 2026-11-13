"""The count a string or bytes is sent the ground after:
``tcslib_protocol::link_type::Count``.

A string or bytes has no width of its own, so tcspecial sends it the ground
after a count of its bytes. Where the ground decodes what it is sent by
XTCE -- under Yamcs -- the count is 32 bits, most significant first, as
``types.xtce`` states it. Everywhere else it is in groups of seven bits,
least significant first, each in a byte whose top bit says another follows
(unsigned LEB128), in as few bytes as hold it: one for a count below 128.
A ground station reading a link's datagrams itself takes the count off
with :func:`uncounted`; one reading through tcslib_moc's ``GatewayReceiver``
is given the datum without it.
"""

#: The bytes of the count where XTCE encodes it.
XTCE_BYTES = 4

#: The most bytes a count in groups of seven bits takes, for the largest
#: datum there is, 4096 bytes: ``tcslib_protocol::types::DATUM_BYTES``.
MOST_GROUP_BYTES = 2


def counted(data, xtce=False):
    """Writes a string or bytes as it is sent the ground: the count of its
    bytes, and them. For what stands in for the vehicle in a test.

    Args:
        data: The datum.
        xtce: Whether to write the count as XTCE encodes it, rather than in
              groups of seven bits.

    Returns:
        The count and the datum.
    """
    if xtce:
        return len(data).to_bytes(XTCE_BYTES, "big") + bytes(data)
    count, groups = len(data), bytearray()
    while True:
        group = count & 0x7F
        count >>= 7
        if count:
            groups.append(group | 0x80)
        else:
            groups.append(group)
            return bytes(groups) + bytes(data)


def uncounted(packet, xtce=False):
    """Reads a string or bytes as the ground is sent it.

    Args:
        packet: What arrived.
        xtce:   Whether its count is as XTCE encodes it, rather than in
                groups of seven bits.

    Returns:
        The datum, without its count.

    Raises:
        ValueError: There is no count, it is written in more bytes than it
                    needs, or it does not say how many bytes follow it.
    """
    if xtce:
        if len(packet) < XTCE_BYTES:
            raise ValueError(f"{len(packet)} bytes arrived, and a string or "
                             f"bytes is sent after a {XTCE_BYTES}-byte "
                             "count")
        count, start = int.from_bytes(packet[:XTCE_BYTES], "big"), XTCE_BYTES
    else:
        count, shift, start = 0, 0, 0
        while True:
            if start == len(packet):
                raise ValueError("the count before a string or bytes ends "
                                 "before it does")
            byte = packet[start]
            count |= (byte & 0x7F) << shift
            shift += 7
            start += 1
            if not byte & 0x80:
                break
        if start > 1 and packet[start - 1] == 0:
            raise ValueError(f"the count {count} is written in {start} "
                             "bytes, and it is written in as few as hold it")
    follow = len(packet) - start
    if count != follow:
        raise ValueError(f"the count says {count} bytes and {follow} follow "
                         "it")
    return bytes(packet[start:])
