================
TCSpecial Design
================

How TCSpecial works inside, and why. Each of its programs has a design
document of its own, as it has a manual of its own.

Production software
===================

`The tcspecial design <tcspecial-design.html>`__
    The flight software, and what the programs share: the terminology, the
    node architecture and typed links, gateways and what the vehicle answers
    to, compute elements and app nodes, how a graph runs, what is checked
    before anything runs, the interface and the link, fault resiliency and
    restarts, payloads and their link terms, platforms and porting, testing
    with simulated devices, and the requirements.

Non-production software
=======================

`The tcs-gui design <tcs-gui-design.html>`__
    The editor: how it draws, reads and writes a configuration, its
    descriptions of the node types, its undo history, autosave and
    recovery.

`The tcs-sim design <tcs-sim-design.html>`__
    The payload simulator: its configuration files, the physics model,
    fault injection, and the program.

`The tcs-moc design <tcs-moc-design.html>`__
    The simulated operations center: its window, what it reads, and where
    the addresses it talks to come from.

`The tcs-verify design <tcs-verify-design.html>`__
    The configuration checker.

How to use each program is in its manual, and what is production software
and what is not is said in `TCS Design and Documentation <manual.html>`__,
which lists these, the manuals and the other TCS projects' documents.
