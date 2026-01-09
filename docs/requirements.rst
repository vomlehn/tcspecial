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
an operations center (OC) and relaying
telemetry`from the payloads to the OC. This document sets for
requirements for its operation

The name of the project is TCSpecial
====================================

TCSpecial shall include a process named tcssvr that runs on the spacecraft
==========================================================================

TCSpecial shall include a library named tcslib that links with misssion control software
========================================================================================

TCSpecial shall include a library named tcsdefs that is shared between tcssvr and tcslib
========================================================================================

TCSpecial shall include an application named tcstest that provides a GUI for testing tcssvr, tcsdefs, and tcslib
================================================================================================================


TCSpecial must have a trait named Command Interpreter (CI) that parses commands sent by tcslib
==============================================================================================

TCSpecial must have a trait named data handler (DH) that is called by CI to process commands
============================================================================================

CI must be implemented by CI_UDP that communicates with tcslib code via UDP/IP
------------------------------------------------------------------------------

DH_UDP must implement DH and use UDP to send telemetry to tcslib
----------------------------------------------------------------

CI must support the following functions for implementing commands: ping(), exit(), status(), config(), start_dh(), stop_dh(), status_dh()

CI must have an element named dh of type BTreeMap
-------------------------------------------------

CI must support sending telemetry responses to all CI commands
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Each CI telemetry response to a command must contain a timestamp, command sequence number, a success or error indication
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CI must periodically send a Telemetry::Beacon message with a timestamp and unique sequence number
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CI must parse commands from the ground receved via UDP/IP
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

All CI commands must include a sequence number that increments for each command sent
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CI must support an EXIT command that terminates the tcssvr process
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CI must support a STATUS command that sends telemetry containing status to ground

Tcssvr includes zero or more components named Data Handler (DH)
---------------------------------------------------------------


The top part of the tcstest window is for sending commands to tcssvr's CI and viewing telemetry from tcssvr's CI
================================================================================================================

The middle part of the tcswindow is for viewing telemetry from individual DHs
=============================================================================
