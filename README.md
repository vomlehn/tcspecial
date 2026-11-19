# TCSpecial
TCSpecial: flight software that relays commands and telemetry between an operation center and payloads

This is TCSpecial. Its TCSpecial Flight Executive is flight software for
devices that relay data between a set of payloads and a remote operations
center (OC). It was designed for spacecraft several light seconds away, such
as at the Moon, which shaped it to work over unreliable communication links.
However, it works over any distance, and can be extended to use reliable
links.

TCSpecial runs as a process of its own, so the kernel keeps it in a fault
domain apart from the rest of the vehicle's software. Because it reaches the
operations center, an F´ flight computer or cFS's software bus over UDP, it
can also run on a processor of its own, isolating faults further still. And
a vehicle may run several TCSpecials, each with its own group of payloads,
so that one that fails leaves the rest running.

> **Warning:** TCSpecial is an alpha release. It is not fully tested, it
> is not feature complete, and it is subject to rapid change. Until 1.0,
> any change to its API changes the minor version number (0.1 to 0.2), so
> a release that changes only the patch number never breaks code built
> against it. Feedback is welcome, even at this stage: open an issue at
> https://github.com/vomlehn/tcspecial/issues.

Work is under way on integrating TCSpecial, with its telemetry log, the TCS
Onboard Log [tcslog](https://github.com/vomlehn/tcslog), and its packet compiler
[tcs-xtce](https://github.com/vomlehn/tcs-xtce), with NASA's core Flight System
(cFS) and JPL's F´ (F Prime): see docs/third-party.rst.

TCSpecial is the hub of the TCS software: tcs-xtce, tcslog, tcslog-tools
and tcs-helpers are the libraries and tools it is built from and works
with, and docs/manual.rst links to their documents.

> **Note:** integration with cFS and F´ is a work in progress, and the plan
> may change as it is. A vehicle can answer to an F´ flight computer through
> the bridge component in fprime/ (`make fprime-test` tests it), and to cFS's
> software bus through SBN (`make cfs-test` flies the cfs example against a
> real cFS).

# Quick start

[Getting started](docs/getting-started.rst) takes you from nothing to a
vehicle, a simulator of its hardware and a ground station running on your
desk, in about fifteen minutes, from the released packages: no build. On
Ubuntu 22.04 or later, or Debian 12 or later:

     release=https://github.com/vomlehn/tcspecial/releases/download/v0.3.1
     for p in tcspecial tcsmoc tcs-tools tcspecial-test tcspecial-examples
     do curl -fLO "$release/${p}_0.3.1-1_amd64.deb"; done
     sudo apt update && sudo apt install ./*.deb
     export PATH=/opt/tcs/bin:$PATH
     tcsx --no-memlock thermostat

Then press `Connect` in tcs-moc's window. The guide also covers Fedora,
the lander through Yamcs, and a container for older systems.

# Building from a checkout

This is an alpha release: it is not fully tested and not feature complete.
There is a design document for each program, listed in docs/design.rst, a
manual for each, listed in docs/manual.rst, and a plan for integration with
cFS and F´ in docs/third-party.rst. The manual's "Installing" section, in
docs/tcspecial.rst, says all of what follows at greater length.

First the tools, then the two checkouts every build needs beside this one:
[tcs-helpers](https://github.com/vomlehn/tcs-helpers), the make rules and
packaging scripts the TCS projects share, and
[tcs-xtce](https://github.com/vomlehn/tcs-xtce), whose Python package
generates the link's codecs. `make install-tools` installs, with apt-get or
dnf under sudo, everything building, testing, checking, documenting and
packaging need (`DRY_RUN=1` shows the commands, `PACKAGING=0` leaves out
the packaging tools), and says what it cannot: a Rust older than 1.92, a
Python older than 3.11, and the checkouts. tcslog, the telemetry log, comes
from crates.io, so it needs no checkout.

     git clone https://github.com/vomlehn/tcspecial.git
     git clone https://github.com/vomlehn/tcs-helpers.git
     git clone https://github.com/vomlehn/tcs-xtce.git
     make -C tcs-xtce python/tcs_xtce/generated.py
     cd tcspecial
     make install-tools

Then build, and run:

     make                 # build tcspecial, then run every test
     cargo build --workspace   # every program and library: tcs-moc,
                          # tcs-sim, tcs-cfge, tcs-verify, tcsmoc-c, ...
     make buildmoc        # tcs-moc alone; buildsim, buildgui (tcs-cfge) too
     make test            # every test; tcs-sim's opens a window
     make check           # cargo check, clippy, rustfmt, and the rest
     make docs            # every page under docs/, with rst2html
     make runmocy         # run tcs-moc on example.ytcs, which starts
                          # tcspecial and tcs-sim; runmocx on example.xtcs
     make run             # run tcspecial alone
     make clean           # and make distclean

`make help` lists the rest. A tcspecial for a vehicle is built with only
the payload types it flies: see "Building tcspecial for a vehicle" in
docs/tcspecial.rst. The examples build and run from examples/, as
[its README](examples/README.md) says, and the Python, the C library and
the packages as *Installing*, below, says.

The cFS and F´ sides are built from their own projects' trees, which this
one does not hold:

     make fprime-build FPRIME_FRAMEWORK=$HOME/fprime FPRIME_VENV=$HOME/fprime-venv
     make fprime-test  FPRIME_FRAMEWORK=$HOME/fprime FPRIME_VENV=$HOME/fprime-venv
     make cfs-build CFS_PRISTINE=$HOME/cFS SBN_SRC=$HOME/sbn
     make cfs-test  CFS_PRISTINE=$HOME/cFS SBN_SRC=$HOME/sbn

The F´ side needs an F´ checkout, v4.4.1 or later, and a Python environment
holding F´'s tools; the cFS side a cFS checkout and a git checkout of SBN
v7.0.1. Left out, each is looked for beside this checkout: see "The fprime
example" and "The cfs example" in docs/tcspecial.rst.
[container/](container/README.md) builds and tries a checkout, and the
packages, in a Podman container of its own.

## The other TCS projects

Each is built from its own checkout, beside this one, as its README says:

* [tcs-xtce](https://github.com/vomlehn/tcs-xtce#building), the XTCE
  library, packet compiler and editor: its README's "Building", with `make
  install-tools` of its own.
* [tcslog](https://github.com/vomlehn/tcslog#building), the telemetry log
  and its C and Python bindings: its README's "Building", with `make
  install-tools` of its own.
* [tcslog-tools](https://github.com/vomlehn/tcslog-tools#building-from-a-checkout),
  the log readers `tcslog-dump` and `tcslog-dumphdr`: its README's
  "Building from a checkout", built beside a tcslog checkout.
* [tcs-helpers](https://github.com/vomlehn/tcs-helpers), the shared make
  rules and packaging scripts: nothing to build, used where it is checked
  out.
* [tcswebasm](https://github.com/vomlehn/tcswebasm), the examples in a web
  page: built from this checkout, beside it, for WebAssembly, with nightly
  Rust, the `wasm32-unknown-unknown` target and `wasm-bindgen-cli`, as its
  manual, docs/manual.rst, says.

# Installing

Everything installs under `/opt/tcs`, as six packages, each built as a
.deb and as an RPM. A package with operational code holds nothing else:

* `tcspecial`, operational code, what flies: `bin/tcspecial`, with its
  README, licences and notices, and a systemd unit, `tcspecial.service`,
  installed but not enabled.
* `tcsmoc`, operational code, what a ground station is built on:
  `libtcsmoc-c` for C and C++ (`lib/libtcsmoc-c.so.4`, `lib/libtcsmoc-c.a`,
  `include/tcsmoc-c.h`, `lib/pkgconfig/tcsmoc-c.pc`), the Python packages
  `tcsprotocol` and `tcsmoc`, and the configuration schemas. It was
  `tcspecial-moc`, which it replaces.
* `tcs-tools`, software tools: `bin/tcs-cfge` with its data,
  `bin/tcs-verify`, and the Python `tcs_ui`.
* `tcspecial-test`, verification tools: the test ground station and
  simulator, `bin/tcs-moc` and `bin/tcs-sim`; `bin/segsend`, the test one,
  which deletes each telemetry log segment; and the five test app programs
  in `libexec/`. A vehicle that keeps a log puts its own segsend in
  `/opt/tcs/bin` instead.
* `tcspecial-doc`, the manuals and designs.
* `tcspecial-examples`, the examples: every one's configurations and the
  programs it runs, its own ground stations among them, in
  `share/examples/tcspecial`, and `bin/tcsx`, which runs one: `tcsx`
  lists them, and `tcsx thermostat` runs one. The manuals' figures are
  the examples' diagrams, so `tcspecial-doc` depends on it.

Building them needs checkouts of
[tcs-helpers](https://github.com/vomlehn/tcs-helpers) and
[tcs-xtce](https://github.com/vomlehn/tcs-xtce) beside this one, the second
with its `python/tcs_xtce/generated.py` made; `make tcs-helpers` and `make
tcs-xtce` say what is missing. To build them, install them, or install
without them:

     make deb                                # build/packages/*.deb
     make rpm                                # build/packages/*.rpm
     make install                            # with apt-get, dnf or zypper
     make install DRY_RUN=1                  # what that would do
     make uninstall                          # remove the packages
     make install-build && sudo make install-files HOOKS=1
     make install-files PREFIX=$HOME/tcs     # no hooks outside the prefix
     pip install ./python                    # the Python alone
     pip install ./tcs-ui/python             # and tcs_ui, which needs it
     cargo install --path tcs-verify --root /opt/tcs
     cc station.c $(pkg-config --cflags --libs tcsmoc-c)

`HOOKS=1`, which the packages have, also installs what puts `/opt/tcs` on
the search paths: `/etc/profile.d/<package>.sh`,
`/etc/ld.so.conf.d/tcsmoc.conf` and a `.pth` file for python3.
Each .deb also carries what Debian expects of every package:
`/usr/share/doc/<package>/copyright` and `changelog.Debian.gz`, made as it
is built, and lintian's overrides in `/usr/share/lintian/overrides`.
`make lint-packages` checks the packages with lintian and rpmlint, and
their shell scripts with shellcheck, and fails on any finding that is not
overridden with its reason.

The profile.d scripts put `/opt/tcs/bin` on `PATH` for login shells only:
sudo, cron and other minimal environments do not see it. To make a
package's programs available everywhere, link them into `/usr/local/bin`,
and to undo it, take the links away:

     sudo /opt/tcs/sbin/tcspecial-links          # likewise tcs-tools-links,
                                                 # tcspecial-test-links and
                                                 # tcspecial-examples-links
     sudo /opt/tcs/sbin/tcspecial-links --remove
     sudo make links                         # after make install-files
     sudo make unlinks

Each script links only its own package's programs, leaves alone a name
something else already has, and takes `--dir DIR` for a directory other
than `/usr/local/bin`. Removing a package takes its links away.

Everything installed is stripped (`--strip-unneeded` for programs and
`libtcsmoc-c.so.4`, `--strip-debug` for `libtcsmoc-c.a`), which takes a
fifth to a quarter off what each package installs. `STRIP=0` keeps everything as
cargo built it: `make packages STRIP=0`.

See "Installing" in docs/tcspecial.rst for where everything goes and why.

# Where things are

Each top-level directory, by the category of TCS code it holds, as
`categories.toml` lists them: see "Operational code and its names" in
[the design](docs/tcspecial-design.rst), and "Code categories" in
[tcs-helpers' README](https://github.com/vomlehn/tcs-helpers#code-categories).

Operational code, which a mission depends on while it operates:

* [`tcspecial/`](tcspecial/): the flight software, the TCSpecial Flight
  Executive.
* [`tcsprotocol/`](tcsprotocol/): TCS Link Support, what the vehicle and
  the ground share: the protocol, and how every configuration is read and
  checked.
* [`tcsmoc/`](tcsmoc/) and [`tcsmoc-c/`](tcsmoc-c/): the TCSpecial MOC
  Correspondent, what a ground station is built on, in Rust and for C and
  C++.
* [`tcsverify/`](tcsverify/): the configuration checker as a library, which
  tcspecial and every program that reads a configuration asks.
* [`tcsapp/`](tcsapp/): the file interfaces an app node's program runs with.
* [`tcsos/`](tcsos/): what tcspecial and the rest ask of the operating
  system -- files, sockets, processes, time -- on Linux, and in a web page
  the browser's substitutes for them, which tcswebasm runs on.
* [`python/`](python/): the Python spellings of tcsprotocol and tcsmoc,
  for ground stations written in Python.

Software tools, which make or check what operational code is built from or
loads:

* [`tcs-cfge/`](tcs-cfge/): the graphical configuration editor.
* [`tcs-verify/`](tcs-verify/): the configuration checker as a program.
* [`tcs-conflict/`](tcs-conflict/): whether two configurations can be run
  at once.
* [`tcs-diagram/`](tcs-diagram/): how a configuration is drawn, for tcs-cfge
  and tcspecial's visual window, which never flies.
* [`tcs-ui/`](tcs-ui/): the window pieces tcs-moc and the desk tools
  share: the beacon and ping panels, fonts and colours, in Rust and Slint,
  and in Python, `tcs-ui/python`, a distribution of its own.
* [`tools/`](tools/): `tools/convert-format.py`, which rewrites a
  configuration from the old shape into the new.

Verification tools, used only to test:

* [`tcs-moc/`](tcs-moc/): the test ground station.
* [`tcs-sim/`](tcs-sim/): the payload simulator.
* [`app_pl/`](app_pl/), [`app_all_pl/`](app_all_pl/),
  [`app_payload_veh/`](app_payload_veh/),
  [`app_gateway_veh/`](app_gateway_veh/) and
  [`app_all_veh/`](app_all_veh/): the five test apps, one for each app node
  type.
* [`tests/`](tests/): the configurations the tests read -- the ids, the
  refused ones, the simulated devices -- and `tests/manual/`, the editor's
  example.
* [`cfs/`](cfs/) and [`fprime/`](fprime/): NASA's cFS and JPL's F´
  software, generated or written for them, that tcspecial is tested
  beside; see [docs/third-party.rst](docs/third-party.rst). It is not TCS
  software, and `make ntss` does not count it.
* [`bin/`](bin/): scripts for a desk, such as those that make and remove the
  kernel's dummy devices (`bin/mk-kernel-dummies`).

Infrastructure, which builds, packages, documents and checks the rest:

* [`docs/`](docs/): every manual and design document.
* [`packaging/`](packaging/): what the `.deb` and `.rpm` packages are made
  from: their control files, specs and install hooks.
* [`container/`](container/): the test container, in which the packages
  are installed and a checkout built without touching the machine.
* `bin/install-tools`, which `make install-tools` runs, and
  `tools/ntss.py`, which `make ntss` runs: it counts the non-test source
  statements of every tcs project checked out beside this one, by category,
  subproject and language, as tables or, with `CSV=1`, comma-separated
  values.
* `.cargo/`, `.claude/` and `.github/`: settings for cargo, Claude Code and
  GitHub's workflows.

Examples:

* [`examples/`](examples/): working configurations and the programs they
  run; see [its README](examples/README.md).
* [`docs/building-a-vehicle/`](docs/building-a-vehicle/): the guide's
  lander, built a stage at a time.

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
    on the vehicle's side (`app_payload_veh`, `app_gateway_veh`, `app_all_veh`)
    or as a payload that is a program (`app_pl`, `app_all_pl`). The program
    talks to tcspecial over file descriptors carrying length-prefixed packets,
    so it links against nothing of tcspecial's; the `tcsapp` crate speaks that
    for Rust, and `examples/simple-app` has a payload written in Python 3 with
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

  * The TCSpecial MOC Correspondent (tcsmoc) is used to send commands and
    receive telemetry from the spacecraft. This can be integrated with mission
    control software such Yamcs.

* Test Software

  * The TCSpecial test interface (tcstest) provides simulated payload devices and
    uses tcsmoc to verify the behavior of TCSpecial.


To Do
=====
* Implement SPI interface

* Implement I2C interface.

* Create HTML and/or YAML interfaces so TCSpecialTest can read them and dynamically
  create network, device, and, eventually, stackable DHs. The later part has to
  consider how to dynamically load code.

See also
========
* [TCS Design and Documentation](https://vomlehn.github.io/tcspecial/docs/manual.html):
  every TCS manual and design document, by the category of the code it
  is about, of which TCSpecial is the hub.
* [The tcspecial
manual](https://vomlehn.github.io/tcspecial/docs/tcspecial.html):
  how to write a configuration, and how to run the flight software and the
  programs that test it. tcs-cfge, tcs-moc, tcs-sim and tcs-verify each have a
  manual of their own, beside it.

* [The tcspecial design](https://vomlehn.github.io/tcspecial/docs/design.html): the specification, and why
  each rule is what it is.

* [The examples](https://vomlehn.github.io/tcspecial/examples/README.html): working configurations, each
  with a diagram.

* [tcsmoc](https://vomlehn.github.io/tcspecial/tcsmoc/README.html), [tcsprotocol](https://vomlehn.github.io/tcspecial/tcsprotocol/README.html) and
  [tcsmoc-c](https://vomlehn.github.io/tcspecial/tcsmoc-c/README.html): the libraries a ground station is
  built on, and what the ground and the vehicle share: the TCSpecial MOC
  Correspondent, TCS Link Support, and the Correspondent for C and C++.

* [Third-party integration](https://vomlehn.github.io/tcspecial/docs/third-party.html): how tcspecial would
  work with cFS and F´.

License
=======
Licensed under either of the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>) or the MIT license
([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

Each package lists the Rust crates compiled into its programs and
libraries, with their licences and the notices those require, in
`/opt/tcs/share/doc/<package>/THIRD-PARTY-NOTICES`.

The graphical programs -- tcs-cfge, tcs-moc, tcs-sim and tcspecial's visual
window -- are built with [Slint](https://slint.dev), under its
royalty-free licence for desktop applications, which asks for this
attribution:

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

That licence does not cover embedded systems, which is one more reason the
visual window is never in a tcspecial that flies.

Mandatory Note About the Name
=============================
The TCSpecial name indicates that it is a central process
for handling telemetry and commands. Yet, it is strangely similar to the TC
Special on the menu of Jose Arrendo's Uptown Enchilada Bar, an item which
brings back very fond memories even after 30 years.
