# Makefile for TCSpecial

.PHONY: all build test clean install help

# Project variables
PROJECT_NAME := tcspecial
MOC_NAME := tcsmoc
SIM_NAME := tcssim
DOCS_DIR := docs

DESIGN=$(DOCS_DIR)/design.rst
RUST = .

# The payload set to run. tcsmoc takes its payload file as a command line
# argument and tcssim takes its simulation file in the environment, so run
# another set with
#   make runmoc PAYLOAD_YAML=payload2.yaml PAYLOAD_SIM_YAML=payload2sim.yaml
#   make runsim PAYLOAD_SIM_YAML=payload2sim.yaml
#
# runmoc passes both: tcsmoc hands its own payload file to the tcspecial and
# tcssim it starts, but it never reads the simulation file, so that one reaches
# tcssim by being in tcsmoc's environment.
PAYLOAD_YAML = payload1.yaml
PAYLOAD_SIM_YAML = payload1sim.yaml

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
	@echo "Payload set: make runmoc PAYLOAD_YAML=payload2.yaml PAYLOAD_SIM_YAML=payload2sim.yaml"

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
		cd $(RUST) && RUST_LOG=info cargo run --bin tcspecial \
	)

# Run the MOC application
.PHONY: runmoc
runmoc:
	( \
		set -eu; \
		echo "Running $(MOC_NAME)..."; \
		cd $(RUST) && RUST_LOG=info PAYLOAD_SIM_YAML=$(PAYLOAD_SIM_YAML) cargo run --bin tcsmoc -- $(PAYLOAD_YAML) \
	)

# Run the simulation application
.PHONY: runsim
runsim:
	( \
		set -eu; \
		echo "Running $(SIM_NAME)..."; \
		cd $(RUST) && RUST_LOG=info PAYLOAD_SIM_YAML=$(PAYLOAD_SIM_YAML) cargo run --bin tcssim \
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

# Install binary globally
.PHONY: install
install: build
	@echo "Installing $(PROJECT_NAME)..."
	cd $(RUST) && cargo install --path .
	@echo "✓ Installed to ~/.cargo/bin/$(PROJECT_NAME)"

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

