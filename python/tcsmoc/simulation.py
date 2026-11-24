"""Simulation done by the ground, allowed only in a run that simulates.

The Python spelling of ``tcsmoc/src/simulation.rs``: what a MOC program
does to stand for something that is not there -- a delay on the link as
long as the light time to the Moon, say -- asks :func:`allow` first, and is
refused under ``simulate: none``, the flight setting, as the vehicle refuses
its own test code.
"""

from tcsprotocol.simulate import FLIGHT, stated


def allow(feature, mode, sim):
    """Allows a simulation feature of the ground's in a run, or refuses it.

    Args:
        feature: What the feature is, as the program's user asked for it:
                 ``"--delay=1.3"``, say.
        mode:    The run's simulation mode.
        sim:     The simulator file that states it, or ``None`` where the
                 run has none.

    Raises:
        ValueError: Under the flight setting, saying what was asked, what
                    the run is, and what to change.
    """
    if mode != FLIGHT:
        return
    whose = (f"{sim} states {stated(mode)}, or no mode" if sim
             else "there is no simulator file")
    raise ValueError(
        f"{feature} is simulation, done by the ground, and {whose}, so this "
        "run is the flight setting, where nothing is simulated: state "
        f"{stated('some')} or {stated('all')} in the simulator file's "
        f"tcspecial section, or leave {feature} out")
