=========================
The Lander Example Design
=========================

.. contents:: Table of Contents
   :depth: 4
   :local:

.. note::

   This is a plan. Nothing it describes is built yet; it says what the
   lander example is to be and how it is to be made, in the order it is
   to be made, so that each stage can be checked against it.

What it is
==========

A lunar landing in two dimensions, flown by hand. tcs-sim draws the lunar
landscape and the lander over it, and below them a joystick; whoever runs
the example flies the lander down onto a landing pad with the joystick,
against the Moon's gravity and with fuel that runs out.

Two tcspecials fly it, called A and B: A has the X axis and B the Y axis.
Each reads its axis of the joystick and drives its axis's thrust, and
sends its axis's position, velocity and acceleration, and the joystick's
value, to the ground. On the ground, Yamcs takes both, and a window of the
example's own, reading Yamcs, draws the landscape and the lander again,
smaller, with a joystick that only shows what was sent. Everything between
the vehicles and Yamcs, both ways, takes as long as it would between the
Moon and the Earth.

The scene
=========

The display
-----------

The display is the world: 1000 m wide and 600 m high, with X increasing to
the right and Y upward. tcs-sim's window and the ground's both draw it
whole, the ground's smaller.

The landing pad
---------------

The pad is a level line twice as wide as the lander, 60 m. Its centre is
within the middle 50% of the display's width, and its height within the
lowest 20% of the display's height. It is drawn as a level line with a
short upright mark at each end, so that it is told from the terrain by its
shape and not by its colour alone.

The terrain
-----------

The terrain is a connected set of straight lines from the left edge of the
display to the pad's left end, and from the pad's right end to the right
edge. Each line goes from left to right, so none overlaps another, and its
slope is no steeper than 70 degrees either way. No point of it is below the
bottom of the display or higher than 60% of the display's height. Below the
lines, the display is filled 50% grey with a little brown, ``#857a6c``.

The landscape is made at random for each run, from a seed. The ground
station takes the seed as ``--seed N``, or chooses one and says which, so
that any landscape can be had again. It makes the pad and the terrain from
the seed, keeping every rule above, and writes them into that run's copy of
the simulator file, which tcs-sim and the ground's window both read: the
landscape is made in one place, and the two draw the same one. The
vehicles do not need it.

The lander
----------

The lander is a simple figure of a lunar lander, drawn for this example
and copied from no other: a body on two splayed legs with feet, about a
twentieth of the display's height, 30 m, and about as wide as it is tall;
its outline is what meets the ground. It starts at the
upper left of the display, wholly in view.

Flying it
=========

The joystick
------------

The joystick, in tcs-sim's window below the X and Y values, is a plus: a
square, the neutral part, with a rectangle joined to each of its four
sides. A press in the square, dragged out of it, moves the axes; each axis
reads on its own, from the pointer's place along it, so a drag outward on a
diagonal moves both.

* X is 0 at the square's left side and falls linearly to -100 at the left
  end of the left rectangle; it stays at -100 further left, and comes back
  to 0 the same way when the pointer comes back. At the square's right side
  it rises in the same way to 100 at the right end of the right rectangle.
* Y is 0 at the square's top and rises linearly to 100 at the top of the
  upper rectangle, and falls from 0 at the square's bottom to -100 at the
  bottom of the lower rectangle, held at the ends in the same way.

A small dot follows the pointer while the button is held, within the
rectangle that encloses the plus, and stays at that rectangle's edge when
the pointer goes beyond it. When the button is let go, the dot goes back to
the centre of the square, and both axes read 0 until the next press in the
square is dragged out of it.

Thrust, mass and fuel
---------------------

Each axis's value, from -100 to 100, is a thrust along that axis, positive
values pushing the lander towards positive X or Y and negative values
towards negative X or Y. The vehicle reads the value as a ratio, from -1
to 1, and drives the lander's throttle on that axis with it:

.. code-block:: text

   F = throttle * (full thrust on that axis)

where each axis's full thrust is a setting, chosen as the acceleration a
full push is to give with a full tank, times the full mass. The lander's mass
is its dry mass and the fuel left, so the same push accelerates it more as
the fuel burns. The fuel flows as

.. code-block:: text

   flow = (|Fx| + |Fy|) / (Isp * g0)

with the tank holding about 60 seconds of full thrust on one axis. When it
is empty, there is no thrust. Gravity is the Moon's, 1.62 m/s\ :sup:`2`,
in the direction of negative Y, on the whole of the lander's mass.

Touchdown
---------

The lander is down the moment any part of its outline first meets a line
of the terrain or the pad. It has landed if both its feet are on the pad,
nothing of it touches the terrain, and it is moving no faster than the
Apollo lunar module's touchdown limits: 3.05 m/s downward and 1.22 m/s
sideways. Anything else that touches has crashed, and so has a lander that
leaves the display entirely.

After a crash, there is a pause, and the lander starts again at the upper
left with a full tank. After a landing, a smiley face is drawn over the
lander, and then there is the same pause and the same start.

The vehicles
============

``lander-a`` (X) and ``lander-b`` (Y) are two configurations, each in
both spellings, each with its own ports and state file so that the two can
run at once; ``tcs-conflict`` checks the pair. Each has:

* payloads that read its axis of the joystick, and its axis's position,
  velocity and acceleration;
* one driven payload, its axis's thrust;
* ``to_value`` nodes for the readings;
* the joystick's reading passed through to the thrust, so that the pilot
  is in the loop and nothing aboard needs floating point;
* a ``gateway_yamcs`` gateway, which takes the four values to the ground.

The link to the ground
======================

What passes between each vehicle and Yamcs is delayed by the time light
takes between the Moon and the Earth, about 1.28 seconds: 1.3 seconds, a
setting of the ground station. The delay is both ways: telemetry down, and
anything sent up, such as the connecting message and any command added
later, is held as long. Between each tcspecial's gateway and Yamcs the
ground station puts a relay that holds each datagram that long before
passing it on. A round trip is then about 2.6 seconds, so every timeout in
the connecting exchange between Yamcs and a vehicle has to be longer than
that.

What the ground shows is therefore 1.3 seconds behind tcs-sim's window;
the pilot's loop, from the joystick to the thrust, is aboard and is not
delayed.

The windows
===========

tcs-sim's window
----------------

For a world that has a two-dimensional body, tcs-sim's window shows:

* the landscape and the lander, drawn to scale, positive X to the right and
  positive Y upward, with the smiley face after a landing;
* "Landed" or "Crashed" in words, and the fuel left, in words and as a
  bar;
* below them, the X and Y position, velocity and acceleration, each named
  and with its unit;
* below those, the joystick.

The ground's window
-------------------

The ground station's window reads the values from Yamcs several times a
second and shows:

* the landscape, from the run's simulator file, and the lander at the
  position sent down, smaller than tcs-sim's;
* the six values, and the fuel left;
* an area like the joystick, with a dot at the X and Y values the joystick
  sent down; the pointer does nothing there;
* "Landed" or "Crashed" when the values sent down say so.

How it is to be made
====================

In this order, each stage checked before the next:

1. **One world for two tcspecials.** tcs-sim takes more than one
   configuration, and one simulator file whose payloads say which
   configuration each belongs to, with one world and one window for both.
2. **Physics.** In ``tcsprotocol`` for the format and in tcs-sim for the
   simulation, as ``body`` and ``engine`` are:

   * ``lander``, the craft and the ground it lands on in one component,
     since a component sees only what is held at its inputs and the ground
     has to see the lander move, and the tank has to be filled again when
     it starts again: dry mass, fuel, specific impulse, full thrust on each
     axis, the Moon's gravity, its start, the display's size, the ground,
     the pad, its outline, the touchdown limits and the pause, read and held
     to the landscape's rules; the throttle on each axis, from -1 to 1, as
     inputs; the X and Y position and velocity, the fuel left, and landed
     and crashed, as outputs. The thrust, mass and fuel are as described
     above, extending what ``body`` and ``engine`` do on one axis.
   * ``joystick``: two inputs, which tcs-sim's window drives as a simulated
     payload drives an input, and two outputs, X and Y from -1 to 1, which
     simulated payloads report.
   * a unit, metres per second squared, for the acceleration: a type in
     ``tcsprotocol/xtce/types.xtce`` and the unit table. It changes the
     interface digest every configuration states, so it comes last, and
     with it the lander's two accelerations as outputs.

3. **tcs-sim's window**, as described above.
4. **The vehicles**, ``lander-a`` and ``lander-b``, as described above.
5. **The ground station**, ``examples/lander/lander_yamcs.py``: it makes the
   landscape from the seed and writes it into the run's simulator file;
   starts Yamcs with an instance for each vehicle, the two delaying relays,
   tcs-sim with both configurations, and both tcspecials; and opens its
   window.
6. **The rest of an example**:

   * ``lander.svg``, drawn by hand and naming every node, and
     ``lander.xtce``, from ``make xtce``;
   * tests: the landscape's rules kept for many seeds; thrust against mass,
     the fuel running out, landing and crashing at the limits and leaving the
     display; the joystick's mapping; the delay; the pair of vehicles, with
     ``tcs-conflict``;
   * ``make run-lander``, and ``lander`` in tcsx's lists of examples run
     by their own ground station and through Yamcs;
   * a section in ``examples/README.md``, "The lander example" in the
     tcspecial manual with its figure, the pair in ``make check``'s
     comparison of spellings, and the new components and unit in the tcs-sim
     manual;
   * the packaging, so that tcspecial-examples carries every new kind of
     file, and so that the container has the example.
