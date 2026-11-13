=================
TCSpecial Manuals
=================

.. warning::
   TCSpecial is an alpha release. It is not fully tested, it is not feature
   complete, and it is subject to rapid change. Until 1.0, any change to its
   API changes the minor version number (0.1 to 0.2), so a release that
   changes only the patch number never breaks code built against it.
   Feedback is welcome, even at this stage: open an issue at
   https://github.com/vomlehn/tcspecial/issues.

TCSpecial is **telemetry and control, special**, and it is flight software.
It connects the devices aboard a vehicle -- on an I\ :superscript:`2`\ C
bus, a SPI link, a serial line, a CAN bus, a set of discrete lines, a
socket or a device file -- to the places their data has to reach: the
mission operations center, the **MOC**, on the ground, and software aboard
that computes from it, keeps it, or sends it on. Every one of those is a
**node**, and what joins two is a **link**, and both are read from a
configuration file at startup, so what a particular vehicle does is a file.

The TCS software
================

TCSpecial is the hub of the TCS software. Everything else is built around
it: the programs that write, check and test its configurations, the
library a ground station uses to talk to it, and the projects it is built
from. This page introduces all of them and links to their manuals.

Some of those projects are products in their own right, used with or
without TCSpecial: tcs-xtce, its XTCE editor tcs-xtcee and its XTCE
libraries, in every language they support, and tcslog and tcslog-tools.
None of them needs TCSpecial; TCSpecial uses them.

In flight
---------

.. figure:: tcs-flight.svg
   :alt: A vehicle runs tcspecial. Its payloads -- temperature sensors on an
         I2C bus, a battery monitor on SPI, an altimeter on a serial line, a
         motor controller on CAN, heaters on GPIO lines and a science
         program on a socket -- are each read and driven by a device node.
         Compute nodes work on what they read, a gateway node sends it to
         the ground, and tcslog keeps a record aboard of what was sent.
         Telemetry and beacons go down the space link to the mission
         operations center, where Yamcs uses tcslib for its end of the link,
         and commands come back up.
   :width: 100%

   A vehicle running tcspecial, and the ground station that commands it.

Aboard the vehicle, **tcspecial** runs the payloads. Each payload is
reached through an interface of its own -- I\ :superscript:`2`\ C, SPI, a
serial line, CAN, GPIO lines, a socket to a program aboard, and others --
and is a device node in the configuration's graph. Compute nodes vote
between redundant sensors, convert units and run thermostats; a gateway
node is the vehicle's end of the link to the ground. tcspecial keeps a
record aboard of what it sent down, in a log written by **tcslog**, a
library of its own.

On the ground, a mission control system such as Yamcs (https://yamcs.org)
holds the other end of the link through **tcslib**, the TCS ground library.
tcslib sends the vehicle its commands, receives its telemetry and beacons,
and knows from the vehicle's configuration where each datum arrives and
what it is; Yamcs's mission database is generated from the same
configuration, so the two ends cannot disagree about the link. The link's
packets are packed and unpacked by code that **tcs-xtce** generates from
their XTCE description. Back on the ground, **tcslog-tools** reads a log
that has come down from the vehicle.

On a desktop
------------

.. figure:: tcs-desktop.svg
   :alt: One desktop computer, with no vehicle and no hardware. make runmocy
         starts tcsmoc, which starts tcspecial and tcssim on the same
         configuration. tcsmoc sends tcspecial commands and shows the
         telemetry and beacons it sends back, over sockets on 127.0.0.1.
         tcspecial reads and writes the devices tcssim simulates, behind
         pseudo-terminals and loopback sim_addresses, as it would real ones.
   :width: 100%

   The same software, every connection made on one desktop.

None of it needs a vehicle. On one desktop, **tcsmoc** stands in for the
mission operations center and **tcssim** for the payloads, and between them
runs the same tcspecial that flies. ``make runmocy`` starts all three on one
configuration, and every connection in the figure above is made on the
desktop: commands, telemetry and beacons over sockets on the loopback
interface, and the devices as pseudo-terminals and loopback addresses that
tcssim answers on a physics model of the payloads. A configuration can be
developed, run and watched this way long before there is hardware to run it
on, and parts of it moved to real devices one at a time.

A configuration is written with **tcsgui**, which draws it as a diagram of
nodes and links, and checked with **tcsverify**, which says what is wrong
with it without running any of it. Both read it through **tcslib-protocol**, the
library the vehicle and the ground share, so the error a person sees on the
desktop is the one the vehicle would refuse the file with.

The link's XTCE can be read and edited with **tcs-xtcee**, the XTCE editor
that comes with tcs-xtce, and every TCS project builds its packages with
the make rules and scripts of **tcs-helpers**.

The manuals
===========

TCSpecial is several programs, each with a manual of its own:

`The tcspecial manual <tcspecial.html>`__
    The flight software, and what is common to all the programs: the
    configuration files and what each payload, computation and gateway in
    them says, the shipped examples, installing, running the programs
    together, the commands the vehicle answers to, the crates used and
    the license.

`The tcsgui manual <tcsgui.html>`__
    The editor. tcsgui draws a configuration and its simulator
    configuration as diagrams of nodes and links, checks them as they are
    edited, and writes them back.

`The tcsmoc manual <tcsmoc.html>`__
    The simulated operations center. tcsmoc shows what the vehicle sends
    the ground, panel by panel and in plots, and sends it commands.

`The tcssim manual <tcssim.html>`__
    The payload simulator. tcssim answers for a vehicle's payloads as their
    devices would; its manual also describes the simulator configuration
    that says how.

`The tcsverify manual <tcsverify.html>`__
    The checker. tcsverify reads a configuration and says what is wrong
    with it, running none of it.

The other documents beside these:

`The TCSpecial design <design.html>`__
    How TCSpecial's flight software, libraries and ground programs are
    built, and why.

`Building a vehicle <building-a-vehicle.html>`__
    From the shipped examples to a whole vehicle, a stage at a time: the
    payloads, computations, files and gateway of a lander's science deck.
    An outline for now.

`From the desk to flight <desk-to-flight.html>`__
    What changes between a configuration that has run on a desk and one
    that flies, as a checklist.

`Writing node types <node-types.html>`__
    How to add a node type to TCSpecial.

`Third-party integration <third-party.html>`__
    The plan for making the tcs projects work with NASA's core Flight
    System (cFS) and JPL's F´ (F Prime). Work in progress.

`Examples <../examples/README.html>`__
    The shipped example configurations, and what each one shows.

The libraries
    `tcslib-moc <../tcslib-moc/README.html>`__ in the MOC, in Rust, and
    `tcslib-c <../tcslib-c/README.html>`__, its C interface; `tcslib-protocol
    <../tcslib-protocol/README.html>`__, used aboard and in the MOC.

The rest of the TCS software
============================

TCSpecial is built from, and works with, the other TCS projects. Each is a
repository of its own. Read from a checkout, the links below lead to its
documents in a checkout of it beside this one, as the build expects
(``../tcs-xtce`` and so on); read on the web, to its published pages.
Where neither has them, its repository does.

tcs-xtce (`repository <https://github.com/vomlehn/tcs-xtce>`__)
    The XTCE tools. TCSpecial's link to the ground is described in XTCE,
    and tcs-xtce generates the code that packs and unpacks it.

    - `The tcs-xtce manuals <../../tcs-xtce/docs/manual.html>`__, which
      list:

      - `the tcs-xtce manual <../../tcs-xtce/docs/tcs-xtce.html>`__, the
        compiler from XTCE to Rust, C, C++ and Python;
      - `the tcs-ccsds manual
        <../../tcs-xtce/docs/tcs-xtce.html#ccsds-time-codes-tcs-ccsds>`__,
        the CCSDS time codes, a chapter of the tcs-xtce manual;
      - `the tcs-xtcee manual <../../tcs-xtce/docs/tcs-xtcee.html>`__, the
        XTCE editor.

    - `The tcs-xtce design documents <../../tcs-xtce/docs/design.html>`__,
      which list `the tcs-xtce design
      <../../tcs-xtce/docs/tcs-xtce-design.html>`__ and `the tcs-xtcee
      design <../../tcs-xtce/docs/tcs-xtcee-design.html>`__.
    - `The tcs-xtce examples <../../tcs-xtce/examples/README.html>`__.

tcslog (`repository <https://github.com/vomlehn/tcslog>`__)
    The library that keeps telemetry aboard until there is a link to send
    it down.

    - `The tcslog manual <../../tcslog/docs/manual.html>`__.
    - `The tcslog design <../../tcslog/docs/design.html>`__.

tcslog-tools (`repository <https://github.com/vomlehn/tcslog-tools>`__)
    The programs that read a tcslog log on the ground.

    - `The tcslog-tools manual
      <../../tcslog-tools/docs/tcslog-tools.html>`__.

tcs-helpers (`repository <https://github.com/vomlehn/tcs-helpers>`__)
    The make rules and scripts the TCS projects share to build their
    packages and keep their pages up to date.

    - `The tcs-helpers README <../../tcs-helpers/README.html>`__.
