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

# Where the examples' logs are written. The examples' configurations name
# the same directory, so a change here is a change there as well.
TELEMETRY = /tmp/telemetry

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
#
# The examples log to $(TELEMETRY), which is deleted before each run so that
# what is in it afterwards is that run's alone. It is made again because a log
# node does not make its own directory: a mistyped path is meant to be
# refused, not to become a log nobody is looking in.
define runexample_on
	( \
		set -eu; \
		echo "Running tcs-moc on examples/$(1)/$(1).ytcs..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		cargo build -q --bin tcspecial --bin tcs-sim --bin tcs-moc; \
		RUST_LOG=$(RUST_LOG) cargo run -q --bin tcs-moc -- \
			--simulate=$(SIMULATE) examples/$(1)/$(1).ytcs \
	)
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
# Both ends are run here. The vehicle goes in the background, and the ground
# station, lorem_gateway, in the foreground: it is what prints the line, and it
# stops at the end of it. Then the vehicle is stopped. The payload,
# lorem_pl.py, is the `executable` the configuration names, a Python 3 script
# beside it, and tcspecial starts it -- so Python 3 is checked for first.
#
# The binaries are run directly rather than through `cargo run`, so that $$!
# is tcspecial itself and its children can be found. Those are stopped first:
# tcspecial does not yet end an app node's program when it stops, and a
# lorem_pl.py left behind would sit there until somebody killed it. The ground
# is given $(SIMPLE_APP_WAITS) seconds, which is several times what the line
# takes, so a vehicle that never answers is a failed run and not a hang. The
# vehicle is --quiet, so that what the run prints is the line; what stops it
# starting it says either way.
SIMPLE_APP_WAITS = 30

.PHONY: run-simple-app
run-simple-app: check-python3
	cd $(ROOT) && cargo build -q --bin tcspecial --bin lorem_gateway
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME) on" \
			"examples/simple-app/simple-app.ytcs..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) target/debug/tcspecial --quiet \
			--simulate=$(SIMULATE) \
			examples/simple-app/simple-app.ytcs & \
		vehicle=$$!; \
		trap 'pkill -P $$vehicle || true; \
			kill -INT $$vehicle 2>/dev/null || true; \
			wait $$vehicle || true' EXIT; \
		trap 'exit 130' INT TERM; \
		timeout $(SIMPLE_APP_WAITS) target/debug/lorem_gateway \
	)

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
	cd $(ROOT) && cargo build -q --bin tcspecial --bin beacon
	( \
		set -eu; \
		echo "Watching $(PROJECT_NAME)'s beacon from" \
			"examples/beacon/beacon.ytcs..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) target/debug/beacon \
			examples/beacon/beacon.ytcs \
	)

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
	cd $(ROOT) && cargo build -q --bin tcspecial --bin tcs-verify --bin segsend
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME) on examples/ping/ping.xtcs..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) python3 examples/ping/ping_gateway.py \
			examples/ping/ping.xtcs \
	)

# The ping example with Yamcs between the ground station and the vehicle.
# YAMCS_HOME is where a Yamcs release is unpacked; ping_yamcs.py starts Yamcs
# and tcspecial, and opens the ping example's window, which pings through
# Yamcs. Closing the window stops them both. With no window,
#   python3 examples/ping-yamcs/ping_yamcs.py --yamcs <yamcs> --pings 3
# pings three times, prints each answer and its times, and stops them.
.PHONY: run-ping-yamcs
run-ping-yamcs: check-tkinter
	@test -x "$(YAMCS_HOME)/bin/yamcsd" || { \
		echo "say where Yamcs is unpacked: make run-ping-yamcs" \
			"YAMCS_HOME=<the directory holding bin/yamcsd>" >&2; \
		exit 1; \
	}
	cd $(ROOT) && cargo build -q --bin tcspecial --bin tcs-verify --bin segsend
	cd $(ROOT) && RUST_LOG=$(RUST_LOG) python3 \
		examples/ping-yamcs/ping_yamcs.py --yamcs "$(YAMCS_HOME)"

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
	@test -x "$(YAMCS_HOME)/bin/yamcsd" || { \
		echo "say where Yamcs is unpacked: make run-thermostat-yamcs" \
			"YAMCS_HOME=<the directory holding bin/yamcsd>" >&2; \
		exit 1; \
	}
	cd $(ROOT) && cargo build -q --bin tcspecial --bin tcs-sim \
		--bin tcs-verify --bin segsend
	rm -rf $(TELEMETRY)
	mkdir $(TELEMETRY)
	cd $(ROOT) && RUST_LOG=$(RUST_LOG) python3 \
		examples/thermostat-yamcs/thermostat_yamcs.py \
		--yamcs "$(YAMCS_HOME)"

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
