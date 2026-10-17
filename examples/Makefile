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
	@echo "  make run-simple      - Run tcspecial on the minimal example"
	@echo "  make run-thermostat  - Run tcspecial on the thermostat example"
	@echo "  make run-simple-app  - Run the simple-app example, printing its line"
	@echo "  make check-python3   - Check for the Python 3 simple-app's payload needs"

# Running tcspecial on one of the examples.
#
# $(1) is the example's directory, whose configuration is named for it.
# tcspecial alone: an example is a version 2 configuration, and tcsmoc and
# tcssim run version 1 payload sets only, so there is no simulator to start
# beside it yet. A payload on a device -- the thermostat's I2C, SPI and GPIO --
# needs that device, which `make mkstubs` in the top-level directory provides
# where there is no hardware.
#
# The examples log to $(TELEMETRY), which is deleted before each run so that
# what is in it afterwards is that run's alone. It is made again because a log
# node does not make its own directory: a mistyped path is meant to be
# refused, not to become a log nobody is looking in.
define runexample_on
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME) on examples/$(1)/$(1).yaml..."; \
		cd $(ROOT); \
		rm -rf $(TELEMETRY); \
		mkdir $(TELEMETRY); \
		RUST_LOG=$(RUST_LOG) cargo run -q --bin tcspecial -- examples/$(1)/$(1).yaml \
	)
endef

# Run tcspecial on the minimal example: one payload, its telemetry going to
# the ground.
.PHONY: run-simple
run-simple: docs
	$(call runexample_on,minimal)

.PHONY: docs
docs: README.html

# README.html, with style.html in its head: see that file for what is in it
# and why it is not in the markdown.
README.html: README.md style.html
	pandoc --standalone --metadata pagetitle=Examples -H style.html README.md -o README.html

# Run tcspecial on the thermostat example.
.PHONY: run-thermostat
run-thermostat:
	$(call runexample_on,thermostat)

# Run tcspecial on the simple-app example: a payload that is a program,
# sending a line of text a word at a time.
#
# Both ends are run here. The vehicle goes in the background, and the ground
# station, lorem_moc, in the foreground: it is what prints the line, and it
# stops at the end of it. Then the vehicle is stopped. The payload,
# lorem_pl.py, is the `program` the configuration names, a Python 3 script
# beside it, and tcspecial starts it -- so Python 3 is checked for first.
#
# The binaries are run directly rather than through `cargo run`, so that $$!
# is tcspecial itself and its children can be found. Those are stopped first:
# tcspecial does not yet end an app node's program when it stops, and a
# lorem_pl.py left behind would sit there until somebody killed it. The ground
# is given $(SIMPLE_APP_WAITS) seconds, which is several times what the line
# takes, so a vehicle that never answers is a failed run and not a hang.
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
		RUST_LOG=$(RUST_LOG) target/debug/tcspecial examples/simple-app/simple-app.yaml & \
		vehicle=$$!; \
		trap 'pkill -P $$vehicle || true; kill -INT $$vehicle 2>/dev/null || true; wait $$vehicle || true' EXIT; \
		trap 'exit 130' INT TERM; \
		timeout $(SIMPLE_APP_WAITS) target/debug/lorem_moc \
	)

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
