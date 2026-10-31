"""The interface: the XTCE documents every packet on the link is stated in.

The Python spelling of ``tcslibgs/src/interface.rs``. Two programs speak the
same interface when the tcs-xtce digests of their documents are the same, and
tcs-xtce computes that digest alike in Python and Rust, so a ground station
in Python can compare its copy of the documents with the digest every beacon
carries.
"""

from pathlib import Path

# tcs-xtce's digest, vendored in this tree until it is published: see
# _xtce/__init__.py.
from ._xtce import digest as _digest

#: Where this tree's interface is stated: ``link.xml``, as ``make
#: interface-digest`` in tcslibgs/ takes it. ``fixed.xml`` beside it holds the
#: three fixed commands, which are not part of the interface: no change to the
#: interface can touch them. Beside this module rather than found in the
#: source tree, so that an installed package has them too: ``xtce`` here is a
#: link to the crate's ``tcslibgs/xtce``, which is the one copy, and installing
#: the package copies what it links to.
DOCUMENTS = Path(__file__).resolve().parent / "xtce"
INTERFACE = (DOCUMENTS / "link.xml",)


def of_files(paths):
    """The interface the XTCE documents at ``paths`` state, as one set: its
    sixteen-byte digest. The order they are given in does not matter."""
    return _digest.digest_set_bytes(Path(path).read_bytes() for path in paths)


def of_this_tree():
    """The interface this tree's documents state: what a tcspecial built
    from this tree speaks."""
    return of_files(INTERFACE)


def of_hex(said):
    """An interface digest as a configuration writes it: 32 lower-case hex
    digits, and nothing else.

    Args:
        said: The digits.

    Returns:
        The sixteen-byte digest, or ``None`` for anything that is not 32
        lower-case hex digits.
    """
    if len(said) != 32 or any(c not in "0123456789abcdef" for c in said):
        return None
    return bytes.fromhex(said)


def of_configuration(said):
    """The interface a configuration's ``tcspecial`` section names, which
    must be this tree's: the Python spelling of
    ``tcslibgs::graph::parameters::interface_of``, in the same words where
    it is stated.

    Args:
        said: The section's ``interface``, or ``None`` where it states none.

    Returns:
        The sixteen-byte digest, which is this tree's.

    Raises:
        ValueError: It is not stated, is not 32 lower-case hex digits, or is
                    another interface's.
    """
    ours = of_this_tree()
    if not said:
        raise ValueError(
            "interface is not stated, and it has no default: write the 32 "
            "lower-case hex digits `tcsverify --interface-digest` prints, "
            f"which for this tree are {ours.hex()}"
        )
    named = of_hex(said)
    if named is None:
        raise ValueError(
            f'interface is "{said}", which is not an interface digest: write '
            "the 32 lower-case hex digits `tcsverify --interface-digest` "
            f"prints, which for this tree are {ours.hex()}"
        )
    if named != ours:
        raise ValueError(
            f"interface is {said}, and this build speaks interface "
            f"{ours.hex()}, the digest of the tcslibgs/xtce/link.xml it was "
            f"built from: fly this configuration with a build of interface "
            f"{said}, or, once its commands have been checked against this "
            f'build\'s, write interface: "{ours.hex()}"'
        )
    return named


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
