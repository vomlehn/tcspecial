# Makefile for TCSpecial

# Project variables
PROJECT_NAME := tcspecial
MOC_NAME := tcsmoc
SIM_NAME := tcssim
GUI_NAME := tcsgui
DOCS_DIR := docs

DESIGN=$(DOCS_DIR)/design.rst
RUST = .

# Where installing puts the files that are not programs.
#
# tcsgui reads two kinds of data at run time: the description tree, which says
# what vehicle nodes there are, and the schemas for the configuration and
# simulator files. Both are architecture-independent data belonging to one
# package, which the filesystem hierarchy standard puts under
# `<prefix>/share/<package>` -- so `/usr/local/share/tcsgui` by default, which
# is the prefix for software built and installed locally, and
# `/usr/share/tcsgui` for a distribution's own package:
#
#   make install-data                      # /usr/local/share/tcsgui
#   make install-data PREFIX=/usr          # /usr/share/tcsgui
#   make install-data DESTDIR=/tmp/package # staged, for building a package
#
# DESTDIR is prepended to everything and is not part of the path the program
# looks in, which is what lets a package be built in a staging directory. See
# `tcsgui::nodes::data_dir` for where the editor looks and in what order.
PREFIX ?= /usr/local
DESTDIR ?=
TCSGUI_DATA := $(DESTDIR)$(PREFIX)/share/$(GUI_NAME)

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
	@echo "  make install     - Install tcspecial globally, data and all"
	@echo "  make install-data - Install tcsgui's descriptions and schemas"
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
		echo "✓ Tests complete"; \
	)

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
	examples/beacon/beacon:examples/beacon/beacon-sim.yaml

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

# Install the flight software binary.
#
# The package has to be named. The workspace root is a virtual manifest, with
# no package of its own, so installing from there fails outright:
#   error: found a virtual manifest at Cargo.toml instead of a package manifest
#
# No dependency on build, either: cargo install does its own release build, so
# depending on build would compile everything in debug first and throw it away.
#
# Where the binary lands is cargo's business -- ~/.cargo/bin unless
# CARGO_INSTALL_ROOT or --root says otherwise -- and cargo prints it, so this
# does not claim a path of its own.
#
# tcsmoc and tcssim are test software and are not installed. Either can be,
# with cargo install --path tcsmoc, if that is wanted. So are app_payload_veh,
# app_moc_veh, app_all_veh, app_pl and app_all_pl, which are what the five app
# node types are tested with: a mission's own app is its own program,
# installed wherever its node's `executable` attribute says.
#
# Note that tcspecial falls back to a configuration named by relative path --
# tests/manual/thermostat.yaml -- so an installed copy run from elsewhere
# needs the file named on its command line, or PAYLOAD_CONFIG_PATH set, or a
# working directory that has it.
.PHONY: install
install: install-data
	@echo "Installing $(PROJECT_NAME)..."
	cargo install --path $(PROJECT_NAME)

# Install tcsgui's data: the description tree and the schemas.
#
# The whole of tcsgui/share, with its structure: the description tree's shape
# is part of what it means -- a description at vehicle/compute/vote.yaml is
# about the Rust at node/vehicle/compute/vote.rs -- so a flat copy would lose
# half of it.
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
