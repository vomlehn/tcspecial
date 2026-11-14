===================
tcs-conflict Manual
===================

.. contents:: Contents
   :depth: 2
   :local:

Introduction
============

``tcs-conflict`` says whether two tcspecial configurations can be run at
once, on one machine or one network. It checks each as ``tcs-verify``
does, then everything both want: a socket, a device, an address on a bus,
a GPIO line, a state file, a log directory, the directory configurations
are kept in, and, with simulator files, where tcs-sim answers.

.. code-block:: console

   $ cargo run --bin tcs-conflict -- vehicle-a.ytcs vehicle-b.ytcs
   $ cargo run --bin tcs-conflict -- vehicle-a.ytcs vehicle-a-sim.yaml \
       vehicle-b.ytcs vehicle-b-sim.yaml

Each configuration comes before its simulator file. ``-h`` or ``--help``
prints how it is run.

What it says
============

Nothing, with status 0, where the two can run together. Otherwise one line
for each thing both want, against the second configuration, naming the
first's file and line:

.. code-block:: text

   b.ytcs:7: error: both configurations have tcs_cmd.recv, where spacecraft
   commands are taken at 0.0.0.0:4000: whichever binds it second fails as it
   starts. Give one of them another (the other is at a.ytcs:5)

An **error** is something the second tcspecial could not have beside the
first: a socket, a device, an address on a bus, a line, a state file or a
directory it writes. A **warning** is something two processes may share on
purpose -- a named pipe one writes and the other reads, or a CAN identifier
both see every frame of -- and is worth a look rather than a change.

Where either configuration has an error of its own, that is printed and
nothing is compared.

To run two on one machine, give the second its own ``state_file``,
``log.directory``, ``configurations`` directory, command, beacon and
telemetry addresses, and devices.

Options
=======

``--error-limit=N``, ``--warnings=on|off|errors`` and ``--fatal-warnings``
are tcs-verify's: see `the tcs-verify manual <tcs-verify.html>`__.

Exit status: 0 where no error was printed, 1 where one was or a warning
under ``--fatal-warnings``, 2 where the command line was not one of the
shapes above.
