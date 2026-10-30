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
#   make install                           # /opt/tcs
#   make install PREFIX=$HOME/tcs          # somewhere a user may write
#   make install DESTDIR=/tmp/package      # staged, for building a package
#
# DESTDIR is prepended to everything and is not part of any path a program
# looks in, which is what lets a package be built in a staging directory.
#
# Under the prefix, by kind, as a package under /usr would have it:
#   bin/         the programs a person runs
#   sbin/        <package>-links, one per package, which an administrator
#                runs to link that package's programs into /usr/local/bin
#   libexec/     programs other programs run: the five test app programs
#   lib/         libtcs, its pkg-config file, and the Python packages, in
#                lib/python3/site-packages, which names no minor version
#   include/     tcs.h
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

# Where the hooks go. Debian's places; the RPM spec moves the .pth file to
# Fedora's, which names python3's minor version: see
# packaging/rpm/tcspecial-ground.spec.
PROFILEDIR ?= /etc/profile.d
LDCONFDIR ?= /etc/ld.so.conf.d
SYSTEMDDIR ?= /usr/lib/systemd/system
PTHDIR ?= /usr/lib/python3/dist-packages

# The configuration to run, named without a suffix.
#
# TCSPECIAL_CONFIG_FILE is the stem, and `$(TCSPECIAL_CONFIG_FILE).yaml` or
# `.xml` the configuration. So:
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
# The file beside it, `<stem>-sim.yaml`, as every example names its own --
# except for the thermostat fixture, which has none: it is simulated by the
# example's, which stands in for the same payloads, as SPELLINGS below says.
# Named rather than worked out by the programs, because tcssim reads it from
# the environment and a name nothing has is a run with nothing simulated.
#   make runsim TCSPECIAL_CONFIG_FILE=mine TCSPECIAL_SIM_FILE=mine-bench.yaml
ifeq ($(TCSPECIAL_CONFIG_FILE),tests/manual/thermostat)
TCSPECIAL_SIM_FILE ?= examples/thermostat/thermostat-sim.yaml
else
TCSPECIAL_SIM_FILE ?= $(TCSPECIAL_CONFIG_FILE)-sim.yaml
endif

# What the programs log, which every run target passes on. A variable rather
# than a word in each recipe, so that it can be overridden the way the payload
# set is:
#   make runmocy RUST_LOG=debug
#   make run TCSPECIAL_CONFIG_FILE=examples/minimal/minimal RUST_LOG=debug
RUST_LOG = info

# The run's simulation mode, given to every program a run target starts:
# none, some or all. All by default, because these targets run on a desk with
# stand-ins, and a program that leaves the flag out gets none -- the flight
# setting -- and refuses every stand-in. See "Simulation Modes" in
# docs/design.rst.
#   make runmocy SIMULATE=some
SIMULATE = all

RELEASE = --release
RELEASE =

# Default target
all: build test

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
	@echo "  make verify      - Check one version 2 configuration"
	@echo "  make mkvcan      - Make the virtual CAN bus a CAN payload" \
		"needs"
	@echo "  make mkstubs     - Make every stand-in: I2C, GPIO, IIO and" \
		"virtual CAN"
	@echo "  make rmvcan      - Remove the virtual CAN buses"
	@echo "  make rmstubs     - Remove every stand-in those made"
	@echo "  make check       - cargo check, clippy, and a format check"
	@echo "  make format      - Reformat the source"
	@echo "  make release     - Build with optimizations"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make distclean   - Remove everything that can be rebuilt"
	@echo "  make install     - Build and install all three packages" \
		"under PREFIX (/opt/tcs)"
	@echo "  make install-build - The release build make install installs"
	@echo "  make install-files - Install a build already made"
	@echo "  make install-tcspecial, install-tcspecial-ground," \
		"install-tcspecial-test"
	@echo "                   - Install one package from a build" \
		"already made"
	@echo "  make install-data - Install tcsgui's descriptions and schemas"
	@echo "  make uninstall   - Take away what make install put in place"
	@echo "  make links       - Link the installed programs into" \
		"LINKDIR (/usr/local/bin)"
	@echo "  make unlinks     - Take those links away"
	@echo "  make stage       - Install each package into build/stage"
	@echo "  make deb         - Build the .debs into build/packages"
	@echo "  make rpm         - Build the RPMs into build/packages"
	@echo "  make packages    - Build both"
	@echo "                     (each strips what it installs;" \
		"STRIP=0 keeps it as built)"
	@echo ""
	@echo "Installing:  make install-build &&" \
		"sudo make install-files HOOKS=1"
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
test:
	( \
		set -eu; \
		echo "Running tests..."; \
		cd $(RUST) && cargo test; \
		$(MAKE) --no-print-directory doc-headers; \
		echo "✓ Tests complete"; \
	)

# Every non-trivial function, in Rust, C, C++ and Python, has a header:
# what it does, what each parameter means and what it returns, in its
# language's own convention. The checker is tcs-xtce's, vendored with the
# rest of it; it checks every file git tracks but vendor/ and target/.
.PHONY: doc-headers
doc-headers:
	python3 -I vendor/tcs-xtce/tests/doc_headers.py

# Run the tcspecial application
.PHONY: run
run:
	( \
		set -eu; \
		echo "Running $(PROJECT_NAME)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) \
			cargo run -q --bin tcspecial -- --simulate=$(SIMULATE) \
			$(TCSPECIAL_CONFIG_FILE).yaml \
	)

# Running tcsmoc on one spelling of the tcspecial configuration.
#
# $(1) is that spelling -- yaml or xml. A configuration may be written in
# either and they describe the same payloads, so which one a run reads is a
# choice, and a run that always read the YAML could not exercise the other at
# all.
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
	$(call runmoc_on,yaml)

.PHONY: runmocx
runmocx:
	$(call runmoc_on,xml)

# Checking one spelling of a payload set without running anything.
#
# $(1) is that spelling -- yaml or xml -- as runmoc_on takes it, and for the
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
	$(call runverify_on,yaml)

.PHONY: runverifyx
runverifyx:
	$(call runverify_on,xml)

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
		echo "Comparing $(TCSPECIAL_CONFIG_FILE).xml with" \
			"$(TCSPECIAL_CONFIG_FILE).yaml..."; \
		cd $(RUST) && cargo run -q --bin tcsverify -- \
			$(TCSPECIAL_CONFIG_FILE).xml \
			$(TCSPECIAL_CONFIG_FILE).yaml \
			$(TCSPECIAL_SIM_FILE) \
	)

# Every configuration shipped in both spellings, with the simulator file each
# is checked beside: the thermostat fixture and the examples. A list of stems
# and simulator files rather than a pattern, because the thermostat fixture
# has no simulator file of its own: it is checked beside the example's, which
# simulates the same payloads.
#
SPELLINGS = \
	tests/manual/thermostat:examples/thermostat/thermostat-sim.yaml \
	examples/minimal/minimal:examples/minimal/minimal-sim.yaml \
	examples/thermostat/thermostat:examples/thermostat/thermostat-sim.yaml \
	examples/simple-app/simple-app:examples/simple-app/simple-app-sim.yaml \
	examples/beacon/beacon:examples/beacon/beacon-sim.yaml \
	example:example-sim.yaml

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
			echo "Comparing $$stem.xml with $$stem.yaml..."; \
			cargo run -q --bin tcsverify -- \
				$$stem.xml $$stem.yaml $$sim \
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
			$(TCSPECIAL_CONFIG_FILE).yaml \
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

# Make every stand-in a payload set may need.
#
# The four types the simulator cannot simply be the far end of: a device on a
# bus has to answer when a master addresses it, a discrete line is a wire, a
# sensor's buffer is filled by the sensor, and a CAN controller is a network
# interface rather than a file. The kernel stands in for each, and there is one
# script per type -- bin/mki2c, bin/mkgpio, bin/mkiio and bin/mkvcan -- because
# a host that needs one commonly does not need the others, and one script that
# made all four could fail on a type nobody had asked for.
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
# carrying a copy, so that undoing a stand-in is one command whether it is done
# on its own or as the first half of remaking one.
#
# One script per type again, and each is the cleanup half of its own mk script,
# which calls it rather than carrying a copy -- so undoing a stand-in is the
# same code whether it is done on its own or as the first half of remaking one.
#
# None is destructive beyond its own stand-in. rmvcan removes the virtual CAN
# interfaces and leaves the driver loaded; rmgpio and rmiio remove the chip and
# the sensor this tree made, by name, since a host's own sit in the same
# numbering, and leave another program's alone. rmi2c is the blunt one: an
# i2c-stub adapter *is* the module, so there is no way to take away one chip
# and leave another.
#
# All of them take a stand-in out from under anything using it, so none is for
# a moment when a tcssim is running. The rmstubs order is the reverse of
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
# beside it -- what a node stands in for is the node's own business -- so this
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
# back with `git checkout example.yaml`, where an editor opened on a name
# nobody keeps leaves its work somewhere nobody looks.
#
#   make runguiy
#   make runguiy TCSGUI_FILE=tests/manual/thermostat
#   make verify  TCSGUI_FILE=example.yaml
TCSGUI_FILE = example.yaml

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
# $(1) is that spelling -- yaml or xml -- as runmoc_on takes it, and for the
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
	$(call rungui_on,yaml)

.PHONY: runguix
runguix:
	$(call rungui_on,xml)

# And with the simulator's configuration beside it, which is what adds the
# simulation settings to each payload's window and lights the menu entry for
# the simulator's groups.
.PHONY: runsimguiy
runsimguiy:
	$(call tcssimgui_on,yaml,$(TCSGUI_SIM))

.PHONY: runsimguix
runsimguix:
	$(call tcssimgui_on,xml,$(TCSGUI_SIM))

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	-cargo clean
	rm -f build.out
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
# what flies, and nothing a vehicle does not need is in it; tcspecial-ground
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
FLIGHT_LIBEXEC := app_pl app_all_pl app_moc_veh app_payload_veh app_all_veh
GROUND_BIN := tcsgui
TEST_BIN := tcsmoc tcssim segsend
PACKAGES := tcspecial tcspecial-ground tcspecial-test

# The crates those come from, built together by install-build.
INSTALL_CRATES := tcspecial tcsverify tcsgui tcslib-c tcsmoc tcssim \
		  $(FLIGHT_LIBEXEC)

# Where cargo puts a release build.
RELEASE_DIR = $(or $(CARGO_TARGET_DIR),target)/release

# The versions, read from where each is stated so that it is stated once:
# the workspace's Cargo.toml for the packages, tcslib-c's for libtcs's own,
# which tcs.pc says, and tcslib-c/abi-version for libtcs's ABI version,
# which is its soname -- libtcs.so.<ABI> -- and the name its file is
# installed under. The ABI version goes up only when a program built against
# the old tcs.h would not work with the new library: see tcslib-c/build.rs,
# which reads the same file to set the soname, for exactly when.
VERSION := $(shell sed -n \
	   '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' \
	   Cargo.toml)
LIBTCS_VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' \
		    tcslib-c/Cargo.toml)
LIBTCS_ABI := $(shell cat tcslib-c/abi-version)
LIBTCS_SONAME := libtcs.so.$(LIBTCS_ABI)

# The example configurations, in both spellings, with their simulator files
# and diagrams; the manual's figures are the diagrams, so it is given them as
# well. Taken from git, so that a scratch file left in examples/ is not
# shipped.
EXAMPLE_FILES = examples/README.html $(shell git ls-files \
		'examples/*/*.yaml' 'examples/*/*.xml' 'examples/*/*.svg')

# Whether what is installed is stripped: 1, the default, or 0.
#
# Stripped by default because what is installed is what is run, on a vehicle
# whose storage is small and on ground stations alike, and a release build
# carries its symbol tables and the standard library's debug information:
# stripping takes a fifth to a quarter off what each package installs, and
# libtcs.a from 33 MB to 24 MB. Executables and shared libraries are stripped of
# everything they do not need to be linked or run (--strip-unneeded); the
# static library only of its debug information (--strip-debug), because a
# program linked with it needs its symbols. `make packages STRIP=0` keeps
# everything as cargo built it, for a build to be debugged.
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
# written into it: see packaging/hooks/links.sh.in. Written in rather than
# found when it runs, because bin is shared by every TCS package and one
# package's links must never touch another's.
define install_links
	install -d -m 755 $(DESTDIR)$(SBINDIR)
	$(SUBST) -e 's|@PACKAGE@|$(1)|g' -e 's|@PROGRAMS@|$(2)|g' \
		packaging/hooks/links.sh.in > $(DESTDIR)$(SBINDIR)/$(1)-links
	chmod 755 $(DESTDIR)$(SBINDIR)/$(1)-links
endef

# A template from packaging/ with the paths it is installed for put in.
SUBST = sed -e 's|@PREFIX@|$(PREFIX)|g' -e 's|@SYSCONFDIR@|$(SYSCONFDIR)|g' \
	    -e 's|@LOCALSTATEDIR@|$(LOCALSTATEDIR)|g' \
	    -e 's|@LIBTCS_VERSION@|$(LIBTCS_VERSION)|g'

# Build and install everything: all three packages, in one place.
#
# The build is cargo's release build of the crates that are installed, and
# no more, run before the install rather than as part of it: `make
# install-build` as the user and `sudo make install-files` installs into
# /opt/tcs without building anything as root.
.PHONY: install
install: install-build
	$(MAKE) --no-print-directory install-files

# A release build of everything that is installed.
#
# One cargo run for every crate, so that the dependencies they share are
# built once. tcspecial is built with its default features: every payload
# type and not the visual window, which never flies.
.PHONY: install-build
install-build:
	cargo build --release $(addprefix -p ,$(INSTALL_CRATES))

# Install all three packages from a build already made.
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
# which have the diagrams beside the configurations.
.PHONY: install-tcspecial
install-tcspecial:
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
	install -m 644 $(DOCS_DIR)/*.html $(DOCS_DIR)/*.svg \
		$(DESTDIR)$(DOCDIR)/$(PROJECT_NAME)/docs
	for file in $(EXAMPLE_FILES); do \
		install -D -m 644 $$file \
			$(DESTDIR)$(EXAMPLEDIR)/$${file#examples/} || exit 1; \
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

# What a ground station is built on: libtcs for C and C++, the Python
# packages, and the configuration editor with its data.
#
# libtcs.so is installed under its soname, libtcs.so.<ABI>, which is what a
# program built against it asks for at run time, and linked to by
# libtcs.so, which is what the linker looks for. tcs.h goes in include/
# itself, as <tcs.h>.
#
# The Python is installed by pip into a directory of its own, without
# compiling it, so that the path names no Python version and any python3 can
# read it; the .pth hook is what puts that directory on python3's path.
.PHONY: install-tcspecial-ground
install-tcspecial-ground: install-data
	@echo "Installing tcspecial-ground under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) $(DESTDIR)$(INCLUDEDIR) \
		$(DESTDIR)$(LIBDIR)/pkgconfig $(DESTDIR)$(PYTHONDIR)
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(GROUND_BIN)) \
		$(DESTDIR)$(BINDIR)
	install -m 644 tcslib-c/include/tcs.h $(DESTDIR)$(INCLUDEDIR)
	install -m 755 $(RELEASE_DIR)/libtcs.so \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME)
	ln -sf $(LIBTCS_SONAME) $(DESTDIR)$(LIBDIR)/libtcs.so
	install -m 644 $(RELEASE_DIR)/libtcs.a $(DESTDIR)$(LIBDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(GROUND_BIN)) \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME), \
		$(DESTDIR)$(LIBDIR)/libtcs.a)
	$(call install_links,tcspecial-ground,$(GROUND_BIN))
	$(SUBST) packaging/tcs.pc.in > $(DESTDIR)$(LIBDIR)/pkgconfig/tcs.pc
	chmod 644 $(DESTDIR)$(LIBDIR)/pkgconfig/tcs.pc
	python3 -m pip install --quiet --upgrade --no-deps \
		--no-build-isolation --no-compile \
		--target $(DESTDIR)$(PYTHONDIR) ./python
	install -d -m 755 $(DESTDIR)$(DOCDIR)/tcspecial-ground/tcslib-c \
		$(DESTDIR)$(DOCDIR)/tcspecial-ground/python
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcspecial-ground
	install -m 644 tcslib-c/README.html \
		$(DESTDIR)$(DOCDIR)/tcspecial-ground/tcslib-c
	install -m 644 python/README.md \
		$(DESTDIR)$(DOCDIR)/tcspecial-ground/python
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR) \
			$(DESTDIR)$(LDCONFDIR) $(DESTDIR)$(PTHDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcspecial-ground|g' \
			packaging/hooks/profile-ground.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcspecial-ground.sh; \
		echo $(LIBDIR) \
			> $(DESTDIR)$(LDCONFDIR)/tcspecial-ground.conf; \
		echo $(PYTHONDIR) \
			> $(DESTDIR)$(PTHDIR)/tcspecial-ground.pth; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcspecial-ground.sh \
			$(DESTDIR)$(LDCONFDIR)/tcspecial-ground.conf \
			$(DESTDIR)$(PTHDIR)/tcspecial-ground.pth; \
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
install-tcspecial-test:
	@echo "Installing tcspecial-test under $(DESTDIR)$(PREFIX)..."
	install -d -m 755 $(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(DOCDIR)/tcspecial-test
	install -m 755 $(addprefix $(RELEASE_DIR)/,$(TEST_BIN)) \
		$(DESTDIR)$(BINDIR)
	$(call strip_installed,$(addprefix $(DESTDIR)$(BINDIR)/,$(TEST_BIN)),)
	$(call install_links,tcspecial-test,$(TEST_BIN))
	install -m 644 README.md LICENSE-APACHE LICENSE-MIT \
		$(DESTDIR)$(DOCDIR)/tcspecial-test
	if [ "$(HOOKS)" = 1 ]; then \
		set -e; \
		install -d -m 755 $(DESTDIR)$(PROFILEDIR); \
		$(SUBST) -e 's|@PACKAGE@|tcspecial-test|g' \
			packaging/hooks/profile.sh.in \
			> $(DESTDIR)$(PROFILEDIR)/tcspecial-test.sh; \
		chmod 644 $(DESTDIR)$(PROFILEDIR)/tcspecial-test.sh; \
	fi

# Install tcsgui's data: the description tree and the schemas.
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
	@echo "✓ $(GUI_NAME) data installed"

# Take away what `make install` put in place, with the same PREFIX, DESTDIR
# and HOOKS it was given. The links `make links` made in LINKDIR go first,
# while the scripts that know which they are are still there, so that none
# is left pointing at a program that is gone.
#
# Not the configuration in /etc/opt/tcs/tcspecial or the state and telemetry
# in /var/opt/tcs/tcspecial, which are the mission's rather than this
# program's: their directories go only if they are empty. Directories under
# the prefix go when nothing else is in them, which leaves those another TCS
# package installed into.
.PHONY: uninstall
uninstall:
	-for package in $(PACKAGES); do \
		script=$(DESTDIR)$(SBINDIR)/$$package-links; \
		[ -x $$script ] && $$script --remove --dir $(LINKDIR); \
	done; true
	rm -f $(addprefix $(DESTDIR)$(BINDIR)/,$(FLIGHT_BIN) $(GROUND_BIN) \
		$(TEST_BIN))
	rm -f $(addprefix $(DESTDIR)$(LIBEXECDIR)/,$(FLIGHT_LIBEXEC))
	rm -f $(addprefix $(DESTDIR)$(SBINDIR)/,$(addsuffix -links,$(PACKAGES)))
	rm -f $(DESTDIR)$(INCLUDEDIR)/tcs.h
	rm -f $(DESTDIR)$(LIBDIR)/libtcs.so $(DESTDIR)$(LIBDIR)/libtcs.a \
		$(DESTDIR)$(LIBDIR)/$(LIBTCS_SONAME)
	rm -f $(DESTDIR)$(LIBDIR)/pkgconfig/tcs.pc
	rm -rf $(addprefix $(DESTDIR)$(PYTHONDIR)/,tcslibgs tcslib tcsui \
		tcspecial_ground-*.dist-info)
	rm -rf $(TCSGUI_DATA) $(DESTDIR)$(EXAMPLEDIR) \
		$(addprefix $(DESTDIR)$(DOCDIR)/,$(PACKAGES))
	if [ "$(HOOKS)" = 1 ]; then \
		rm -f $(addprefix $(DESTDIR)$(PROFILEDIR)/,$(addsuffix .sh, \
			$(PACKAGES))) \
			$(DESTDIR)$(LDCONFDIR)/tcspecial-ground.conf \
			$(DESTDIR)$(PTHDIR)/tcspecial-ground.pth \
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

# Link every package's programs into LINKDIR, or take the links away, for an
# install made with `make install`; the packages' own users run
# /opt/tcs/sbin/<package>-links themselves. Each package's script touches
# only its own programs, and a name already taken by something else is left
# alone and said. Usually as root, for /usr/local/bin:
#   sudo make links
#   sudo make unlinks LINKDIR=/opt/bin
LINKDIR ?= /usr/local/bin
.PHONY: links
links:
	status=0; \
	for package in $(PACKAGES); do \
		$(DESTDIR)$(SBINDIR)/$$package-links --dir $(LINKDIR) || \
			status=1; \
	done; \
	exit $$status

.PHONY: unlinks
unlinks:
	status=0; \
	for package in $(PACKAGES); do \
		$(DESTDIR)$(SBINDIR)/$$package-links --remove \
			--dir $(LINKDIR) || status=1; \
	done; \
	exit $$status

# Where the packages are built: build/, which git ignores.
#
# The stage is one directory per package, each installed into by itself
# with the paths the packages are for and the hooks on, so that a package is
# exactly what its install target installs. What pip and install -D made
# carries the builder's umask, so the stage is made writable by its owner
# alone: a package installs nothing that anyone but root may write.
STAGE := $(CURDIR)/build/stage
PACKAGE_DIR := $(CURDIR)/build/packages

# Install each package into the stage, from a fresh release build.
.PHONY: stage
stage: install-build
	rm -rf $(STAGE)
	for package in $(PACKAGES); do \
		$(MAKE) --no-print-directory install-$$package \
			DESTDIR=$(STAGE)/$$package PREFIX=/opt/tcs \
			SYSCONFDIR=/etc/opt/tcs LOCALSTATEDIR=/var/opt/tcs \
			HOOKS=1 || exit 1; \
	done
	chmod -R go-w $(STAGE)
	@echo "✓ Staged in $(STAGE)"

# The Debian packages, one per staged package: see packaging/deb.
#
# DEB_REVISION is the Debian revision, for a package rebuilt from the same
# source.
DEB_REVISION ?= 1
.PHONY: deb
deb: stage
	for package in $(PACKAGES); do \
		packaging/deb/build-deb.sh $$package $(STAGE)/$$package \
			$(CURDIR)/build/deb $(PACKAGE_DIR) \
			$(VERSION)-$(DEB_REVISION) \
			$$(dpkg --print-architecture) || exit 1; \
	done
	@echo "✓ .debs in $(PACKAGE_DIR)"

# The RPMs, one per staged package: see packaging/rpm.
#
# rpmbuild is asked first, before anything is built, so that a machine
# without it says so in one line and goes on: `make packages` there still
# builds the .debs. rpmbuild is given a package database of its own under
# build/, because it opens one even for a package that requires nothing to
# build, and a Debian machine has no system one it may read.
RPM_TOP := $(CURDIR)/build/rpm
.PHONY: rpm
rpm:
	@if ! command -v rpmbuild >/dev/null 2>&1; then \
		echo "rpmbuild is not installed, so no RPMs were built:" \
			"install it (sudo apt install rpm, or sudo dnf" \
			"install rpm-build) and run make rpm again"; \
		exit 0; \
	fi; \
	set -e; \
	$(MAKE) --no-print-directory stage; \
	rm -rf $(RPM_TOP); \
	mkdir -p $(PACKAGE_DIR); \
	for package in $(PACKAGES); do \
		rpmbuild -bb --quiet \
			--define "_topdir $(RPM_TOP)" \
			--define "_dbpath $(RPM_TOP)/rpmdb" \
			--define "_sourcedir $(CURDIR)/packaging/rpm" \
			--define "tcs_version $(VERSION)" \
			--define "tcs_stage $(STAGE)/$$package" \
			packaging/rpm/$$package.spec; \
	done; \
	cp $(RPM_TOP)/RPMS/*/*.rpm $(PACKAGE_DIR); \
	echo "✓ RPMs in $(PACKAGE_DIR)"

# Both kinds of package.
.PHONY: packages
packages: deb rpm

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
