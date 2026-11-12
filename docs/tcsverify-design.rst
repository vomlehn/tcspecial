================
tcsverify Design
================

.. contents:: Contents
   :depth: 3
   :local:

How tcsverify, the configuration checker, works and why.

Tcsverify reads a configuration -- the file itself, and the simulator file
beside it when one is given -- and prints what is wrong with it. It opens no socket and no device node, so it can be run anywhere, by a
person editing a file or by a recipe: it is given the files to read and says
nothing when they are good.

Requirement
    Tcsverify takes one file or two. The first is the tcspecial configuration
    file and the second, if there is one, the simulator configuration. Which
    they are follows from the order they are given in and not from their
    names, a set being free to name its files as it likes.

Requirement
    Every problem is printed as the file it is in, the line of that file,
    which kind it is, and what is wrong: ``file:line: error: message`` or
    ``file:line: warning: message``. The shape an editor jumps to and a build
    log is read with. Both kinds say which they are, rather than only the
    warnings: a report where an unlabelled line had to be worked out would be
    a report read wrongly.

Requirement
    A problem is an **error** where the configuration says something wrong --
    two nodes of one name, a link to a node that is not there, a link carrying
    one stream into the other, a node of a type that does not exist -- and a
    **warning** where it has not finished
    saying it: a node whose attributes are not all in yet, a node nothing is
    linked to. One statement of which is which,
    ``tcslibgs::graph::check::Problem::severity``, so that the verifier, the
    editor and the flight software agree about what a half-built graph is.

    A type that does not exist is an error and not an unfinished attribute,
    because no amount of finishing makes it work: it is most likely a
    misspelling, and the flight software would have nothing to open it as.
    The types are ``tcslibgs::graph::types_in``, one list per section, which
    the configuration schemas beside tcsgui are tested against.

    The split changes no rule. The flight software refuses a configuration
    with any problem in it, warnings included: a sensor whose device nobody
    named cannot be opened, and that is not a matter of degree aboard. What it
    is for is the person editing. A configuration is unfinished for as long as
    somebody is drawing it, and a verifier that failed on that would fail on
    every save -- burying the mistakes that matter under the fields nobody has
    typed yet.

Requirement
    Tcsverify prints every error it finds unless ``--error-limit=<n>`` asks
    for a limit -- any whole number, nought included -- and then stops as soon
    as it has printed that many. ``--error-limit=unlimited`` is the default
    said aloud. Nothing is looked for after the last one the limit allows. ``--warnings=on``, the default, prints warnings without
    counting them; ``--warnings=off`` prints none; ``--warnings=errors``
    prints them and counts them against the limit as errors are. What is found
    is printed in the order it was found, so that a run stopped by its limit
    ends on the finding that stopped it. Every one by default, because a
    report cut short by a limit nobody asked for hides errors from somebody
    who believes they have seen them all; a limit is for whoever wants the
    first few on the screen.

Requirement
    ``--digest`` prints each configuration's digest as well, one line per file
    and before the problems: the same digest tcspecial takes of the graph it
    loaded, prints as it starts and puts in every beacon, so that the question
    the digest exists to answer -- is the vehicle flying the configuration on
    this desk -- can be asked of a file without running anything. It is of the
    graph a file parses into and not of the file's bytes, so the two spellings
    of one configuration answer alike.

Requirement
    ``--keep=<directory>``, with one configuration and its simulator file if
    it has one, checks it as tcsverify checks any and, where nothing is
    wrong with it, keeps it in ``<directory>`` under its checksum, as the
    vehicle keeps a configuration a RESTART may name: see `Keeping
    configurations on board <tcspecial-design.html#keeping-configurations-on-board>`__. It prints where it is kept, or that it was
    kept already, which writes nothing. One of several files is kept with
    each file it includes. It is how a configuration is put on board for
    testing and at first provisioning.

Requirement
    ``--set-interface <configuration>...``, given configurations and
    nothing else, writes this build's interface digest into each as its
    ``tcspecial`` section's ``interface``, in place, leaving every other
    byte as it was -- comments, order, layout and quoting -- and says for
    each what it changed. One that names it already is not written. The
    file written is read back, and must name this build's interface,
    before it replaces the old one.

Requirement
    ``--interface-digest``, given alone, prints the digest of the interface
    this build speaks and nothing else: the 32 lower-case hex digits a
    configuration's ``tcspecial`` section gives as its ``interface``, and
    the last field of every beacon this build's tcspecial sends. It is how
    a person writing a configuration finds the value: see `The
    configuration names its interface <tcspecial-design.html#the-configuration-names-its-interface>`__.

Requirement
    ``--ambient <file>``, given alone, prints the simulated world's
    ``ambient_temp`` in kelvin and nothing else, from a simulator file, or
    from the one beside a configuration that tcssim would read. It is read
    by ``WorldSection::ambient``, the one reading of it, so that a ground
    program in another language -- the Yamcs example's Python -- shows the
    world's temperature without stating the rule for reading a quantity a
    second time. A file with no world, a figure that does not read, or a
    configuration with no simulator file beside it is said on stderr, with
    status 1.

Requirement
    ``--summary``, with one configuration and its simulator file if it has
    one, checks it as any run does and then prints, after the problems,
    what it uses of what must not clash or overflow: every address and
    port it states or implies and who binds it, every node's device id,
    every message ID and APID, and how many data and bytes a second each
    link to the ground carries, with the beacon's and the total. Plain
    text in aligned columns under a heading each, in orders no file can
    change, so that two summaries can be compared with ``diff``. Every list
    is ``tcslibgs``'s -- ``graph::sockets::of``, ``graph::telemetry::ids``,
    ``msgids`` and ``apids``, ``graph::rates::of`` -- and tcsverify lays
    them out and states none of them. A configuration that does not read
    has no summary, which fails the run, as a digest not given does.

Requirement
    No two sockets a configuration states or implies are one socket:
    one port of one protocol on one host, the names of this host being one
    host, or a port on every interface and the same port on this host.
    ``tcslibgs::graph::sockets::clashes`` is the rule, asked by
    ``Graph::check``, and an error naming both: whichever binds second
    would fail as it starts, aboard or on a desk. It leaves to their own
    rules the pairs those hold apart already -- two nodes that want one
    thing, two sources on one port of the ground -- so that one mistake is
    one complaint. A wildcard and a host named otherwise are not compared:
    whether that host is this machine is the network's to say, and
    refusing it would refuse a vehicle whose payload is a box at that
    address.

    The rates are an estimate, ``tcslibgs::graph::rates``: a node read on
    a schedule sends as its schedule says, any other at most one datum for
    each that arrives at it, and one with no schedule and nothing linked
    into it at a rate no file says. What it is for is a sum that shows
    which sources are most of a link's load, and roughly how much that
    is; it refuses nothing.

Requirement
    ``--types=<types>`` checks a configuration as a tcspecial built with only
    those payload types would; see `Which Node Types Are Built <tcspecial-design.html#which-node-types-are-built>`__.

Requirement
    The digest is taken of the configurations among the pathnames and of
    nothing else: the first of two and the first two of three, the last being
    the payload simulator file. A simulator file has no digest of this kind.
    It is not a graph to take one over, and it does not fly, so there is
    nothing at the other end to compare one with.

Requirement
    A configuration with problems still has a digest, having said something
    for the digest to be taken of. One that cannot be read has none, and that
    is a failed run: a digest asked for and not given must not end as a
    silent success, which it otherwise would under ``--error-limit=0``.

Requirement
    The exit status is about what was printed: failure if an error was, and
    success if none was -- a run stopped before it reached one included, and a
    run whose only findings are warnings, whatever ``--warnings`` says. A
    digest asked for and not given fails as well, being the one failure that
    is not about a file's contents. The library's ``verify`` takes the same
    two settings and its report passes and fails as tcsverify does.

Requirement
    The programs that check a file before running it -- tcspecial, tcsmoc,
    tcssim and tcsgui -- set no limit and keep warnings on, so they look at
    everything, and show all of it in the order found, as tcsverify prints
    it: a configuration is small, and a problem left out of what is shown is
    a problem somebody has to run something else to find.

Requirement
    Tcsverify exits with a status of nought when it printed no errors and
    non-nought when it printed one, so that a recipe or a shell ``&&`` can be
    told by it. Warnings do not change the status: a configuration somebody is
    in the middle of writing must not fail a build it is not part of yet. A
    command line that is neither of the two shapes above is a third status,
    which is not a statement about any file.

Requirement
    Tcsverify states no rule of its own. Every problem it reports comes from
    the same rules in ``tcslibgs`` that the programs reading these files use,
    so a set it accepts is a set they accept. A verifier with rules of its own
    would eventually disagree with them, and then a passing check would mean
    nothing.

Requirement
    Every program that reads these files asks tcsverify about them as it
    starts, before anything else is read from them, and says what it says in
    its words: tcspecial, tcsmoc, tcssim and tcsgui report the same problems,
    each with its file and line, as ``tcsverify`` does for the same files.
    What a program then refuses for its own reasons -- tcssim a payload it has
    it cannot simulate, tcsmoc a configuration it does not control -- is
    said beside them. Where a rule is the simulator's rather than the file's,
    it is in ``tcslibgs`` all the same (``simulable``), so that tcsverify asks
    it too.

    Which of them stops a program is the program's business. tcspecial
    refuses a configuration with a warning in it, as above; tcsmoc
    reads the simulator file only to show it, so a problem there is said and
    costs the panels their simulator half.

Requirement
    A program with a main window shows what it found as it started in a
    window of its own, before the main window, and the main window opens only
    once that one has been closed -- or not at all, where what was found stops
    the program. tcsmoc and tcssim share the window
    (``tcsverify/ui/startup.slint``); tcsgui's is the one that offers a fix for
    each problem, and lists beside its own faults whatever else tcsverify
    finds, warnings included. Everything is said on stderr as well.

Requirement
    A configuration's simulator file is checked by name: that it
    parses, and that every simulated payload is a payload of the graph, every
    group it names is declared and every group it declares is used, and every
    setting is one. A payload with no entry is not a problem: the one a bench
    has for real is left out on purpose.

That tcsverify states no rule of its own is what the library's ``verify::Problem`` type is for. A loader wants the
first thing wrong with a file and nothing else -- it cannot run, and which of
several mistakes it names changes nothing -- where a reader wants all of them.
So the rules are stated once and collect their answers:
``Graph::load`` refuses a file at the first thing in it that does not read and
``SimConfigFile::resolve`` at the first problem; ``Graph::check`` and
``SimConfigFile::problems`` return the lot.

**Where a line number comes from**

Nothing in the library has seen the file. A parser hands up a document and the
lines are gone by then, so a problem says which payload or group it is *about*
and tcsverify works out where that name is written -- the same thing a reader
does with the error a program prints. The alternative, carrying a line number
through every struct in the library, would put one file format's shape into
the types that exist so that two formats can describe one thing.

Requirement
    A problem about a payload or a group is reported from the line its name is
    written on. One about a section is reported from the line the section
    begins on, and one about the file as a whole from its first line.

Requirement
    A file that does not parse is reported from where the parser stopped.
    YAML's parser says where that was. XML's does not, so the line is the
    reader's position when it failed: the ``<`` of markup that does not parse,
    and otherwise just past the element it was refused at -- an element nothing
    has a field for, or a missing one's parent closing. The message says it too,
    as ``at line N column M``, which is how YAML's is written.

Requirement
    The simulator file is checked against the tcspecial configuration only when
    that configuration has no problems of its own. There would be nothing to
    check it against otherwise -- the payloads that did not settle are not
    handlers -- and a join against the rest reports every one of them as
    unsimulated, which is a page of problems the verifier caused rather than
    found. It says that it did not do it, rather than leaving it unsaid.

See Also
========

`The TCSpecial design documents <design.html>`__
    What each of TCSpecial's design documents covers.

`The tcspecial design <tcspecial-design.html>`__
    The flight software and what the programs share: the node architecture,
    the link, fault resiliency and the requirements.

`The tcsverify manual <tcsverify.html>`__
    Using tcsverify.

