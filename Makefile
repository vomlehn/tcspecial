# Makefile for TCSpecial

# Project variables
PROJECT_NAME := tcspecial
MOC_NAME := tcsmoc
SIM_NAME := tcssim
GUI_NAME := tcsgui
DOCS_DIR := docs

DESIGN=$(DOCS_DIR)/design.rst
RUST = .

# Where installing puts things.
#
# Everything under one prefix, /opt/tcs, which the filesystem hierarchy
# standard (3.0, section 3.13) gives an add-on software package of its own:
# /opt/<package>, with its host configuration in /etc/opt/<package> and its
# variable data in /var/opt/<package>. One prefix for every TCS package --
# tcspecial, tcslog, tcslog-tools and tcs-xtce alike -- so that a ground
# station or a vehicle has one bin, one lib and one include to be told of:
#
#   make install                         # the packages, with apt-get, dnf
#                                        # or zypper, into /opt/tcs
#   make install-files PREFIX=$HOME/tcs  # a copy somewhere a user may write
#   make install-files DESTDIR=/tmp/pkg  # staged, for building a package
#
# DESTDIR is prepended to everything and is not part of any path a program
# looks in, which is what lets a package be built in a staging directory.
#
# Under the prefix, by kind, as a package under /usr would have it:
#   bin/         the programs a person runs
#   sbin/        <package>-links, one per package, which an administrator
#                runs to link that package's programs into /usr/local/bin
#   libexec/     programs other programs run: the five test app programs
#   lib/         libtcs_moc_c, its pkg-config file, and the Python packages,
#                in lib/python3/site-packages, which names no minor version
#   include/     tcslib-moc-c.h
#   share/       tcsgui's data, the documents and the example configurations
#
# HOOKS=1 also installs what makes /opt/tcs usable without being told: a
# profile.d script per package that puts bin/ on PATH, an ld.so.conf.d file
# that puts lib/ on the loader's path, a .pth file that puts the Python on
# python3's, and tcspecial's systemd unit. Each lies outside the prefix -- in
# /etc and /usr/lib -- and is named after its package, so that no two
# packages own a file. Off by default, so that an install by a developer
# changes nothing outside the prefix without being asked; `make stage`, which
# the packages are built from, turns it on.
PREFIX ?= /opt/tcs
DESTDIR ?=
SYSCONFDIR ?= /etc/opt/tcs
LOCALSTATEDIR ?= /var/opt/tcs
HOOKS ?= 0
# Where what sits beside this tree is, BESIDE, and the ports' lock,
# TCS_PORTS_LOCK: see beside.mk.
include $(CURDIR)/beside.mk
# tcs-helpers, the packaging scripts and make rules the TCS projects share:
# a checkout of https://github.com/vomlehn/tcs-helpers, beside this one
# unless TCS_HELPERS names another, used where it is rather than installed.
# Installing and packaging check it first: see the tcs-helpers target.
TCS_HELPERS ?= $(BESIDE)/tcs-helpers
TCS_HELPERS_REQUIRED := 0.8.0
# tcs-xtce, whose Python package generates the link's codecs and holds the
# runtime python/tcslib_protocol/_xtce links to: a checkout of
# https://github.com/vomlehn/tcs-xtce beside this one, always, since a link
# cannot follow a variable. See the tcs-xtce target in tcslib-protocol/Makefile.
TCS_XTCE := $(BESIDE)/tcs-xtce
BINDIR = $(PREFIX)/bin
SBINDIR = $(PREFIX)/sbin
LIBEXECDIR = $(PREFIX)/libexec
LIBDIR = $(PREFIX)/lib
INCLUDEDIR = $(PREFIX)/include
DATADIR = $(PREFIX)/share
DOCDIR = $(DATADIR)/doc
EXAMPLEDIR = $(DATADIR)/examples/$(PROJECT_NAME)
PYTHONDIR = $(LIBDIR)/python3/site-packages
TCSGUI_DATA = $(DESTDIR)$(DATADIR)/$(GUI_NAME)
# The XTCS, YTCS and simulator schemas, tcslib-protocol/schema: tcspecial's and
# not tcsgui's, since they define the formats every program reads.
SCHEMADIR = $(DATADIR)/tcspecial/schema

# Where the hooks go. Debian's places; the RPM spec moves the .pth file to
# Fedora's, which names python3's minor version: see
# packaging/rpm/tcspecial-moc.spec.
PROFILEDIR ?= /etc/profile.d
LDCONFDIR ?= /etc/ld.so.conf.d
SYSTEMDDIR ?= /usr/lib/systemd/system
PTHDIR ?= /usr/lib/python3/dist-packages

# The configuration to run, named without a suffix.
#
# TCSPECIAL_CONFIG_FILE is the stem, and `$(TCSPECIAL_CONFIG_FILE).ytcs` or
# `.xtcs` the configuration. So:
#   make run     TCSPECIAL_CONFIG_FILE=examples/minimal/minimal
#   make runmocy TCSPECIAL_CONFIG_FILE=examples/thermostat/thermostat
#   make runsim  TCSPECIAL_CONFIG_FILE=examples/minimal/minimal
#
# It names a configuration to run and is nothing the programs look for. A
# configuration states where commands are taken and where beacons go in its
# own `tcspecial` section, so there is one file.
#
# Every program takes the configuration file as a command line argument. The
# simulator file reaches tcssim through the environment: runmocy passes it
# because tcsmoc hands its own configuration to the tcspecial and tcssim it
# starts but never reads the simulation file, and runsim passes it because
# tcssim run on its own needs both, and the two must describe the same set or
# the names will not match.
#
# The fixture lives under tests/manual because running it is something a
# person does at a terminal: it opens sockets and device nodes.
TCSPECIAL_CONFIG_FILE=tests/manual/thermostat

# The simulator settings for that configuration.
#
# The file beside it, `<stem>-sim.yaml`, as every example names its own.
# Named rather than worked out by the programs, because tcssim reads it from
# the environment and a name nothing has is a run with nothing simulated.
#   make runsim TCSPECIAL_CONFIG_FILE=mine TCSPECIAL_SIM_FILE=mine-bench.yaml
TCSPECIAL_SIM_FILE ?= $(TCSPECIAL_CONFIG_FILE)-sim.yaml

# What the programs log, which every run target passes on. A variable rather
# than a word in each recipe, so that it can be overridden the way the payload
# set is:
#   make runmocy RUST_LOG=debug
#   make run TCSPECIAL_CONFIG_FILE=examples/minimal/minimal RUST_LOG=debug
RUST_LOG = info

# The run's simulation mode, given to every program a run target starts:
# none, some or all. All by default, because these targets run on a desk with
# simulated and dummy devices, and a program that leaves the flag out gets
# none -- the flight setting -- and refuses every one. See "Simulation Modes"
# in docs/design.rst.
#   make runmocy SIMULATE=some
SIMULATE = all

RELEASE = --release
RELEASE =

# Default target
all: hooks-if-there build test

# Display help
.PHONY: help
help:
	@echo "Makefile for TCSpecial"
	@echo ""
	@echo "Usage:"
	@echo "  make all         - Build and test the project"
	@echo "  make build       - Build tcspecial"
	@echo "  make buildmoc    - Build tcsmoc"
	@echo "  make buildsim    - Build tcssim"
	@echo "  make buildgui    - Build tcsgui"
	@echo "  make test        - Run all tests"
	@echo "  make run         - Run tcspecial"
	@echo "  make runmocy     - Run tcsmoc, which starts tcspecial and" \
		"tcssim"
	@echo "  make runmocx     - As runmocy, on the XML configuration"
	@echo "  make runsim      - Run tcssim alone"
	@echo "  make runguiy     - Run tcsgui on the YAML configuration"
	@echo "  make runguix     - Run tcsgui on the XML configuration"
	@echo "  make runsimguiy  - Run tcsgui on the YAML configuration and" \
		"the simulator's"
	@echo "  make runsimguix  - Run tcsgui on the XML configuration and" \
		"the simulator's"
	@echo "  make runverifyy  - Check the payload set, reading its YAML"
	@echo "  make runverifyx  - Check the payload set, reading its XML"
	@echo "  make runcomparexy - Check the payload set's XML and YAML say" \
		"the same thing"
	@echo "  make comparespellings - Do that for every shipped" \
		"configuration"
	@echo "  make set-interfaces - Write this build's interface digest" \
		"into every shipped configuration"
	@echo "  make verify      - Check one version 2 configuration"
	@echo "  make mkvcan      - Make the virtual CAN bus a CAN payload" \
		"needs"
	@echo "  make mkstubs     - Make every dummy device: I2C, GPIO, IIO" \
		"and virtual CAN"
	@echo "  make rmvcan      - Remove the virtual CAN buses"
	@echo "  make rmstubs     - Remove every dummy device those made"
	@echo "  make fprime-build - Build the F´ bridge and its test" \
		"deployment, with F´'s tools"
	@echo "  make fprime-test  - Build them and test the bridge against" \
		"tcspecial and tcssim"
	@echo "                     (FPRIME_SCRATCH, or FPRIME_FRAMEWORK and" \
		"FPRIME_VENV: see fprime/Makefile)"
	@echo "  make simulated-devices-test - Run tcspecial against tcssim" \
		"with a payload of each type"
	@echo "  make cfs-build   - Build cFS with SBN for this machine, in" \
		"a scratch directory"
	@echo "  make cfs-test    - Build it and fly the cfs example against" \
		"it, tested from the cFS's ground"
	@echo "                     (CFS_PRISTINE, SBN_SRC and CFS_SCRATCH:" \
		"see cfs/Makefile)"
	@echo "  make kernel-dummies-test - Run tcspecial against the" \
		"kernel's vcan, gpio-sim and iio_dummy"
	@echo "                     (make them first with sudo" \
		"bin/mk-kernel-dummies)"
	@echo "  make check       - cargo check, clippy, a format check," \
		"the spellings compared and an HTML check"
	@echo "  make screenshots - Make the GUI manuals' screenshots" \
		"again, drawn in memory"
	@echo "  make html-check  - Fail where a committed HTML page is" \
		"behind its source, or a link leads nowhere"
	@echo "  make hooks       - Have git run tcs-helpers' pre-commit" \
		"hook, which remakes"
	@echo "                     the page of each document a commit" \
		"changes and adds it"
	@echo "  make format      - Reformat the source"
	@echo "  make release     - Build with optimizations"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make distclean   - Remove everything that can be rebuilt"
	@echo "  make install     - Build the packages and install them with" \
		"apt-get, dnf or zypper"
	@echo "                     (DRY_RUN=1: show what it would do)"
	@echo "  make uninstall   - Remove the installed packages the same way"
	@echo "  make install-build - The release build install-files copies"
	@echo "  make install-files - Copy a build already made under PREFIX" \
		"(/opt/tcs)"
	@echo "  make install-tcspecial, install-tcspecial-moc," \
		"install-tcspecial-test"
	@echo "                   - Copy one package's files from a build" \
		"already made"
	@echo "  make install-data - Install tcsgui's descriptions and the" \
		"configuration schemas"
	@echo "  make uninstall-files - Take away what install-files put in" \
		"place"
	@echo "  make links       - Link the installed programs into" \
		"LINKDIR (/usr/local/bin)"
	@echo "  make unlinks     - Take those links away"
	@echo "  make stage       - Install each package into build/stage"
	@echo "  make deb         - Build the .debs into build/packages"
	@echo "  make rpm         - Build the RPMs into build/packages"
	@echo "  make packages    - Build both"
	@echo "                     (each strips what it installs;" \
		"STRIP=0 keeps it as built)"
	@echo "  make lint-packages - Check the packages and their shell" \
		"scripts"
	@echo "                     (with lintian, rpmlint and shellcheck)"
	@echo ""
	@echo "Installing:  make install, which asks for sudo to install" \
		"the packages"
	@echo ""
	@echo "Configuration: make runmocy" \
		"TCSPECIAL_CONFIG_FILE=examples/minimal/minimal"
	@echo "Logging:     make runmocy RUST_LOG=debug"

# Build the project
.PHONY: build
build:
	( \
		set -eu; \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcspecial; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the project
.PHONY: buildmoc
buildmoc:
	( \
		set -eu; \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcsmoc; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the project
.PHONY: buildsim
buildsim:
	( \
		set -eu; \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcssim; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the configuration editor
.PHONY: buildgui
buildgui:
	( \
		set -eu; \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcsgui; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Run tests
.PHONY: test
test: tcs-xtce
	( \
		set -eu; \
		echo "Running tests..."; \
		cd $(RUST) && cargo test; \
		$(MAKE) --no-print-directory doc-headers; \
		echo "✓ Tests complete"; \
	)

# Every non-trivial function, in Rust, C, C++ and Python, has a header:
# what it does, what each parameter means and what it returns, in its
# language's own convention. The checker is tcs-xtce's, run from its
# checkout; it checks every file git tracks but target/.
.PHONY: doc-headers
doc-headers: tcs-xtce
	python3 -I $(TCS_XTCE)/tests/doc_headers.py

# Stop, saying what to do, unless the tcs-xtce checkout beside this one is
# there, at the version tcslib-protocol pins the tcs-xtce crate to, and ready to
# run: the tests reach its Python runtime through python/tcslib_protocol/_xtce,
# and installing the ground package copies it from there.
.PHONY: tcs-xtce
tcs-xtce:
	@$(MAKE) --no-print-directory -C tcslib-protocol tcs-xtce

# Stop, saying what to do, unless the tcs-helpers checkout is there and
# at least TCS_HELPERS_REQUIRED.
.PHONY: tcs-helpers
tcs-helpers:
	@if [ ! -x "$(TCS_HELPERS)/bin/tcs-helpers" ]; then \
		echo "No tcs-helpers checkout at $(TCS_HELPERS): clone" \
			"https://github.com/vomlehn/tcs-helpers beside this" \
			"repository, or set TCS_HELPERS to where it is." >&2; \
		exit 1; \
	fi
	@"$(TCS_HELPERS)/bin/tcs-helpers" --require $(TCS_HELPERS_REQUIRED)

# Run the tcspecial application
.PHONY: run
run:
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) \
			cargo run -q --bin tcspecial -- --simulate=$(SIMULATE) \
			$(TCSPECIAL_CONFIG_FILE).ytcs \
	)

# Running tcsmoc on one spelling of the tcspecial configuration.
#
# $(1) is that spelling, by its extension -- ytcs or xtcs. A configuration
# may be written in either and they describe the same payloads, so which one
# a run reads is a choice, and a run that always read the YAML could not
# exercise the other at all.
#
# Only the suffix is given here. The stem comes from TCSPECIAL_CONFIG_FILE, so
# a payload set is still chosen the way every other target chooses it, and the
# simulator file is untouched: it is tcssim's, and tcsmoc only passes it on.
define runmoc_on
	( \
		set -eu; \
		echo "Running $(MOC_NAME) on" \
			"$(basename $(TCSPECIAL_CONFIG_FILE)).$(1)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) \
			PAYLOAD_SIM_YAML=$(TCSPECIAL_SIM_FILE) \
			cargo run -q --bin tcsmoc -- --simulate=$(SIMULATE) \
			$(basename $(TCSPECIAL_CONFIG_FILE).$(1)).$(1) \
	)
endef

# Run the MOC application, on each spelling of the tcspecial configuration.
#
# One target per spelling rather than one target and a variable, so that a
# spelling is asked for the way everything else here is asked for -- by name,
# with nothing to remember about which variable selects it.
.PHONY: runmocy
runmocy:
	$(call runmoc_on,ytcs)

.PHONY: runmocx
runmocx:
	$(call runmoc_on,xtcs)

# Checking one spelling of a payload set without running anything.
#
# $(1) is that spelling -- ytcs or xtcs -- as runmoc_on takes it, and for the
# same reason: a set may be written in either, the two are transcribed by hand,
# and a check that only ever read the YAML would leave the XML unchecked.
#
# Both files of the set are given, because tcsverify checks the pair: every
# payload simulated, every simulated payload a payload, and every setting one
# that payload can have. A good set prints nothing and the target succeeds; a
# bad one prints a line per problem and the target fails, which is what makes
# this usable from another recipe.
define runverify_on
	( \
		set -eu; \
		echo "Verifying $(basename $(TCSPECIAL_CONFIG_FILE)).$(1)..."; \
		cd $(RUST) && cargo run -q --bin tcsverify -- \
			$(basename $(TCSPECIAL_CONFIG_FILE).$(1)).$(1) \
			$(TCSPECIAL_SIM_FILE) \
	)
endef

# Verify a payload set, in each spelling of the tcspecial configuration.
#
# One target per spelling rather than one target and a variable, as with the
# runmoc targets: a spelling is asked for by name, with nothing to remember
# about which variable selects it.
.PHONY: runverifyy
runverifyy:
	$(call runverify_on,ytcs)

.PHONY: runverifyx
runverifyx:
	$(call runverify_on,xtcs)

# Check that the two spellings of a payload set say the same thing.
#
# tcsverify's three-file form: the XML and the YAML of one configuration, each
# checked with the simulator file as runverifyy and runverifyx check them, and
# then compared -- every section, attribute and value, as the programs read
# them, so two that pass are two whose digests tcspecial and tcsmoc agree on.
# The two are written by hand, and an edit made to one and not the other leaves
# both valid: nothing else notices.
.PHONY: runcomparexy
runcomparexy:
	( \
		set -eu; \
		echo "Comparing $(TCSPECIAL_CONFIG_FILE).xtcs with" \
			"$(TCSPECIAL_CONFIG_FILE).ytcs..."; \
		cd $(RUST) && cargo run -q --bin tcsverify -- \
			$(TCSPECIAL_CONFIG_FILE).xtcs \
			$(TCSPECIAL_CONFIG_FILE).ytcs \
			$(TCSPECIAL_SIM_FILE) \
	)

# The stages of docs/building-a-vehicle.rst, each a directory holding
# deck.ytcs, deck.xtcs and deck-sim.yaml. Its common directory holds the
# files the later stages include, which are not stages.
VEHICLE_GUIDE_STAGES = $(wildcard docs/building-a-vehicle/stage-*)

# Every configuration shipped in both spellings, with the simulator file each
# is checked beside: the thermostat fixture and the examples. A list of stems
# and simulator files rather than a pattern, so that a configuration may be
# checked beside a simulator file not named for it. THERMOSTAT_YAMCS is a
# stem too long to write twice on one line.
#
THERMOSTAT_YAMCS = examples/thermostat-yamcs/thermostat-yamcs
SPELLINGS = \
	tests/manual/thermostat:tests/manual/thermostat-sim.yaml \
	examples/minimal/minimal:examples/minimal/minimal-sim.yaml \
	examples/thermostat/thermostat:examples/thermostat/thermostat-sim.yaml \
	$(THERMOSTAT_YAMCS):$(THERMOSTAT_YAMCS)-sim.yaml \
	examples/simple-app/simple-app:examples/simple-app/simple-app-sim.yaml \
	examples/beacon/beacon:examples/beacon/beacon-sim.yaml \
	examples/fprime/fprime:examples/fprime/fprime-sim.yaml \
	examples/cfs/cfs:examples/cfs/cfs-sim.yaml \
	example:example-sim.yaml \
	$(foreach stage,$(VEHICLE_GUIDE_STAGES), \
		$(stage)/deck:$(stage)/deck-sim.yaml)

# Every shipped configuration, both spellings: the examples, the thermostat
# fixture, the top-level example and the stages of "Building a Vehicle", but
# not the simulator files, which name no interface, nor the files in
# examples/*-common and docs/building-a-vehicle/common that others include,
# which have no tcspecial section to name one in.
SHIPPED_CONFIGURATIONS = example.ytcs example.xtcs \
	tests/manual/thermostat.ytcs tests/manual/thermostat.xtcs \
	$(filter-out $(wildcard examples/*-common/*), \
		$(wildcard examples/*/*.ytcs) \
		$(wildcard examples/*/*.xtcs)) \
	$(VEHICLE_GUIDE_CONFIGURATIONS)

# The stages' configurations, both spellings.
VEHICLE_GUIDE_CONFIGURATIONS = \
	$(foreach stage,$(VEHICLE_GUIDE_STAGES),$(stage)/deck.ytcs \
		$(stage)/deck.xtcs)

# Write this build's interface digest into every shipped configuration, in
# place and changing nothing else, after tcslib-protocol/xtce/link.xtce changes:
# tcsverify --set-interface, which says what it changed. A test in
# tcsverify/tests/examples.rs fails while one is stale.
.PHONY: set-interfaces
set-interfaces:
	cd $(RUST) && cargo run -q --bin tcsverify -- --set-interface \
		$(SHIPPED_CONFIGURATIONS)

# Every shipped configuration's own XTCE, the file beside it of its name
# with the extension .xtce: what the vehicle flying it sends the ground. One
# for each configuration, its two spellings saying the same; written from
# the YAML spelling, and from example.ytcs's. Not the stages of "Building a
# Vehicle", which are teaching material and have no own XTCE beside them.
OWN_XTCE_CONFIGURATIONS = $(filter %.ytcs, \
	$(filter-out $(VEHICLE_GUIDE_CONFIGURATIONS),$(SHIPPED_CONFIGURATIONS)))

# Write each one's own XTCE again, after a configuration or
# tcslib-protocol/xtce/types.xtce changes: tcsverify --xtce, which says what it
# wrote.
.PHONY: xtce
xtce:
	cd $(RUST) && cargo run -q --bin tcsverify -- --xtce \
		$(OWN_XTCE_CONFIGURATIONS)

# Fail where one is not what tcsverify would write, naming each, so that a
# configuration and what the ground is told of it are reviewed together.
.PHONY: xtce-check
xtce-check:
	cd $(RUST) && cargo run -q --bin tcsverify -- --xtce-check \
		$(OWN_XTCE_CONFIGURATIONS) >/dev/null

# Compare the two spellings of every one of them.
#
# Every pair is compared before the target fails, so one run says every set
# that has drifted rather than only the first; the failures are named again
# at the end, under what was printed for each.
.PHONY: comparespellings
comparespellings:
	( \
		set -u; \
		cd $(RUST) && cargo build -q --bin tcsverify || exit 1; \
		failed=""; \
		for pair in $(SPELLINGS); do \
			stem=$${pair%%:*}; sim=$${pair#*:}; \
			echo "Comparing $$stem.xtcs with $$stem.ytcs..."; \
			cargo run -q --bin tcsverify -- \
				$$stem.xtcs $$stem.ytcs $$sim \
				|| failed="$$failed $$stem"; \
		done; \
		if [ -n "$$failed" ]; then \
			echo "The two spellings differ, or do not verify," \
				"for:$$failed"; \
			exit 1; \
		fi; \
		echo "Every configuration says the same thing in both" \
			"spellings" \
	)

# Run the simulation application
.PHONY: runsim
runsim:
	( \
		set -eu; \
		echo "Running $(SIM_NAME)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) \
			PAYLOAD_SIM_YAML=$(TCSPECIAL_SIM_FILE) \
			cargo run -q --bin tcssim -- --simulate=$(SIMULATE) \
			$(TCSPECIAL_CONFIG_FILE).ytcs \
	)

# Make the virtual CAN bus a CAN payload needs.
#
# A CAN controller is a network interface rather than a device file, so a host
# with no CAN hardware has no bus at all and a CAN payload has nowhere to run.
# mkvcan creates the kernel's virtual one -- removing any left from a previous
# run first -- which is enough for both ends to be real: tcssim joins the bus
# as one member and tcspecial's handler as another.
#
# Deliberately not a prerequisite of test or of the run targets. Creating an
# interface needs root, and a target that asked for a password in the middle of
# a build would be a surprise; the CAN tests skip where no bus is up rather
# than fail, so not having run this costs a skipped test and not a broken one.
#
# Nothing it does survives a reboot, so it is run again after one. It also
# takes the bus out from under anything already on it, which is why it is a
# target of its own and not part of a run.
.PHONY: mkvcan
mkvcan:
	bin/mkvcan

# Make every dummy device a payload set may need.
#
# The four types the simulator cannot simply be the far end of: a device on a
# bus has to answer when a master addresses it, a discrete line is a wire, a
# sensor's buffer is filled by the sensor, and a CAN controller is a network
# interface rather than a file. The kernel makes a dummy device for each, and
# there is one script per type -- bin/mki2c, bin/mkgpio, bin/mkiio and
# bin/mkvcan -- because a host that needs one commonly does not need the
# others, and one script that made all four could fail on a type nobody had
# asked for.
#
# This target is the convenience of having them all: four commands, in any
# order, each saying what it made. For one type alone, run its script, or the
# mkvcan target above.
#
# Not a prerequisite of anything, for the reasons mkvcan is not, and for one
# more: apart from the CAN tests, no test is gated on these. The scans that
# find a stub are tested against a tree handed to them rather than the real
# one, so they pass with no module loaded. This is for running a payload set
# that names an I2C, GPIO, IIO or CAN payload.
#
# Each prints the numbers it made, which are assigned as devices appear and so
# are not the same from one run to the next.
.PHONY: mkstubs
mkstubs:
	bin/mki2c
	bin/mkgpio
	bin/mkiio
	bin/mkvcan

# Put the host back the way it was found.
#
# The cleanup halves of the two targets above, which call these rather than
# carrying a copy, so that undoing a dummy device is one command whether it is
# done on its own or as the first half of remaking one.
#
# One script per type again, and each is the cleanup half of its own mk script,
# which calls it rather than carrying a copy -- so undoing a dummy device is the
# same code whether it is done on its own or as the first half of remaking one.
#
# None is destructive beyond its own dummy device. rmvcan removes the virtual
# CAN interfaces and leaves the driver loaded; rmgpio and rmiio remove the chip
# and the sensor this tree made, by name, since a host's own sit in the same
# numbering, and leave another program's alone. rmi2c is the blunt one: an
# i2c-stub adapter *is* the module, so there is no way to take away one chip
# and leave another.
#
# All of them take a dummy device out from under anything using it, so none is
# for a moment when a tcssim is running. The rmstubs order is the reverse of
# making them, which costs nothing and reads as undoing.
.PHONY: rmvcan
rmvcan:
	bin/rmvcan

.PHONY: rmstubs
rmstubs:
	bin/rmvcan
	bin/rmiio
	bin/rmgpio
	bin/rmi2c

# Check one version 2 configuration.
#
# One file and not a pair: a version 2 configuration has no simulator file
# beside it -- how a node is simulated is the node's own business -- so this
# names the file itself rather than a stem the way the runverify targets do.
#
# The same TCSGUI_FILE the editor takes, so that checking what was just edited
# is the same name in both commands.
.PHONY: verify
verify:
	( \
		set -eu; \
		echo "Checking $(TCSGUI_FILE)..."; \
		cd $(RUST) && cargo run -q --bin tcsverify -- $(TCSGUI_FILE) \
	)

# The configuration to edit, and to check.
#
# A name of its own rather than TCSPECIAL_CONFIG_FILE, which the run targets
# use: that variable names a payload *set* -- a stem, with a tcspecial
# configuration and a simulator file either side of it -- where this is one
# configuration, which has no simulator file beside it.
#
# It need not exist yet: tcsgui writes it as edits are made and keeps its
# history beside it, so starting on a name that is not there is how a new
# configuration is begun.
#
# The default is the shipped example, which is what somebody opening the
# editor to see what it does wants to be looking at. It does get rewritten --
# tcsgui saves every edit as it is made, that being the point of it -- and
# that is a thing the example can survive where a scratch file could not: it
# is in the repository, so a stray edit shows up in `git status` and comes
# back with `git checkout example.ytcs`, where an editor opened on a name
# nobody keeps leaves its work somewhere nobody looks.
#
#   make runguiy
#   make runguiy TCSGUI_FILE=tests/manual/thermostat
#   make verify  TCSGUI_FILE=example.ytcs
TCSGUI_FILE = example.ytcs

# The simulator's own configuration, for the runsimgui targets.
#
# Named by its stem, like TCSGUI_FILE, and defaulting to the configuration's
# stem with `sim` on the end -- which is what the shipped sets are called, and
# is the one convention there is. The runguiy and runguix targets ignore it: a
# session given no simulator file edits the configuration alone, and the menu
# entry that would edit the simulator's groups is greyed.
#
#   make runsimguiy
#   make runsimguiy TCSGUI_FILE=vehicle TCSGUI_SIM=othersim
TCSGUI_SIM = $(basename $(TCSGUI_FILE))sim

# Running tcsgui on one spelling of a configuration.
#
# $(1) is that spelling -- ytcs or xtcs -- as runmoc_on takes it, and for the
# same reason: a configuration may be written in either, the two describe the
# same graph, and the extension is what says which to write. An editor that
# only ever opened the YAML would leave the XML to be written by hand.
#
# The spelling comes from the target and not from TCSGUI_FILE, so whatever
# extension that variable carries is replaced -- exactly as runmoc_on replaces
# the one on a payload set's stem.
define rungui_on
	( \
		set -eu; \
		mkdir -p $(dir $(TCSGUI_FILE)); \
		echo "Running $(GUI_NAME) on" \
			"$(basename $(TCSGUI_FILE)).$(1)..."; \
		cd $(RUST) && cargo run -q --bin tcsgui -- \
			$(basename $(TCSGUI_FILE)).$(1) \
	)
endef

# Running tcsgui on one spelling of a configuration and a simulator
# configuration beside it.
#
# $(1) is the configuration's spelling, as rungui_on takes it. $(2) is the
# simulator configuration, by its stem.
#
# The simulator file is YAML whichever spelling the configuration is in, and
# that is not an oversight: a simulator configuration is a YAML file, and
# tcsgui refuses a second argument whose name does not say so. A session
# editing the XML spelling of a graph is still editing the one simulator file
# beside it.
define tcssimgui_on
	( \
		set -eu; \
		mkdir -p $(dir $(TCSGUI_FILE)) $(dir $(2)); \
		echo "Running $(GUI_NAME) on" \
			"$(basename $(TCSGUI_FILE)).$(1) and" \
			"$(basename $(2)).yaml..."; \
		cd $(RUST) && cargo run -q --bin tcsgui -- \
			$(basename $(TCSGUI_FILE)).$(1) $(basename $(2)).yaml \
	)
endef

# Run the configuration editor, on each spelling.
#
# One target per spelling rather than one target and a variable, as with the
# runmoc and runverify targets: a spelling is asked for by name, with nothing
# to remember about which variable selects it.
.PHONY: runguiy
runguiy:
	$(call rungui_on,ytcs)

.PHONY: runguix
runguix:
	$(call rungui_on,xtcs)

# And with the simulator's configuration beside it, which is what adds the
# simulation settings to each payload's window and lights the menu entry for
# the simulator's groups.
.PHONY: runsimguiy
runsimguiy:
	$(call tcssimgui_on,ytcs,$(TCSGUI_SIM))

.PHONY: runsimguix
runsimguix:
	$(call tcssimgui_on,xtcs,$(TCSGUI_SIM))

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	-cargo clean
	rm -f build.out
	rm -rf $(SIMULATED_DEVICES_RUN)
	@echo "✓ Clean complete"


# Remove everything that can be rebuilt.
#
# Everything here is either ignored by git or not tracked by it: the build
# directory, the dependency lock file, and the HTML built from the .rst
# sources. Nothing git tracks is touched, so this cannot lose work.
#
# It used to delete the crates and Cargo.toml as well, from when the tree was
# generated out of design.rst and could be regenerated. The source is written
# by hand now, so deleting it was a way to lose a working tree and nothing
# else.
#
# Note it removes Cargo.lock, which the project does not track: the next build
# resolves dependency versions afresh.
.PHONY: distclean
distclean: clean
	@echo "Removing everything that can be rebuilt..."
	rm -rf target
	rm -f Cargo.lock
	$(MAKE) -C $(DOCS_DIR) clean
	@echo "✓ Clean complete"

# What is installed, by package.
#
# Three packages, because three kinds of machine take them. tcspecial is
# what flies, and nothing a vehicle does not need is in it; tcspecial-moc
# is what a ground station is built on; tcspecial-test is the test ground
# station and the payload simulator, which never fly. Each install-<package>
# target below installs one, so the lists here are the whole account of
# which file is in which package, for `make install`, the .debs and the RPMs
# alike.
#
# The five app programs are what the five app node types are tested with,
# and are run by tcspecial rather than by a person, so they go in libexec: a
# configuration names one as `executable: /opt/tcs/libexec/app_pl`. A
# mission's own app is its own program, installed wherever its node's
# `executable` says. tcsapp's stray_app and the examples' programs are tests
# and are not installed.
#
# segsend is in the test package rather than the flight one: the segsend
# this tree builds deletes each segment it is given, which is for testing,
# and a vehicle that flew it would throw its telemetry away. A mission puts
# its own segsend beside tcspecial instead, and no package of ours then owns
# the file or overwrites it on an upgrade. The test one goes in the same
# bin, beside tcspecial, which is where tcspecial looks for it.
FLIGHT_BIN := tcspecial tcsverify
FLIGHT_LIBEXEC := app_pl app_all_pl app_gateway_veh app_payload_veh app_all_veh
GROUND_BIN := tcsgui
TEST_BIN := tcsmoc tcssim segsend
PACKAGES := tcspecial tcspecial-moc tcspecial-test
# Its programs with windows are built with Slint, whose royalty-free
# licence asks for its badge where they are downloaded: make
# publish-release adds it to the notes of every release (see tcs-helpers'
# README).
RELEASE_ATTRIBUTION := slint

# The crates those come from, built together by install-build.
INSTALL_CRATES := tcspecial tcsverify tcsgui tcslib-moc-c tcsmoc tcssim \
		  $(FLIGHT_LIBEXEC)

# The crates each package's programs and libraries are built from, whose
# dependencies its THIRD-PARTY-NOTICES lists: every crate compiled into a
# program has licence notices that must go with it, which tcs-helpers'
# third-party-notices.py gathers. segsend is a program of the tcspecial
# crate's.
FLIGHT_CRATES := tcspecial tcsverify $(FLIGHT_LIBEXEC)
GROUND_CRATES := tcsgui tcslib-moc-c
TEST_CRATES := tcsmoc tcssim tcspecial

# Where cargo puts a release build.
RELEASE_DIR = $(or $(CARGO_TARGET_DIR),target)/release

# The versions, read from where each is stated so that it is stated once: the
# workspace's Cargo.toml for the packages, tcslib-moc-c's for libtcs_moc_c's
# own, which tcslib-moc-c.pc says, and tcslib-moc-c/abi-version for
# libtcs_moc_c's ABI version, which is its soname -- libtcs_moc_c.so.<ABI>
# -- and the name its file is installed under. The ABI version goes up only when
# a program built against the old tcslib-moc-c.h would not work with the new
# library: see tcslib-moc-c/build.rs, which reads the same file to set the
# soname, for exactly when.
VERSION := $(shell sed -n \
	   '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' \
	   Cargo.toml)
LIBTCS_VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' \
		    tcslib-moc-c/Cargo.toml)
LIBTCS_ABI := $(shell cat tcslib-moc-c/abi-version)
LIBTCS_SONAME := libtcs_moc_c.so.$(LIBTCS_ABI)

# The example configurations, in both spellings, with their simulator files
# and diagrams; the manual's figures are the diagrams, so it is given them as
# well. Taken from git, so that a scratch file left in examples/ is not
# shipped. A simulator file two examples share is a link in each to the one
# in their -common directory, and is installed as the same link.
EXAMPLE_FILES = examples/README.html $(shell git ls-files \
		'examples/*/*.ytcs' 'examples/*/*.xtcs' \
		'examples/*/*-sim.yaml' 'examples/*/*.svg')

# Whether what is installed is stripped: 1, the default, or 0.
#
# Stripped by default because what is installed is what is run, on a vehicle
# whose storage is small and on ground stations alike, and a release build
# carries its symbol tables and the standard library's debug information:
# stripping takes a fifth to a quarter off what each package installs, and
# libtcs_moc_c.a from 33 MB to 24 MB. Executables and shared libraries are
# stripped of everything they do not need to be linked or run
# (--strip-unneeded); the static library only of its debug information
# (--strip-debug), because a program linked with it needs its symbols. `make
# packages STRIP=0` keeps everything as cargo built it, for a build to be
# debugged.
#
# Stripped as it is installed, so that the stage, and so the .debs and the
# RPMs cut from it, hold the same files; the RPM specs keep rpmbuild from
# stripping again or splitting out debug information of its own.
STRIP ?= 1

# Strips installed files, unless STRIP=0: $(1) the executables and shared
# libraries, $(2) the static libraries. Any value but 0 strips, so that a
# STRIP left in the environment by a cross toolchain, naming its strip
# program, does not quietly turn stripping off.
define strip_installed
	if [ "$(STRIP)" != 0 ]; then \
		set -e; \
		for file in $(1); do strip --strip-unneeded $$file; done; \
		for file in $(2); do strip --strip-debug $$file; done; \
	fi
endef

# Installs $(1)-links in sbin, with the package's own programs, $(2),
# written into it: see tcs-helpers' packaging/links.sh.in. Written in
# rather than found when it runs, because bin is shared by every TCS
# package and one package's links must never touch another's.
define install_links
	install -d -m 755 $(DESTDIR)$(SBINDIR)
	$(SUBST) -e 's|@PACKAGE@|$(1)|g' -e 's|@PROGRAMS@|$(2)|g' \
		$(TCS_HELPERS)/packaging/links.sh.in \
		> $(DESTDIR)$(SBINDIR)/$(1)-links
	chmod 755 $(DESTDIR)$(SBINDIR)/$(1)-links
endef

# A template from packaging/ with the paths it is installed for put in.
SUBST = sed -e 's|@PREFIX@|$(PREFIX)|g' -e 's|@SYSCONFDIR@|$(SYSCONFDIR)|g' \
	    -e 's|@LOCALSTATEDIR@|$(LOCALSTATEDIR)|g' \
	    -e 's|@LIBTCS_VERSION@|$(LIBTCS_VERSION)|g'

# A release build of everything that is installed.
#
# One cargo run for every crate, so that the dependencies they share are
# built once. tcspecial is built with its default features: every payload
# type and not the visual window, which never flies.
.PHONY: install-build
install-build:
	cargo build --release $(addprefix -p ,$(INSTALL_CRATES))

# Copy all three packages' files into place from a build already made,
# under PREFIX and DESTDIR: what `make install` once did, kept for
# development and for `make stage`, which cuts the packages from it; `make
# install` installs the packages. The build is cargo's release build of the
# crates that are installed, and no more, made before the copy rather than
# as part of it: `make install-build` as the user and `sudo make
# install-files` installs without building anything as root.
.PHONY: install-files
install-files: $(addprefix install-,$(PACKAGES))

# The flight software: what a vehicle runs.
#
# /etc/opt/tcs/tcspecial and /var/opt/tcs/tcspecial are made empty. The
# first is where the systemd unit looks for the vehicle's configuration,
# which is the mission's and is not shipped: an example put there would be
# flown by a vehicle whose own had not been installed yet, and a vehicle
# with no configuration is one that says so, rather than one flying the
# wrong payloads. The second is for the state file and the telemetry log,
# which a configuration names.
#
# The manual finds its figures at ../examples, as it does in the source
# tree, so share/doc/tcspecial/examples links to the installed examples,
# which have the diagrams beside the configurations. An example that starts
# with "#!" is a script to run, so it would be installed executable and the
# rest are not; none of today's does.
.PHONY: install-tcspecial
install-tcspecial: tcs-helpers
	@echo "Installing $(PROJECT_NAME) under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) $(DESTDIR)$(LIBEXECDIR)
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(FLIGHT_BIN)) \
		$(DESTDIR)$(BINDIR)
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(FLIGHT_LIBEXEC)) \
		$(DESTDIR)$(LIBEXECDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(FLIGHT_BIN)) \
		$(addprefix $(DESTDIR)$(LIBEXECDIR)/,$(FLIGHT_LIBEXEC)),)
	$(call install_links,tcspecial,$(FLIGHT_BIN))
	install -d -m 755 $(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/docs
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/THIRD-PARTY-NOTICES \
		tcspecial $(FLIGHT_CRATES)
	install -m 644 $(DOCS_DIR)/*.html $(DOCS_DIR)/*.svg \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/docs
	install -d -m 755 $(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/docs/shots
	install -m 644 $(DOCS_DIR)/shots/*.png \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/docs/shots
	for file in $(EXAMPLE_FILES); do \
		to=$(DESTDIR)$(EXAMPLEDIR)/$${file#examples/}; \
		if [ -L $$file ]; then \
			install -d -m 755 $$(dirname $$to) && \
				ln -sfn $$(readlink $$file) $$to || exit 1; \
			continue; \
		fi; \
		mode=644; \
		[ "$$(head -c 2 $$file)" != '#!' ] || mode=755; \
		install -D -m $$mode $$file $$to || exit 1; \
	done
	ln -sfn ../../examples/$(PROJECT_NAME) \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/examples
	install -d -m 755 $(DESTDIR)$(SYSCONFDIR)/$(PROJECT_NAME) \
		$(DESTDIR)$(LOCALSTATEDIR)/$(PROJECT_NAME)
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR) \
			$(DESTDIR)$(SYSTEMDDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcspecial|g' \
			packaging/hooks/profile.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcspecial.sh; \
		$(SUBST) packaging/hooks/tcspecial.service.in \
			> $(DESTDIR)$(SYSTEMDDIR)/tcspecial.service; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcspecial.sh \
			$(DESTDIR)$(SYSTEMDDIR)/tcspecial.service; \
	fi

# What a ground station is built on: libtcs_moc_c for C and C++, the Python
# packages, and the configuration editor with its data.
#
# libtcs_moc_c.so is installed under its soname, libtcs_moc_c.so.<ABI>,
# which is what a program built against it asks for at run time, and linked to
# by libtcs_moc_c.so, which is what the linker looks for. tcslib-moc-c.h goes
# in include/ itself, as <tcslib-moc-c.h>. The library is not executable, as
# Debian wants a shared library to be; the RPM spec makes it so, as rpm wants.
#
# The Python is installed by pip into a directory of its own, without
# compiling it, so that the path names no Python version and any python3 can
# read it; the .pth hook is what puts that directory on python3's path.
# pip is given a copy of python/ made for the purpose, with the links in it
# followed: setuptools builds in the directory it is given and keeps what
# it built there, so a file renamed or removed in the tree would otherwise
# go on being installed from python/build, and a build left in the tree by
# `sudo make install-files` would be root's. The direct_url.json pip
# writes names that copy, which is gone by then, so it is removed, with its
# line in RECORD.
.PHONY: install-tcspecial-moc
install-tcspecial-moc: install-data tcs-helpers tcs-xtce
	@echo "Installing tcspecial-moc under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) $(DESTDIR)$(INCLUDEDIR) \
		$(DESTDIR)$(LIBDIR)/pkgconfig $(DESTDIR)$(PYTHONDIR)
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(GROUND_BIN)) \
		$(DESTDIR)$(BINDIR)
	install -m 644 tcslib-moc-c/include/tcslib-moc-c.h \
		$(DESTDIR)$(INCLUDEDIR)
	install -m 644 $(RELEASE_DIR)/libtcs_moc_c.so \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME)
	ln -sf $(LIBTCS_SONAME) $(DESTDIR)$(LIBDIR)/libtcs_moc_c.so
	install -m 644 $(RELEASE_DIR)/libtcs_moc_c.a $(DESTDIR)$(LIBDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(GROUND_BIN)) \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME), \
		$(DESTDIR)$(LIBDIR)/libtcs_moc_c.a)
	$(call install_links,tcspecial-moc,$(GROUND_BIN))
	$(SUBST) packaging/tcslib-moc-c.pc.in \
		> $(DESTDIR)$(LIBDIR)/pkgconfig/tcslib-moc-c.pc
	chmod 644 $(DESTDIR)$(LIBDIR)/pkgconfig/tcslib-moc-c.pc
# The schemas the Python reads are tcslib-protocol/schema, which install-data
# puts under $(SCHEMADIR): the installed package reaches them through a link, as
# python/tcslib_protocol/schema does in the source tree, rather than carrying a
# second copy that could differ from the first.
	set -e; \
	tmp=$$(mktemp -d -t tcspecial-py.XXXXXXXXXX); \
	trap 'rm -rf "$$tmp"' EXIT; \
	cp -RL python "$$tmp/"; \
	rm -rf "$$tmp/python/build" "$$tmp"/python/*.egg-info; \
	find "$$tmp" -name __pycache__ -type d -prune -exec rm -rf {} +; \
	python3 -m pip install --quiet --upgrade --no-deps \
		--no-build-isolation --no-compile \
		--target $(DESTDIR)$(PYTHONDIR) "$$tmp/python"; \
	dist=$$(echo $(DESTDIR)$(PYTHONDIR)/tcspecial_moc-*.dist-info); \
	rm -f "$$dist/direct_url.json"; \
	sed -i '/^tcspecial_moc-[^/]*\.dist-info\/direct_url\.json,/d' \
		"$$dist/RECORD"; \
	schema=$(DESTDIR)$(PYTHONDIR)/tcslib_protocol/schema; \
	rm -rf "$$schema"; \
	ln -s $$(realpath -m --relative-to=$(PYTHONDIR)/tcslib_protocol \
		$(SCHEMADIR)) "$$schema"; \
	sed -i '/^tcslib_protocol\/schema\//d' "$$dist/RECORD"
	install -d -m 755 $(DESTDIR)$(DOCDIR)/tcspecial-moc/tcslib-moc-c \
		$(DESTDIR)$(DOCDIR)/tcspecial-moc/python
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcspecial-moc
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/tcspecial-moc/THIRD-PARTY-NOTICES \
		tcspecial-moc $(GROUND_CRATES)
	install -m 644 tcslib-moc-c/README.html \
		$(DESTDIR)$(DOCDIR)/tcspecial-moc/tcslib-moc-c
	install -m 644 python/README.md \
		$(DESTDIR)$(DOCDIR)/tcspecial-moc/python
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR) \
			$(DESTDIR)$(LDCONFDIR) $(DESTDIR)$(PTHDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcspecial-moc|g' \
			packaging/hooks/profile-moc.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcspecial-moc.sh; \
		echo $(LIBDIR) \
			> $(DESTDIR)$(LDCONFDIR)/tcspecial-moc.conf; \
		echo $(PYTHONDIR) \
			> $(DESTDIR)$(PTHDIR)/tcspecial-moc.pth; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcspecial-moc.sh \
			$(DESTDIR)$(LDCONFDIR)/tcspecial-moc.conf \
			$(DESTDIR)$(PTHDIR)/tcspecial-moc.pth; \
	fi
	@if [ "$(HOOKS)" = 1 ] && [ -z "$(DESTDIR)" ]; then \
		echo "Run ldconfig as root so the loader finds" \
			"$(LIBTCS_SONAME)."; \
	fi

# The test ground station and the payload simulator, and the test segsend.
#
# tcsmoc starts the tcspecial and the tcssim beside it, so these go in the
# same bin as tcspecial does: see child_command in tcsmoc/src/main.rs.
.PHONY: install-tcspecial-test
install-tcspecial-test: tcs-helpers
	@echo "Installing tcspecial-test under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(DOCDIR)/tcspecial-test
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(TEST_BIN)) \
		$(DESTDIR)$(BINDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(TEST_BIN)),)
	$(call install_links,tcspecial-test,$(TEST_BIN))
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcspecial-test
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/tcspecial-test/THIRD-PARTY-NOTICES \
		tcspecial-test $(TEST_CRATES)
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcspecial-test|g' \
			packaging/hooks/profile.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcspecial-test.sh; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcspecial-test.sh; \
	fi

# Install tcsgui's data -- the description tree and its schemas -- and the XTCS,
# YTCS and simulator schemas of tcslib-protocol/schema, under $(SCHEMADIR).
#
# The whole of tcsgui/share, with its structure: the description tree's shape
# is part of what it means -- a description at vehicle/compute/vote.yaml is
# about the Rust at node/vehicle/compute/vote.rs -- so a flat copy would lose
# half of it.
#
# Into share/tcsgui under the prefix, which is where an installed tcsgui
# looks: one level up from its own bin. See `tcsgui::nodes::data_dir` for
# where the editor looks and in what order.
#
# Mode 644 and directories 755: these are read by everybody and written by
# whoever installed them. A person's own descriptions belong in
# $XDG_DATA_HOME/tcsgui, which the editor looks in first, rather than in a
# directory that an upgrade overwrites.
.PHONY: install-data
install-data:
	@echo "Installing $(GUI_NAME) data into $(TCSGUI_DATA)..."
	@cd $(GUI_NAME)/share && find . -type d \
		-exec install -d -m 755 $(TCSGUI_DATA)/{} \;
	@cd $(GUI_NAME)/share && find . -type f \
		-exec install -m 644 {} $(TCSGUI_DATA)/{} \;
	@install -d -m 755 $(DESTDIR)$(SCHEMADIR)
	@install -m 644 tcslib-protocol/schema/README \
		tcslib-protocol/schema/*.xsd tcslib-protocol/schema/*.json \
		$(DESTDIR)$(SCHEMADIR)
	@echo "✓ $(GUI_NAME) data installed"

# Take away what `make install-files` put in place, with the same PREFIX,
# DESTDIR and HOOKS it was given; `make uninstall` removes the packages.
# The links `make links` made in LINKDIR go first, while the scripts that
# know which they are are still there, so that none is left pointing at a
# program that is gone.
#
# Not the configuration in /etc/opt/tcs/tcspecial or the state and telemetry
# in /var/opt/tcs/tcspecial, which are the mission's rather than this
# program's: their directories go only if they are empty. Directories under
# the prefix go when nothing else is in them, which leaves those another TCS
# package installed into.
.PHONY: uninstall-files
uninstall-files:
	-for package in $(PACKAGES); do \
		script=$(DESTDIR)$(SBINDIR)/$$package-links; \
		[ -x $$script ] && $$script --remove --dir $(LINKDIR); \
	done; true
	rm -f $(addprefix $(DESTDIR)$(BINDIR)/,$(FLIGHT_BIN) $(GROUND_BIN) \
		$(TEST_BIN))
	rm -f $(addprefix $(DESTDIR)$(LIBEXECDIR)/,$(FLIGHT_LIBEXEC))
	rm -f $(addprefix $(DESTDIR)$(SBINDIR)/,$(addsuffix -links,$(PACKAGES)))
	rm -f $(DESTDIR)$(INCLUDEDIR)/tcslib-moc-c.h
	rm -f $(DESTDIR)$(LIBDIR)/libtcs_moc_c.so \
		$(DESTDIR)$(LIBDIR)/libtcs_moc_c.a \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME)
	rm -f $(DESTDIR)$(LIBDIR)/pkgconfig/tcslib-moc-c.pc
	rm -rf $(addprefix $(DESTDIR)$(PYTHONDIR)/,tcslib_protocol tcslib_moc \
		tcsui tcspecial_moc-*.dist-info)
	rm -rf $(TCSGUI_DATA) $(DESTDIR)$(DATADIR)/tcspecial \
		$(DESTDIR)$(EXAMPLEDIR) \
		$(addprefix $(DESTDIR)$(DOCDIR)/,$(PACKAGES))
	if [ "$(HOOKS)" = 1 ]; then \
		rm -f $(addprefix $(DESTDIR)$(PROFILEDIR)/,$(addsuffix .sh, \
			$(PACKAGES))) \
			$(DESTDIR)$(LDCONFDIR)/tcspecial-moc.conf \
			$(DESTDIR)$(PTHDIR)/tcspecial-moc.pth \
			$(DESTDIR)$(SYSTEMDDIR)/tcspecial.service; \
	fi
	-for dir in $(DESTDIR)$(INCLUDEDIR) \
		$(DESTDIR)$(LIBDIR)/pkgconfig $(DESTDIR)$(PYTHONDIR) \
		$(DESTDIR)$(LIBDIR)/python3 $(DESTDIR)$(LIBDIR) \
		$(DESTDIR)$(LIBEXECDIR) $(DESTDIR)$(SBINDIR) \
		$(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(DATADIR)/examples $(DESTDIR)$(DOCDIR) \
		$(DESTDIR)$(DATADIR) $(DESTDIR)$(PREFIX) \
		$(DESTDIR)$(SYSCONFDIR)/$(PROJECT_NAME) \
		$(DESTDIR)$(SYSCONFDIR) \
		$(DESTDIR)$(LOCALSTATEDIR)/$(PROJECT_NAME) \
		$(DESTDIR)$(LOCALSTATEDIR); do \
		[ -d $$dir ] && rmdir --ignore-fail-on-non-empty $$dir; \
	done; true

# Where `make links` links the installed programs, and from where
# uninstall-files removes them: see tcs-helpers' packaging.mk.
LINKDIR ?= /usr/local/bin

# The packaging targets -- install, uninstall, stage, deb, rpm, packages,
# lint-packages, links, unlinks and publish-release -- are tcs-helpers'
# packaging.mk's, shared by the TCS projects; see it for what each does.
# Each of PACKAGES is staged by its own install-<package>, from one
# release build, with the configuration and state directories under
# /etc/opt/tcs and /var/opt/tcs, and built from packaging/deb/<package>
# and packaging/rpm/<package>.spec, which read the parts they share from
# packaging/rpm. Without a tcs-helpers checkout new enough to have
# packaging.mk, the targets stop as the tcs-helpers target says.
STAGE_TARGET = install-$$package
STAGE_VARS   = SYSCONFDIR=/etc/opt/tcs LOCALSTATEDIR=/var/opt/tcs
RPM_DEFINES  = --define "_sourcedir $(CURDIR)/packaging/rpm"
# The scripts in bin/ that make and remove the dummy devices are not
# packaged, but lint-packages checks them with shellcheck as well, so that
# they are held to what the packaged scripts are.
LINT_SCRIPTS = $(wildcard bin/*)
ifneq ($(wildcard $(TCS_HELPERS)/packaging/packaging.mk),)
include $(TCS_HELPERS)/packaging/packaging.mk
stage: install-build
else
.PHONY: install
install: tcs-helpers
.PHONY: uninstall
uninstall: tcs-helpers
.PHONY: stage
stage: tcs-helpers
.PHONY: deb
deb: tcs-helpers
.PHONY: rpm
rpm: tcs-helpers
.PHONY: packages
packages: tcs-helpers
.PHONY: lint-packages
lint-packages: tcs-helpers
.PHONY: links
links: tcs-helpers
.PHONY: unlinks
unlinks: tcs-helpers
.PHONY: publish-release
publish-release: tcs-helpers
endif

# Check code quality
#
# With tcspecial built with no optional payload type and with each one alone
# as well as with all of them: a gate that leaves a reference to a type's
# module outside it breaks only the builds without that type, which a
# workspace built with every type never makes. The types are what a build
# with the defaults says it contains, which is every type, so that the list
# is not written down a second time here; see "Which Node Types Are Built" in
# docs/design.rst. And with the visual window, which no default build has
# and so nothing else would ever build.
#
# And every configuration compared in its two spellings, which must say the
# same thing: a change made to one and not the other leaves both files valid,
# and an edit by hand is made to one file at a time.
.PHONY: check
check:
	@echo "Running cargo check..."
	cd $(RUST) && cargo check
	cd $(RUST) && cargo clippy -- -D warnings
	cd $(RUST) && cargo clippy -p $(PROJECT_NAME) --no-default-features \
		-- -D warnings
	cd $(RUST) && for type in $$(cargo run -q -p $(PROJECT_NAME) \
			--bin $(PROJECT_NAME) -- --types | tr , ' '); do \
		echo "tcspecial with $$type alone"; \
		cargo clippy -q -p $(PROJECT_NAME) --no-default-features \
			--features $$type \
			-- -D warnings || exit 1; \
	done
	cd $(RUST) && cargo clippy -p $(PROJECT_NAME) --features visual \
		--all-targets -- -D warnings
	cd $(RUST) && cargo test -q -p $(PROJECT_NAME) --features visual \
		--lib visual
	cd $(RUST) && cargo test -q -p $(PROJECT_NAME) --features visual \
		--test visual
	cd $(RUST) && cargo fmt -- --check
	$(MAKE) --no-print-directory comparespellings
	$(MAKE) --no-print-directory xtce-check
	$(MAKE) --no-print-directory html-check

# The F´ bridge, which F´'s own tools build rather than cargo: see
# fprime/Makefile, which says where it looks for F´ and how to say
# otherwise. Whatever is given here -- FPRIME_SCRATCH, FPRIME_FRAMEWORK,
# FPRIME_VENV, FPRIME_CONFIG -- is passed on.
.PHONY: fprime-build
fprime-build:
	$(MAKE) -C fprime build

# Build the bridge and run its test: tcssim simulating the fprime
# example's payloads, tcspecial flying it, and F´'s ground system commanding
# the bridge in an F´ deployment and reading its channels, holding the ports'
# lock while it runs. tcssim's window opens for the length of the test.
.PHONY: fprime-test
fprime-test:
	$(MAKE) -C fprime test

# Where simulated-devices-test keeps what a run leaves, emptied at the start of
# each.
SIMULATED_DEVICES_RUN = $(CURDIR)/tests/simulated-devices/run

# Run tcspecial against tcssim with a payload of each type tcssim simulates,
# and check that each one's data reaches the ground: see
# tests/simulated-devices/simulated-devices-test. CAN and GPIO are tested only
# where a virtual CAN bus or a gpio-sim chip is already there. tcssim's window
# opens for the length of the test.
.PHONY: simulated-devices-test
simulated-devices-test:
	cd $(RUST) && cargo build --bin tcspecial --bin tcssim --bin tcsverify
	flock $(TCS_PORTS_LOCK) \
		$(CURDIR)/tests/simulated-devices/simulated-devices-test \
		$(CURDIR) $(SIMULATED_DEVICES_RUN)

# cFS itself, with SBN v7.0.1 and its UDP module, built with cFS's own tools
# from a cFS checkout and an SBN checkout into a scratch directory: see
# cfs/Makefile, which says where it looks for each and how to say
# otherwise. Whatever is given here -- CFS_PRISTINE, SBN_SRC, CFS_SCRATCH,
# CFS_CONFIG -- is passed on.
.PHONY: cfs-build
cfs-build:
	$(MAKE) -C cfs build

# Build cFS and fly the cfs example against it: the test that the wire
# format tcspecial speaks is SBN's and not only ours. tcspecial is peered
# with cFS's SBN, and the test commands it through CI_LAB and reads it
# through TO_LAB, holding the ports' lock while it runs.
.PHONY: cfs-test
cfs-test:
	$(MAKE) -C cfs test

# Fly tcspecial against the kernel's dummy devices for a CAN bus, a GPIO chip
# and an IIO sensor, playing each device's far end through the kernel: see
# tcspecial/tests/kernel-dummies.rs. The same tests cargo test runs, run
# alone and saying which flew and which were skipped, and why: a test whose
# device is not there skips, naming the command that makes it. Making the
# devices needs root, so this does not; run `sudo bin/mk-kernel-dummies`
# first, once after each boot. The tests bind no fixed port, so they take no
# lock.
.PHONY: kernel-dummies-test
kernel-dummies-test:
	cd $(RUST) && cargo test -p tcspecial --test kernel-dummies \
		-- --nocapture

# Format code
.PHONY: format
format:
	cd $(RUST) && cargo fmt

# Create release build
.PHONY: release
release: test
	@echo "Creating release build..."
	cd $(RUST) && cargo build --release
	@echo "✓ Release binary: target/release/$(PROJECT_NAME)"

# Generate HTML files
.PHONY: docs
docs:
	make -C $(DOCS_DIR)

# Make the tcssim manual's screenshots, docs/shots/tcssim-*.png, again: its
# window, built by its own code on the thermostat example and drawn in memory
# by Slint's software renderer, so that no window opens and nothing is bound
# or sent. An ignored test, run alone because Slint takes one platform per
# process; never part of make test, make check or make html-check.
.PHONY: screenshots-tcssim
screenshots-tcssim:
	cd $(RUST) && cargo test -p tcssim --bin tcssim -- --ignored --exact \
		shots::screenshots

# Make the tcsgui manual's screenshots, docs/shots/tcsgui-*.png, again: its
# windows, opened by its own code on copies of the battery example, driven
# through the presses and callbacks a person's gestures reach, and drawn in
# memory by Slint's software renderer, so that no window opens and nothing
# under examples is written. An ignored test, run alone because Slint takes
# one platform per process; never part of make test, make check or make
# html-check, the fonts differing between machines.
.PHONY: screenshots-tcsgui
screenshots-tcsgui:
	cd $(RUST) && cargo test -p tcsgui --bin tcsgui -- --ignored --exact \
		shots::screenshots

# Make the tcsmoc manual's screenshots, docs/shots/tcsmoc-*.png, again: its
# windows, on the thermostat and ping examples, wired as it wires them, fed
# real beacons, command answers and data by a simulated vehicle on loopback
# ports of the test's own choosing, and drawn in memory by Slint's software
# renderer, so that no window opens and no other program's ports are
# touched. An ignored test, run alone because Slint takes one platform per
# thread and one event-loop queue per process; about two minutes, most of
# it the beacon indicator ageing. Never part of make test, make check or
# make html-check.
.PHONY: screenshots-tcsmoc
screenshots-tcsmoc:
	cd $(RUST) && cargo test -p tcsmoc --bin tcsmoc -- --ignored --exact \
		vehicle::screenshots::screenshots

# Make every manual's screenshots again, one program after another, each in
# a process of its own; the pages that show them are made again by the
# pre-commit hook when they are committed.
.PHONY: screenshots
screenshots: screenshots-tcsgui screenshots-tcsmoc screenshots-tcssim

# Every directory whose HTML pages are made from its markdown or
# reStructuredText and committed beside it, which make in each builds.
HTML_DIRS = $(DOCS_DIR) examples tcslib-moc tcslib-moc-c tcslib-protocol

# Fail where a committed page has fallen behind its source, naming it, having
# made every page again in place: tcs-helpers' html-pages check, which also
# compares the figures a page is drawn from.
.PHONY: html-check
html-check: tcs-helpers
	"$(TCS_HELPERS)/bin/html-pages" check $(HTML_DIRS)
	$(MAKE) --no-print-directory link-check

# Fail where a link in the docs pages leads to no file, or to no anchor in
# one: manual.html links the other TCS projects' documents, in their
# checkouts beside this one, which are checked when they are there.
.PHONY: link-check
link-check:
	python3 $(DOCS_DIR)/check_links.py $(DOCS_DIR)/*.html

# Have git run tcs-helpers' pre-commit hook in this checkout, which remakes
# the page of each document a commit changes and adds it to the commit.
.PHONY: hooks
hooks: tcs-helpers
	git config core.hooksPath "$(TCS_HELPERS)/githooks"

# What make does of make hooks: the same where the tcs-helpers checkout is
# there and new enough, and only a note where it is not, so that building
# never needs it.
.PHONY: hooks-if-there
hooks-if-there:
	@if "$(TCS_HELPERS)/bin/tcs-helpers" --require \
	        $(TCS_HELPERS_REQUIRED) > /dev/null 2>&1; then \
	    git config core.hooksPath "$(TCS_HELPERS)/githooks"; \
	else \
	    echo "note: no tcs-helpers $(TCS_HELPERS_REQUIRED) at" \
	        "$(TCS_HELPERS), so no pre-commit hook: see make hooks"; \
	fi
