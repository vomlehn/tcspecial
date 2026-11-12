====================================================
Building a Vehicle: From the Examples to a Whole One
====================================================

.. note::
   Outline, for review. Each section says what it will teach, what the
   reader adds to the configuration, how they check it, and which figures
   it will have. Nothing below is written out yet, and the stage
   directories, figures and screenshots it names do not exist yet.

.. contents:: Table of Contents
   :depth: 2
   :local:

What this guide is for
======================
The shipped examples each show one idea: one payload, one computation, one
gateway. A mission's vehicle has all of them at once, and the ways they
combine -- shared names, ports and IDs, files that include others, rates
that add up on one link -- are what no single example shows.

This guide builds one vehicle, a small lunar lander's science deck, a stage
at a time, from the minimal example to a configuration a mission could start
from. Each stage adds one thing, says why it is added the way it is, and
ends with a configuration that ``tcsverify`` passes and that runs on a desk
with ``--simulate=all``.

It is a companion to the manuals, not a replacement: each stage links to
the manual section that says everything about what it adds.

* **Who it is for:** someone who has run an example and now has a vehicle
  of their own to describe.
* **What it assumes:** the programs are built or installed (the tcspecial
  manual's "Installing"), and the reader has run the minimal example once.

**Figure 1 -- The finished vehicle.** tcsgui's drawing of the stage-10
configuration, every node and link, with the subsystem files as boxes. The
picture the guide works towards, shown first so that each stage can be
placed in it.

The vehicle
===========
A lander's science deck, chosen so that each stage needs a different part of
TCSpecial, and so that every payload has a simulated device tcssim already
provides:

==================  ==========================  ==============================
Payload             Interface                   Stage it arrives in
==================  ==========================  ==============================
Seismometer         network (UDP)               1
Soil thermometer    serial                      2
Heater              GPIO                        4
Three hull sensors  I\ :sup:`2`\ C on a mass    6
Camera              app node (existing code)    9
==================  ==========================  ==============================

**Figure 2 -- The deck.** A drawing of the deck: each payload where it sits,
labelled with its name and interface, so the reader knows what the nodes
stand for before meeting them. Drawn by hand in SVG; any colour is the
Okabe-Ito palette's and is paired with a label, never alone.

Each stage's configuration is in ``examples/vehicle/stage-N/``, in both
spellings, with its simulator file, so that a reader can start at any stage,
or compare two with ``diff``.

Stage 1: one payload to the ground
==================================
*Starts from:* ``examples/minimal``.

* The anatomy of a configuration: the ``tcspecial`` section (where commands
  arrive, where beacons go, the log), one gateway, one payload, and the
  line between them.
* Names: every node has one, and lines are written as ``node.name``.
* Running it: ``tcsverify``, then tcsmoc with ``--simulate=all``, which
  starts tcspecial and tcssim.
* What the ground sees: the beacon, the seismometer's panel.

*Figures:* tcsgui's drawing of the stage; tcsmoc's window with the beacon
and the seismometer's panel, the parts the text names marked with numbered
callouts.

Stage 2: a second payload
=========================
* Adding the soil thermometer, on a serial line: a payload type with
  attributes of its own (the tcspecial manual's "A Serial Payload").
* What changes elsewhere: the simulator file gains its device; the ground
  receives it on a port of its own.
* Where ports and IDs come from: a node's device id is its place among the
  nodes sorted by name, and the ground port is the gateway's base plus that
  id. Adding a node can move another's port -- why, and how to tell.

*Figures:* the stage's drawing; **Figure -- where each payload arrives**: a
table-like figure of nodes, device ids and ground ports, before and after
the new node, the changed rows marked with a symbol as well as a colour.

*Common mistake shown:* two payloads given one name, and ``tcsverify``'s
message for it.

Stage 3: a computation between payload and ground
=================================================
* A ``to_kelvin`` computation on the thermometer's reading: a vehicle node
  between a payload and the gateway.
* The four terminals, ``world-in``, ``world-out``, ``moc-in``, ``moc-out``,
  by the corner they sit at, and why a reading runs world-in to world-out.
* Typed links: the computation states what it takes and makes, and the
  configuration is refused when a line joins two that do not agree.

*Figures:* the stage's drawing; **Figure -- the four terminals**, one node
large, each corner labelled with its terminal's name and what flows there.

Stage 4: commands down to a payload
===================================
* The heater, on a GPIO line, commanded from the ground through ``moc-out``
  and ``moc-in``.
* A ``thermostat`` computation that switches the heater from the
  thermometer's temperature, and the ground's command that overrides it.
* Sending a command from tcsmoc, and reading its answer.

*Figures:* the stage's drawing; tcsmoc sending the heater a command, with
the command and its answer marked.

Stage 5: rates and groups
=========================
* How often each payload is read, and why some are read together: the
  groups section.
* What the beacon interval is for, and what it costs.
* Adding up what crosses the link: each payload's rate times its size, the
  beacon's, and the margin the link needs. Worked by hand here; the
  summary ``tcsverify`` is to print (see "Checking the whole vehicle") does
  it for you.

*Figures:* **Figure -- a second of the link**: a timeline of what is sent in
one second, each payload's packets as labelled blocks with a fill pattern
of their own.

Stage 6: redundant sensors and a vote
=====================================
* Three hull temperature sensors on one thermal mass, and a ``vote``
  computation that takes them in and sends one value on.
* Why a vote, rather than one sensor: what the vote does when one sensor
  disagrees or stops.
* Watching it in tcsmoc while tcssim fails a sensor.

*Figures:* the stage's drawing; tcsmoc's panels as one sensor is failed in
tcssim, before and after, the failed sensor marked.

Stage 7: splitting the vehicle into files
=========================================
* The configuration has grown past one screen. Each subsystem -- thermal,
  science -- moves into a file of its own, included by the top file.
* Includes and subdiagrams: names, relative paths, and why nothing in a path
  is expanded.
* Exports: which of a file's nodes a line from another file may reach, and
  the message when a line crosses where nothing is exported.
* Two vehicles sharing a subsystem file, as the thermostat examples do.

*Figures:* **Figure -- the files**: the top file and the files it includes,
as boxes with their exports at the corners; tcsgui's drawing of the same,
with the subsystems collapsed to boxes.

Stage 8: keeping a log aboard
=============================
* A ``log`` node: what is kept aboard for when the link is down, and how the
  ground asks for it later.
* The segment files, their sizes, and what fills the storage.

*Figures:* the stage's drawing; **Figure -- the log through a pass**: what
is logged while the ground is out of sight and sent when it returns, on a
timeline.

Stage 9: a payload with code of its own
=======================================
* The camera, whose software exists already: an ``app`` node runs it as a
  program over file interfaces, rather than being written as a node type.
* When to use an app node, and when a node type is the better choice
  (pointing to "Writing node types").

*Figures:* the stage's drawing; **Figure -- an app node**: the program, its
file interfaces and the node, labelled.

Stage 10: choosing the gateway
==============================
* Until now the vehicle has spoken to the ground directly by radio. Most
  missions put a flight computer in between: F´, cFS, or a ground system
  such as Yamcs.
* What changes: only the gateway, and the ground's side of the link; the
  rest of the vehicle is untouched. The same deck, three times, each with
  a different gateway.

*Figures:* the three gateways side by side, the unchanged rest of the
vehicle drawn the same in each, the gateway node labelled with its type.

Checking the whole vehicle
==========================
* ``tcsverify`` on the whole configuration, and what its messages say at
  this size.
* Both spellings, and ``tcsverify a.xtcs a.ytcs`` to compare them.
* The summary of everything the vehicle uses -- ports, message IDs, APIDs,
  rates against the link -- and the clashes it warns of. (To be added to
  ``tcsverify`` as ``--summary``; this section waits for it.)

*Figures:* a ``tcsverify --summary`` run on the finished vehicle, with its
parts named.

From the desk to flight
=======================
What changes between the configuration that has run on a desk and the one
that flies is a checklist of its own, `From the desk to flight
<desk-to-flight.html>`__, each item linking to where it is said in full: the
flight setting, the real addresses, storage that survives a power cycle, the
build, the packages, the supervisor, the configurations kept aboard, and the
ground's agreeing to the configuration.

Where to go next
================
* The tcspecial manual, for every payload type and attribute.
* "Writing node types", for what no shipped type does.
* The tcsgui manual, for drawing and editing the configuration.
* The examples' README, for the examples this guide drew on.

How this guide is made
======================
*For maintainers; not part of the published guide.*

* The source is ``docs/building-a-vehicle.rst``; ``make`` in ``docs/``
  writes ``building-a-vehicle.html``, which ``make html-check`` holds
  current.
* The stage configurations live in ``examples/vehicle/stage-N/``, and a
  test verifies every stage, both spellings, so that the guide cannot
  drift from what the programs accept.
* The tcsgui drawings and tcsmoc screenshots are made by a screenshots
  test, as the manuals' are (``make screenshots``), so they are made again
  rather than redrawn when a window changes. The drawings that are not
  screenshots -- the deck, the terminals, the files, the timelines -- are
  SVG by hand, Okabe-Ito colours paired with labels, patterns or shapes.
