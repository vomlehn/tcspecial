===================
tcs-conflict Design
===================

.. contents:: Contents
   :depth: 2
   :local:

How tcs-conflict, the check that two configurations can run at once, works
and why.

Two tcspecials may run on one machine -- a desk running two vehicles, a
test bench -- or on one network. Each configuration can be right by itself
and the pair still wrong: both taking commands at one port, both reading
one serial line, both keeping one state file. The second tcspecial to
start is refused where it can be (a port it cannot bind, a device or a
state file the first has locked: see "No two tcspecials want the same
device" in the tcspecial design), but a refusal at a start is a refusal
found late, on the vehicle, and some of what is shared refuses nothing. So
the pair is checked on the ground, before either runs.

The rules are ``tcsprotocol::graph::conflict``'s, the check that prints
them with their files and lines is ``tcsverify::conflict``, and the
program is the crate **tcs-conflict**, as tcs-verify's are.

Requirement
    tcs-conflict takes two configurations, or two configurations each
    followed by its simulator file: ``FIRST SECOND`` or ``FIRST FIRST-SIM
    SECOND SECOND-SIM``. A configuration is named ``.ytcs`` or ``.xtcs``,
    and a file named otherwise where a configuration belongs is refused,
    since it is a command line in the wrong order. Any other shape is
    refused, with status 2 and the usage.

Requirement
    Each configuration is first verified as tcs-verify verifies it, with
    its simulator file where one is given, and every finding printed as
    tcs-verify prints it. Where either has an error, nothing is compared:
    what a configuration holds is listed from what it says, and a list
    made from a file with errors in it would be a list of half-read
    things.

Requirement
    What each configuration holds is listed by the functions its own
    checks use, so that the check of one configuration and the check of a
    pair cannot disagree about what a node claims:

    * every socket, as ``tcsprotocol::graph::sockets`` lists it -- the
      ``tcspecial`` section's links, each source's link to the ground,
      each node's own address and each ``sim_address``;
    * every claim of every node that is opened, as the rule that no two
      nodes want one thing claims it -- a device path, an address on an
      I\ :superscript:`2`\ C bus, a GPIO line, a CAN identifier on an
      interface, a raw IP protocol from a host -- a port being a socket
      and compared as one;
    * the files and directories the configuration writes: ``state_file``,
      ``configurations``, ``log.directory`` and each log node's
      ``directory``, each compared as it really is where it can be found,
      links followed, and as written otherwise;
    * with a simulator file, where tcs-sim answers: its ``tcs_sim.ready``
      address and each simulated payload's ``payload_address``.

Requirement
    Everything both hold is reported against the second configuration, at
    the line that says it, naming the first's file and line, in
    tcs-verify's shape: ``file:line: error: message``. It is an
    **error** where the second tcspecial could not start beside the first
    or would take what the first is given: a socket, a device, an address
    on a bus, a line, a file or directory. It is a **warning** where two
    processes may share it by design:

    * a path that is not a device -- a named pipe, a socket, a plain file
      -- or that is not on this machine to look at, being right for a
      pipe one writes and the other reads;
    * a CAN identifier or a raw IP protocol, every socket on one being
      handed every frame, so that neither loses any but both may send.

Requirement
    ``--error-limit``, ``--warnings`` and ``--fatal-warnings`` are
    tcs-verify's, spelled and read by the same code
    (``tcsverify::flags``), and the exit status is tcs-verify's: 0 where
    no error was printed, 1 where one was or a warning under
    ``--fatal-warnings``, 2 for a command line of another shape.

Requirement
    A pair is compared, not a set: three tcspecials are three pairs. A
    configuration's includes are followed as every reader follows them,
    to any depth, an include loop refused (see ``Graph::load``).

    **Built** (2026-11-14).
