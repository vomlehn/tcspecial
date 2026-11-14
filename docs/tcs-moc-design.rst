==============
tcs-moc Design
==============

.. contents:: Contents
   :depth: 3
   :local:

How tcs-moc, the simulated operations center, works and why.

The tcs-moc is a GUI program used to control simulated payloads interacting with
tcspecial using the TCSpecial MOC Correspondent (tcsmoc) over a datagram
connection to tcspecial.
For testing purposes, tcs-moc uses tcsmoc, along with simulated payloads, to
support a simple GUI.

Tcs-moc gets the payload definition from a file named on its command line, and
reads the shipped fixture when the command line names none. It takes an argument
rather than an environment variable because it starts tcspecial and tcs-sim as
subprocesses and they inherit its environment: a variable naming tcs-moc's file
would name theirs too, and so could not point tcs-moc at one file and its
children at another. An argument belongs to tcs-moc alone.

Requirement
    Tcs-moc accepts one tcspecial configuration file. A second path is an error
    rather than an argument with no effect, because it is more likely a mistake
    about which file is being read than something meant to be ignored.

The beacon box carries the indicator and, beside it, the time the last beacon
arrived.

Requirement
    To the right of the indicator a field that cannot be edited reads
    ``Restarted—connect`` while the last beacon's restart flag said
    ``restart``, and is empty while it said ``normal``. The flag is set
    from every start of tcspecial, and from its taking a RESTART, until it
    takes a CONNECT carrying its configuration's checksum, so the field
    says what the operator is to do next: press Connect, with the
    configuration whose checksum the beacons carry, which clears it. The ``beacon`` example's
    window, which commands nothing and so cannot connect, does not show it.

Requirement
    When a wait on a restart ends, ``Last Response`` says how: ``tcspecial
    has started again as commanded: Connect before anything else``, or
    ``tcspecial started again without being commanded to, so the restart
    initiated here did not happen: Connect before anything else``.

Requirement
    The time the last beacon arrived is shown, and is the time it arrived at
    the MOC rather than the spacecraft time the beacon carries. The line is
    read to find out whether beacons are still coming, so a spacecraft whose
    clock had stopped must not look as though its beacons had. It is shown the
    way every other time in the window is, by the one function that shows them.

Requirement
    The time survives the passes that receive nothing, and a socket error as
    well. The colour beside it is what ages; the beacon arrived when it
    arrived, and blanking the line on a timeout would say that none ever had.

Requirement
    Until a beacon has arrived the line says so, in the words a panel uses for
    a direction that has carried nothing. It was this and nothing else for the
    life of the program before the time was ever set: the window declared the
    line, the receiver set the colour beside it, and the line itself said no
    beacon had arrived while the colour said they were arriving steadily.

Requirement
    The beacons also say whether the spacecraft is being heard from at all,
    which the panels act on. Contact is held for as long as the last beacon's
    age is within the leading run of stages that count as being heard from --
    blinking or steady, an indicator that has begun to blink being beacons
    that are arriving and aging rather than gone -- and is lost past that run,
    when no beacon has ever arrived, and when the system clock has moved so
    that the last one is in the future. One reading serves both the indicator
    and the panels, so the two cannot say different things about one beacon.

Requirement
    The indicator is grey before any beacon has arrived, and never again after
    one has. Wherever it shows no colour once a beacon has arrived -- the dark
    half of a blink, a last beacon a moved clock puts in the future -- it is
    white. Grey says nothing has been heard; an indicator that went from green
    back to grey would say the spacecraft had never been heard from, and look
    like a window that had just opened. ``tcs_ui::indicator`` is the rule,
    and the Python panel says it again with the same tests.

**Where the beacon's code lives**

Not here. A beacon is the one thing a ground station knows without asking, so
every ground station wants it, and what it means does not depend on the window
it is shown in.

Requirement
    What a beacon is and what has become of it is a library's: the socket, the
    datagram, the digest comparison, the beacons that did not arrive and how
    stale the last one is. No toolkit is linked into it, because a ground
    station with no screen -- a logger, a test, a program that only watches --
    needs the facts and not the look. It is ``tcsmoc::beacon``.

Requirement
    How stale the last beacon is, is a named stage and not a colour. A stage
    is what is true of the spacecraft; a colour is one way of saying it, and
    the stages are what both the indicator and the panels are worked out from.
    The stages and their timings are one schedule, which is the library's, so
    two ground stations watching one spacecraft cannot differ about how late a
    beacon is.

Requirement
    The panel is shared markup, imported by every window that shows it, with
    what to show on it worked out beside it: ``tcs-ui``. Two windows show it
    -- this one, beside its command link, and the ``beacon`` example, which
    shows it and nothing else -- and an operator who has read one has read the
    other, where an indicator in a different place or a label in different
    words would be two things to learn about one spacecraft.

Requirement
    What is left in a ground program is the wiring: read the configuration to
    find where the beacons are, run the loop on a thread, put each pass in the
    window. The ``beacon`` example is that and nothing else, which is what
    makes it worth reading.

Requirement
    A panel starts unknown and stays unknown until somebody asks. A MOC that
    has just opened has been told nothing about any handler, so a panel
    claiming anything is a panel claiming what nobody said: it claimed
    Stopped, and was commonly wrong, since the MOC attaches to a tcspecial
    that is already serving the set rather than starting a second one, and
    that one's handlers may have been moving data for hours. Enable, Disable
    and Query, in the Command menu, are what ask.

Requirement
    A panel's status is what a ``START_NODE`` or ``STOP_NODE`` was answered,
    and the beacons are what can take it away. Beacons stopping makes every
    panel's status unknown: the status is the answer to a command and is worth
    as much as the ground's hearing from the spacecraft, where a node may
    since have stopped or gone with a process that restarted, and a panel
    still claiming a state would be read as a reading. It is said once, when
    contact is lost, rather than every pass of the silence.

Requirement
    The panels are made unknown whether or not there is a command link. A MOC
    that has been disconnected cannot find out either, and the beacon is what
    it has to go on.

Requirement
    The spacecraft is asked what its handlers are doing at two moments, and
    only two: when the first beacon arrives, and when a beacon outage ends.
    Both are a MOC that has just learnt the spacecraft is there and knows
    nothing about its payloads -- it may have been serving them for hours, or
    have restarted during the silence with none of them running.

Requirement
    A moment owes the question; it is asked at the first chance after it.
    Neither moment can always ask at once: every command but CONNECT,
    RESTART_ARM and RESTART waits for the window to be matched for
    commanding (see
    `Matching the configurations <tcs-moc.html#matching-the-configurations>`__),
    which needs a clear beacon carrying this window's
    checksum and a CONNECT of its own answered since the match last ended.
    The first beacon usually arrives with the link down, the window having
    just opened, and a fresh start's carries the restart flag; and an outage
    ends the match. So the question is owed from the moment, for as long as
    contact holds, and asked at once where the window is matched, at the
    Connect that matches it, or at the clear beacon that does. Losing contact
    forgets it, the next contact owing its own, and a Connect pressed with
    nothing owed asks nothing: a third moment would be a question nobody's
    reading of the beacons called for.

Requirement
    At the end of an outage the window sends the CONNECT itself, where
    nothing can have changed what it would say: the link is still up, its
    last CONNECT agreed, and the returning beacons carry this window's
    checksum with the restart flag clear and no start of tcspecial heard
    (one is the operator's to Connect to, and takes the link down). The
    operator chose to command this vehicle with that Connect; an outage is
    the vehicle not being heard, not the operator changing their mind, and a
    window that waited for Disconnect and Connect to ask what it owed would
    leave every panel Stale until somebody thought to press them.

Requirement
    A payload whose last press was Disable is not asked about. The operator
    said to stop it; asking the spacecraft would spend a command to be told
    what the ground already knows, and a panel saying Stopped because
    somebody stopped it is not a panel with a question hanging over it. An
    Enable clears that, a Disable sets it, and a payload nobody has pressed
    anything about is asked.

Requirement
    The asking happens off the event loop, on a thread of its own, and what
    comes back is handed to the window's thread to paint as it arrives. Each
    question waits on the spacecraft for as long as the timeout allows, and
    a burst of them on the event loop would stop the window repainting at
    exactly the moment an operator is watching it.

Nothing is asked while the command link is down. The question owed is held
until the link comes up only while contact holds since the moment that owed
it: a Connect pressed then is the operator choosing the moment, and it is
the first moment the first beacon's question can be asked at all, since the
window opens with the link down. It is not held across a loss of contact,
so a question is never owed indefinitely, to go out at a moment nobody
chose.

Requirement
    The ground does not ask about its payloads on a timer. It asks at the two
    moments above and when somebody chooses Query, and otherwise watches
    what arrives.

    An enabled payload sends on its own -- that is what enabling it does --
    and every datagram it sends arrives on that payload's own telemetry
    socket, where the ground times it. So the ground has what it needs to say
    whether a payload is still delivering without asking anybody.

    There was a poller: a thread that asked every payload what it had moved
    once a second and painted the panels from the answers. It cost a command
    and a response per payload per second -- for forty payloads, eighty
    datagrams a second of housekeeping on the link the spacecraft is also
    commanded over -- and most of what it learnt the arrivals had already
    said. It was also the MOC's second sender of payload commands, which is
    why the ground's end of that link could not be a socket the configuration
    names.

Requirement
    A panel says when its source's data last arrived and nothing more about
    whether that was recent enough. Nothing in a configuration states how
    often a node's data should arrive -- a thermostat's heaters hear from it
    when a decision changes, which may be hours apart -- so an indicator that
    judged it would be an indicator whose colour means nothing, and one that
    never changes is worse than none.

Requirement
    Query still asks the spacecraft directly, and sets both the status and
    the counters from what it answers. It is the one path by which the
    spacecraft's own account of a handler -- its state, its byte counters,
    the triggers it has sent -- reaches a panel, and somebody pressing it is
    somebody who wants exactly that.

Every program is pointed at its tcspecial configuration the same way: as a
command line argument, failing that the program's own environment variable --
``PAYLOAD_CONFIG_PATH`` for tcspecial and ``SIM_PAYLOAD_CONFIG_PATH`` for
tcs-sim, while tcs-moc has none for the reason below -- and failing that
the shipped fixture. Tcs-sim names its simulator configuration separately again,
through ``PAYLOAD_SIM_YAML``.

Requirement
    An argument naming the tcspecial configuration takes precedence over any
    environment variable naming one. The argument is the unambiguous of the
    two: tcs-moc starts the other programs and they inherit its environment, so
    a variable can reach further than it was meant to, where an argument
    reaches exactly the program it is given to.

Requirement
    One configuration file is named, and no other file. A further path is an
    error rather than one with no effect, because it is more likely a mistake
    about which file is being read than something meant to be ignored. A flag
    is not a path: tcspecial's ``--quiet`` and ``--types`` (see `Which Node
    Types Are Built <tcspecial-design.html#which-node-types-are-built>`__), and the ``--simulate`` all three programs take (see
    `Simulation Modes <tcspecial-design.html#simulation-modes>`__), say how a program runs and not what it reads.

Where the Addresses Are
-----------------------
Requirement
    No address appears on tcspecial's command line. The configuration file is
    the only thing it names, and its flags say how it runs, not where anything
    is.

Requirement
    The address and ports the command interpreter binds are in the
    configuration's ``tcspecial`` section, so one file says where commands are
    taken and what is flown.

Requirement
    The command interpreter takes commands on two links: one for the
    spacecraft and one for every node. A command about the spacecraft --
    ``PING``, ``RESTART_ARM``, ``RESTART``, ``CONFIGURE`` -- arrives on
    ``tcs_cmd.recv``; a command about a node -- ``START_NODE``, ``STOP_NODE``,
    ``SET_NODE``, ``QUERY_NODE``, ``QUERY_NODE_SAMPLE`` -- arrives on
    ``payload_command.dest_base``, with nothing added to it, and is answered
    there. ``CONNECT`` is the spacecraft's: it asks whether there is a
    connection to the vehicle, which is what that link is, and a ground
    station without one has nothing to ask a node. A ``CONNECT`` that arrives
    on the node link is refused like any other command on the wrong one.

    One socket for every node: a node command carries the node's id, so the
    socket does not have to say which node a command is about.

Requirement
    Each payload's link is served by a loop in a thread of its own, and every
    socket is bound before any loop runs -- so a port already in use is
    reported at startup, naming the payload whose link wanted it.

Separate links because what the ground needs in a hurry must never be queued
behind what it asked for at leisure. A ``RESTART`` waiting behind a payload's
statistics is a spacecraft that cannot be rescued while it is busy, and the
two types of command are unlike in every way that matters: one is rare,
urgent, and about the vehicle, the other routine, bulky, and about an
instrument. Missions divide them for the same reason, often between different
operators with different authority. One link per payload divides them again,
for a smaller version of the same reason: a payload that takes a second to
start should not hold up a question about one that would answer at once.

On a space link the division is by virtual channel, which is where the
priority between them is really decided and where a command link gets COP-1's
sequence control. Over IP there is no such thing, so it is two ports -- what
an IP network has to divide traffic with. A payload set that is flown rather
than tested would map ``tcs_cmd.recv`` to the command virtual channel and the
payload links to payload ones.

Requirement
    One table says which link a command belongs on, read by the ground when
    it sends and by the spacecraft when it answers:
    ``CommandType::link``. The two ends cannot come to disagree about where a
    command belongs, which is a disagreement nothing could report except as a
    command that went unanswered.

Requirement
    The table is a match over every type of command, not a rule with a
    default. A command added later is placed deliberately, by someone who has
    to decide, rather than by whichever side of a default it fell on.

Requirement
    Both ports are required of every command interpreter configuration, with
    no default, as the beacon attributes are: where commands are taken is not
    a question to be answered by whatever a file was silently given.

Requirement
    The two ports are two ports. A configuration giving both the same number
    describes a split it has not made, and the half of it that bound second
    would fail at startup with the other half's address.

Requirement
    No payload may bind a port a command link binds. It is the one collision
    that does not stop a payload from working -- it stops the commanding that
    would have started it -- and which of the two fails depends on the order
    the programs happened to start in. A command link bound to every
    interface, which ``0.0.0.0`` is, takes its port on every address, so a
    payload at ``localhost:4000`` collides with a link at ``0.0.0.0:4000``
    without either naming the other's host.

Requirement
    Each link is served by a loop of its own, in a thread of its own. Two
    sockets read by one loop are two queues into one bottleneck: a payload
    command that takes its time -- a ``START_NODE`` opens the node, which may
    be a device -- would still be ahead of whatever the ground sent next. The split is only worth having if what it separates
    cannot wait for the other.

Requirement
    Both sockets are bound as the interpreter is created, before either loop
    runs, so a port already in use is reported where every other
    configuration error is -- at startup, naming itself -- rather than when
    the first command of that type arrives.

Requirement
    Every link takes what arrives only from where the configuration says it
    comes from, and ignores the rest. tcspecial takes a spacecraft command
    only from ``tcs_cmd.send``, which is the socket the MOC binds to send
    them; the MOC takes a response only from the socket its command went to,
    which is ``tcs_cmd.recv``; it takes a beacon only from ``beacon.send``;
    and it takes a payload's telemetry only from
    ``payload_telemetry.source_base`` counted on by that payload's id. Where
    the configuration names every interface -- ``0.0.0.0``, which the vehicle's
    side of every link usually is -- the port is all there is to compare, the
    address a datagram carries then being whichever interface the route chose.

This is what writing both ends of every link down buys. A command is a command
to restart a spacecraft, and a response is what the ground believes about one:
before this, either would be acted on whoever sent it, which a bound port
invites -- another program on the host, a stale process from an earlier run, a
packet from the wrong mission on a shared network. Nothing is sent back to such
a sender, either; an answer would tell it that something is listening.

The dropped datagram is said on the console, with where it came from and where
the configuration expected it from. A link that is deaf because two files
disagree looks exactly like a link that is deaf because nothing is sending, and
the two have nothing in common to do about them.

Requirement
    A command that arrives on the link it does not belong on is refused with
    ``InvalidCommand``, and the refusal is of the telemetry type the command
    asks for. The type is how the ground pairs an answer with what it asked,
    so a refusal it cannot place reads as a command that went unanswered --
    which is the one thing a refusal exists to deny.

Serving it instead would be worse than refusing it. A command on the wrong
link means one end is not reading the configuration the other is, and that is
exactly the fault this split must not hide: the link the ground restarts the
spacecraft over is the last place to learn that the two ends disagree about
where commands go.

Requirement
    The ground sends each command on the link the table names. A client holds
    a connection to each, and which one a command goes on is read from the
    same ``CommandType::link`` the spacecraft refuses it by, so the ground
    cannot spend a round trip learning what it already knows.

Requirement
    A client has both links or neither. One would send only half of what
    there is to send, and the half it could not send is the half the panels
    use.

Requirement
    The ground asks ``CONNECT`` on the spacecraft's link alone. It is the
    question "is there a connection to the vehicle", and the vehicle's own
    link is where that is asked; a payload's link carries what is asked of a
    payload.

    What this gives up is a check: two links pointed at two spacecraft is the
    one way this arrangement can be wrong that nothing else catches -- each
    link works, each answer looks right, and the payloads being commanded are
    not the ones being restarted. It was caught by asking both links and
    comparing the answers. Catching it again means a payload command's answer
    carrying what that end read, which is where to put it if it is wanted:
    on the link whose correctness is in question, in an answer the ground
    already waits for.

Requirement
    The MOC takes the payload port from the command interpreter's own
    configuration file, which is where tcspecial takes it from. The host is
    the operator's -- it is what the address box says, and a bind address of
    ``0.0.0.0`` is no destination -- so moving the link moves both of them by
    typing one address.

Requirement
    ``tcs-verify`` reports a missing or colliding port from the line that
    states it.

Requirement
    A beacon is sent from one stated socket to another. Tcspecial makes the
    socket it sends beacons on out of ``beacon.send`` -- that is the address
    and port it binds, so every beacon leaves from there -- and sends to
    ``beacon.recv``. Tcs-moc makes the socket it receives beacons on out of
    ``beacon.recv``, binding the same address and port. Each is an address and
    a port.

A beacon was sent to a multicast group, with a second attribute naming the
local interface to join it on. That bought something real: a beacon is an
announcement to whoever is listening, the spacecraft does not know how many
ground stations there are, and multicast is the primitive for one-to-many with
no knowledge of the many. A unicast beacon reaches one listener, chosen in
advance by whoever wrote the configuration, a port being bindable once.

What it cost was two attributes that had to be right and said nothing when they
were not. The group address is nobody's, so neither end could be reached at it
to see whether the other was there. The interface attribute decided delivery
and could not be defaulted: a sender bound to every interface sends a multicast
datagram out the default route, a listener that joins with ``INADDR_ANY`` joins
on the default route, and on a host where those differ -- one with a second
interface, or a tunnel -- not one beacon arrives with nothing anywhere to say
so. Measured on a development machine with three interfaces: of the four
combinations of naming and not naming, only naming the same interface at both
ends delivered anything. A mission whose beacons stop is a mission that cannot
tell a silent spacecraft from a misjoined socket.

Two stated sockets trade the many for the saying. Each end is an address and a
port somebody wrote down, which means a firewall rule, a capture filter or a
colleague can describe the link beforehand, and either end can be probed. More
than one ground station listening is then something configured -- a relay that
fans the beacons out -- rather than something the network did silently.

Requirement
    Every configuration states where beacons come from, where they are
    received, and how often: ``beacon.send``, ``beacon.recv``,
    ``beacon.default_interval`` and ``beacon.reset_interval``, in its
    ``tcspecial`` section, and where it keeps what must survive a restart,
    ``state_file``. None has a default and none may be left out.

Requirement
    How often is a length of time, like every other length of time in these
    files: ``5 s`` rather than ``5000``. The protocol still counts
    milliseconds, the command that changes the interval carrying four bytes of
    them, so the conversion happens where the file is read and in one place:
    ``CIConfigJson::to_ci_config``.

Beacons are how the ground knows the spacecraft is alive. A configuration that
has not been asked where to send them has not answered, and a default is a way
of finding that out later -- by the beacons arriving somewhere nobody is
listening, which looks from the ground exactly like a spacecraft that has
stopped.

Requirement
    Tcs-moc reads both beacon addresses from the configuration tcspecial reads,
    through the one function that answers it:
    ``graph::parameters::beacon_of``. The two programs read one file, so they
    cannot name different addresses.

    One file states them, so there is no second answer to resolve against
    and no rule about which wins.

That is what makes the earlier drift impossible rather than merely unlikely.
The MOC used to bind a compiled-in address and read neither file, so a mission
that moved the beacon moved it for tcspecial alone and the MOC said only that
nothing was arriving. A MOC that cannot work out where to listen says so and
runs without beacons, as it does without a simulator file: controlling payloads
does not depend on it.

Requirement
    A ``beacon.send`` or ``beacon.recv`` that is not an address and a port is
    refused where it is read, naming the attribute.

Refused there rather than where a beacon is sent, because a beacon goes out on
a timer with nobody to report to: the address used to be a constant parsed with
``unwrap``, which could only ever have panicked, and a configured one has to be
caught while someone is still reading errors.

The command address was an argument for a while, and tcs-moc passed the address
it was about to send to, so that the end that listens and the end that sends
could not differ. One address in one place is the simpler rule, and it is the
rule now; what it gives up is that agreement. The MOC and a tcspecial it starts
agree about the command address by reading the same default, and about the
beacon address the same way, so a ``tcspecial.ytcs`` that moves either moves it
for tcspecial alone -- commands then go to a port nothing is bound to, which no
error on either end reports, since a UDP datagram sent where nobody is
listening is not refused. What the two ends do check on connecting is each
other's *configuration*; see `What Each End Read <tcspecial-design.html#what-each-end-read>`__.

Requirement
    Tcspecial says both addresses as it starts, on stderr, whether or not
    anything was asked of the log, and names the file they came from.

Said on the way past rather than only when something fails, and said before
either socket is made, so that a failure to bind follows the address it was
trying to bind. They are the addresses the ground has to be pointed at, and
until this was printed, finding out what a running tcspecial had bound meant
asking the operating system.

Requirement
    Tcs-moc passes its own configuration file to each program it starts, as that
    program's argument. Tcs-moc builds its panels from that file, so a child
    reading a different one would serve or simulate payloads the panels do not
    describe: handlers that never connect, with nothing on screen to say why.

Requirement
    Tcs-moc does not start a tcspecial when one is already running. It asks by
    PING, and attaches to the one that answers instead of starting a second.

Starting a second never worked: the command interpreter's address can be bound
once, so the second exits and tcs-moc comes up with a window, no spacecraft
behind it, and the reason on a terminal nobody is reading. Asking by PING
rather than by looking for a process or a bound port is deliberate -- what
matters is whether a command interpreter answers, where a bound port might be
anything and a process of that name might be wedged -- and it finds a
tcspecial that is not a local process at all.

Requirement
    Tcs-moc stops only what it started. A tcspecial it attached to rather than
    started is left running when tcs-moc closes: something else started it,
    something else may still be using it, and shutting down a command
    interpreter because a ground display was closed is not the display's
    decision.

Requirement
    What tcs-moc started ends when tcs-moc ends, however it ends. Quit, the
    close box, SIGINT and SIGTERM stop tcs-sim and tcspecial and then exit;
    a tcs-moc killed outright, or one that panics, never runs that code, so
    each child is also given the kernel's parent-death signal, SIGTERM, and
    ends with it. Under ``cargo run`` the signal reaches the program itself,
    cargo replacing itself with it. The children are started from tcs-moc's
    main thread, the signal being sent when the starting thread ends. See
    ``dies_with_the_moc`` in ``tcs-moc/src/main.rs``.

A child left behind holds the command ports, the beacon socket and every
payload's port, and the next tcs-moc on that configuration fails to bind them
-- or, worse, attaches to the stale tcspecial and runs against it.

Requirement
    A ``START_NODE`` for a node tcspecial does not have says which id came
    in, how many nodes this tcspecial does serve, and that the two ends are
    reading different configurations.

Attaching is what makes this worth saying. A tcspecial left running from an
earlier payload set is attached to rather than replaced, and it goes on serving
the set it was started with while the panels in front of the operator are the
new set's -- so a payload the configuration plainly contains is answered
``NotFound``. The command carries the payload's name, which put the name in the
answer and made it read as a name that had been looked up and missed; nothing
is looked up by name, the id is what addresses a handler, and the file tcspecial
read twenty hours ago is the fact that explains it. A status cannot carry that,
so the log says it.

An argument is what makes this possible. Tcs-moc has no variable of its own
because its children inherit its environment: a variable naming tcs-moc's file
would name theirs as well, and could not point one at a different file from
the other. An argument reaches exactly the program it is given to, and beats
anything that program would otherwise have taken from the environment, so for
the run of a payload set tcs-moc's file is the one that counts.

Tcs-moc builds each child's command without running it, so what a child would
be started with is checked by a test rather than by starting it: the programs
open windows and bind fixed ports, which makes running them a poor way to test
anything.

The one file tcs-moc does not pass on is tcs-sim's simulator configuration.
Tcs-sim takes it from ``PAYLOAD_SIM_YAML``, inherited from tcs-moc's environment
like any other variable, and so is already looking at the file tcs-moc was
pointed at. Tcs-moc reads that file as well, but only to show what it says; it
acts on none of it and so has nothing to say about which one is right.

The Makefile names a set once for this reason, and names it as a stem:
``TCSPECIAL_CONFIG_FILE`` is the set, ``$(TCSPECIAL_CONFIG_FILE).ytcs`` the
tcspecial configuration that reaches every program as its argument, and
``$(TCSPECIAL_CONFIG_FILE)sim.yaml`` the simulator file that reaches tcs-sim in
the environment. One target then runs a whole set, and its last letter is the
spelling of the configuration the MOC reads -- ``runmocy`` the YAML and
``runmocx`` the XML -- both adding the suffix to the same stem, so they are one
set read two ways:

.. code-block:: console

   $ make runmocy
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/thermostat
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2

Requirement
    The two files of a set are named by one variable rather than two. Naming
    one of a pair is how the two come to describe different sets, which is
    reported rather than run -- every data handler of the tcspecial
    configuration must have a simulated payload naming it -- but it is reported
    by tcs-sim, after tcs-moc has already come up, which makes it a confusing
    way to find out.

The sets live under ``tests/manual`` rather than at the repository root
because running one is something a person does at a terminal: each opens
sockets or device nodes, so none of them belongs in a test that runs on every
build. The tests that read them only parse them.

``make run`` runs tcspecial alone and ``make runsim`` the simulator alone, both
from the same stem -- tcs-sim run by itself needs the tcspecial configuration as
well as the simulator file, and one variable is what keeps the two from being
given for different sets:

.. code-block:: console

   $ make run TCSPECIAL_CONFIG_FILE=tests/manual/thermostat
   $ make runsim TCSPECIAL_CONFIG_FILE=tests/manual/thermostat
   $ make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/thermostat

Started in that order, the three run as separately as they can: tcs-moc finds
the tcspecial already there, attaches to it rather than starting a second, and
leaves it running when its window closes. Which is what one program at a time
is for -- a debugger on the one being worked on, and the others left alone.

``make help`` lists the targets, and ``RUST_LOG`` reaches every one of them:

.. code-block:: console

   $ make runmocy RUST_LOG=debug
   $ make run TCSPECIAL_CONFIG_FILE=tests/manual/thermostat RUST_LOG=debug

The manual says the same thing from the other end -- what to type to run a
set -- under "Running the Programs".

The window is built from that file. Nothing in the GUI names a data handler or
fixes how many there are, so a source added to or removed from the configuration
adds or removes a rectangle.

Requirement
    Panels are laid out across the window before they are laid out down it,
    three to a row, and a fourth starts a row under the first.

Requirement
    The MOC opens the payload data path of every payload that has a ground
    link: one socket per payload, bound where
    ``payload_telemetry.dest_base`` counted on by that payload's id says the
    ground's end is, which reads what arrives and shows the latest in the
    panel with the time it arrived. It sends nothing.

Requirement
    tcspecial sends a payload's telemetry to the end the configuration names
    -- ``payload_telemetry.dest_base`` counted on by the payload's id -- and
    not to wherever that socket was last spoken to from. A payload's own
    socket still learns its peer, because the payload is the end that speaks
    first over UDP and no configuration says where it will speak from; the
    ground link is the one whose far end is written down.

Before the ground's end was named there was a hello: an empty datagram from
the MOC to every payload's handler, once a second, because the handler learnt
where to answer from what arrived and with nobody there it had no peer at all
-- every write to the ground failed, and a payload's data went into a handler
and no further. Once a second because starting or stopping a payload rebound
that socket and the learnt peer went with it, so a ground station that said
hello once was silently cut off by the next Enable, with nothing anywhere to
say so.

Naming both ends removed all of it: the hello, that silence, and the last
traffic the MOC sent without being asked. What remains of the reason the
listener exists is the other half of it -- every reading in a panel used to be
the spacecraft's own account of itself, which is worth having and is not
evidence that anything reached the ground.

Requirement
    The window asks to be put against the top of the display, and tcs-sim's
    against the bottom. The two are read together -- what the ground sees of
    a payload and what the payload is doing about it -- and left where a
    window manager puts them they land on top of each other.

Requirement
    It is a request and nothing depends on it. X11 honours a client's
    position; Wayland leaves placement to the compositor and may ignore it,
    and a tiling window manager places windows as it sees fit. A window that
    lands elsewhere is a window in the wrong place, not a program that
    failed.

Requirement
    The display's height is read from ``/sys/class/drm``: the tallest
    connected output's first mode. Nothing can know which display the windows
    opened on, and the tallest is the one that makes a window asking for the
    bottom land on a display rather than above it. With no height to be had,
    neither window asks for anything.

Requirement
    The position is asked for once the event loop is running, because that is
    when a window has a size. Before it is shown it reports what was
    requested rather than what it was given, and the bottom of the display is
    the display's height less the window's.

Requirement
    The window opens wide enough for the row.

It already did. Three columns come to 798 pixels including the margins, which
is wider than the window's own width floor: the command interpreter's controls
need 700, and the floor is at least what two rows of panels stand, so that a
two-row grid is wider than it is tall. That was not obvious before the panels
were measured: the constant the window width was worked out from said a panel
was 320 wide, with a comment claiming the layout stretched them to fill it, and
a panel is as wide as its widest row and does not stretch. Three columns of 320
would have opened a 960-wide window with a third of it empty to the right of
the panels.

A panel's widest row is its three buttons, which is why spelling Config out as
Configuration moved the window: 206 pixels became 256, and three columns 644
became 798. A row of buttons that does not fit keeps them at their own width
and hangs the last one over the panel's edge onto the panel beside it, so a
test holds every button to the panel that contains it and not merely to the
window.

So the shape follows from the count alone: three handlers are one row of three,
and the five of the shipped thermostat are a row of three and a row of
one, in a 977x1108 window. A test reads that size out of this document and
checks it against the one the program opens at, as one does for tcs-sim.

This replaced a rule that chose the shape nearest square among those at least
as wide as they were tall. It read well and laid the panels out badly: two
panels came out as one column of two, because a 739x739 window is squarer
than a 739x549 one, so the set the Makefile runs went down the window instead
of
across it. Which row and column each panel is given was row-major all along --
the shape was the whole of the fault. Squareness was never the thing wanted
either; it was a proxy for a window that fits a screen, and a column count says
that directly.

What is given up is that the grid is no longer always wider than it is tall.
Seven panels or more take a third row, which is 929 against a 798-wide window;
they scroll, as too many panels always have. Up to six it still is, and a test
says so. The window the MOC opens has been taller than wide since it gained its
three-row floor, which is a different thing: the floor is room to grow into.

A test measures where the panels land in a laid-out window rather than reading
the arithmetic back, because the arithmetic was not the part that was wrong.

The GUI has a section at the top of its single window for commanding the
spacecraft and reading what it answers, with the beacon's panel beside it.
Below that is a rectangle for every source the configuration sends to the
ground, each showing the node's name, what it is, where its data arrives and
what has arrived -- whether or not anything has, because what a panel is for
is comparing a node against the file that describes it.

It shows no packet interval: that is a simulator setting, stated in the
simulator file and shown by tcs-sim.

Requirement
    A panel shows its source's packet size as the configuration states it:
    the width of the type the source sends at its ``world-out``, or, for a
    string or bytes, its ``read_size`` where it gives one, and ``not fixed``
    where the configuration gives neither. It was nought on every panel,
    which reads as a source sending empty packets.

Requirement
    Nothing the operator reads is cut off at the window's edge. ``Last
    Response`` and the packet count under it wrap; the Configuration pop-up
    is as wide and as tall as what it shows, within the window, and scrolls
    only past that; and the plot window's legends wrap onto as many lines as
    they need, the panels sharing the height that is left, one plot height
    for all of them so that Rust draws every panel to one size.

The two lines of what arrived are the ground's own reading: when something
last came from that source and the head of it, counted from the moment the MOC
started. A source that has sent nothing shows ``--:--:--`` and no data, which
is what a panel says before anything arrives.

What a panel shows is what reached the ground, which is the thing an operator
is watching the window for.

Requirement
    A panel says which payload of the configuration it is, beside the name:
    ``auto-send (dev id 0)``. The number is the payload's device id, its
    place among the names of the configuration's nodes, counted from nought,
    and nothing in the file states it -- a payload stated a ``dh_id`` once and
    that was a second identity to keep consistent by hand. Both windows count
    the same way, which is the use of it: the two are read side by side, and a
    payload is then the same number in each.

Each panel has three buttons: ``Enable``, ``Disable``, and ``Configuration``,
which shows what the configuration files said about the handler and is
described under `The tcs-sim program <tcs-sim-design.html#the-tcs-sim-program>`__ where the same button is. It read ``Config`` while
the panel was narrower than the word.

Requirement
    A panel has a button per command, each labelled with the command it
    sends. ``Enable`` sends ``START_NODE`` and ``Disable`` sends
    ``STOP_NODE``, with
    Enable first: that is the press a panel is opened to make, and the status
    beside the name is what says which of the two has taken effect.

Requirement
    Both are pressable whatever the node is doing. tcspecial answers a
    ``START_NODE`` for a node already running and a ``STOP_NODE`` for one
    already stopped without harm, and what an operator wants when a panel
    looks wrong
    is what the spacecraft says to the command -- which a button that cannot
    be pressed does not say.

There was one button, labelled with the one thing worth doing next:
``Discard`` for a handler that was moving data and ``Receive`` for one that
was not. It had to be read both ways -- Slint built the label from the status
and the program decided the command from the same status -- so the two had to
be held to one rule or they would drift into offering one thing and doing the
other. Two buttons have nothing to drift: a press is the command on the
label. What a panel gives up is that it no longer suggests which press to
make, and what it gains is that either can be made without first getting the
handler into the state that offers it.

Requirement
    The MOC has a button per command where the simulator has one button for
    both, and the difference is the link each press crosses. Pressing a button
    in tcs-sim starts or stops a payload in that same program: the press is the
    act, it cannot be lost, and one button labelled with what it will do says
    everything there is to say. A press in the MOC sends a datagram to a
    spacecraft, which may not arrive -- so the same ``START_NODE`` or
    ``STOP_NODE`` may have to be sent again, and again, before tcspecial has
    it. A single button
    offering the press that suits the status cannot be pressed twice: the
    status it reads is the ground's own guess about a handler it may not have
    reached, and the press worth making is exactly the one such a button
    refuses. Two buttons, both always pressable, are a command an operator can
    repeat.

Requirement
    The panels are refreshed without being asked. A handler moving data must
    not look like one doing nothing, which is what a panel that changed only
    when a button was pressed showed.

Requirement
    Asking a handler what to show does not happen on the window's own thread.
    Each command waits on the spacecraft for as long as the client's timeout
    allows, and a window that stops repainting while tcspecial is slow to
    answer is a worse fault than a panel a second out of date.

Requirement
    A panel shows what arrived from its payload, the time it arrived, and how
    many packets have arrived -- and not the spacecraft's account of what it
    last sent and last received, nor its count of the bytes it sent to the
    payload. Three rows and a counter carried that account and are gone: a
    panel is watched for what reached the ground, and the claim about it took
    a third of the panel's height to say nearly the same thing.
    The spacecraft's own sample is not asked for any more either -- a command
    a second per node whose answer nothing showed.

Requirement
    All three of those readings are kept by the end that received them, which
    is the ground. The count was taken from the spacecraft's statistics
    instead, and from the wrong half of them: what a handler had *received*
    was what it read from the ground, which is the direction nobody sends in,
    so a panel beside a payload sending steadily read ``0 bytes``. The listener that
    already kept the time and the data counts the packets as it receives
    them.

Requirement
    The window says when the last command went out and when its answer came
    back, under ``Last Response``. A response that is slow to come and one
    that never comes read alike until those two times are beside it.

Requirement
    Both are the ground's own clock, and they are the window link's own
    commands -- what the operator pressed -- not the panels' polling. The
    times are read from the client that sends them, which is the one place
    every command passes, so a press that forgot to record one cannot
    happen.

Testing requires starting up tcs-sim before other operations and shutting it
down when tcs-moc is halted.

See Also
========

`The TCSpecial design documents <design.html>`__
    What each of TCSpecial's design documents covers.

`The tcspecial design <tcspecial-design.html>`__
    The flight software and what the programs share: the node architecture,
    the link, fault resiliency and the requirements.

`The tcs-moc manual <tcs-moc.html>`__
    Using tcs-moc.

