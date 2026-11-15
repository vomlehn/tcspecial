"""Relaying a consolidator's inputs to Yamcs.

A consolidator packs its inputs into one datum for the link from the vehicle,
where bandwidth is short. Yamcs reads the gateway node's ports itself, so it
would be given the datum; instead, a relay binds the consolidators' ports,
takes each datum apart and hands each new input on to Yamcs as though its
source were drawn into the gateway node: on the source's own port, under its
own APID (:meth:`tcsprotocol.configuration.Configuration.relayed`). Yamcs
decodes those as it decodes every other source, by the configuration's own
XTCE, and never sees a consolidation.

The relay is tcsmoc's, ``tcsmoc::relay``: a thread in this process, started
through libtcsmoc-c, so that it is the one unpacking of a consolidation on
the ground and it ends when the station does. The library is loaded only for
a configuration that has something to relay, so a station whose
configuration has no consolidator does not need it built.
"""

import ctypes
import ctypes.util

from tcsprotocol.configuration import find_built

#: The library's file in this tree's build.
BUILT = "libtcsmoc_c.so"

#: The library's name, where it is installed.
INSTALLED = "tcsmoc-c"


def gateways_to_relay(config):
    """The ``gateway_yamcs`` nodes a consolidator's inputs are relayed to.

    Args:
        config: The :class:`tcsprotocol.configuration.Configuration`.

    Returns:
        Their names, in the order the file writes them.
    """
    return [node.name for node in config.nodes
            if node.node_type == "gateway_yamcs" and config.relayed(node.name)]


def library():
    """Loads libtcsmoc-c: this tree's build first, then where it is
    installed.

    Returns:
        The library, its functions given their types.

    Raises:
        OSError: It is not built or installed.
    """
    path = find_built(BUILT) or ctypes.util.find_library(INSTALLED)
    if path is None:
        raise OSError(
            "a consolidator's inputs are relayed to Yamcs by libtcsmoc-c, "
            "which is not built or installed: build it with "
            "`cargo build -p tcsmoc-c`")
    lib = ctypes.CDLL(path)
    lib.tcs_last_error.restype = ctypes.c_char_p
    lib.tcs_last_error.argtypes = []
    lib.tcs_configuration_open.restype = ctypes.c_void_p
    lib.tcs_configuration_open.argtypes = [ctypes.c_char_p]
    lib.tcs_configuration_close.restype = None
    lib.tcs_configuration_close.argtypes = [ctypes.c_void_p]
    lib.tcs_relay_open.restype = ctypes.c_int
    lib.tcs_relay_open.argtypes = [ctypes.c_void_p, ctypes.c_char_p,
                                   ctypes.POINTER(ctypes.c_void_p)]
    lib.tcs_relay_close.restype = None
    lib.tcs_relay_close.argtypes = [ctypes.c_void_p]
    return lib


class Relays:
    """The relays of every gateway node a consolidator's inputs are
    relayed to, each a thread, stopped by :meth:`close` or when the process
    ends.

    Args:
        path:   The configuration file.
        config: The :class:`tcsprotocol.configuration.Configuration` read
                from it.

    Raises:
        OSError: libtcsmoc-c is not built or installed, where something is
                 to be relayed.
        ValueError: The library refuses the configuration or a relay, saying
                    why.
    """

    def __init__(self, path, config):
        self._lib = None
        self._relays = []
        gateways = gateways_to_relay(config)
        if not gateways:
            return
        self._lib = library()
        configuration = self._lib.tcs_configuration_open(
            str(path).encode())
        if not configuration:
            raise ValueError(self._why())
        try:
            for gateway in gateways:
                relay = ctypes.c_void_p()
                said = self._lib.tcs_relay_open(
                    configuration, gateway.encode(), ctypes.byref(relay))
                if said < 0:
                    raise ValueError(self._why())
                if relay.value:
                    self._relays.append(relay.value)
        except ValueError:
            self.close()
            raise
        finally:
            # The relays keep what they need of it.
            self._lib.tcs_configuration_close(configuration)

    def __len__(self):
        """How many relays are running."""
        return len(self._relays)

    def _why(self):
        """What the library says went wrong with its last call.

        Returns:
            The reason, as text.
        """
        return self._lib.tcs_last_error().decode(errors="replace")

    def close(self):
        """Stops every relay, waiting for its thread to end."""
        while self._relays:
            self._lib.tcs_relay_close(self._relays.pop())

    def __enter__(self):
        """Returns these relays, for a ``with`` statement."""
        return self

    def __exit__(self, *_exc):
        """Stops every relay, at the end of a ``with`` statement."""
        self.close()
