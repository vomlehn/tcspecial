=================
tcs-verify Manual
=================

.. contents:: Contents
   :depth: 2
   :local:

Introduction
============

``tcs-verify`` reads a configuration and says what is wrong with it, without
opening a socket or a device node. It takes one file, two or three, the
tcspecial configuration first, and ``-h`` or ``--help`` prints how:

.. code-block:: console

   $ cargo run --bin tcs-verify -- examples/minimal/minimal.ytcs
   $ cargo run --bin tcs-verify -- examples/minimal/minimal.ytcs \
       examples/minimal/minimal-sim.yaml
   $ cargo run --bin tcs-verify -- examples/minimal/minimal.xtcs \
       examples/minimal/minimal-sim.yaml

Either spelling of the configuration will do -- the last line reads the XML of
the same set -- and the simulator configuration is named in full rather than
worked out from the first file's name.

Three files are both spellings of one configuration and its simulator file:
an XML file and a YAML file, in either order, and then the simulator file.

.. code-block:: console

   $ cargo run --bin tcs-verify -- examples/thermostat/thermostat.xtcs \
       examples/thermostat/thermostat.ytcs \
       examples/thermostat/thermostat-sim.yaml

Each spelling is checked with the simulator file, the first named first, and
then the two are compared: every section, every attribute and every value, as
the programs read them -- which is what a configuration's digest is taken over,
so two spellings this finds no difference between are two that tcspecial and
tcs-moc agree on. Files a configuration includes are compared with it, each
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

   $ cargo run --bin tcs-verify -- /tmp/draft.ytcs
   /tmp/draft.ytcs:33: error: "both" states poll_interval and a trigger: a node that is asked is read when it has been asked, so say trigger_interval
   /tmp/draft.ytcs:23: warning: "udp-steady": read_size must be more than nought -- a read of no bytes is not a read, and a node that is not read says nothing about it
   /tmp/draft.ytcs:29: warning: "random-device" is a device-file node, so port does not locate it: it is located by path

Each line says which kind it is. An **error** is a configuration that says
something wrong -- two nodes of one name, a link to a node that is not there,
a node whose type does not exist (the message lists the types each section
takes), two things on one port, message ID or APID, or a port counted past
65535. A **warning** is about one node's own attributes, or a node nothing
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
tcs-moc, each reporting one of them. This asks the same rules -- the library's,
the ones those programs use -- for all of their answers at once.

It prints everything it finds, in the order it finds it. Two options change
that::

   $ cargo run --bin tcs-verify -- --error-limit=unlimited example.ytcs
   $ cargo run --bin tcs-verify -- --warnings=off --error-limit=1 example.ytcs

``--digest`` asks for each configuration's digest as well, one line per file
and before the problems::

   $ cargo run --bin tcs-verify -- --digest examples/thermostat/thermostat.ytcs
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

   $ cargo run --bin tcs-verify -- --types=$(tcspecial --types) \
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
in for: a payload that is a program, which tcspecial runs itself. That is the question tcs-sim
and tcs-moc ask before a simulator is started, so a set tcs-verify passes is one
they start. A tcspecial configuration with problems of its own is
not joined to -- there would be nothing to join to, and a join against half of
a set reports the other half as unsimulated -- and tcs-verify says so rather
than leaving it unsaid.

Summary
=======

``--summary``, with one configuration and its simulator file or alone,
checks it as any run does and then prints what the vehicle uses of what
must not clash or overflow, so that somebody assembling a vehicle from many
nodes sees it in one place:

* **Addresses and ports**: every socket the configuration states or
  implies, who binds it -- the vehicle, the ground, a payload, tcs-sim, or
  a software bus's far end -- and what it is. The ``tcspecial`` section's
  links (``tcs_cmd``, ``payload_command`` and ``beacon``, or ``sbn`` on a
  software bus); each gateway node's far end, its ``ground_base`` or
  ``bridge_base`` plus each source's device id, by source; each untouched
  payload's ``payload_telemetry`` ports, plus its device id at both ends;
  each gateway node's and network payload's own address and port; and each
  ``sim_address``. ``payload_command.dest_base`` is one socket, with
  nothing added to it: the node a command is about is in the command.
* **Device ids**: every node's, which every counted port and ID is counted
  by -- its place among the configuration's nodes sorted by name.
* **Message IDs**, where the configuration is on a cFS software bus: the
  ``msgid_range`` allotted, and every ID in it, command or telemetry, and
  what it carries.
* **APIDs**, where its packets are CCSDS Space Packets, under Yamcs.
* **Rates to the ground**: for each source's link, how many data a second
  it carries, the bytes of each datum where its type gives them, and the
  bytes a second; the beacon's; and the total. A node read on a schedule --
  its group's ``interval``, its ``trigger_interval``, its ``poll_interval``
  (at most), or a set of lines' ``state_interval`` -- sends as often as
  that. Any other node sends at most one datum for each that arrives at
  it, shown as ``<=``; one with no schedule and nothing linked into it
  sends when it has something, which no file says, shown as ``?`` and left
  out of the total, which says how many were. The bytes are the data's
  alone, not the packets', and a string or bytes has no size its type
  gives.

The lists are each in an order no file can change -- by kind, then by
gateway node and device id, then by name -- so two summaries can be
compared with ``diff``. The summary of the thermostat example:

.. code-block:: console

   $ cargo run --bin tcs-verify -- --summary examples/thermostat/thermostat.ytcs
   examples/thermostat/thermostat.ytcs: summary

   Addresses and ports
     address         bound by  what
     0.0.0.0:4000    vehicle   tcs_cmd.recv, where spacecraft commands are taken
     127.0.0.1:4100  ground    tcs_cmd.send, where spacecraft commands come from
     0.0.0.0:4200    vehicle   payload_command.dest_base, where node commands are taken
     127.0.0.1:4300  ground    payload_command.source_base, where node commands come from
     0.0.0.0:5550    vehicle   beacon.send, where beacons come from
     127.0.0.1:5650  ground    beacon.recv, where beacons go
     127.0.0.1:5100  ground    "gateway" ground_base plus device id 0: what "consensus_temp" sends
     127.0.0.1:5102  ground    "gateway" ground_base plus device id 2: what "gpio-out-tmass1" sends
     127.0.0.1:5103  ground    "gateway" ground_base plus device id 3: what "gpio-out-tmass2" sends
     127.0.0.1:5108  ground    "gateway" ground_base plus device id 8: what "to_k1" sends
     127.0.0.1:5109  ground    "gateway" ground_base plus device id 9: what "to_k2" sends
     127.0.0.1:5110  ground    "gateway" ground_base plus device id 10: what "to_k3" sends
     127.0.0.1:5000  vehicle   "gateway" address and port, where the ground sends to it
     127.0.0.1:5204  tcs-sim    "gpio-out-tmass1" sim_address
     127.0.0.1:5205  tcs-sim    "gpio-out-tmass2" sim_address
     127.0.0.1:5201  tcs-sim    "temp_1" sim_address
     127.0.0.1:5202  tcs-sim    "temp_2" sim_address
     127.0.0.1:5203  tcs-sim    "temp_3" sim_address

   Device ids
     id  node
      0  consensus_temp
      1  gateway
      2  gpio-out-tmass1
      3  gpio-out-tmass2
      4  temp_1
      5  temp_2
      6  temp_3
      7  thermostat
      8  to_k1
      9  to_k2
     10  to_k3

   Message IDs
     none: the configuration is not on a software bus

   APIDs
     none: the configuration's packets carry no CCSDS header

   Rates to the ground
     link                        data/s  bytes/datum  bytes/s  how
     consensus_temp to gateway      <=3            4     <=12  one per datum from to_k1, to_k2, to_k3
     gpio-out-tmass1 to gateway       1            1        1  state_interval 1 s
     gpio-out-tmass2 to gateway       1            1        1  state_interval 1 s
     to_k1 to gateway               <=1            4      <=4  one per datum from temp_1
     to_k2 to gateway               <=1            4      <=4  one per datum from temp_2
     to_k3 to gateway               <=1            4      <=4  one per datum from temp_3
     beacon                         0.2            ?        ?  beacon.default_interval 5 s
     total                        <=8.2                  <=26

The cfs example's lists its message IDs instead, beginning:

.. code-block:: console

   Message IDs
     msgid_range 0x4200 to 0x42ff
     ID      carries    what
     0x4200  telemetry  tcs_cmd.response_msgid
     0x4201  telemetry  beacon.tlm_msgid
     0x4210  telemetry  the answer message ID of "fsw", payload_command.response_base plus its device id 0

The summary says what is there; what is wrong with it is said with the
other problems, above it, by the checks every run makes:

* Two sockets that are one -- one port of one protocol on one host,
  ``localhost``, ``127.0.0.1`` and ``::1`` being one host, or a port on
  every interface (``0.0.0.0``) and the same port on this host -- are an
  **error**, naming both: whichever binds second would fail as it starts.
  Two nodes at one address was checked already, and so were
  ``tcs_cmd.recv`` and ``payload_command.dest_base`` written alike. The
  rest is new with the summary: any of the section's sockets, a source's
  port on the ground, an untouched payload's port aboard, a node's address
  and a ``sim_address``, against any other of them. A wildcard and a host
  named otherwise are not compared, since whether ``10.0.0.5`` is this
  machine is the network's to say; and a sender given port ``0`` claims no
  port.
* Two sources on one port of the ground, and a port counted past 65535
  from a base, were checked already, as were a message ID used twice,
  outside ``msgid_range`` or of the wrong type for its use, an APID used
  twice or past ``0x7FF``, and an F´ ID shared or past 32 bits. Each is
  an **error**, which fails the run, whether it is said about a node or
  about the ``tcspecial`` section: a clash is not a node half filled in
  but a configuration whose every field is answered and which cannot be
  flown as it stands.

A configuration that does not read has no summary: the run says so on
stderr, after the problems that say why, and exits 1, as a digest that
cannot be taken does. With two spellings and a simulator file there is
none either -- the two say one thing -- and the command line is none of
the shapes.

See Also
========

`The TCSpecial manuals <manual.html>`__
    What each of TCSpecial's manuals covers.

`The tcspecial manual <tcspecial.html>`__
    The flight software, its configuration files, the examples, installing and
    running the programs.

`The tcs-gui manual <tcs-gui.html>`__
    The editor, which checks a configuration as it is edited.

`The TCSpecial design <design.html>`__
    How TCSpecial works inside, and why.

