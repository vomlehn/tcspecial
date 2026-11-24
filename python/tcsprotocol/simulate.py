"""The simulation mode: a run's, as its simulator file states it.

The Python spelling of ``tcsprotocol/src/simulate.rs``'s reading of the
mode. The mode is stated in the run's simulator file, as ``simulate`` in
its ``tcspecial`` section -- ``none``, ``some`` or ``all`` -- and never on a
command line: :func:`run_for` finds the file as every program of the run
finds it, and :func:`mode_of` reads it. A run with no simulator file, or one
that states no mode, is ``none``, the flight setting.
"""

import os
from pathlib import Path

#: The modes, in the words the simulator file takes.
MODES = ("none", "some", "all")

#: The flight setting.
FLIGHT = "none"

#: The environment variable naming the run's simulator file, looked at
#: before the file beside the configuration.
SIM_PATH_VAR = "PAYLOAD_SIM_YAML"


def sim_path_beside(config):
    """The simulator file beside a configuration and named for it.

    ``<stem>-sim.yaml``, which the examples use, or the older
    ``<stem>sim.yaml``, the stem being the configuration's name without
    ``.ytcs`` or ``.xtcs``.

    Args:
        config: The configuration's path.

    Returns:
        The file's path as a string, or ``None`` where there is none.
    """
    text = str(config)
    for ext in (".ytcs", ".xtcs"):
        if text.endswith(ext):
            stem = text[:-len(ext)]
            for candidate in (f"{stem}-sim.yaml", f"{stem}sim.yaml"):
                if Path(candidate).is_file():
                    return candidate
            return None
    return None


def mode_of(sim):
    """Reads the mode a simulator file states.

    Args:
        sim: The simulator file's path.

    Returns:
        ``"none"``, ``"some"`` or ``"all"``: ``"none"`` where it states
        none.

    Raises:
        ValueError: The file cannot be read, or states a word that is no
                    mode; either way the run's mode cannot be known.
    """
    import yaml

    try:
        text = Path(sim).read_text()
        document = yaml.safe_load(text) or {}
    except (OSError, yaml.YAMLError) as why:
        raise ValueError(f"{sim} cannot be read, so the run's simulation "
                         f"mode, which it states, cannot be known: {why}")
    section = document.get("tcspecial") if isinstance(document, dict) else None
    said = section.get("simulate") if isinstance(section, dict) else None
    if said is None or str(said).strip() == "":
        return FLIGHT
    said = str(said).strip()
    if said not in MODES:
        raise ValueError(f"{sim}: simulate: {said} is not a simulation mode: "
                         f"give one of {', '.join(MODES)}, or nothing for "
                         "none, the flight setting")
    return said


def run_for(config, environ=None):
    """Finds a run's simulator file and reads its mode.

    The file is the one :data:`SIM_PATH_VAR` names, and otherwise the one
    beside the configuration.

    Args:
        config:  The configuration's path.
        environ: The environment to look in, or ``None`` for this
                 program's.

    Returns:
        ``(mode, sim)``: the mode, and the file's path, or ``None`` where
        there is no file and the mode is ``"none"``.

    Raises:
        ValueError: As :func:`mode_of` raises it.
    """
    environ = os.environ if environ is None else environ
    sim = environ.get(SIM_PATH_VAR) or sim_path_beside(config)
    if not sim:
        return FLIGHT, None
    return mode_of(sim), str(sim)


def stated(mode):
    """How a mode is stated, for a message saying what to state.

    Args:
        mode: The mode.

    Returns:
        The words, as ``simulate: all``.
    """
    return f"simulate: {mode}"
