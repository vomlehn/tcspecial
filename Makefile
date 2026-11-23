# Makefile for TCSpecial

# Project variables
PROJECT_NAME := tcspecial
MOC_NAME := tcs-moc
SIM_NAME := tcs-sim
GUI_NAME := tcs-cfge
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
#   lib/         libtcsmoc-c, its pkg-config file, and the Python packages,
#                in lib/python3/site-packages, which names no minor version
#   include/     tcsmoc-c.h
#   share/       tcs-cfge's data, the documents and the example configurations
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
TCS_HELPERS_REQUIRED := 0.10.0
# tcs-xtce, whose Python package generates the link's codecs and holds the
# runtime python/tcsprotocol/_xtce links to: a checkout of
# https://github.com/vomlehn/tcs-xtce beside this one, always, since a link
# cannot follow a variable. See the tcs-xtce target in tcsprotocol/Makefile.
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
TCS_CFGE_DATA = $(DESTDIR)$(DATADIR)/$(GUI_NAME)
# The XTCS, YTCS and simulator schemas, tcsprotocol/schema: tcspecial's and
# not tcs-cfge's, since they define the formats every program reads.
SCHEMADIR = $(DATADIR)/tcspecial/schema

# Where the hooks go. Debian's places; the RPM writes its .pth file when
# it is installed, in Fedora's place, which names python3's minor version:
# see packaging/rpm/tcsmoc.spec.
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
# simulator file reaches tcs-sim through the environment: runmocy passes it
# because tcs-moc hands its own configuration to the tcspecial and tcs-sim it
# starts but never reads the simulation file, and runsim passes it because
# tcs-sim run on its own needs both, and the two must describe the same set or
# the names will not match.
#
# The fixture lives under tests/manual because running it is something a
# person does at a terminal: it opens sockets and device nodes.
TCSPECIAL_CONFIG_FILE=tests/manual/thermostat

# The simulator settings for that configuration.
#
# The file beside it, `<stem>-sim.yaml`, as every example names its own.
# Named rather than worked out by the programs, because tcs-sim reads it from
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

# Whether the tcspecial a run target starts locks its memory into RAM, which
# it does unless given --no-memlock. NO_MEMLOCK=1 gives it, for a desk whose
# limit on locked memory is too small for the lock: see "Locking memory" in
# docs/tcspecial.rst.
#   make runmocy NO_MEMLOCK=1
NO_MEMLOCK =
MEMLOCK_FLAG = $(if $(NO_MEMLOCK),--no-memlock)

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
	@echo "  make buildmoc    - Build tcs-moc"
	@echo "  make buildsim    - Build tcs-sim"
	@echo "  make buildgui    - Build tcs-cfge"
	@echo "  make test        - Run all tests"
	@echo "  make run         - Run tcspecial"
	@echo "  make runmocy     - Run tcs-moc, which starts tcspecial and" \
		"tcs-sim"
	@echo "  make runmocx     - As runmocy, on the XML configuration"
	@echo "  make runsim      - Run tcs-sim alone"
	@echo "  make runguiy     - Run tcs-cfge on the YAML configuration"
	@echo "  make runguix     - Run tcs-cfge on the XML configuration"
	@echo "  make runsimguiy  - Run tcs-cfge on the YAML configuration" \
		"and the simulator's"
	@echo "  make runsimguix  - Run tcs-cfge on the XML configuration and" \
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
		"tcspecial and tcs-sim"
	@echo "                     (FPRIME_SCRATCH, or FPRIME_FRAMEWORK and" \
		"FPRIME_VENV: see fprime/Makefile)"
	@echo "  make simulated-devices-test - Run tcspecial against tcs-sim" \
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
		"the spellings"
	@echo "                     compared, the names checked and an" \
		"HTML check"
	@echo "  make operational-names - Fail where operational code is" \
		"built from a"
	@echo "                     tool's crate, or a directory is in no" \
		"category"
	@echo "  make screenshots - Make the GUI manuals' screenshots" \
		"again, drawn in memory"
	@echo "  make example-pdfs - Print every example's configuration to" \
		"examples/NAME/NAME.pdf"
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
	@echo "  make ntss        - Count each tcs project's non-test source" \
		"statements (CSV=1: as CSV)"
	@echo "  make install-tools - Install the tools building, testing and" \
		"packaging need"
	@echo "                     (DRY_RUN=1: show what it would do;"
	@echo "                     PACKAGING=0: no packaging tools)"
	@echo "  make install     - Build the packages and install them with" \
		"apt-get, dnf or zypper"
	@echo "                     (DRY_RUN=1: show what it would do)"
	@echo "  make uninstall   - Remove the installed packages the same way"
	@echo "  make install-build - The release build install-files copies"
	@echo "  make install-files - Copy a build already made under PREFIX" \
		"(/opt/tcs)"
	@echo "  make install-tcspecial, install-tcsmoc, install-tcs-tools," \
		"install-tcspecial-test, install-tcspecial-doc," \
		"install-tcspecial-examples"
	@echo "                   - Copy one package's files from a build" \
		"already made"
	@echo "  make install-data - Install tcs-cfge's descriptions"
	@echo "  make install-schemas - Install the configuration schemas"
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
		cd $(RUST) && cargo build $(RELEASE) --bin tcs-moc; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the project
.PHONY: buildsim
buildsim:
	( \
		set -eu; \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcs-sim; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the configuration editor
.PHONY: buildgui
buildgui:
	( \
		set -eu; \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcs-cfge; \
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
# there, at the version tcsprotocol pins tcsxtce, its runtime, to, and ready
# to run: the tests reach its Python through python/tcsprotocol/_xtce,
# and installing the ground package copies it from there.
.PHONY: tcs-xtce
tcs-xtce:
	@$(MAKE) --no-print-directory -C tcsprotocol tcs-xtce

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
		cd $(RUST); \
		cargo build -q --bin segsend; \
		RUST_LOG=$(RUST_LOG) \
			cargo run -q --bin tcspecial -- --simulate=$(SIMULATE) \
			$(MEMLOCK_FLAG) $(TCSPECIAL_CONFIG_FILE).ytcs \
	)

# Running tcs-moc on one spelling of the tcspecial configuration.
#
# $(1) is that spelling, by its extension -- ytcs or xtcs. A configuration
# may be written in either and they describe the same payloads, so which one
# a run reads is a choice, and a run that always read the YAML could not
# exercise the other at all.
#
# Only the suffix is given here. The stem comes from TCSPECIAL_CONFIG_FILE, so
# a payload set is still chosen the way every other target chooses it, and the
# simulator file is untouched: it is tcs-sim's, and tcs-moc only passes it on.
#
# tcs-moc starts the tcspecial and tcs-sim built beside it, and tcspecial the
# segsend beside it, so all three are built first: in a checkout never built,
# they would not be there, and in one built before they could be stale.
define runmoc_on
	( \
		set -eu; \
		echo "Running $(MOC_NAME) on" \
			"$(basename $(TCSPECIAL_CONFIG_FILE)).$(1)..."; \
		cd $(RUST); \
		cargo build -q --bin tcspecial --bin tcs-sim --bin segsend; \
		RUST_LOG=$(RUST_LOG) \
			PAYLOAD_SIM_YAML=$(TCSPECIAL_SIM_FILE) \
			cargo run -q --bin tcs-moc -- --simulate=$(SIMULATE) \
			$(MEMLOCK_FLAG) \
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
# Both files of the set are given, because tcs-verify checks the pair: every
# payload simulated, every simulated payload a payload, and every setting one
# that payload can have. A good set prints nothing and the target succeeds; a
# bad one prints a line per problem and the target fails, which is what makes
# this usable from another recipe.
define runverify_on
	( \
		set -eu; \
		echo "Verifying $(basename $(TCSPECIAL_CONFIG_FILE)).$(1)..."; \
		cd $(RUST) && cargo run -q --bin tcs-verify -- \
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
# tcs-verify's three-file form: the XML and the YAML of one configuration, each
# checked with the simulator file as runverifyy and runverifyx check them, and
# then compared -- every section, attribute and value, as the programs read
# them, so two that pass are two whose digests tcspecial and tcs-moc agree on.
# The two are written by hand, and an edit made to one and not the other leaves
# both valid: nothing else notices.
.PHONY: runcomparexy
runcomparexy:
	( \
		set -eu; \
		echo "Comparing $(TCSPECIAL_CONFIG_FILE).xtcs with" \
			"$(TCSPECIAL_CONFIG_FILE).ytcs..."; \
		cd $(RUST) && cargo run -q --bin tcs-verify -- \
			$(TCSPECIAL_CONFIG_FILE).xtcs \
			$(TCSPECIAL_CONFIG_FILE).ytcs \
			$(TCSPECIAL_SIM_FILE) \
	)

# The stages of docs/building-a-vehicle.rst, each a directory holding
# deck.ytcs, deck.xtcs and deck-sim.yaml. Its common directory holds the
# files the later stages include, which are not stages.
VEHICLE_GUIDE_STAGES = $(wildcard docs/building-a-vehicle/stage-*)

# Every configuration shipped in both spellings, with the simulator file each
# is checked beside: the fixtures and the examples. A list of stems
# and simulator files rather than a pattern, so that a configuration may be
# checked beside a simulator file not named for it. THERMOSTAT_YAMCS is a
# stem too long to write twice on one line.
#
THERMOSTAT_YAMCS = examples/thermostat-yamcs/thermostat-yamcs
SPELLINGS = \
	tests/manual/thermostat:tests/manual/thermostat-sim.yaml \
	tests/consolidator/yamcs:tests/consolidator/yamcs-sim.yaml \
	examples/minimal/minimal:examples/minimal/minimal-sim.yaml \
	examples/consolidator/consolidator:examples/consolidator/consolidator-sim.yaml \
	examples/thermostat/thermostat:examples/thermostat/thermostat-sim.yaml \
	$(THERMOSTAT_YAMCS):$(THERMOSTAT_YAMCS)-sim.yaml \
	examples/simple-app/simple-app:examples/simple-app/simple-app-sim.yaml \
	examples/beacon/beacon:examples/beacon/beacon-sim.yaml \
	examples/fprime/fprime:examples/fprime/fprime-sim.yaml \
	examples/cfs/cfs:examples/cfs/cfs-sim.yaml \
	examples/lander-a/lander-a:examples/lander-a/lander-a-sim.yaml \
	examples/lander-b/lander-b:examples/lander-b/lander-b-sim.yaml \
	example:example-sim.yaml \
	$(foreach stage,$(VEHICLE_GUIDE_STAGES), \
		$(stage)/deck:$(stage)/deck-sim.yaml)

# Every shipped configuration, both spellings: the examples, the thermostat
# fixture, the consolidator-to-Yamcs fixture, the top-level example and the
# stages of "Building a Vehicle", but
# not the simulator files, which name no interface, nor the files in
# examples/*-common and docs/building-a-vehicle/common that others include,
# which have no tcspecial section to name one in.
SHIPPED_CONFIGURATIONS = example.ytcs example.xtcs \
	tests/manual/thermostat.ytcs tests/manual/thermostat.xtcs \
	tests/consolidator/yamcs.ytcs tests/consolidator/yamcs.xtcs \
	$(filter-out $(wildcard examples/*-common/*), \
		$(wildcard examples/*/*.ytcs) \
		$(wildcard examples/*/*.xtcs)) \
	$(VEHICLE_GUIDE_CONFIGURATIONS)

# The stages' configurations, both spellings.
VEHICLE_GUIDE_CONFIGURATIONS = \
	$(foreach stage,$(VEHICLE_GUIDE_STAGES),$(stage)/deck.ytcs \
		$(stage)/deck.xtcs)

# Write this build's interface digest into every shipped configuration, in
# place and changing nothing else, after tcsprotocol/xtce/link.xtce changes:
# tcs-verify --set-interface, which says what it changed. A test in
# tcs-verify/tests/examples.rs fails while one is stale.
.PHONY: set-interfaces
set-interfaces:
	cd $(RUST) && cargo run -q --bin tcs-verify -- --set-interface \
		$(SHIPPED_CONFIGURATIONS)

# Every shipped configuration's own XTCE, the file beside it of its name
# with the extension .xtce: what the vehicle flying it sends the ground. One
# for each configuration, its two spellings saying the same; written from
# the YAML spelling, and from example.ytcs's. Not the stages of "Building a
# Vehicle", which are teaching material and have no own XTCE beside them.
OWN_XTCE_CONFIGURATIONS = $(filter %.ytcs, \
	$(filter-out $(VEHICLE_GUIDE_CONFIGURATIONS),$(SHIPPED_CONFIGURATIONS)))

# Write each one's own XTCE again, after a configuration or
# tcsprotocol/xtce/types.xtce changes: tcs-verify --xtce, which says what it
# wrote.
.PHONY: xtce
xtce:
	cd $(RUST) && cargo run -q --bin tcs-verify -- --xtce \
		$(OWN_XTCE_CONFIGURATIONS)

# Fail where one is not what tcs-verify would write, naming each, so that a
# configuration and what the ground is told of it are reviewed together.
.PHONY: xtce-check
xtce-check:
	cd $(RUST) && cargo run -q --bin tcs-verify -- --xtce-check \
		$(OWN_XTCE_CONFIGURATIONS) >/dev/null

# Fail where operational code -- what flies, and what a ground station is
# built on -- is built from or imports code with a non-operational name, a
# name with a hyphen or an underscore after tcs or tcslog, or where
# categories.toml leaves a directory out of every category or disagrees
# with a name. The rules, and the check, are tcs-helpers'
# operational-names, which every TCS project runs; see "Operational code
# and its names" in docs/tcspecial-design.rst. The tcspecial checked is the
# one install-build builds, with every crate installed, whose features
# cargo pools: should it have the visual window, that is a warning, since
# such a build is for the desk and never flies.
.PHONY: operational-names
operational-names: tcs-helpers
	$(TCS_HELPERS)/bin/operational-names \
		$(addprefix --install ,$(INSTALL_CRATES)) \
		--desk-feature tcspecial/visual=tcs-diagram \
		--pyproject python/pyproject.toml \
		--pyproject tcs-ui/python/pyproject.toml

# Compare the two spellings of every one of them.
#
# Every pair is compared before the target fails, so one run says every set
# that has drifted rather than only the first; the failures are named again
# at the end, under what was printed for each.
.PHONY: comparespellings
comparespellings:
	( \
		set -u; \
		cd $(RUST) && cargo build -q --bin tcs-verify || exit 1; \
		failed=""; \
		for pair in $(SPELLINGS); do \
			stem=$${pair%%:*}; sim=$${pair#*:}; \
			echo "Comparing $$stem.xtcs with $$stem.ytcs..."; \
			cargo run -q --bin tcs-verify -- \
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
			cargo run -q --bin tcs-sim -- --simulate=$(SIMULATE) \
			$(TCSPECIAL_CONFIG_FILE).ytcs \
	)

# Make the virtual CAN bus a CAN payload needs.
#
# A CAN controller is a network interface rather than a device file, so a host
# with no CAN hardware has no bus at all and a CAN payload has nowhere to run.
# mkvcan creates the kernel's virtual one -- removing any left from a previous
# run first -- which is enough for both ends to be real: tcs-sim joins the bus
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
# for a moment when a tcs-sim is running. The rmstubs order is the reverse of
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
# The same TCS_CFGE_FILE the editor takes, so that checking what was just edited
# is the same name in both commands.
.PHONY: verify
verify:
	( \
		set -eu; \
		echo "Checking $(TCS_CFGE_FILE)..."; \
		cd $(RUST) && cargo run -q --bin tcs-verify -- \
			$(TCS_CFGE_FILE) \
	)

# The configuration to edit, and to check.
#
# A name of its own rather than TCSPECIAL_CONFIG_FILE, which the run targets
# use: that variable names a payload *set* -- a stem, with a tcspecial
# configuration and a simulator file either side of it -- where this is one
# configuration, which has no simulator file beside it.
#
# It need not exist yet: tcs-cfge writes it as edits are made and keeps its
# history beside it, so starting on a name that is not there is how a new
# configuration is begun.
#
# The default is the shipped example, which is what somebody opening the
# editor to see what it does wants to be looking at. It does get rewritten --
# tcs-cfge saves every edit as it is made, that being the point of it -- and
# that is a thing the example can survive where a scratch file could not: it
# is in the repository, so a stray edit shows up in `git status` and comes
# back with `git checkout example.ytcs`, where an editor opened on a name
# nobody keeps leaves its work somewhere nobody looks.
#
#   make runguiy
#   make runguiy TCS_CFGE_FILE=tests/manual/thermostat
#   make verify  TCS_CFGE_FILE=example.ytcs
TCS_CFGE_FILE = example.ytcs

# The simulator's own configuration, for the runsimgui targets.
#
# Named by its stem, like TCS_CFGE_FILE, and defaulting to the configuration's
# stem with `sim` on the end -- which is what the shipped sets are called, and
# is the one convention there is. The runguiy and runguix targets ignore it: a
# session given no simulator file edits the configuration alone, and the menu
# entry that would edit the simulator's groups is greyed.
#
#   make runsimguiy
#   make runsimguiy TCS_CFGE_FILE=vehicle TCS_CFGE_SIM=othersim
TCS_CFGE_SIM = $(basename $(TCS_CFGE_FILE))sim

# Running tcs-cfge on one spelling of a configuration.
#
# $(1) is that spelling -- ytcs or xtcs -- as runmoc_on takes it, and for the
# same reason: a configuration may be written in either, the two describe the
# same graph, and the extension is what says which to write. An editor that
# only ever opened the YAML would leave the XML to be written by hand.
#
# The spelling comes from the target and not from TCS_CFGE_FILE, so whatever
# extension that variable carries is replaced -- exactly as runmoc_on replaces
# the one on a payload set's stem.
define rungui_on
	( \
		set -eu; \
		mkdir -p $(dir $(TCS_CFGE_FILE)); \
		echo "Running $(GUI_NAME) on" \
			"$(basename $(TCS_CFGE_FILE)).$(1)..."; \
		cd $(RUST) && cargo run -q --bin tcs-cfge -- \
			$(basename $(TCS_CFGE_FILE)).$(1) \
	)
endef

# Running tcs-cfge on one spelling of a configuration and a simulator
# configuration beside it.
#
# $(1) is the configuration's spelling, as rungui_on takes it. $(2) is the
# simulator configuration, by its stem.
#
# The simulator file is YAML whichever spelling the configuration is in, and
# that is not an oversight: a simulator configuration is a YAML file, and
# tcs-cfge refuses a second argument whose name does not say so. A session
# editing the XML spelling of a graph is still editing the one simulator file
# beside it.
define tcs_simgui_on
	( \
		set -eu; \
		mkdir -p $(dir $(TCS_CFGE_FILE)) $(dir $(2)); \
		echo "Running $(GUI_NAME) on" \
			"$(basename $(TCS_CFGE_FILE)).$(1) and" \
			"$(basename $(2)).yaml..."; \
		cd $(RUST) && cargo run -q --bin tcs-cfge -- \
			$(basename $(TCS_CFGE_FILE)).$(1) \
			$(basename $(2)).yaml \
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
	$(call tcs_simgui_on,ytcs,$(TCS_CFGE_SIM))

.PHONY: runsimguix
runsimguix:
	$(call tcs_simgui_on,xtcs,$(TCS_CFGE_SIM))

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
# Four packages, since a package with operational code holds nothing else
# (see "The categories of TCS software" in docs/manual.rst):
#
#   tcspecial       operational code: what flies, the flight executive
#   tcsmoc          operational code: what a ground station is built on,
#                   libtcsmoc-c and the Python tcsprotocol and tcsmoc
#   tcs-tools       software tools: tcs-cfge and tcs-verify, which write and
#                   check configurations, and tcs_ui
#   tcspecial-test  verification tools: tcs-moc, tcs-sim, the test segsend
#                   and the five test apps
#   tcspecial-doc   the manuals and designs
#   tcspecial-examples
#                   examples: every example, its configurations and the
#                   programs it runs, and tcsx, which runs one
#
# Each install-<package> target below installs one, so the lists here are
# the whole account of which file is in which package, for `make install`,
# the .debs and the RPMs alike.
#
# The five test apps are what the five app node types are tested with, and
# are run by tcspecial rather than by a person, so they go in libexec: a
# configuration names one as `executable: /opt/tcs/libexec/app_pl`. A
# mission's own app is its own program, installed wherever its node's
# `executable` says. tcsapp's stray_app is a test and is not installed.
#
# The examples' Rust ground stations, beacon and lorem_gateway, go in bin
# beside tcspecial, where beacon looks for the tcspecial it starts; only
# tcsx, which runs them, is linked onto everybody's path.
#
# segsend is in the test package rather than the flight one: the segsend
# this tree builds deletes each segment it is given, which is for testing,
# and a vehicle that flew it would throw its telemetry away. A mission puts
# its own segsend beside tcspecial instead, and no package of ours then owns
# the file or overwrites it on an upgrade. The test one goes in the same
# bin, beside tcspecial, which is where tcspecial looks for it.
FLIGHT_BIN := tcspecial
TOOLS_BIN := tcs-cfge tcs-verify tcs-conflict
TEST_BIN := tcs-moc tcs-sim segsend
TEST_LIBEXEC := app_pl app_all_pl app_gateway_veh app_payload_veh app_all_veh
EXAMPLES_BIN := beacon lorem_gateway
PACKAGES := tcspecial tcsmoc tcs-tools tcspecial-test tcspecial-doc \
	    tcspecial-examples
# Its programs with windows are built with Slint, whose royalty-free
# licence asks for its badge where they are downloaded: make
# publish-release adds it to the notes of every release (see tcs-helpers'
# README).
RELEASE_ATTRIBUTION := slint

# The crates the examples' ground stations come from.
EXAMPLES_CRATES := beacon_example simple_app

# The crates those come from, built together by install-build.
INSTALL_CRATES := tcspecial tcs-verify tcs-conflict tcs-cfge tcsmoc-c \
		  tcs-moc tcs-sim \
		  $(TEST_LIBEXEC) $(EXAMPLES_CRATES)

# The crates each package's programs and libraries are built from, whose
# dependencies its THIRD-PARTY-NOTICES lists: every crate compiled into a
# program has licence notices that must go with it, which tcs-helpers'
# third-party-notices.py gathers. segsend is a program of the tcspecial
# crate's.
FLIGHT_CRATES := tcspecial
MOC_CRATES := tcsmoc-c
TOOLS_CRATES := tcs-cfge tcs-verify tcs-conflict
TEST_CRATES := tcs-moc tcs-sim tcspecial $(TEST_LIBEXEC)

# Where cargo puts a release build.
RELEASE_DIR = $(or $(CARGO_TARGET_DIR),target)/release

# The versions, read from where each is stated so that it is stated once: the
# workspace's Cargo.toml for the packages, tcsmoc-c's for libtcsmoc-c's
# own, which tcsmoc-c.pc says, and tcsmoc-c/abi-version for
# libtcsmoc-c's ABI version, which is its soname -- libtcsmoc-c.so.<ABI>
# -- and the name its file is installed under. The ABI version goes up only when
# a program built against the old tcsmoc-c.h would not work with the new
# library: see tcsmoc-c/build.rs, which reads the same file to set the
# soname, for exactly when.
VERSION := $(shell sed -n \
	   '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' \
	   Cargo.toml)
LIBTCS_VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' \
		    tcsmoc-c/Cargo.toml)
LIBTCS_ABI := $(shell cat tcsmoc-c/abi-version)
LIBTCS_SONAME := libtcsmoc-c.so.$(LIBTCS_ABI)

# The examples, for tcspecial-examples: their configurations, in both
# spellings, with their XTCE, simulator files and diagrams, and the Python
# programs they run -- payloads, compute nodes and ground stations, with
# what ping-common's two share -- and the KWin window rules the lander's
# windows are placed by, each at its path in the checkout,
# examples/NAME/..., which is how each configuration names its programs and
# how each ground station finds its neighbours. Taken from git, so that a
# scratch file left in examples/ is not shipped, and without the tests. A
# simulator file two examples share is a link in each to the one in their
# -common directory, and is installed as the same link. A program's
# #!/usr/bin/env python3 is installed as #!/usr/bin/python3, where Debian
# and Fedora both put it and as both want an installed script to say.
EXAMPLE_FILES = examples/README.html $(shell git ls-files \
		'examples/*/*.ytcs' 'examples/*/*.xtcs' 'examples/*/*.xtce' \
		'examples/*/*-sim.yaml' 'examples/*/*.svg' 'examples/*/*.pdf' \
		'examples/*/*.py' \
		'examples/*/*.kwinrule' \
		| grep -v '/test_[^/]*\.py$$')

# Whether what is installed is stripped: 1, the default, or 0.
#
# Stripped by default because what is installed is what is run, on a vehicle
# whose storage is small and on ground stations alike, and a release build
# carries its symbol tables and the standard library's debug information:
# stripping takes a fifth to a quarter off what each package installs, and
# libtcsmoc-c.a from 33 MB to 24 MB. Executables and shared libraries are
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

# Count each tcs project's and subproject's non-test source statements, of
# every ../tcs* checkout beside this one, by the categories each one's
# categories.toml puts its code in: see tools/ntss.py for what is counted. Printed, not kept: the counts
# are of other repositories too, as they stand when it is run. CSV=1 prints
# them as comma-separated values, for a spreadsheet. NTSS_SKIP names the
# checkouts beside this one not to count: tcspecial-factor is an old copy
# of tcspecial, which would count it twice.
NTSS_SKIP ?= tcspecial-factor

.PHONY: ntss
ntss:
	python3 tools/ntss.py --tcs-helpers $(TCS_HELPERS) \
		$(addprefix --skip ,$(NTSS_SKIP)) $(if $(filter 1,$(CSV)),--csv)

# Install the tools building, testing, checking, documenting and packaging
# this project need, with the system's own package manager, under sudo:
# see bin/install-tools, which a container can run as root too. DRY_RUN=1
# shows the commands and runs none; PACKAGING=0 leaves out what only make
# packages and make lint-packages need.
.PHONY: install-tools
install-tools:
	bin/install-tools $(if $(filter 1,$(DRY_RUN)),--dry-run) \
		$(if $(filter 0,$(PACKAGING)),--no-packaging)

# Copy every package's files into place from a build already made,
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
# Its README, licences and notices go with it; the manuals are
# tcspecial-doc's.
.PHONY: install-tcspecial
install-tcspecial: tcs-helpers
	@echo "Installing $(PROJECT_NAME) under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR)
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(FLIGHT_BIN)) \
		$(DESTDIR)$(BINDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(FLIGHT_BIN)),)
	$(call install_links,tcspecial,$(FLIGHT_BIN))
	install -d -m 755 $(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/THIRD-PARTY-NOTICES \
		tcspecial $(FLIGHT_CRATES)
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

# What a ground station is built on: libtcsmoc-c for C and C++, and the
# Python tcsprotocol and tcsmoc, with the schemas they read.
#
# Cargo builds it as libtcsmoc_c, a crate's name having no hyphen, and it is
# installed as libtcsmoc-c. libtcsmoc-c.so is installed under its soname,
# libtcsmoc-c.so.<ABI>, which is what a program built against it asks for at
# run time, and linked to by libtcsmoc-c.so, which is what the linker looks
# for. tcsmoc-c.h goes
# in include/ itself, as <tcsmoc-c.h>. The library is not executable, as
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
.PHONY: install-tcsmoc
install-tcsmoc: install-schemas tcs-helpers tcs-xtce
	@echo "Installing tcsmoc under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(INCLUDEDIR) \
		$(DESTDIR)$(LIBDIR)/pkgconfig $(DESTDIR)$(PYTHONDIR)
	install -m 644 tcsmoc-c/include/tcsmoc-c.h \
		$(DESTDIR)$(INCLUDEDIR)
	install -m 644 $(RELEASE_DIR)/libtcsmoc_c.so \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME)
	ln -sf $(LIBTCS_SONAME) $(DESTDIR)$(LIBDIR)/libtcsmoc-c.so
	install -m 644 $(RELEASE_DIR)/libtcsmoc_c.a \
		$(DESTDIR)$(LIBDIR)/libtcsmoc-c.a
	$(call strip_installed,$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME), \
		$(DESTDIR)$(LIBDIR)/libtcsmoc-c.a)
	$(SUBST) packaging/tcsmoc-c.pc.in \
		> $(DESTDIR)$(LIBDIR)/pkgconfig/tcsmoc-c.pc
	chmod 644 $(DESTDIR)$(LIBDIR)/pkgconfig/tcsmoc-c.pc
# The schemas the Python reads are tcsprotocol/schema, which install-schemas
# puts under $(SCHEMADIR): the installed package reaches them through a link, as
# python/tcsprotocol/schema does in the source tree, rather than carrying a
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
	schema=$(DESTDIR)$(PYTHONDIR)/tcsprotocol/schema; \
	rm -rf "$$schema"; \
	ln -s $$(realpath -m --relative-to=$(PYTHONDIR)/tcsprotocol \
		$(SCHEMADIR)) "$$schema"; \
	sed -i '/^tcsprotocol\/schema\//d' "$$dist/RECORD"
	install -d -m 755 $(DESTDIR)$(DOCDIR)/tcsmoc/tcsmoc-c \
		$(DESTDIR)$(DOCDIR)/tcsmoc/python
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcsmoc
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/tcsmoc/THIRD-PARTY-NOTICES \
		tcsmoc $(MOC_CRATES)
	install -m 644 tcsmoc-c/README.html \
		$(DESTDIR)$(DOCDIR)/tcsmoc/tcsmoc-c
	install -m 644 python/README.md \
		$(DESTDIR)$(DOCDIR)/tcsmoc/python
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR) \
			$(DESTDIR)$(LDCONFDIR) $(DESTDIR)$(PTHDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcsmoc|g' \
			packaging/hooks/profile-moc.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcsmoc.sh; \
		echo $(LIBDIR) \
			> $(DESTDIR)$(LDCONFDIR)/tcsmoc.conf; \
		echo $(PYTHONDIR) \
			> $(DESTDIR)$(PTHDIR)/tcsmoc.pth; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcsmoc.sh \
			$(DESTDIR)$(LDCONFDIR)/tcsmoc.conf \
			$(DESTDIR)$(PTHDIR)/tcsmoc.pth; \
	fi
	@if [ "$(HOOKS)" = 1 ] && [ -z "$(DESTDIR)" ]; then \
		echo "Run ldconfig as root so the loader finds" \
			"$(LIBTCS_SONAME)."; \
	fi

# The software tools: tcs-cfge, the configuration editor, with its data;
# tcs-verify, the configuration checker; and tcs_ui, the Python window
# pieces, whose distribution, tcs-ui, is pip's to install from a copy of
# tcs-ui/python, as tcsmoc's is, and which has a .pth file of its own.
.PHONY: install-tcs-tools
install-tcs-tools: install-data tcs-helpers
	@echo "Installing tcs-tools under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) $(DESTDIR)$(PYTHONDIR) \
		$(DESTDIR)$(DOCDIR)/tcs-tools
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(TOOLS_BIN)) \
		$(DESTDIR)$(BINDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(TOOLS_BIN)),)
	$(call install_links,tcs-tools,$(TOOLS_BIN))
	set -e; \
	tmp=$$(mktemp -d -t tcs-ui-py.XXXXXXXXXX); \
	trap 'rm -rf "$$tmp"' EXIT; \
	cp -RL tcs-ui/python "$$tmp/"; \
	rm -rf "$$tmp/python/build" "$$tmp/python/tests" \
		"$$tmp"/python/*.egg-info; \
	find "$$tmp" -name __pycache__ -type d -prune -exec rm -rf {} +; \
	python3 -m pip install --quiet --upgrade --no-deps \
		--no-build-isolation --no-compile \
		--target $(DESTDIR)$(PYTHONDIR) "$$tmp/python"; \
	dist=$$(echo $(DESTDIR)$(PYTHONDIR)/tcs_ui-*.dist-info); \
	rm -f "$$dist/direct_url.json"; \
	sed -i '/^tcs_ui-[^/]*\.dist-info\/direct_url\.json,/d' \
		"$$dist/RECORD"
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcs-tools
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/tcs-tools/THIRD-PARTY-NOTICES \
		tcs-tools $(TOOLS_CRATES)
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR) $(DESTDIR)$(PTHDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcs-tools|g' \
			packaging/hooks/profile.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcs-tools.sh; \
		echo $(PYTHONDIR) > $(DESTDIR)$(PTHDIR)/tcs-tools.pth; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcs-tools.sh \
			$(DESTDIR)$(PTHDIR)/tcs-tools.pth; \
	fi

# The documents: the manuals and designs. Their figures are the examples'
# diagrams, which they find at ../examples, as in the source tree, so
# share/doc/tcspecial-doc/examples links to the examples tcspecial-examples
# installs, which tcspecial-doc depends on.
.PHONY: install-tcspecial-doc
install-tcspecial-doc: tcs-helpers
	@echo "Installing tcspecial-doc under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(DOCDIR)/tcspecial-doc/docs/shots
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcspecial-doc
	# Its notices say it owes none: nothing in it is compiled, and the one
	# thing not TCS's own, the stylesheet each page embeds, is in the
	# public domain. Every package carries the file, which a release
	# checks for.
	install -m 644 packaging/tcspecial-doc-notices \
		$(DESTDIR)$(DOCDIR)/tcspecial-doc/THIRD-PARTY-NOTICES
	install -m 644 $(DOCS_DIR)/*.html $(DOCS_DIR)/*.svg \
		$(DESTDIR)$(DOCDIR)/tcspecial-doc/docs
	install -m 644 $(DOCS_DIR)/shots/*.png \
		$(DESTDIR)$(DOCDIR)/tcspecial-doc/docs/shots
	ln -sfn ../../examples/$(PROJECT_NAME) \
		$(DESTDIR)$(DOCDIR)/tcspecial-doc/examples

# The examples, the ground stations built for them and tcsx, which runs one
# from them: see examples/tcsx. An example file that starts with "#!" is a
# program to run, so it is installed executable and the rest are not.
.PHONY: install-tcspecial-examples
install-tcspecial-examples: tcs-helpers
	@echo "Installing tcspecial-examples under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(DOCDIR)/tcspecial-examples
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(EXAMPLES_BIN)) \
		examples/tcsx $(DESTDIR)$(BINDIR)
	$(call strip_installed, \
		$(addprefix $(DESTDIR)$(BINDIR)/,$(EXAMPLES_BIN)),)
	$(call install_links,tcspecial-examples,tcsx)
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcspecial-examples
	$(THIRD_PARTY_NOTICES) . \
		$(DESTDIR)$(DOCDIR)/tcspecial-examples/THIRD-PARTY-NOTICES \
		tcspecial-examples $(EXAMPLES_CRATES)
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcspecial-examples|g' \
			packaging/hooks/profile.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcspecial-examples.sh; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcspecial-examples.sh; \
	fi
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
		if [ "$$(head -n 1 $$file)" = '#!/usr/bin/env python3' ]; then \
			sed -i '1s|.*|#!/usr/bin/python3|' $$to || exit 1; \
		fi; \
	done

# The verification tools: the test ground station, the payload simulator,
# the test segsend and the five test apps.
#
# tcs-moc starts the tcspecial and the tcs-sim beside it, so these go in the
# same bin as tcspecial does: see child_command in tcs-moc/src/main.rs.
.PHONY: install-tcspecial-test
install-tcspecial-test: tcs-helpers
	@echo "Installing tcspecial-test under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) $(DESTDIR)$(LIBEXECDIR) \
		$(DESTDIR)$(DOCDIR)/tcspecial-test
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(TEST_BIN)) \
		$(DESTDIR)$(BINDIR)
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(TEST_LIBEXEC)) \
		$(DESTDIR)$(LIBEXECDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(TEST_BIN)) \
		$(addprefix $(DESTDIR)$(LIBEXECDIR)/,$(TEST_LIBEXEC)),)
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

# Install tcs-cfge's data -- the description tree and its schemas -- for
# tcs-tools.
#
# The whole of tcs-cfge/share, with its structure: the description tree's shape
# is part of what it means -- a description at vehicle/compute/vote.yaml is
# about the Rust at node/vehicle/compute/vote.rs -- so a flat copy would lose
# half of it.
#
# Into share/tcs-cfge under the prefix, which is where an installed tcs-cfge
# looks: one level up from its own bin. See `tcs_cfge::nodes::data_dir` for
# where the editor looks and in what order.
#
# Mode 644 and directories 755: these are read by everybody and written by
# whoever installed them. A person's own descriptions belong in
# $XDG_DATA_HOME/tcs-cfge, which the editor looks in first, rather than in a
# directory that an upgrade overwrites.
.PHONY: install-data
install-data:
	@echo "Installing $(GUI_NAME) data into $(TCS_CFGE_DATA)..."
	@cd $(GUI_NAME)/share && find . -type d \
		-exec install -d -m 755 $(TCS_CFGE_DATA)/{} \;
	@cd $(GUI_NAME)/share && find . -type f \
		-exec install -m 644 {} $(TCS_CFGE_DATA)/{} \;
	@echo "✓ $(GUI_NAME) data installed"

# Install the XTCS, YTCS and simulator schemas of tcsprotocol/schema under
# $(SCHEMADIR), for tcsmoc: they define the formats every program reads,
# and tcsprotocol's Python reads them.
.PHONY: install-schemas
install-schemas:
	@install -d -m 755 $(DESTDIR)$(SCHEMADIR)
	@install -m 644 tcsprotocol/schema/README \
		tcsprotocol/schema/*.xsd tcsprotocol/schema/*.json \
		$(DESTDIR)$(SCHEMADIR)
	@echo "✓ schemas installed"

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
	rm -f $(addprefix $(DESTDIR)$(BINDIR)/,$(FLIGHT_BIN) $(TOOLS_BIN) \
		$(TEST_BIN))
	rm -f $(addprefix $(DESTDIR)$(LIBEXECDIR)/,$(TEST_LIBEXEC))
	rm -f $(addprefix $(DESTDIR)$(SBINDIR)/,$(addsuffix -links,$(PACKAGES)))
	rm -f $(DESTDIR)$(INCLUDEDIR)/tcsmoc-c.h
	rm -f $(DESTDIR)$(LIBDIR)/libtcsmoc-c.so \
		$(DESTDIR)$(LIBDIR)/libtcsmoc-c.a \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME)
	rm -f $(DESTDIR)$(LIBDIR)/pkgconfig/tcsmoc-c.pc
	rm -rf $(addprefix $(DESTDIR)$(PYTHONDIR)/,tcsprotocol tcsmoc \
		tcs_ui tcspecial_moc-*.dist-info tcs_ui-*.dist-info)
	rm -rf $(TCS_CFGE_DATA) $(DESTDIR)$(DATADIR)/tcspecial \
		$(DESTDIR)$(EXAMPLEDIR) \
		$(addprefix $(DESTDIR)$(DOCDIR)/,$(PACKAGES))
	if [ "$(HOOKS)" = 1 ]; then \
		rm -f $(addprefix $(DESTDIR)$(PROFILEDIR)/,$(addsuffix .sh, \
			$(PACKAGES))) \
			$(DESTDIR)$(LDCONFDIR)/tcsmoc.conf \
			$(DESTDIR)$(PTHDIR)/tcsmoc.pth \
			$(DESTDIR)$(PTHDIR)/tcs-tools.pth \
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
.PHONY: portable-packages
portable-packages: tcs-helpers
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
# Every crate is linted with its tests, examples and benches as well as its
# library and programs: a warning in a test is a warning, and a check that
# looks only at what ships lets one sit until it is copied somewhere that
# does.
#
# With tcspecial built with no optional payload type and with each one alone
# as well as with all of them: a gate that leaves a reference to a type's
# module outside it breaks only the builds without that type, which a
# workspace built with every type never makes. The types are what a build
# with the defaults says it contains, which is every type, so that the list
# is not written down a second time here; see "Which Node Types Are Built" in
# docs/design.rst. And with the visual window and with the bit errors,
# test code that no default build has and so nothing else would ever build.
#
# And every configuration compared in its two spellings, which must say the
# same thing: a change made to one and not the other leaves both files valid,
# and an edit by hand is made to one file at a time. And the names held to
# what they say: operational code is built only from operational code, and
# categories.toml puts every directory in a category.
.PHONY: check
check:
	@echo "Running cargo check..."
	cd $(RUST) && cargo check
	cd $(RUST) && cargo clippy --workspace --all-targets -- -D warnings
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
	cd $(RUST) && cargo clippy -p $(PROJECT_NAME) --features bit-errors \
		--all-targets -- -D warnings
	cd $(RUST) && cargo test -q -p $(PROJECT_NAME) --features bit-errors \
		--lib -- bit_errors flight_checks
	cd $(RUST) && cargo fmt -- --check
	$(MAKE) --no-print-directory comparespellings
	$(MAKE) --no-print-directory xtce-check
	$(MAKE) --no-print-directory operational-names
	$(MAKE) --no-print-directory html-check
	$(MAKE) --no-print-directory -C container check

# The F´ bridge, which F´'s own tools build rather than cargo: see
# fprime/Makefile, which says where it looks for F´ and how to say
# otherwise. Whatever is given here -- FPRIME_SCRATCH, FPRIME_FRAMEWORK,
# FPRIME_VENV, FPRIME_CONFIG -- is passed on.
.PHONY: fprime-build
fprime-build:
	$(MAKE) -C fprime build

# Build the bridge and run its test: tcs-sim simulating the fprime
# example's payloads, tcspecial flying it, and F´'s ground system commanding
# the bridge in an F´ deployment and reading its channels, holding the ports'
# lock while it runs. tcs-sim's window opens for the length of the test.
.PHONY: fprime-test
fprime-test:
	$(MAKE) -C fprime test

# Where simulated-devices-test keeps what a run leaves, emptied at the start of
# each.
SIMULATED_DEVICES_RUN = $(CURDIR)/tests/simulated-devices/run

# Run tcspecial against tcs-sim with a payload of each type tcs-sim simulates,
# and check that each one's data reaches the ground: see
# tests/simulated-devices/simulated-devices-test. CAN and GPIO are tested only
# where a virtual CAN bus or a gpio-sim chip is already there. tcs-sim's window
# opens for the length of the test.
.PHONY: simulated-devices-test
simulated-devices-test:
	cd $(RUST) && cargo build --bin tcspecial --bin tcs-sim --bin tcs-verify
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

# Make the tcs-sim manual's screenshots, docs/shots/tcs-sim-*.png, again: its
# window, built by its own code on the thermostat example and drawn in memory
# by Slint's software renderer, so that no window opens and nothing is bound
# or sent. An ignored test, run alone because Slint takes one platform per
# process; never part of make test, make check or make html-check.
.PHONY: screenshots-tcs-sim
screenshots-tcs-sim:
	cd $(RUST) && cargo test -p tcs-sim --lib -- --ignored --exact \
		shots::screenshots

# Make the tcs-cfge manual's screenshots, docs/shots/tcs-cfge-*.png, again: its
# windows, opened by its own code on copies of the battery example, driven
# through the presses and callbacks a person's gestures reach, and drawn in
# memory by Slint's software renderer, so that no window opens and nothing
# under examples is written. An ignored test, run alone because Slint takes
# one platform per process; never part of make test, make check or make
# html-check, the fonts differing between machines.
.PHONY: screenshots-tcs-cfge
screenshots-tcs-cfge:
	cd $(RUST) && cargo test -p tcs-cfge --bin tcs-cfge -- --ignored \
		--exact shots::screenshots

# Make the tcs-moc manual's screenshots, docs/shots/tcs-moc-*.png, again: its
# windows, on the thermostat and ping examples, wired as it wires them, fed
# real beacons, command answers and data by a simulated vehicle on loopback
# ports of the test's own choosing, and drawn in memory by Slint's software
# renderer, so that no window opens and no other program's ports are
# touched. An ignored test, run alone because Slint takes one platform per
# thread and one event-loop queue per process; about two minutes, most of
# it the beacon indicator ageing. Never part of make test, make check or
# make html-check.
.PHONY: screenshots-tcs-moc
screenshots-tcs-moc:
	cd $(RUST) && cargo test -p tcs-moc --lib -- --ignored --exact \
		vehicle::screenshots::screenshots

# Print every example's configuration to a PDF beside it,
# examples/NAME/NAME.pdf: every page, the top page and each subdiagram's, as
# tcs-cfge's File > Print prints them, by tcs-cfge --print, which draws in
# memory with Slint's software renderer and the panes divided as they are
# until somebody moves them, so that no window opens and nothing but the PDFs
# is written. An example is a directory with a configuration of its own name,
# printed with its simulator file where it has one. Each PDF records a digest
# of the files it was printed from -- the configuration in both spellings,
# each file it includes, the simulator file -- and only one whose files have
# changed since is printed again, so that the fonts of the machine running
# this change no picture that is current. A test in tcs-cfge fails while any
# example's PDF is out of date, saying to run this.
.PHONY: example-pdfs
example-pdfs:
	cd $(RUST) && cargo build -q --bin tcs-cfge
	cd $(RUST) && for top in $$(git ls-files 'examples/*/*.ytcs'); do \
		dir=$${top%/*}; name=$${dir##*/}; \
		[ "$$top" = "$$dir/$$name.ytcs" ] || continue; \
		sim=$$dir/$$name-sim.yaml; [ -e "$$sim" ] || sim=; \
		cargo run -q --bin tcs-cfge -- \
			--print "$$dir/$$name.pdf" --if-changed "$$top" $$sim \
			|| exit 1; \
	done

# Make every manual's screenshots again, one program after another, each in
# a process of its own; the pages that show them are made again by the
# pre-commit hook when they are committed.
.PHONY: screenshots
screenshots: screenshots-tcs-cfge screenshots-tcs-moc screenshots-tcs-sim

# Every directory whose HTML pages are made from its markdown or
# reStructuredText and committed beside it, which make in each builds.
HTML_DIRS = $(DOCS_DIR) examples tcsmoc tcsmoc-c tcsprotocol container

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
