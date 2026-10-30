# TCSpecial
TCSpecial: flight software that relays commands and telemetry between an operation center and payloads

This is TCSpecial, flight software for running on devices that relay data from set of
payloads to a remote operations center (OC). It has been designed within the context
of spacecraft operating at distances of several light seconds, such as the Moon, 
which has influenced the design to work with non-reliable communication links.
However, it will work find over any distance and is extensible to use reliable links
and.

Work is under way on integrating TCSpecial, with its telemetry log
[tcslog](https://github.com/vomlehn/tcslog) and its packet compiler
[tcs-xtce](https://github.com/vomlehn/tcs-xtce), with NASA's core Flight System
(cFS) and JPL's F´ (F Prime): see docs/third-party.rst.

> **Note:** integration with cFS and F´ is a work in progress. None of it is
> built yet, and the plan may change as it is.

# To Run

This is a pre-pre-pre-alpha release that mostly displays the GUI. There is a
design document in docs/design.rst, a manual in docs/manual.rst, and a plan
for integration with cFS and F´ in docs/third-party.rst. The docs/Makefile
will build HTML pages of each for viewing.

* Clone the source

* Type:

     make runmoc

  to run the test software

* To run just TCSpecial, use:

     make run

* There are make clean and make distclean targets.

# Installing

Everything installs under `/opt/tcs`, as three packages:

* `tcspecial`, what flies: `bin/tcspecial`, `bin/tcsverify`,
  the five test app programs in `libexec/`, the documents and the example
  configurations, and a systemd unit, `tcspecial.service`, installed but not
  enabled.
* `tcspecial-ground`, what a ground station is built on: `libtcs` for C and
  C++ (`lib/libtcs.so.1`, `lib/libtcs.a`, `include/tcs.h`,
  `lib/pkgconfig/tcs.pc`), the Python packages `tcslibgs`, `tcslib` and
  `tcsui`, and `bin/tcsgui` with its data.
* `tcspecial-test`, the test ground station and simulator: `bin/tcsmoc` and
  `bin/tcssim`, and `bin/segsend`, the test one, which deletes each
  telemetry log segment. A vehicle that keeps a log puts its own segsend in
  `/opt/tcs/bin` instead.

To build them, or to install without them:

     make deb                                # build/packages/*.deb
     make rpm                                # build/packages/*.rpm
     make install-build && sudo make install-files HOOKS=1
     make install PREFIX=$HOME/tcs           # no hooks outside the prefix
     pip install ./python                    # the Python alone
     cargo install --path tcsverify --root /opt/tcs
     cc station.c $(pkg-config --cflags --libs tcs)

`HOOKS=1`, which the packages have, also installs what puts `/opt/tcs` on
the search paths: `/etc/profile.d/<package>.sh`,
`/etc/ld.so.conf.d/tcspecial-ground.conf` and a `.pth` file for python3.

The profile.d scripts put `/opt/tcs/bin` on `PATH` for login shells only:
sudo, cron and other minimal environments do not see it. To make a
package's programs available everywhere, link them into `/usr/local/bin`,
and to undo it, take the links away:

     sudo /opt/tcs/sbin/tcspecial-links          # likewise -ground, -test
     sudo /opt/tcs/sbin/tcspecial-links --remove
     sudo make links                             # for a make install
     sudo make unlinks

Each script links only its own package's programs, leaves alone a name
something else already has, and takes `--dir DIR` for a directory other
than `/usr/local/bin`. Removing a package takes its links away.

Everything installed is stripped (`--strip-unneeded` for programs and
`libtcs.so.1`, `--strip-debug` for `libtcs.a`), which takes a fifth to a
quarter off what each package installs. `STRIP=0` keeps everything as cargo
built it:
`make packages STRIP=0`.

See "Installing" in docs/manual.rst for where everything goes and why.

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

  * Extensible in any language. A node can be a program of its own -- in
    Python, C, a shell script, Rust, or anything else the vehicle can execute --
    that tcspecial starts from the configuration and connects into the graph:
    on the vehicle's side (`app_payload_veh`, `app_moc_veh`, `app_all_veh`) or
    as a payload that is a program (`app_pl`, `app_all_pl`). The program talks
    to tcspecial over file descriptors carrying length-prefixed packets, so it
    links against nothing of tcspecial's; the `tcsapp` crate speaks that for
    Rust, and `examples/simple-app` has a payload written in Python 3 with
    nothing outside its standard library. A `process` computation likewise
    hands what arrives to any program, a line at a time, and takes back what
    it writes. A capability added this way needs no change to the flight
    software and no rebuild of it.

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

See also
========
* [The tcspecial manual](https://vomlehn.github.io/tcspecial/docs/manual.html): how to write a
  configuration, and how to run the flight software and the programs that
  test it.

* [The tcspecial design](https://vomlehn.github.io/tcspecial/docs/design.html): the specification, and why
  each rule is what it is.

* [The examples](https://vomlehn.github.io/tcspecial/examples/README.html): working configurations, each
  with a diagram.

* [tcslib](https://vomlehn.github.io/tcspecial/tcslib/README.html), [tcslibgs](https://vomlehn.github.io/tcspecial/tcslibgs/README.html) and
  [tcslib-c](https://vomlehn.github.io/tcspecial/tcslib-c/README.html): the libraries a ground station is
  built on, and what the ground and the vehicle share.

* [Third-party software](https://vomlehn.github.io/tcspecial/docs/third-party.html): what tcspecial is built
  with, and under which licenses.

License
=======
Licensed under either of the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT license
([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

Mandatory Note About the Name
=============================
The TCSpecial name indicates that it is a central process
for handling telemetry and commands. Yet, it is strangely similar to the TC
Special on the menu of Jose Arrendo's Uptown Enchilada Bar, an item which
brings back very fond memories even after 30 years.
