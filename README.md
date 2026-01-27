# TCSpecial
TCSpecial: Relays commands and telemetry between an operation center and payloads

This is TCSpecial, a utility for running on devices that relay data from set of
payloads to a remote operations center (OC). It has been designed within the context
of spacecraft operating at distances of several light seconds, such as the Moon, 
which has influenced the design to work with non-reliable communication links.
However, it will work find over any distance and is extensible to use reliable links
and.

# To Run

This is a pre-pre-pre-alpha release that mostly displays the GUI. There is a
design document in docs/design.rst. The docs/Makefile will build design.html
for viewing.

* Clone the source

* Type:

     make runmoc

  to run the test software

* To run just TCSpecial, use:

     make run

* There are make clean and make distclean targets.

# Stuff

TCSpecial supports standard network protocols and devices for communication, as
well as allowing custom protocols. So, it would be possible to implement your
own SLIP over COBS[#] implementation.

.. [#] COBS is the appreviation for "Constant Overhead Byte Stuffing", a protocol
       that does byte stuffing with a predictable and constant overhead.

Key TCSpecial features are:

* On-Board

  * Centralized control facility (the Command Interface or CI) that takes commands from
    the operations center (OC) and sends back telemetry. Telemetry includes an
    automatic beaconing message

  * Data handlers (DHs) that pass payload command and telemetry data to OC using a wide
    variety of interfaces and protocols.

    * Supports standard Linux networking protocols

    * Works standard Linux devices, such as RS-422, SPI,

    * Custom stackable DHs allow implementation of custom protocols

  * Per payload and overall TCSpecial statistics include:

    * Byte counts in both directions
    * I/O counts in both directions

  * "Memory allocation before main loop" philosophy. All TCSpecial memory allocatons
    are done on CI and DH startup.  As long as CI and DHs are started before the main
    loop,
    and any custom DHs allocate memory before the main loop, runtime memory exhaustion
    will not occur.

  * If necessary, DHs can be created and terminated at will with configurations
    either built in or specified as in a CI command. DHs may require memory allocation,
    so they will normally be started before the main loop. If not, they will still
    allocate all required memory on DH start up.

* Operations Center

  * The TCSpecial library (tcslib) is used to send commands and receive
    telemetry from
    the spacecraft. This can be integrated with mission control software such
    Yamcs.

* Test Software

  * The TCSpecial test interface (tcstest) provides simulated payload devices and
    uses tcslib to verify the behavior of TCSpecial.


To Do
=====
* Implement SPI interface

* Implement I2C interface.

* Create HTML and/or YAML interfaces so TCSpecialTest can read them and dynamically
  create network, device, and, eventually, stackable DHs. The later part has to
  consider how to dynamically load code.

Mandatory Note About the Name
=============================
The TCSpecial name indicates that it is a central process
for handling telemetry and commands. Yet, it is strangely similar to the TC
Special on the menu of Jose Arrendo's Uptown Enchilada Bar, an item which
brings back very fond memories even after 30 years.
