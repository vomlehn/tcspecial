"""Relays between Yamcs and a vehicle, which hold each datagram a while,
lose some, or both: the link's simulation, done by the ground.

A Yamcs instance's links are moved off the vehicle's ports by
:func:`relayed`, or :func:`relay_instances` for instances already written,
and :class:`Relays` -- the ``tcs-relay`` program, in Rust, which this starts
-- listen where they were, passing each datagram on after a delay, the light
time to the Moon say, and losing those a bit error would lose where the
run's simulator file asks for bit errors. Both are simulation, refused in a
flight run: by ``tcs-relay`` itself, and asked here first, through
:func:`tcsmoc.simulation.allow`, so that a ground says so before it starts
anything. See ``tcs-moc/src/relay.rs``.
"""

from pathlib import Path

from tcsmoc.processes import Child, built
from tcsmoc.simulation import allow

#: How far above its own port each link Yamcs is given is moved, so that a
#: relay can have the port the vehicle sends to or is sent from.
SHIFT = 10000


def loss_asked(sim, mode):
    """Whether a run's simulator file asks for bit errors on the link,
    which only a run that simulates may.

    Args:
        sim:  The run's simulator file, or ``None`` where it has none.
        mode: The run's simulation mode.

    Returns:
        True where its ``gateway`` section states a ``bit_error_rate``.

    Raises:
        ValueError: The file cannot be read, or asks for bit errors in a
                    flight run.
    """
    import yaml

    if sim is None:
        return False
    try:
        document = yaml.safe_load(Path(sim).read_text()) or {}
    except (OSError, yaml.YAMLError) as why:
        raise ValueError(f"{sim} cannot be read, so whether it asks for bit "
                         f"errors on the link cannot be known: {why}")
    gateway = document.get("gateway") if isinstance(document, dict) else None
    rate = gateway.get("bit_error_rate") if isinstance(gateway, dict) \
        else None
    if rate is None or str(rate).strip() == "":
        return False
    allow(f"bit errors on the link (bit_error_rate in {sim})", mode, sim)
    return True


def relayed(links, shift=SHIFT):
    """A Yamcs instance's links moved for relays to stand between it and the
    vehicle, and the relays.

    A link Yamcs only listens on -- telemetry, the beacon -- is moved
    ``shift`` above its port, and its relay listens on the port the vehicle
    sends to and passes each datagram on to Yamcs. It listens on every
    address the ground has, as the Yamcs link it stands in for does, which
    a link that names only a port does: a vehicle on another machine sends
    to the ground's own address, and not to its loopback. A link Yamcs sends on --
    commands, whose answers come back to the socket that sent them -- is
    pointed at a port ``shift`` above the vehicle's, where its relay
    listens, passing each command on to the vehicle and each answer back.

    Args:
        links: The links, as :func:`tcsprotocol.yamcs.data_links` gives
               them. Left as they are.
        shift: How far each port is moved.

    Returns:
        The moved links, and each relay as ``(listen, forward, both_ways)``,
        each address a ``(host, port)``.
    """
    moved, relays = [], []
    for link in links:
        link = dict(link)
        port = link["port"]
        if "host" in link:
            vehicle = (link["host"], port)
            link["host"] = "127.0.0.1"
            link["port"] = port + shift
            relays.append((("127.0.0.1", port + shift), vehicle, True))
        else:
            link["port"] = port + shift
            relays.append((("0.0.0.0", port), ("127.0.0.1", port + shift),
                           False))
        moved.append(link)
    return moved, relays


def relay_instances(yamcs_dir, instances, writer):
    """Moves the links of Yamcs instances already written for relays to
    stand between each and its vehicle.

    Args:
        yamcs_dir: Where Yamcs's configuration was written: its ``etc/``
                   holds ``yamcs.<instance>.yaml``.
        instances: The instances' names.
        writer:    The program moving them, which each file says.

    Returns:
        Each relay, as :func:`relayed` gives them, for every instance.
    """
    import yaml

    relays = []
    for instance in instances:
        path = Path(yamcs_dir) / "etc" / f"yamcs.{instance}.yaml"
        text = path.read_text()
        header = "".join(line + "\n" for line in text.splitlines()
                         if line.startswith("#"))
        said = yaml.safe_load(text)
        said["dataLinks"], theirs = relayed(said["dataLinks"])
        relays += theirs
        path.write_text(header + f"# Its links moved for {writer}'s "
                        "relays.\n" + yaml.safe_dump(said, sort_keys=False))
    return relays


class Relays:
    """A set of relays, served by ``tcs-relay``, which this starts and
    stops.

    Args:
        delay:   How long each datagram is held, in seconds.
        sim:     The run's simulator file, which ``tcs-relay`` reads for
                 the mode and the bit errors; ``None`` for none.
        program: The ``tcs-relay`` to run, or ``None`` for the build's.
        verbose: Whether to say on stderr that it stopped by itself.
        ground:  The ground's own address, which each relay both ways
                 passes commands on from, where the vehicle takes them
                 only from it -- the configuration's ``tcs_cmd.send`` --
                 and which reaches a vehicle on another machine; ``None``
                 for the loopback, for a vehicle on this one.
    """

    def __init__(self, delay, sim=None, program=None, verbose=False,
                 ground=None):
        self.delay = delay
        self.ground = ground
        self.sim = sim
        self.program = program
        self.verbose = verbose
        self.routes = []
        self.child = None

    def one_way(self, listen, forward):
        """Adds a relay passing what arrives at one address on to another.

        Args:
            listen:  Where it listens, ``(host, port)``.
            forward: Where it passes each datagram on to.
        """
        self.routes.append(f"{listen[0]}:{listen[1]}>{forward[0]}:"
                           f"{forward[1]}")

    def both_ways(self, listen, forward):
        """Adds a relay passing what arrives at one address on to another,
        and the answers back to whoever last sent.

        Args:
            listen:  Where it listens, ``(host, port)``.
            forward: Where it passes each datagram on to.
        """
        route = f"{listen[0]}:{listen[1]}<>{forward[0]}:{forward[1]}"
        if self.ground is not None:
            route += f"@{self.ground}:0"
        self.routes.append(route)

    def args(self):
        """``tcs-relay``'s arguments for these relays.

        Returns:
            The arguments.
        """
        args = [f"--delay={self.delay:g}"]
        if self.sim is not None:
            args.append(f"--simulator={self.sim}")
        return args + self.routes

    def start(self):
        """Starts ``tcs-relay`` on the relays.

        Raises:
            ValueError: ``tcs-relay`` is not built or on the PATH.
        """
        program = self.program or built("tcs-relay",
                                        "build it with cargo build")
        self.child = Child(program, self.args(), "tcs-relay", self.verbose)
        self.child.start()

    def stop(self):
        """Stops ``tcs-relay``, where it was started."""
        if self.child is not None:
            self.child.stop()
