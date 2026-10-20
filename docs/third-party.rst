=======================
Third-Party Integration
=======================
.. note::
   Work in progress. Integration with cFS and F´ is being planned and has not
   been built: nothing here describes code that exists yet, and the plan will
   change as the work finds out what it did not know. Each section says what
   is intended, not what is done.

.. contents:: Table of Contents
   :depth: 3
   :local:

Introduction
============
This is how the tcs projects -- tcspecial, tcslog, tcslog-tools and
tcsxtce -- would work with the two open-source flight software frameworks
most missions build on: NASA's core Flight System, **cFS**, and JPL's
**F´** (F Prime). It says what each project brings that the frameworks do not
have, what has to change in each to fit, what that costs, and an order to do
it in.

None of it is built yet. This is the plan, and the projects may be refactored
as far as it needs: the integration is worth more than keeping their present
shapes.

What is said here about cFS and F´ is from their published designs, not from
code in this tree, and both change from release to release. Check each claim
against the versions a mission targets before relying on it; the two
decisions under `Decisions to Make First`_ settle which those are.

The Projects
------------

``tcspecial``
    Flight software. It runs a vehicle's payloads as nodes in a graph that a
    configuration file describes, takes commands from the ground and sends
    beacons. See ``manual.rst`` and ``design.rst``.

``tcslog``
    Flight software. A segmented telemetry log for storage that is fixed
    before launch and links that come and go, which recovers what it can from
    lost or damaged segments. Rust, C/C++ and Python 3.
    https://github.com/vomlehn/tcslog

``tcslog-tools``
    Ground software. ``tcslog-dump`` and ``tcslog-dumphdr``, which read a log
    whichever language wrote it. https://github.com/vomlehn/tcslog-tools

``tcsxtce``
    Flight software. It compiles an XTCE description of telemetry and
    commands into code that packs and unpacks them, byte for byte the same in
    Rust, C/C++ and Python, with bit packing. tcspecial's beacon is already
    packed with it; see ``vendor/tcsxtce``. https://github.com/vomlehn/tcsxtce

The Frameworks
--------------

cFS
    C. Applications are loaded by the Executive Services (ES) and talk over
    the Software Bus (SB), which carries CCSDS Space Packets addressed by
    message ID. Behaviour is changed at run time through tables (TBL), which
    are loaded from files and checked by a validation function the
    application supplies. Events go through EVS, time through TIME. The
    common applications include Scheduler (SCH), Command Ingest and Telemetry
    Output (CI and TO, of which the ``_lab`` versions are the open-source
    ones), Data Storage (DS), CFDP file transfer (CF), Housekeeping (HK) and
    Health and Safety (HS). It is built with CMake. Recent releases describe
    messages in Electronic Data Sheets (EDS).

F´
    C++. Components exchange data over typed ports, and are wired into a
    topology described in FPP, the F´ modelling language, at build time.
    Commands, telemetry channels, events and parameters are declared in FPP;
    parameters are changed at run time through the parameter database
    (PrmDb). Rate groups drive periodic work. The ground system is the F´
    GDS, which reads a dictionary the build generates. It is built with CMake.

Both frameworks allocate at initialisation and not afterwards, which is the
rule tcspecial and tcslog already follow, and both have flown.

What the tcs Projects Add
=========================

tcspecial
----------

Devices are configured at run time
    A cFS mission's device applications are fixed when it is built, and
    tables change how they behave rather than which devices there are. An F´
    topology is fixed when it is built. tcspecial's device graph is data: a
    device swapped, added or reconfigured is a new configuration file, not a
    new build, and the same software serves a vehicle whose payload set
    changes.

A configuration is refused when it is loaded, saying where
    Every rule about what a configuration may say is checked in one place,
    ``tcsverify``, and tcspecial, the simulator, the graphical editor and the
    ground station all check through it. A configuration the vehicle would
    refuse is refused in the editor, with the same message, naming the file
    and the line. A table validation function in cFS, as usually written,
    answers only yes or no.

The ground can see which configuration is flying
    The digest of the configuration tcspecial read is in every beacon and in
    the answer to every CONNECT, so the ground knows which configuration is
    running without having to ask.

The simulator is built from the same file
    tcssim reads the configuration and stands in for its payloads, so a
    device change can be tried on the ground, against the ground station,
    before it is uplinked.

The device interfaces are in one place
    I²C, SPI, serial lines, CAN, GPIO, IIO, rpmsg, HDLC, sockets, raw sockets
    and device files are each a node type rather than an application per
    device.

Programs in any language
    An app node runs a program over file interfaces, so existing payload code
    can be wrapped rather than ported.

Reliability
    The main loop allocates nothing, and the ground's input is parsed in
    Rust. A restart is two commands, RESTART_ARM and then RESTART, so one
    corrupted command cannot restart the vehicle.

tcslog
------

Logs survive damage
    A log is segments of a size chosen beforehand, and a reader recovers what
    it can from a segment that was lost or damaged. That suits storage fixed
    before launch and links that come and go better than one file per
    product, as cFS's DS and F´'s ComLogger write.

Segments are handed on when they fill
    Each filled segment is handed to a ``segsend`` function the caller
    supplies, which is where a file-downlink service picks it up: CFDP, which
    is cFS's CF, or F´'s FileDownlink.

C and C++ already
    ``tcslog-c`` is a C ABI whose header compiles as C and as C++, which is
    what either framework needs.

tcslog-tools
------------

The ground's half of the log: reading a log back, and recovering what a
damaged one still holds, whichever language wrote it.

tcsxtce
-------

Packet formats are stated once, in a standard
    XTCE is an OMG standard for telemetry and command definitions. Yamcs
    reads it as it is, and OpenC3 COSMOS can import it. A format is stated
    once and every program generates its code from that statement.

Every language packs the same bytes
    The Rust, C/C++ and Python it generates produce byte-identical packets,
    and its tests check that they do. A ground script in Python decodes
    exactly as the flight code does.

It fits both frameworks
    Its C/C++ code is one header, allocates nothing and reports errors as
    return codes, which suits cFS in C and F´ in C++.

Bit packing
    Fields can be packed without padding between them, which saves downlink
    bandwidth. Changing a format is an edit to the XML and a regeneration,
    not hand-edited structures at both ends that must be kept in step.

What Has to Change
==================

1. Commands and Telemetry in XTCE
---------------------------------
This is the change everything else depends on. tcspecial's commands and the
answers to them are JSON, and cFS's Software Bus carries CCSDS Space Packets,
while ground systems expect binary formats from a dictionary. The beacon has
already moved; the rest should follow.

* Describe every tcspecial command and answer in XTCE, on a base container
  holding the CCSDS primary header. tcsxtce's container inheritance and
  restriction criteria already handle this; its demonstration mission is
  CCSDS.
* Retire the JSON on the link. The link to the MOC stays UDP.
* Put the digest of the dictionary in the beacon beside the configuration's,
  so that the ground knows which formats are flying as well as which
  devices.

2. tcsxtce Fits the Frameworks
------------------------------

* **A Rust decoder that does not allocate.** It decodes fixed-size strings and
  binary into ``String`` and ``Vec``, which is three allocations for every
  beacon tcspecial's ground hears; ``tcslib/src/running.rs`` counts them. The C
  decoder already uses fixed arrays, and the Rust should do the same.
* **cFS output.** Message structures with the cFE headers and message IDs, or
  the C codec wrapped for use on the Software Bus. On a cFS release that uses
  EDS, a path between EDS and XTCE may be the better way to do this.
* **F´ output.** FPP -- commands, telemetry channels and serializable types --
  or F´ ``Serializable`` classes generated from XTCE, or the reverse: F´'s
  dictionary read into XTCE.
* **Ground dictionaries.** XTCE as it is for Yamcs, and a generated F´ GDS
  dictionary.

3. tcslog in cFS and F´
-----------------------
Small, and independent of the rest.

* **cFS.** ``tcslog-c`` wrapped as a cFS library. ``segsend`` puts each filled
  segment in CF's playback directory. tcslog may sit beside DS or replace the
  file writing DS does.
* **F´.** A ``TcsLogger`` component that takes telemetry and events on its
  ports and hands each filled segment to FileDownlink.
* **The ground.** tcslog-tools learns to decode records with the definitions
  tcsxtce generates, so that what was stored reaches the ground archive as
  telemetry.

4. tcspecial in cFS and F´
--------------------------
In two stages.

a. **A separate process, and a bridge.** A cFS application, and an F´
   component, that pass messages between the Software Bus or F´'s ports and
   tcspecial over a local socket. tcspecial needs nothing beyond the change
   in section 1. This is the safest way to start. The framework does not
   look after tcspecial's health, though, so something must watch it: ES or
   HS in cFS, F´'s health component.

b. **A library.** tcspecial split in three:

   ``tcspecial-core``
       The graph, the nodes, the configuration and the pass.

   A host interface
       The link to the ground, the tick that drives a pass, time, events and
       file access, each supplied by whoever hosts the core.

   ``tcspecial-c``
       A C ABI whose header compiles as C and as C++, as ``tcslib-c``'s
       does.

   A cFS application then drives a pass on each wakeup from SCH and maps the
   link onto the Software Bus, and an F´ component does the same from a rate
   group and its ports.

   In cFS the configuration can be a table, with ``tcsverify`` as its
   validation function. A device is then reconfigured with the table-load
   commands cFS operators already use, and a configuration that would fail
   is refused when it is loaded, saying why.

   In either framework, tcspecial's log messages become events (EVS in cFS,
   events in F´) and its timestamps become cFE TIME or ``Fw::Time``. A Rust
   panic must not cross into C: abort on panic, or catch it at the boundary.

   The renaming of ``payload`` to ``node``, held back until the configuration
   schema is complete, would be best done here if this or section 1 reshapes
   the schema anyway.

5. Building, and the Target
---------------------------

* **Building.** Cargo inside cFS's CMake (``add_cfe_app``) and F´'s, by a
  bridge such as Corrosion.
* **The target.** This is the biggest unknown. Linux is straightforward.
  RTEMS and VxWorks have Rust targets, but less mature ones, and tcspecial's
  device nodes assume Linux interfaces: IIO, rpmsg, raw sockets. A mission
  that must run an RTOS would run tcspecial on a Linux companion processor,
  with only tcslog and tcsxtce's C codecs inside the RTOS.

Risks
=====
Weighed by the order this project ranks its qualities: reliability, then
ease of use, then maintainability, then flexibility.

Heritage
    cFS and F´ have flown and the tcs projects have not, so run-time
    reconfiguration has to be argued for in review. The argument is not that
    it is flexible, which ranks last, but that it is checked: every
    configuration is verified when it is loaded, the digest proves which one
    is running, and a restart takes two commands.

Allocation across the boundary
    Stage 4b must keep the rule that nothing is allocated after the main loop
    starts, through the C ABI and the host interface as well as inside the
    core.

Two frameworks
    Every piece of integration is written twice, once for cFS and once for
    F´. Keeping what each framework needs behind one host interface, and
    generating what can be generated from XTCE, keeps the two from drifting
    apart.

Order of Work
=============

1. tcsxtce's Rust decoder without allocation. Small, and everything after it
   benefits.
2. tcslog in cFS and F´ (section 3). Self-contained, and shows the approach
   working.
3. Commands and answers in XTCE (section 1), and dictionaries from XTCE for
   Yamcs, cFS and F´ (section 2).
4. The bridge application and component (stage 4a).
5. tcspecial as a library, with its configuration as a cFS table (stage 4b).

Decisions to Make First
=======================

The flight operating system and processor
    Whether tcspecial can run inside the framework's process, or on a Linux
    companion beside it, depends on this.

The versions of cFS and F´
    Whether cFS describes its messages in EDS changes how section 2 is best
    done, and both frameworks' interfaces move between releases.
