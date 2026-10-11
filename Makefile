# Makefile for TCSpecial

.PHONY: all build test clean install help

# Project variables
PROJECT_NAME := tcspecial
MOC_NAME := tcsmoc
SIM_NAME := tcssim
DOCS_DIR := docs

DESIGN=$(DOCS_DIR)/design.rst
RUST = .

# The payload set to run, named without a suffix.
#
# One variable rather than two, because the two files of a set are named for
# each other: TCSPECIAL_CONFIG_FILE is the stem,
# `$(TCSPECIAL_CONFIG_FILE).yaml` the tcspecial configuration -- in YAML or
# XML -- and `$(TCSPECIAL_CONFIG_FILE)sim.yaml` the simulator settings beside
# it. So:
#   make run     TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2
#   make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2
#   make runsim  TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2
#
# Not TCSPECIAL_CONFIG_PATH, which is the environment variable tcspecial and
# tcsmoc read for the command interpreter's own settings: this names a set to
# run and is nothing the programs look for.
#
# Every program takes the configuration file as a command line argument. The
# simulator file reaches tcssim through the environment: runmocy passes it
# because tcsmoc hands its own configuration to the tcspecial and tcssim it
# starts but never reads the simulation file, and runsim passes it because
# tcssim run on its own needs both, and the two must describe the same set or
# the names will not match.
#
# The sets live under tests/manual because running one is something a person
# does at a terminal: each opens sockets or device nodes.
TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial2

# What the programs log, which every run target passes on. A variable rather
# than a word in each recipe, so that it can be overridden the way the payload
# set is:
#   make runmocy RUST_LOG=debug
#   make run TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1 RUST_LOG=tcspecial::ci=trace
RUST_LOG = info

RELEASE = --release
RELEASE =

# Default target
all: build test

# Display help
help:
	@echo "Makefile for TCSpecial"
	@echo ""
	@echo "Usage:"
	@echo "  make all         - Build and test the project"
	@echo "  make build       - Build tcspecial"
	@echo "  make buildmoc    - Build tcsmoc"
	@echo "  make buildsim    - Build tcssim"
	@echo "  make test        - Run all tests"
	@echo "  make run         - Run tcspecial"
	@echo "  make runmocy     - Run tcsmoc, which starts tcspecial and tcssim"
	@echo "  make runmocx     - As runmocy, on the XML configuration"
	@echo "  make runsim      - Run tcssim alone"
	@echo "  make runverifyy  - Check the payload set, reading its YAML"
	@echo "  make runverifyx  - Check the payload set, reading its XML"
	@echo "  make mkvcan      - Make the virtual CAN bus a CAN payload needs"
	@echo "  make mkstubs     - Make the I2C and GPIO stand-ins their payloads need"
	@echo "  make check       - cargo check, clippy, and a format check"
	@echo "  make format      - Reformat the source"
	@echo "  make release     - Build with optimizations"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make distclean   - Remove everything that can be rebuilt"
	@echo "  make install     - Install tcspecial globally"
	@echo ""
	@echo "Payload set: make runmocy TCSPECIAL_CONFIG_FILE=tests/manual/tcspecial1"
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
		cd $(RUST) && RUST_LOG=$(RUST_LOG) cargo run -q --bin tcspecial -- $(TCSPECIAL_CONFIG_FILE).yaml \
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
		echo "Running $(MOC_NAME) on $(basename $(TCSPECIAL_CONFIG_FILE)).$(1)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) PAYLOAD_SIM_YAML=$(TCSPECIAL_CONFIG_FILE)sim.yaml cargo run -q --bin tcsmoc -- $(basename $(TCSPECIAL_CONFIG_FILE).$(1)).$(1) \
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
		cd $(RUST) && cargo run -q --bin tcsverify -- $(basename $(TCSPECIAL_CONFIG_FILE).$(1)).$(1) $(TCSPECIAL_CONFIG_FILE)sim.yaml \
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

# Run the simulation application
.PHONY: runsim
runsim:
	( \
		set -eu; \
		echo "Running $(SIM_NAME)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) PAYLOAD_SIM_YAML=$(TCSPECIAL_CONFIG_FILE)sim.yaml cargo run -q --bin tcssim -- $(TCSPECIAL_CONFIG_FILE).yaml \
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

# Make the I2C and GPIO stand-ins their payloads need.
#
# The two kinds the simulator cannot be the far end of: a device on a bus has
# to answer when a master addresses it and a discrete line is a wire, and user
# space is neither. The kernel's i2c-stub and gpio-sim stand in -- a bank of
# registers and a chip whose lines are memory -- and bin/mkstubs makes both.
#
# Not a prerequisite of anything, for the reasons mkvcan is not, and for one
# more: unlike the CAN tests, no test is gated on these. The scans that find a
# stub are tested against a tree handed to them rather than the real one, so
# they pass with no module loaded. This is for running a payload set that names
# an I2C or GPIO payload.
#
# It prints the bus and chip numbers it made, which are assigned as devices
# appear and so are not the same from one run to the next.
.PHONY: mkstubs
mkstubs:
	bin/mkstubs

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
# with cargo install --path tcsmoc, if that is wanted.
#
# Note that tcspecial finds its configuration by relative path -- tcspecial1.yaml
# and tcspecial/src/tcspecial.yaml -- so an installed copy run from elsewhere
# needs PAYLOAD_CONFIG_PATH and TCSPECIAL_CONFIG_PATH set, or a working
# directory that has those files.
.PHONY: install
install:
	@echo "Installing $(PROJECT_NAME)..."
	cargo install --path $(PROJECT_NAME)

# Check code quality
.PHONY: check
check:
	@echo "Running cargo check..."
	cd $(RUST) && cargo check
	cd $(RUST) && cargo clippy -- -D warnings
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
