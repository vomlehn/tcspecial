======================
TCSpecial Requirements
======================

Introduction
============

.. table:: NOTE

   +--------------------------------------------------------------------------------+
   | TCSpecial has been designed for spacecraft and ground communication with high  |
   | latency communication links. It is just as applicable for other environments,  |
   | such as submersibles, drones, etc. Simply translate "spacecraft" to your       |
   | device type.                                                                   |
   +--------------------------------------------------------------------------------+

TCSpecial is a framework for passing commands to payload devices from            |
an operations center (OC) and relaying
telemetry`from the payloads to the OC. This document sets for
requirements for its operation

The name of the project is TCSpecial
====================================

TCSpecial shall include a process named tcssvr that runs on the spacecraft
--------------------------------------------------------------------------

Tcssvr must have a trait named Command Interpreter (CI)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

CI must send telemetry to the ground via UDP/IP
"""""""""""""""""""""""""""""""""""""""""""""""

CI must support sending telemetry responses to all CI commands
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""

Each CI telemetry response to a command must contain a command sequence number, a success or error indication
"""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""

CI must periodically send a BEACON telemetry message with a timestamp and unique sequence number
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""

CI must parse commands from the ground receved via UDP/IP
"""""""""""""""""""""""""""""""""""""""""""""""""""""""""

All CI commands must include a sequence number that increments for each command sent
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""

CI must support an EXIT command that terminates the tcssvr process
""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""""

CI must support a STATUS command that sends telemetry containing status to ground

Tcssvr includes zero or more components named Data Handler (DH)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
