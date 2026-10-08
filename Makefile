# Makefile for TCSpecial

.PHONY: all build test clean install help

# Project variables
PROJECT_NAME := tcspecial
MOC_NAME := tcsmoc
SIM_NAME := tcssim
DOCS_DIR := docs

DESIGN=$(DOCS_DIR)/design.rst
RUST = .

# The payload set to run.
#
# PAYLOAD_YAML names the file defining the payloads, which may be written as a
# payload configuration or as an endpoint configuration; every program takes it
# as a command line argument. PAYLOAD_SIM_YAML names the simulator settings,
# which only tcssim reads and which it takes from the environment. So:
#   make run    PAYLOAD_YAML=payload2.yaml
#   make runmoc PAYLOAD_YAML=payload2.yaml PAYLOAD_SIM_YAML=payload2sim.yaml
#   make runsim PAYLOAD_YAML=payload2.yaml PAYLOAD_SIM_YAML=payload2sim.yaml
#
# runmoc passes both because tcsmoc hands its own payload file to the tcspecial
# and tcssim it starts, but never reads the simulation file, so that one
# reaches tcssim by being in tcsmoc's environment.
#
# runsim passes both for a different reason: run on its own, tcssim needs the
# payload file as well as the simulation file, and the two must describe the
# same set or the names will not match.
PAYLOAD_YAML = payload2.yaml
PAYLOAD_SIM_YAML = payload2sim.yaml

# What the programs log, which every run target passes on. A variable rather
# than a word in each recipe, so that it can be overridden the way the payload
# set is:
#   make runmoc RUST_LOG=debug
#   make run PAYLOAD_YAML=payload1.yaml RUST_LOG=tcspecial::ci=trace
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
	@echo "  make runmoc      - Run tcsmoc, which starts tcspecial and tcssim"
	@echo "  make runsim      - Run tcssim alone"
	@echo "  make check       - cargo check, clippy, and a format check"
	@echo "  make format      - Reformat the source"
	@echo "  make release     - Build with optimizations"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make distclean   - Remove everything that can be rebuilt"
	@echo "  make install     - Install tcspecial globally"
	@echo ""
	@echo "Payload set: make runmoc PAYLOAD_YAML=payload1.yaml PAYLOAD_SIM_YAML=payload1sim.yaml"
	@echo "Logging:     make runmoc RUST_LOG=debug"

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
		cd $(RUST) && RUST_LOG=$(RUST_LOG) cargo run --bin tcspecial -- $(PAYLOAD_YAML) \
	)

# Run the MOC application
.PHONY: runmoc
runmoc:
	( \
		set -eu; \
		echo "Running $(MOC_NAME)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) PAYLOAD_SIM_YAML=$(PAYLOAD_SIM_YAML) cargo run --bin tcsmoc -- $(PAYLOAD_YAML) \
	)

# Run the simulation application
.PHONY: runsim
runsim:
	( \
		set -eu; \
		echo "Running $(SIM_NAME)..."; \
		cd $(RUST) && RUST_LOG=$(RUST_LOG) PAYLOAD_SIM_YAML=$(PAYLOAD_SIM_YAML) cargo run --bin tcssim -- $(PAYLOAD_YAML) \
	)

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
# Note that tcspecial finds its configuration by relative path -- payload1.yaml
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

