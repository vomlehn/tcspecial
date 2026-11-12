================
tcsverify Manual
================

.. contents:: Contents
   :depth: 2
   :local:

Introduction
============

``tcsverify`` reads a configuration and says what is wrong with it, without
opening a socket or a device node. It takes one file, two or three, the
tcspecial configuration first, and ``-h`` or ``--help`` prints how:

.. code-block:: console

   $ cargo run --bin tcsverify -- examples/minimal/minimal.ytcs
   $ cargo run --bin tcsverify -- examples/minimal/minimal.ytcs \
       examples/minimal/minimal-sim.yaml
   $ cargo run --bin tcsverify -- examples/minimal/minimal.xtcs \
       examples/minimal/minimal-sim.yaml

Either spelling of the configuration will do -- the last line reads the XML of
the same set -- and the simulator configuration is named in full rather than
worked out from the first file's name.

Three files are both spellings of one configuration and its simulator file:
an XML file and a YAML file, in either order, and then the simulator file.

.. code-block:: console

   $ cargo run --bin tcsverify -- examples/thermostat/thermostat.xtcs \
       examples/thermostat/thermostat.ytcs \
       examples/thermostat/thermostat-sim.yaml

Each spelling is checked with the simulator file, the first named first, and
then the two are compared: every section, every attribute and every value, as
the programs read them -- which is what a configuration's digest is taken over,
so two spellings this finds no difference between are two that tcspecial and
tcsmoc agree on. Files a configuration includes are compared with it, each
with its own spelling. A difference is reported at the line of the second
spelling it is on, in whichever of its files that is, saying what each says:
the thermostat is in the file the thermostat example includes.

.. code-block:: console

   examples/thermostat/../thermostat-common/thermostat.ytcs:72: error: nodes / thermostat / attributes / setpoint: examples/thermostat/thermostat.xtcs says "290" and examples/thermostat/thermostat.ytcs says "293.15"

The order anything is written in is not compared: nothing in a configuration
is in an order that means anything, and a node's device id comes from its
name, not from where the file writes it. A list is compared as the set it is,
so one entry missing is one difference. A file that cannot be read is not
compared, and says so.

A good set prints nothing. A bad one prints a line per problem, each naming
the file, the line and what is wrong there:

.. code-block:: console

   $ cargo run --bin tcsverify -- /tmp/draft.ytcs
   /tmp/draft.ytcs:33: error: "both" states poll_interval and a trigger: a node that is asked is read when it has been asked, so say trigger_interval
   /tmp/draft.ytcs:23: warning: "udp-steady": read_size must be more than nought -- a read of no bytes is not a read, and a node that is not read says nothing about it
   /tmp/draft.ytcs:29: warning: "random-device" is a device-file node, so port does not locate it: it is located by path

Each line says which kind it is. An **error** is a configuration that says
something wrong -- two nodes of one name, a link to a node that is not there,
a node whose type does not exist (the message lists the types each section
takes). A **warning** is about one node's own attributes, or a node nothing
is linked to: an attribute not in yet, and one that is there and wrong, as
both warnings above are. The first is the state every configuration is in
while it is being drawn, and a reader of either has the same thing to do --
open that node and look at its words -- so warnings do not fail the run.

It exits 0 when it printed no errors, 1 when it printed one -- a difference between
two spellings is one -- and 2 when the command line is none of those shapes,
including three files that are not one XML and one YAML configuration and a
simulator file -- which a script can tell apart from a bad file. ``--help``
exits 0. A configuration with warnings and no errors exits 0, and the vehicle
will still refuse it: the flight software requires every attribute a node
needs.

Every program that runs a set refuses the first thing wrong with it and stops,
which is what a program that cannot run without it should do and is not what a
person fixing the file wants: a set with three mistakes in it is three runs of
tcsmoc, each reporting one of them. This asks the same rules -- the library's,
the ones those programs use -- for all of their answers at once.

It prints everything it finds, in the order it finds it. Two options change
that::

   $ cargo run --bin tcsverify -- --error-limit=unlimited example.ytcs
   $ cargo run --bin tcsverify -- --warnings=off --error-limit=1 example.ytcs

``--digest`` asks for each configuration's digest as well, one line per file
and before the problems::

   $ cargo run --bin tcsverify -- --digest examples/thermostat/thermostat.ytcs
   examples/thermostat/thermostat.ytcs: md5 bd9fd88e7cf85dbd02d6cb834f0b9eaf

That is the digest tcspecial prints as it starts and puts in every beacon, and
the MOC compares with its own: two ends whose digests agree have read one
configuration, and the one case this was written for is a tcspecial left
running from an earlier file and attached to rather than replaced. So the
answer to "is the vehicle flying what is on my desk" is this line and the
beacon's.

With two files it is the digest of the first, and with three the digests of
the first two: a payload simulator file has none. It does not fly, so there is
nothing at the other end to compare one with, and it is not a configuration to
be digested.

``--types=`` checks a configuration for a tcspecial built with only the
payload types it names, separated by commas, and reports each node of another
type as that tcspecial would refuse it. It takes what ``tcspecial --types``
prints, so a configuration can be held to the binary that will fly::

   $ cargo run --bin tcsverify -- --types=$(tcspecial --types) \
       examples/thermostat/thermostat.ytcs

Without it, every type is taken to be built. See `Building tcspecial for a
vehicle <tcspecial.html#building-tcspecial-for-a-vehicle>`__.

The digest is of what a file **says** and not of its bytes, so ``md5sum`` of
the same file answers something else. Comments, indentation, the order of the
sections and the order of a node's attributes are spelling; the two spellings
of one configuration -- the YAML and the XML -- have the same digest, which is
what makes a MOC reading one of them and a vehicle reading the other able to
agree. A configuration with problems in it still has a digest, it having said
something; one that cannot be read at all has none, and a run asked for a
digest it could not take says so and fails.

``--error-limit=<n>`` stops as soon as ``n`` errors have been printed, the
``n``\ th being the last line: any whole number, ``0`` included, or
``unlimited``, the default. In a half-built configuration most of the later
errors are one unfinished node said another way, and a limit keeps the first of
them on the screen.

``--warnings=<what>`` says what is done with warnings. ``on``, the default,
prints them and does not count them against the limit, so they are printed for
as far as the run got. ``off`` prints none. ``errors`` prints them and counts
them against the limit as errors are -- for the limit only: they still do not
fail the run.

The exit status is about what was printed: 1 if an error was, 0 if none was.
So a run that stopped before it reached an error passes, as
``--error-limit=0`` always does, having been asked for none of the reasons. A
value either option does not take is a command line of none of the shapes, and
exits 2.

Both files of the set are checked, and the second against the first: every
payload simulated, every simulated payload a payload, and every setting one
that payload can have. And none simulated that the simulator would not stand
in for: a payload that is a program, which tcspecial runs itself. That is the question tcssim
and tcsmoc ask before a simulator is started, so a set tcsverify passes is one
they start. A tcspecial configuration with problems of its own is
not joined to -- there would be nothing to join to, and a join against half of
a set reports the other half as unsimulated -- and tcsverify says so rather
than leaving it unsaid.

See Also
========

`The TCSpecial manuals <manual.html>`__
    What each of TCSpecial's manuals covers.

`The tcspecial manual <tcspecial.html>`__
    The flight software, its configuration files, the examples, installing and
    running the programs.

`The tcsgui manual <tcsgui.html>`__
    The editor, which checks a configuration as it is edited.

`The TCSpecial design <design.html>`__
    How TCSpecial works inside, and why.

