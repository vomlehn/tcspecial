===============
Getting Started
===============

.. contents:: Contents
   :depth: 2
   :local:

From nothing to a vehicle flying on your desk, in about fifteen minutes:
install the released packages, run one command, and watch a vehicle, a
simulator of its hardware and a ground station talk to one another. No
hardware, no build and no root, beyond installing.

TCSpecial is flight software. It runs aboard a vehicle and relays data
between the vehicle's payloads -- its sensors, switches and instruments --
and the ground. What a vehicle carries and does is a configuration file,
checked when it is loaded. On a desk, ``tcs-sim`` stands in for the
payloads and ``tcs-moc`` for the ground, so all of it runs on one machine.

What you need
=============

* An x86-64 Linux desktop with a C library, glibc, of 2.35 or later:
  Ubuntu 22.04 or later, Debian 12 or later, or a current Fedora.
  ``ldd --version`` says which you have. On an older system, run it in a
  container instead: see `On another system`_.
* ``curl``, and ``sudo`` to install packages.

Install
=======

Download the 0.3.1 packages from the GitHub release, and install them
with your package manager, which fetches what they depend on.

On Ubuntu or Debian:

.. code-block:: console

   $ mkdir tcs-0.3.1 && cd tcs-0.3.1
   $ release=https://github.com/vomlehn/tcspecial/releases/download/v0.3.1
   $ for p in tcspecial tcsmoc tcs-tools tcspecial-test tcspecial-examples
   > do curl -fLO "$release/${p}_0.3.1-1_amd64.deb"; done
   $ sudo apt update && sudo apt install ./*.deb

On Fedora:

.. code-block:: console

   $ mkdir tcs-0.3.1 && cd tcs-0.3.1
   $ release=https://github.com/vomlehn/tcspecial/releases/download/v0.3.1
   $ for p in tcspecial tcsmoc tcs-tools tcspecial-test tcspecial-examples
   > do curl -fLO "$release/${p}-0.3.1-1.x86_64.rpm"; done
   $ sudo dnf install ./*.rpm

Everything goes under ``/opt/tcs``. The packages put ``/opt/tcs/bin`` on
the path of login shells, so log out and in again, or, in the shell you
have:

.. code-block:: console

   $ export PATH=/opt/tcs/bin:$PATH

Then ``tcsx``, which runs the examples, lists them:

.. code-block:: console

   $ tcsx
   battery
   beacon
   ...
   thermostat

The manuals and designs are a sixth package, ``tcspecial-doc``, which you
can install the same way; they are also on the web, beside this page.

A first run
===========

.. code-block:: console

   $ tcsx --no-memlock simple-app
   Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do

In a few seconds that is a vehicle, tcspecial, starting a payload that is
a program, which sends a line of text a word at a time, and a ground
station that prints the words as they reach it.

``--no-memlock`` is there because of your desk, not the vehicle. Aboard,
tcspecial locks its memory into RAM, so that it is never paged out, and
refuses to run where it cannot: a vehicle whose memory is not held is not
the vehicle that was tested. A desktop usually lets a program lock only
8 MiB, a little less than tcspecial needs, so without the flag it stops
and says so. Raising the limit, with ``ulimit -l unlimited`` in a shell
whose limit allows it, does as well.

A vehicle, a simulator and a ground station
===========================================

.. code-block:: console

   $ tcsx --no-memlock thermostat

This is the worked example of computing aboard. The vehicle reads three
temperature sensors of one place, converts each reading to kelvin, votes
on the three so that one failing cannot mislead it, and runs a
thermostat that switches two heaters. Three programs start:

* **tcs-moc**, the test ground station, which starts the other two.
* **tcs-sim**, which simulates the vehicle's hardware: the three sensors
  and the two heater switches, joined to a model of the place they are
  in, which warms while the heaters are on and cools while they are off.
* **tcspecial**, the flight software, running the thermostat's
  configuration against the simulated hardware.

tcs-moc's window opens reading ``Disconnected``. Press ``Connect``: a
vehicle sends nothing down until a ground station has connected with the
configuration it is running. Then the readings come down, panel by panel:
each sensor in kelvin, the vote, and what each heater switch was told.
In tcs-sim's window, each payload has a panel of its own, and the
temperature moves as the heaters go on and off.

Closing tcs-moc's window stops all three.

Fly the lander
==============

.. code-block:: console

   $ tcsx --no-memlock lander

A lunar landing in two dimensions, flown by hand. Two tcspecials fly it,
one for each axis, and the ground watches through
`Yamcs <https://yamcs.org>`__, a mission control system, with everything
between them delayed 1.3 seconds each way, as light from the Moon is.
Fly it from the joystick in tcs-sim's window: press in its square and
drag along an arm. Land on the pad at no more than 3.05 m/s down and
1.22 m/s across, the Apollo lunar module's limits; then lift off again if
you like. Autoland flies it for you.

It needs Yamcs, which runs on Java. Install a Java runtime --
``default-jre-headless`` on Ubuntu, ``java-25-openjdk-headless`` on
Fedora -- and run it:

.. code-block:: console

   $ tcsx --no-memlock lander

The first run fetches Yamcs 5.14.0, checks it against the release's
checksum, and keeps it in ``~/.cache/tcspecial`` for every run after. To
use a Yamcs of your own instead, set ``YAMCS_HOME`` to where it is
unpacked, the directory holding ``bin/yamcsd``.

``tcsx`` lists the other examples through Yamcs too: ``ping-yamcs``,
``thermostat-yamcs`` and ``memusage``.

On another system
=================

Where the packages will not install -- an older distribution, or one
that is neither Debian-like nor Fedora-like -- run them in the test
container, a Kubuntu 26.04 or Fedora system of its own, made with
`Podman <https://podman.io>`__, that shows its windows on your desktop.
It needs Podman, git and a clone of tcspecial, but no build:

.. code-block:: console

   $ git clone https://github.com/vomlehn/tcspecial
   $ mkdir -p tcs-0.3.1/build/packages && cd tcs-0.3.1/build/packages
   $ release=https://github.com/vomlehn/tcspecial/releases/download/v0.3.1
   $ for p in tcspecial tcsmoc tcs-tools tcspecial-test tcspecial-examples
   > do curl -fLO "$release/${p}_0.3.1-1_amd64.deb"; done
   $ cd ../../.. && tcspecial/container/bin/start tcs-0.3.1

The first start builds the container's image, which takes some minutes,
installs the packages in it, and opens a terminal with a shell inside,
where ``tcsx --no-memlock thermostat`` and the rest run as above. Yamcs is
already there. Closing the terminal stops the container, which keeps
what was done in it for the next start. ``container/README.md`` says
more, including how to use the Fedora image and the RPMs instead.

Where next
==========

* `Building a vehicle <building-a-vehicle.html>`__ builds a small lunar
  lander's science deck in ten stages, from one payload to a whole
  vehicle, each stage a configuration that runs on your desk.
* `The tcspecial manual <tcspecial.html>`__ is the reference: every kind
  of payload and node, the configuration's every attribute, and each
  example, the thermostat and the lander among them.
* `The TCS documentation <manual.html>`__ lists the manuals of tcs-moc,
  tcs-sim, tcs-cfge -- the configuration editor -- and the rest, and the
  design documents.
* To build from source, see "Building from a checkout" in the README.

TCSpecial is an alpha release: it is not fully tested and not feature
complete. Feedback is welcome: open an issue at
https://github.com/vomlehn/tcspecial/issues.
