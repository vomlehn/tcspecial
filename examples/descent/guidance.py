#!/usr/bin/env python3
"""The descent example's guidance: a throttle from the radar's altitude.

A ``process`` compute node, which tcspecial starts when the configuration is
loaded and talks to a line at a time -- see ``process.rs`` in tcspecial for
the whole of that arrangement:

* Each line on standard input is an altitude, in metres, as decimal text:
  the radar altimeter's reading, as ``altitude`` makes it.
* Each line written on standard output is an answer, the engine's throttle as
  a fraction of its full thrust, which tcspecial passes on as a ratio and
  sends to the engine.
* What goes wrong is written on standard error.

The rate of descent is not measured here -- the Doppler velocimeter's is in
metres a second, and a process takes values in one unit -- so it is worked
out from the altitudes, by an alpha-beta filter: each reading corrects a
predicted altitude and rate by fixed fractions of the error, which smooths
the radar's noise out of the rate without falling far behind it.

The descent it flies is a rate in proportion to the altitude, six hundredths
of it a second, so the lander slows as it nears the ground: at most twenty
metres a second, and at least one, at which it comes down the last metres.
The throttle is what holds its weight, plus what closes the gap between the
rate it has and the rate it wants within a second, over the engine's
thrust. Half a metre up the engine is cut, and the lander drops the rest,
which at a metre a second is a fall of a tenth of a second or so; the
touchdown switch says when it is down.
"""

import sys
import time

# The lander's and the engine's figures, from the world section of
# examples/descent/descent-sim.yaml. The mass is taken as it starts:
# what it burns is a tenth of it, which the closing of the gap makes up.
MASS_KG = 1500.0
GRAVITY = 1.62
THRUST_N = 4500.0

# The descent: a rate of RATE_PER_METRE of the altitude, between SLOWEST
# and FASTEST, closed within CLOSING seconds, and the engine cut at CUTOFF.
RATE_PER_METRE = 0.06
SLOWEST = 1.0
FASTEST = 20.0
CLOSING = 1.0
CUTOFF_M = 0.5

# The alpha-beta filter's fractions of the error, for the altitude and the
# rate.
ALPHA = 0.5
BETA = 0.1


def throttle(altitude, rate):
    """The throttle that flies the descent from where the lander is.

    Args:
        altitude: Metres above the ground.
        rate: Metres a second, up more than nought.

    Returns:
        The throttle, from nought to one.
    """
    wanted = -min(FASTEST, max(SLOWEST, RATE_PER_METRE * altitude))
    force = MASS_KG * (GRAVITY + (wanted - rate) / CLOSING)
    return min(1.0, max(0.0, force / THRUST_N))


def main():
    """Answers a throttle for each altitude, until the input ends.

    Returns:
        The exit status: nought.
    """
    estimate = None
    cut = False
    for line in sys.stdin:
        said = line.strip().split("=")[-1]
        try:
            altitude = float(said)
        except ValueError:
            print(f"guidance.py: {said!r} is not an altitude", file=sys.stderr)
            print("", flush=True)
            continue
        now = time.monotonic()
        if estimate is None:
            # The first reading: the descent's starting rate, until the
            # filter has seen enough to say.
            estimate = (now, altitude, -FASTEST)
        else:
            then, height, rate = estimate
            dt = max(now - then, 1e-3)
            predicted = height + rate * dt
            error = altitude - predicted
            estimate = (
                now,
                predicted + ALPHA * error,
                rate + BETA * error / dt,
            )
        _, height, rate = estimate
        # Cut for good: a lander on its legs does not want the engine back.
        cut = cut or height <= CUTOFF_M
        print(f"{0.0 if cut else throttle(height, rate):.6f}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
