"""The interface: the XTCE documents every packet on the link is stated in.

The Python spelling of ``tcslibgs/src/interface.rs``. Two programs speak the
same interface when the tcs-xtce digests of their documents are the same, and
tcs-xtce computes that digest alike in Python and Rust, so a ground station
in Python can compare its copy of the documents with the digest every beacon
carries.
"""

import sys
from pathlib import Path

# tcs-xtce's Python runtime, vendored in this tree until tcs-xtce is
# published: see vendor/tcs-xtce/README.md.
_XTCE = Path(__file__).resolve().parents[2] / "vendor" / "tcs-xtce" / "python"
if str(_XTCE) not in sys.path:
    sys.path.insert(0, str(_XTCE))

from xtce import digest as _digest  # noqa: E402

#: Where this tree's interface is stated: every XTCE document in it is part
#: of the one interface, as ``make interface-digest`` in tcslibgs/ takes them.
DOCUMENTS = Path(__file__).resolve().parents[2] / "tcslibgs" / "xtce"


def of_files(paths):
    """The interface the XTCE documents at ``paths`` state, as one set: its
    sixteen-byte digest. The order they are given in does not matter."""
    return _digest.digest_set_bytes(Path(path).read_bytes() for path in paths)


def of_this_tree():
    """The interface this tree's documents state: what a tcspecial built
    from this tree speaks."""
    return of_files(sorted(DOCUMENTS.glob("*.xml")))


def differs(theirs, ours):
    """Why ``theirs`` is not ``ours``: the same words as the Rust's.

    Args:
        theirs: The interface the vehicle speaks, as its sixteen-byte digest.
        ours:   The interface this station read, as its sixteen-byte digest.

    Returns:
        ``None`` when they are the same, else a sentence saying how they
        differ and what to do.
    """
    if theirs == ours:
        return None
    return (
        f"the vehicle speaks interface {theirs.hex()} and this station read "
        f"{ours.hex()}: give this station the XTCE documents the vehicle was "
        "built from, or upload the station's to the vehicle"
    )
