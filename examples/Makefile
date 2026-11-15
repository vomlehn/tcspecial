# Makefile for the examples

PROJECT_NAME := tcspecial

# The repository's root, where cargo is run and the examples' paths start.
ROOT = ..

# What tcspecial logs, as in the top-level Makefile:
#   make run-simple RUST_LOG=debug
RUST_LOG = info

# The run's simulation mode, given to every program a run target starts:
# none, some or all. All by default, because these targets run on a desk with
# simulated and dummy devices, and a program that leaves the flag out gets
# none -- the flight setting -- and refuses every one. See "Simulation Modes" in
# docs/tcspecial-design.rst.
#   make run-simple SIMULATE=some
SIMULATE = all

# Whether the tcspecial a run target starts locks its memory into RAM, which
# it does unless given --no-memlock. NO_MEMLOCK=1 gives it, for a desk whose
# limit on locked memory is too small for the lock: see "Locking memory" in
# docs/tcspecial.rst.
#   make run-simple NO_MEMLOCK=1
NO_MEMLOCK =
MEMLOCK_FLAG = $(if $(NO_MEMLOCK),--no-memlock)

# Where cargo puts the debug build the run targets run, as an absolute
# path, since tcsx runs from a directory of its own.
TARGET_DIR = $(or $(CARGO_TARGET_DIR),target)
DEBUG_DIR = $(strip $(if $(filter /%,$(TARGET_DIR)),$(TARGET_DIR), \
	$(abspath $(ROOT)/$(TARGET_DIR))))/debug

all:	docs

# Display help
.PHONY: help
help:
	@echo "Makefile for the tcspecial examples"
	@echo ""
	@echo "Usage:"
	@echo "  make README.html     - Build README.html from README.md"
	@echo "  make run-simple      - Run tcs-moc on the minimal example"
	@echo "  make run-thermostat  - Run tcs-moc on the thermostat example"
	@echo "  make run-battery     - Run tcs-moc on the battery example:" \
		"a power bus through an orbit, a load shed in eclipse"
	@echo "  make run-descent     - Run tcs-moc on the descent example:" \
		"a lander flown down by a guidance program"
	@echo "  make run-simple-app  - Run the simple-app example, printing" \
		"its line"
	@echo "  make run-beacon      - Run the beacon example: a window" \
		"watching the beacon, with a button that starts and stops" \
		"tcspecial"
	@echo "  make run-ping        - Run the ping example: a Python window" \
		"that pings the vehicle"
	@echo "  make run-ping-yamcs  - Run the ping-yamcs example: a Python" \
		"window that pings the vehicle through Yamcs, over CCSDS" \
		"packets (YAMCS_HOME=<yamcs>)"
	@echo "  make run-memusage    - Run the memusage example: tcspecial's" \
		"memory and every thread's stack, watched through Yamcs" \
		"(YAMCS_HOME=<yamcs>; MEMUSAGE=<configurations> for more" \
		"than one tcspecial)"
	@echo "  make run-thermostat-yamcs - Run the thermostat-yamcs" \
		"example: the thermostat, watched through Yamcs, over CCSDS" \
		"packets (YAMCS_HOME=<yamcs>)"
	@echo "  make run-tees        - Run tcs-moc on the tees example: tees" \
		"and null nodes"
	@echo "  make run-fprime      - Run tcs-moc on the fprime example," \
		"playing the F´ bridge"
	@echo "  make run-cfs         - Run tcs-moc on the cfs example," \
		"tcs-sim simulating the cFS end"
	@echo "  make run-can         - Run tcs-moc on the can example, on" \
		"the kernel's virtual CAN bus"
	@echo "  make run-gpio        - Run tcs-moc on the gpio example, on" \
		"the kernel's gpio-sim chip"
	@echo "  make run-iio         - Run tcs-moc on the iio example, on" \
		"the kernel's iio_dummy sensor"
	@echo "  make check-python3   - Check for the Python 3 simple-app's" \
		"payload needs"
	@echo "  make check-tkinter   - Check for the tkinter the ping" \
		"example's window needs"

# Running an example: tcsx, examples/tcsx, runs it from these examples and
# the programs of this tree's debug build, as it does an installed one from
# what tcspecial-examples installs, so how each example is run is said
# once, there. tcsx makes /tmp/telemetry, where the examples log, afresh
# for each run, so that what is in it afterwards is that run's alone.
#
# $(1) is the example, $(2) the programs built for it first, and $(3) what
# tcsx gives its ground station. tcspecial gives each filled log segment to
# the segsend built beside it, and refuses to start without one, so
# segsend is built with the rest. YAMCS_HOME, where a Yamcs release is
# unpacked, is for the examples through Yamcs.
define tcsx
	cd $(ROOT) && cargo build -q $(addprefix --bin ,$(2))
	cd $(ROOT) && RUST_LOG=$(RUST_LOG) YAMCS_HOME="$(YAMCS_HOME)" \
		TCSX_EXAMPLES=examples TCSX_BIN=$(DEBUG_DIR) \
		examples/tcsx --simulate=$(SIMULATE) $(MEMLOCK_FLAG) $(1) $(3)
endef

# Running tcs-moc on one of the examples.
#
# $(1) is the example's directory, whose configuration is named for it. tcs-moc
# starts tcspecial on the same file, and tcs-sim where the simulator file
# beside it -- $(1)-sim.yaml, found by name -- simulates anything; its panels
# show what each source sends to the ground. A payload on a device -- the
# thermostat's I2C, SPI and GPIO -- needs that device. `make mkstubs` in the
# top-level directory makes dummy devices for I2C, GPIO, IIO and CAN where
# there is no hardware; the kernel has no dummy for SPI, so the thermostat's SPI
# sensor needs a real /dev/spidev0.0.
define runexample_on
	$(call tcsx,$(1),tcspecial tcs-sim tcs-moc segsend)
endef

# Run tcs-moc on the minimal example: one payload, simulated by tcs-sim, its
# telemetry going to the ground.
.PHONY: run-simple
run-simple: docs
	$(call runexample_on,minimal)

.PHONY: docs
docs: README.html

# README.html, with style.html in its head: see that file for what is in it
# and why it is not in the markdown.
README.html: README.md style.html
	pandoc --standalone --metadata pagetitle=Examples -H style.html \
		README.md -o README.html

# Run tcs-moc on the thermostat example.
.PHONY: run-thermostat
run-thermostat:
	$(call runexample_on,thermostat)

# Run tcs-moc on the battery example: a power bus through a six minute orbit,
# its charge counted by a program and its science load shed in eclipse.
.PHONY: run-battery
run-battery:
	$(call runexample_on,battery)

# Run tcs-moc on the descent example: a lander flown down from five hundred
# metres by a guidance program throttling its engine.
.PHONY: run-descent
run-descent:
	$(call runexample_on,descent)

# Run tcs-moc on the tees example: a command copied back to the ground, a
# reading copied aboard, and data dropped.
.PHONY: run-tees
run-tees:
	$(call runexample_on,tees)

# Run tcs-moc on the fprime example: a vehicle that answers to an F´ flight
# computer, tcs-moc playing the bridge component's part.
.PHONY: run-fprime
run-fprime:
	$(call runexample_on,fprime)

# Run tcs-moc on the cfs example: a vehicle that answers to a cFS flight
# computer, tcs-sim simulating the cFS end of the software bus and tcs-moc
# joining that bus as the ground.
.PHONY: run-cfs
run-cfs:
	$(call runexample_on,cfs)

# Run tcs-moc on the examples whose payloads are the kernel's dummy devices:
# a virtual CAN bus, a gpio-sim chip and an iio_dummy sensor. Each
# device has to be there first, and making one needs root, so these do not:
# `sudo bin/mk-kernel-dummies can gpio iio`, in the top-level directory,
# makes all three, where the kernel allows, and gives them to the user who
# ran it. See "Payloads on the kernel's dummy devices" in README.md.
.PHONY: run-can
run-can:
	$(call runexample_on,can)

.PHONY: run-gpio
run-gpio:
	$(call runexample_on,gpio)

.PHONY: run-iio
run-iio:
	$(call runexample_on,iio)

# Run tcspecial on the simple-app example: a payload that is a program,
# sending a line of text a word at a time.
#
# Both ends are run, by tcsx. The vehicle goes in the background, and the
# ground station, lorem_gateway, in the foreground: it is what prints the
# line, and it stops at the end of it, or after a time several times what
# the line takes, so that a vehicle that never answers is a failed run and
# not a hang. Then the vehicle is stopped, with the payload it started. The
# payload, lorem_pl.py, is the `executable` the configuration names, a
# Python 3 script beside it, and tcspecial starts it -- so Python 3 is
# checked for first.

.PHONY: run-simple-app
run-simple-app: check-python3
	$(call tcsx,simple-app,tcspecial segsend lorem_gateway)

# Run the beacon example: a window watching the beacon, with a button that
# starts tcspecial and stops it.
#
# Both ends again, and no simulator: a beacon comes from tcspecial itself, so
# nothing has to simulate a payload for one to arrive. Only the
# watcher is started here. tcspecial is not running until its button is
# pressed, and the window starts it from the binary built beside it -- not
# through cargo, so that stopping it with kill() stops tcspecial and not a
# cargo with tcspecial left behind it. Closing the window stops it too, and
# so does the watcher dying: the kernel is told to.
#
# It opens a window, so it wants a display. Without one Slint says so and the
# run stops there, which is a thing to know rather than a thing to work
# around: what the window shows is what the example is.
.PHONY: run-beacon
run-beacon:
	$(call tcsx,beacon,tcspecial segsend beacon)

# Run the ping example: tcspecial answering pings, and a window sending them.
#
# Both ends, and only the ground station is started here: it starts the
# vehicle itself, tcspecial from the build, on the same configuration, and
# kills it when it exits -- however it exits, the kernel being told to kill
# tcspecial if the ground station is killed or crashes. It stays until its
# window is closed. The ground station is Python 3,
# examples/ping/ping_gateway.py, so Python and its tkinter are checked for
# first; tcs-verify is built beside the vehicle because the window asks it for
# the configuration's digest, which it compares every beacon's with.
#
# It opens a window, so it wants a display. Without one,
#   python3 examples/ping/ping_gateway.py --pings 3
# starts the vehicle, sends three pings, prints each answer and its times,
# and stops the vehicle.
.PHONY: run-ping
run-ping: check-tkinter
	$(call tcsx,ping,tcspecial tcs-verify segsend)

# The ping example with Yamcs between the ground station and the vehicle.
# YAMCS_HOME is where a Yamcs release is unpacked; ping_yamcs.py starts Yamcs
# and tcspecial, and opens the ping example's window, which pings through
# Yamcs. Closing the window stops them both. With no window,
#   python3 examples/ping-yamcs/ping_yamcs.py --yamcs <yamcs> --pings 3
# pings three times, prints each answer and its times, and stops them.
.PHONY: run-ping-yamcs
run-ping-yamcs: check-tkinter
	$(call tcsx,ping-yamcs,tcspecial tcs-verify segsend)

# The memusage example: tcspecial's memory, and each of its threads'
# stacks, watched through Yamcs. YAMCS_HOME is where a Yamcs release is
# unpacked; memusage.py starts Yamcs, with an instance for each tcspecial,
# and a tcspecial on each configuration MEMUSAGE names, memusage's own where
# it names none, and prints what each answers QUERY_THREADS with every five
# seconds until it is stopped. Yamcs's web interface, at
# http://localhost:18090, shows the same. Two tcspecials, which tcs-conflict
# says can run at once:
#   make run-memusage MEMUSAGE="examples/memusage/memusage.xtcs \
#       examples/ping-yamcs/ping-yamcs.xtcs"
MEMUSAGE = examples/memusage/memusage.xtcs

.PHONY: run-memusage
run-memusage:
	$(call tcsx,memusage,tcspecial tcs-verify segsend,$(MEMUSAGE))

# The thermostat example with Yamcs between the ground station and the
# vehicle. YAMCS_HOME is where a Yamcs release is unpacked;
# thermostat_yamcs.py starts Yamcs, tcs-sim and tcspecial, and opens a window
# of the zone's temperature, the three readings, the thermostat's decision
# and the heaters it drives, each read from Yamcs. Closing the window stops
# them all. tcs-sim draws a window of its own, so this wants a display. With
# no window of the program's own,
#   python3 examples/thermostat-yamcs/thermostat_yamcs.py --yamcs <yamcs> \
#       --seconds 120
# prints what arrives for two minutes, and a line each time the heaters are
# switched, and stops them.
.PHONY: run-thermostat-yamcs
run-thermostat-yamcs: check-tkinter
	$(call tcsx,thermostat-yamcs,tcspecial tcs-sim tcs-verify \
		segsend)

# Whether there is a tkinter for the ping example's window to be drawn with.
#
# The window is the one part of the ground station that needs more than the
# interpreter: tkinter comes with Python but is packaged apart from it on some
# systems -- python3-tk on Debian and Ubuntu -- so a Python that runs the
# no-window mode may still have nothing to draw with.
.PHONY: check-tkinter
check-tkinter: check-python3
	@python3 -c 'import tkinter' 2>/dev/null || { \
		echo "the ping example's window is drawn with tkinter, and" \
			"this python3 has none: install it" \
			"(python3-tk on Debian and Ubuntu), or run python3" \
			"examples/ping/ping_gateway.py --pings 3 for no" \
			"window" >&2; \
		exit 1; \
	}

# Whether there is a Python 3 to run simple-app's payload with.
#
# Asked for by name, as `python3`, because that is what the script's own
# first line asks the system for: a Python found any other way would not be
# the one tcspecial runs it with. 3.6 is the oldest the script has been
# written for; it uses nothing outside the standard library.
.PHONY: check-python3
check-python3:
	@command -v python3 >/dev/null 2>&1 || { \
		echo "simple-app's payload, lorem_pl.py, is a Python 3" \
			"program, and there is no python3 on the PATH:" \
			"install Python 3.6 or later" >&2; \
		exit 1; \
	}
	@python3 -c 'import sys; sys.exit(sys.version_info < (3, 6))' || { \
		echo "simple-app's payload, lorem_pl.py, needs Python 3.6 or" \
			"later, and python3 is" \
			"$$(python3 --version 2>&1)" >&2; \
		exit 1; \
	}

.PHONY: clean
clean:
	rm -f README.html
