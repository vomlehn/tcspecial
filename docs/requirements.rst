======================
TCSpecial Requirements
======================

Introduction
============

.. table:: NOTE

   +--------------------------------------------------------------------------------+
   | TCSpecial has been designed for spacecraft and ground communication with high  |
   | latency communication links. It is just as applicable for other environments,  |
   | such as submersibles, drones, etc. Simply translate ^spacecraft^ to your       |
   | device type.                                                                   |
   +--------------------------------------------------------------------------------+

TCSpecial is a framework for passing commands to payload devices from            |
an operations center (OC) and conduiting
telemetry`from the payloads to the OC. This document sets for
requirements for its operation

About the names and the notes
-----------------------------

The requirements below are as they were written, with two kinds of change.

The names are the ones the software goes by. Three were settled differently
after this was written: the spacecraft process is **tcspecial**, not
``tcssvr``; the library both ends share is **tcslibgs**, not ``tcsdefs``; and
the testing GUI became two programs, **tcsmoc** for the ground and **tcssim**
for the payloads, rather than one ``tcstest``. A third test program,
**tcsverify**, checks the configuration files without running any of them.

A ``Not implemented`` or ``Implemented differently`` note says where the
software does not do what a requirement says. The requirement stands as
written: a note records what is so today, and is not an amendment. What each
of them does instead is described in ``design.rst``.

The name of the project is TCSpecial
====================================

TCSpecial shall include a process named tcspecial that runs on the spacecraft
=============================================================================

TCSpecial shall include a library named tcslib that links with misssion control software
========================================================================================

TCSpecial shall include a library named tcslibgs that is shared between tcspecial and tcslib
============================================================================================

TCSpecial shall include applications named tcsmoc and tcssim that provide GUIs for testing tcspecial, tcslibgs, and tcslib
==========================================================================================================================

.. note::
   Implemented as two programs rather than one. Tcsmoc stands in for the
   operations center and tcssim for the payloads, because a test needs both
   ends of every link and one window driving both says less about either.

TCSpecial must have a trait named Command Interpreter (CI) that parses commands sent by tcslib
==============================================================================================

.. note::
   Implemented differently. ``CommandInterpreter`` is a struct, not a trait:
   there is one command interpreter and nothing has needed a second
   implementation of it. The traits in tcspecial are at the layer below --
   ``EndpointWaitable``, ``EndpointReadable``, ``EndpointWritable`` -- where
   there really are many implementations, one per kind of endpoint.

TCSpecial must have a trait named data handler (DH) that is called by CI to process commands
============================================================================================

.. note::
   Implemented differently, for the reason above: ``DataHandler`` is a struct.
   What differs between handlers is the endpoints they open, which is where
   the traits are.

CI must be implemented by CI_UDP that communicates with tcslib code via UDP/IP
------------------------------------------------------------------------------

.. note::
   Implemented differently. There is no ``CI_UDP``: the command interpreter
   speaks UDP itself, on two sockets -- one for commands about the spacecraft
   and one for commands about a payload.

DH_UDP must implement DH and use UDP to send telemetry to tcslib
----------------------------------------------------------------

.. note::
   Implemented differently. There is no ``DH_UDP``. A handler's endpoints are
   per kind -- ``UdpEndpoint``, ``TcpEndpoint``, ``SerialEndpoint``,
   ``I2cEndpoint``, ``SpiEndpoint``, ``DeviceEndpoint``, and the Unix socket
   pair -- and a handler's OC endpoint is UDP whichever kind its payload is.

CI must support the following functions for implementing commands: ping(), exit(), status(), config(), start_dh(), stop_dh(), status_dh()

.. note::
   ``ping``, ``config``, ``start_dh`` and ``stop_dh`` are there.
   ``status_dh`` is ``query_dh``, which also has a companion,
   ``query_dh_sample``, for what a handler last sent and received. ``exit``
   and ``status`` are not implemented; see below. Two commands this did not
   ask for were added: ``connect``, which is each end saying what
   configuration it read, and ``restart_arm`` with ``restart``.

CI must have an element named dh of type BTreeMap
-------------------------------------------------

.. note::
   Implemented, named ``data_handlers``, and behind a mutex and a reference
   count: ``Arc<Mutex<BTreeMap<DHId, DataHandler>>>``. Two command links and
   the handlers' own threads reach it.

CI must support sending telemetry responses to all CI commands
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Each CI telemetry response to a command must contain a timestamp, command sequence number, a success or error indication
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. note::
   Implemented. ``TelemetryHeader`` carries the sequence number, the
   telemetry type, the status and the time the spacecraft made the answer.
   Two responses used to carry a time of their own and the other eight
   carried none; it is in the header for all of them now.

CI must periodically send a Telemetry::Beacon message with a timestamp and unique sequence number
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. note::
   Implemented, and to a multicast group so that any number of ground
   stations can hear it. The number is counted by the sender from one, so it
   starts again when tcspecial does, and the MOC tells a gap from a restart
   by that. A beacon also carries what a CONNECT is answered with -- the
   software version, the version the payload set states, and the digest of
   the configuration file the spacecraft read -- which this did not ask for.

CI must parse commands from the ground receved via UDP/IP
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

All CI commands must include a sequence number that increments for each command sent
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CI must support an EXIT command that terminates the tcspecial process
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

.. note::
   Not implemented. There is no EXIT command. RESTART comes nearest, and is
   deliberately not the same thing: it has to be armed by a RESTART_ARM
   first, so that one lost or malformed datagram cannot stop the flight
   software.

CI must support a STATUS command that sends telemetry containing status to ground

.. note::
   Not implemented. There is no STATUS command for the interpreter as a
   whole. QUERY_DH answers for one handler -- its statistics -- and
   QUERY_DH_SAMPLE for what it last sent and received; a ground station asks
   about every handler by asking about each.

Tcspecial includes zero or more components named Data Handler (DH)
------------------------------------------------------------------


The top part of the tcsmoc window is for sending commands to tcspecial's CI and viewing telemetry from tcspecial's CI
=====================================================================================================================

The middle part of the tcsmoc window is for viewing telemetry from individual DHs
=================================================================================
