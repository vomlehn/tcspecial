# Makefile for the examples

PROJECT_NAME := tcspecial

# The repository's root, where cargo is run and the examples' paths start.
ROOT = ..

# What tcspecial logs, as in the top-level Makefile:
#   make run-simple RUST_LOG=debug
RUST_LOG = info

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
	@echo "  make run-simple      - Run tcsmoc on the minimal example"
	@echo "  make run-thermostat  - Run tcsmoc on the thermostat example"
	@echo "  make run-simple-app  - Run the simple-app example, printing its line"
	@echo "  make run-beacon      - Run the beacon example: a window watching the beacon, with a button that starts and stops tcspecial"
	@echo "  make run-ping        - Run the ping example: a Python window that pings the vehicle"
	@echo "  make run-tees        - Run tcsmoc on the tees example: tees and null nodes"
	@echo "  make check-python3   - Check for the Python 3 simple-app's payload needs"
	@echo "  make check-tkinter   - Check for the tkinter the ping example's window needs"

# Running tcsmoc on one of the examples.
#
# $(1) is the example's directory, whose configuration is named for it. tcsmoc
# starts tcspecial on the same file, and tcssim where the simulator file
# beside it -- $(1)-sim.yaml, found by name -- simulates anything; its panels
# show what each source sends to the ground. A payload on a device -- the
# thermostat's I2C, SPI and GPIO -- needs that device. `make mkstubs` in the
# top-level directory makes stand-ins for I2C, GPIO, IIO and CAN where there
# is no hardware; there is no stand-in for SPI, so the thermostat's SPI
# sensor needs a real /dev/spidev0.0.
#
# The examples log to $(TELEMETRY), which is deleted before each run so that
# what is in it afterwards is that run's alone. It is made again because a log
# node does not make its own directory: a mistyped path is meant to be
# refused, not to become a log nobody is looking in.
define runexample_on
	( \
		set -eu; \
		echo "Running tcsmoc on examples/$(1)/$(1).yaml..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		cargo build -q --bin tcspecial --bin tcssim --bin tcsmoc; \
		RUST_LOG=$(RUST_LOG) cargo run -q --bin tcsmoc -- examples/$(1)/$(1).yaml \
	)
endef

# Run tcsmoc on the minimal example: one payload, simulated by tcssim, its
# telemetry going to the ground.
.PHONY: run-simple
run-simple: docs
	$(call runexample_on,minimal)

.PHONY: docs
docs: README.html

# README.html, with style.html in its head: see that file for what is in it
# and why it is not in the markdown.
README.html: README.md style.html
	pandoc --standalone --metadata pagetitle=Examples -H style.html README.md -o README.html

# Run tcsmoc on the thermostat example.
.PHONY: run-thermostat
run-thermostat:
	$(call runexample_on,thermostat)

# Run tcsmoc on the tees example: a command copied back to the ground, a
# reading copied aboard, and data dropped.
.PHONY: run-tees
run-tees:
	$(call runexample_on,tees)

# Run tcspecial on the simple-app example: a payload that is a program,
# sending a line of text a word at a time.
#
# Both ends are run here. The vehicle goes in the background, and the ground
# station, lorem_moc, in the foreground: it is what prints the line, and it
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
	cd $(ROOT) && cargo build -q --bin tcspecial --bin lorem_moc
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME) on examples/simple-app/simple-app.yaml..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) target/debug/tcspecial --quiet examples/simple-app/simple-app.yaml & \
		vehicle=$$!; \
		trap 'pkill -P $$vehicle || true; kill -INT $$vehicle 2>/dev/null || true; wait $$vehicle || true' EXIT; \
		trap 'exit 130' INT TERM; \
		timeout $(SIMPLE_APP_WAITS) target/debug/lorem_moc \
	)

# Run the beacon example: a window watching the beacon, with a button that
# starts tcspecial and stops it.
#
# Both ends again, and no simulator: a beacon comes from tcspecial itself, so
# nothing has to be standing in for a payload for one to arrive. Only the
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
		echo "Watching $(PROJECT_NAME)'s beacon from examples/beacon/beacon.yaml..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) target/debug/beacon examples/beacon/beacon.yaml \
	)

# Run the ping example: tcspecial answering pings, and a window sending them.
#
# Both ends, and only the ground station is started here: it starts the
# vehicle itself, tcspecial from the build, on the same configuration, and
# kills it when it exits -- however it exits, the kernel being told to kill
# tcspecial if the ground station is killed or crashes. It stays until its
# window is closed. The ground station is Python 3,
# examples/ping/ping_moc.py, so Python and its tkinter are checked for first;
# tcsverify is built beside the vehicle because the window asks it for the
# configuration's digest, which it compares every beacon's with.
#
# It opens a window, so it wants a display. Without one,
#   python3 examples/ping/ping_moc.py --pings 3
# starts the vehicle, sends three pings, prints each answer and its times,
# and stops the vehicle.
.PHONY: run-ping
run-ping: check-tkinter
	cd $(ROOT) && cargo build -q --bin tcspecial --bin tcsverify
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME) on examples/ping/ping.xml..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) python3 examples/ping/ping_moc.py examples/ping/ping.xml \
	)

# Whether there is a tkinter for the ping example's window to be drawn with.
#
# The window is the one part of the ground station that needs more than the
# interpreter: tkinter comes with Python but is packaged apart from it on some
# systems -- python3-tk on Debian and Ubuntu -- so a Python that runs the
# no-window mode may still have nothing to draw with.
.PHONY: check-tkinter
check-tkinter: check-python3
	@python3 -c 'import tkinter' 2>/dev/null || { \
		echo "the ping example's window is drawn with tkinter, and this python3 has none: install it (python3-tk on Debian and Ubuntu), or run python3 examples/ping/ping_moc.py --pings 3 for no window" >&2; \
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
		echo "simple-app's payload, lorem_pl.py, is a Python 3 program, and there is no python3 on the PATH: install Python 3.6 or later" >&2; \
		exit 1; \
	}
	@python3 -c 'import sys; sys.exit(sys.version_info < (3, 6))' || { \
		echo "simple-app's payload, lorem_pl.py, needs Python 3.6 or later, and python3 is $$(python3 --version 2>&1)" >&2; \
		exit 1; \
	}

.PHONY: clean
clean:
	rm -f README.html
