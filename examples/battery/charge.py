#!/usr/bin/env python3
"""The battery example's state of charge: a coulomb count.

A ``process`` compute node, which tcspecial starts when the configuration is
loaded and talks to a line at a time -- see ``process.rs`` in tcspecial for
the whole of that arrangement:

* Each line on standard input is a current, in amperes, as decimal text: the
  shunt's reading, as ``battery_amps`` makes it. More than nought charges the
  battery and less discharges it.
* Each line written on standard output is an answer, the fraction of its
  capacity the battery holds, which tcspecial passes on as a ratio.
* What goes wrong is written on standard error.

The charge is counted from the current and the time between readings: each
reading is taken to hold until the next, which is how a shunt sampled twice a
second is integrated. It starts where the battery is launched, six tenths
full, and is kept within nought and one.

A count drifts, and starts from a guess, so it is set right whenever the
battery says where it is. A full battery takes no current -- what would
charge it is shunted before the shunt in its lead sees it -- and on this bus
nothing else brings its current near nought: the array always gives more
than the loads take in sunlight, and less in shadow. So a reading within a
few times the shunt's noise of nought is a full battery, and the count is
set to one. A real gauge does the same at the end of a charge.
"""

import sys
import time

# The battery's capacity, in ampere-hours, and how full it is launched: the
# figures of the world section of examples/battery/battery-sim.yaml.
CAPACITY_AH = 0.2
LAUNCHED = 0.6

# A current closer to nought than this, in amperes, is a full battery: five
# times the shunt's noise in examples/battery/battery-sim.yaml.
FULL_BELOW = 0.05


def main():
    """Counts the charge in each reading and answers it, until the input ends.

    Returns:
        The exit status: nought.
    """
    coulombs = CAPACITY_AH * 3600.0
    charge = LAUNCHED
    last = None
    for line in sys.stdin:
        said = line.strip().split("=")[-1]
        try:
            amperes = float(said)
        except ValueError:
            print(f"charge.py: {said!r} is not a current", file=sys.stderr)
            print("", flush=True)
            continue
        now = time.monotonic()
        if last is not None:
            then, held = last
            charge += held * (now - then) / coulombs
            charge = min(1.0, max(0.0, charge))
        if abs(amperes) < FULL_BELOW:
            charge = 1.0
        last = (now, amperes)
        print(f"{charge:.6f}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
