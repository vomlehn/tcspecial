=======================
From the Desk to Flight
=======================

A configuration that has run on a desk, against tcs-sim, is most of the way
to one that flies, and on purpose: the configuration flown is meant to be
the configuration tested, byte for byte, so that its checksum is the one the
ground has agreed to. What changes between the two is mostly not in the
configuration at all, but in how tcspecial is built, installed and started.

This page is that difference as one checklist. Each item says what to do or
check, why, and where the whole of it is said; none of it is a rule stated
here for the first time.

.. contents:: The checklist
   :depth: 1
   :local:

1. Fly with the flight setting, ``--simulate=none``
===================================================
*Do:* start tcspecial with no ``--simulate`` flag, or with
``--simulate=none``, which is the same.

*Why:* under ``none`` every payload must be a real device, and tcspecial
refuses to start at the first one that is a simulated or a dummy device,
naming it. A configuration whose bus number is wrong then fails on the
ground instead of driving the wrong hardware. The flag is not part of the
configuration, so the checksum is the same under every mode.

*Check:* the startup summary says the mode and how each payload was judged.

*See:* "Simulation Modes" in the `tcspecial design
<tcspecial-design.html#simulation-modes>`__.

2. Leave ``sim_address`` where it is
====================================
*Do:* nothing. A node's ``sim_address`` says where tcs-sim answers for it
under ``some`` and ``all``; under ``none`` tcspecial ignores it and opens the
node's real bus, line or chip.

*Why:* removing it would change the configuration, and so its checksum, from
the one that was tested. Keeping it costs nothing in flight.

*See:* each node type that takes it in the `tcspecial manual
<tcspecial.html>`__, such as "Temperature Sensors on a Thermal Mass".

3. Name the real addresses
==========================
*Do:* go through the ``tcspecial`` section and every gateway node, and
replace the loopback addresses a desk run uses -- ``127.0.0.1`` for where the
ground receives beacons, answers and telemetry -- with the vehicle's and the
ground's own.

*Why:* the examples run every program on one machine, so everything they
send goes to loopback. Flown, those datagrams must reach the radio, the
flight computer or the ground station. The addresses tcspecial binds,
``0.0.0.0`` in the examples, may be narrowed to one interface.

*Check:* ``tcs-verify --summary`` lists every address and port the vehicle
uses, and warns where two claim one.

4. Keep the state and the log where a power cycle leaves them
=============================================================
*Do:* put ``state_file`` and ``log.directory`` under ``/var/opt/tcs/tcspecial``,
or on whatever storage of the vehicle survives a power cycle -- not under
``/tmp``, where the examples put them.

*Why:* the state file says whether the next start is one the ground
commanded, and which configuration it is to run, so that a restart the ground
asked for is not taken for a crash; the log is what is kept aboard for the
ground to ask for later. Both are worthless if a reboot empties them.

*See:* "Running tcspecial under a supervisor" in the `tcspecial manual
<tcspecial.html#running-tcspecial-under-a-supervisor>`__, and "The state
file" in the `design <tcspecial-design.html#the-state-file>`__.

5. Build without the visual window
==================================
*Do:* build tcspecial with its default features, as ``make install-build``
and the packages do. Never with ``--features visual``.

*Why:* the visual window brings a window toolkit into the flight software,
which no vehicle needs, and the toolkit's licence does not cover embedded
systems. A build that has it says so as a warning every time it starts.

*See:* "The Visual Window" in the `design
<tcspecial-design.html#the-visual-window>`__.

6. Check the configuration against the build that flies
=======================================================
*Do:* run tcs-verify with the node types the flying tcspecial was built with:

.. code-block:: console

   $ tcs-verify --types=$(tcspecial --types) vehicle.ytcs

*Why:* a tcspecial built without a node type refuses a configuration that
uses it, and the place to find that out is on the ground. The configuration's
``interface`` must also be the interface this build speaks, which tcs-verify
checks.

*See:* the `tcs-verify manual <tcs-verify.html>`__.

7. Install the flight package, and only it
==========================================
*Do:* install the ``tcspecial`` package on the vehicle; ``tcspecial-moc``
on the ground station; ``tcspecial-test`` on neither.

*Why:* ``tcspecial-test`` holds tcs-moc and tcs-sim, which never fly, and the
test ``segsend``, which deletes each segment it is given: a vehicle that flew
it would throw its telemetry away. A vehicle that keeps a log puts its own
``segsend`` beside tcspecial.

*See:* "The packages" in the `tcspecial manual
<tcspecial.html#the-packages>`__.

8. Install the configuration, and start tcspecial under its supervisor
======================================================================
*Do:* copy the configuration to ``/etc/opt/tcs/tcspecial/tcspecial.ytcs``,
check it there (item 6), and enable the systemd unit the package installs.

*Why:* tcspecial relies on a supervisor to start it again whenever it exits,
after a restart or a crash; the unit is that supervisor. It is installed
disabled, and does nothing until a configuration is in place, because no
package ships one: a vehicle that flew an example would fly the wrong
payloads.

*See:* "Running tcspecial under a supervisor" in the `tcspecial manual
<tcspecial.html#running-tcspecial-under-a-supervisor>`__.

9. Keep the configurations the vehicle must never lose
======================================================
*Do:* decide which configurations aboard are never to be removed, and set
``configurations_kept`` to at least that many. A configuration installed
when the vehicle is provisioned is written read-only.

*Why:* configurations aboard are named by their checksum, and the ground
restarts the vehicle onto one by naming it. The one the operators mean never
to lose -- once called the golden configuration -- is simply one they do not
remove; which that is, is their policy, not the protocol's.

*See:* "Configurations named by their checksum" in the `design
<tcspecial-design.html#configurations-named-by-their-checksum>`__.

10. Agree the configuration with the ground
===========================================
*Do:* give the ground station the same configuration file the vehicle flies.

*Why:* the ground sends a CONNECT carrying the checksum of its configuration,
and the vehicle sends nothing but beacons and answers, and acts on no command
but the few that always go, until the two checksums match. A ground station
with another configuration is told so rather than misreading what the
vehicle sends.

*See:* "Starting, and the restart flag" in the `design
<tcspecial-design.html#starting-and-the-restart-flag>`__.
