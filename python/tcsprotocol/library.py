"""libtcsmoc-c, loaded with ctypes: the Rust a Python ground station calls
rather than says again.

What a ground station works out of a configuration -- each node's device
id, the ports, APIDs and message IDs of its links, each consolidation's
layout, and the digests a beacon is compared with -- is worked out by
tcsprotocol, once, and handed to Python as tcs-verify --links would print
it, through the C library every ground station may be built on. So the
rules are stated in one place, and a Python station runs on operational code
alone: tcs-verify is a software tool, and a station does not run one.

The library is this tree's build where there is one, and otherwise where it
is installed. Each function is given its types once, when the library is
first loaded.
"""

import ctypes
import ctypes.util

from tcsprotocol.configuration import find_built

#: The library's file in this tree's build.
BUILT = "libtcsmoc_c.so"

#: The library's name, where it is installed.
INSTALLED = "tcsmoc-c"

#: The library, once loaded.
_loaded = None


def library():
    """Loads libtcsmoc-c: this tree's build first, then where it is
    installed.

    Returns:
        The library, its functions given their types.

    Raises:
        OSError: It is not built or installed.
    """
    global _loaded
    if _loaded is not None:
        return _loaded
    path = find_built(BUILT) or ctypes.util.find_library(INSTALLED)
    if path is None:
        raise OSError(
            "libtcsmoc-c, which a Python ground station reads its "
            "configuration's links through, is not built or installed: "
            "build it with `cargo build -p tcsmoc-c`")
    lib = ctypes.CDLL(path)
    lib.tcs_last_error.restype = ctypes.c_char_p
    lib.tcs_last_error.argtypes = []
    lib.tcs_links_of_file.restype = ctypes.c_int64
    lib.tcs_links_of_file.argtypes = [ctypes.c_char_p, ctypes.c_char_p,
                                      ctypes.c_size_t]
    lib.tcs_digest_of_file.restype = ctypes.c_int
    lib.tcs_digest_of_file.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
    lib.tcs_configuration_open.restype = ctypes.c_void_p
    lib.tcs_configuration_open.argtypes = [ctypes.c_char_p]
    lib.tcs_configuration_close.restype = None
    lib.tcs_configuration_close.argtypes = [ctypes.c_void_p]
    lib.tcs_relay_open.restype = ctypes.c_int
    lib.tcs_relay_open.argtypes = [ctypes.c_void_p, ctypes.c_char_p,
                                   ctypes.POINTER(ctypes.c_void_p)]
    lib.tcs_relay_close.restype = None
    lib.tcs_relay_close.argtypes = [ctypes.c_void_p]
    _loaded = lib
    return lib


def last_error(lib):
    """What the library says went wrong with its last call on this thread.

    Args:
        lib: The library.

    Returns:
        The reason, as text.
    """
    return lib.tcs_last_error().decode(errors="replace")


def links_text(path):
    """The link table of the configuration at ``path``, as YAML, as
    ``tcs-verify --links`` prints it.

    Args:
        path: The configuration file.

    Returns:
        The text.

    Raises:
        OSError: The library is not built or installed.
        ValueError: The configuration does not read, or its links do not
                    work out, saying why.
    """
    lib = library()
    encoded = str(path).encode()
    length = lib.tcs_links_of_file(encoded, None, 0)
    if length < 0:
        raise ValueError(last_error(lib))
    out = ctypes.create_string_buffer(length + 1)
    if lib.tcs_links_of_file(encoded, out, length + 1) < 0:
        raise ValueError(last_error(lib))
    return out.value.decode()


def digest_of_file(path):
    """The digest of the configuration at ``path``: what the vehicle flying
    it announces in every beacon.

    Args:
        path: The configuration file.

    Returns:
        The digest, as thirty-two hex digits.

    Raises:
        OSError: The library is not built or installed.
        ValueError: The configuration does not read, saying why.
    """
    lib = library()
    out = ctypes.create_string_buffer(33)
    if lib.tcs_digest_of_file(str(path).encode(), out) != 0:
        raise ValueError(last_error(lib))
    return out.value.decode()
