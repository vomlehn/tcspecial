=================
TCSpecial Manuals
=================

.. warning::
   TCSpecial is very early software and is subject to rapid change. Until
   1.0, any change to its API changes the minor version number (0.1 to
   0.2), so a release that changes only the patch number never breaks code
   built against it. Feedback is welcome, even at this stage: open an issue
   at https://github.com/vomlehn/tcspecial/issues.

TCSpecial is **telemetry and control, special**, and it is flight software.
It connects the devices aboard a vehicle -- on an I\ :superscript:`2`\ C
bus, a SPI link, a serial line, a CAN bus, a set of discrete lines, a
socket or a device file -- to the places their data has to reach: the
mission operations center, the **MOC**, on the ground, and software aboard
that computes from it, keeps it, or sends it on. Every one of those is a
**node**, and what joins two is a **link**, and both are read from a
configuration file at startup, so what a particular vehicle does is a file.

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

`Writing node types <node-types.html>`__
    How to add a node type to TCSpecial.

`Third-party integration <third-party.html>`__
    The plan for making the tcs projects work with NASA's core Flight
    System (cFS) and JPL's F´ (F Prime). Work in progress.
