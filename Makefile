# Makefile for automated Rust project creation with Claude Code

.PHONY: all setup generate build test clean install help

# Project variables
PROJECT_NAME := task-manager
SIM_NAME := simulator
SRC_DIR := src
DOCS_DIR := docs
PROMPTS_DIR := prompts

DESIGN=$(DOCS_DIR)/design.rst
TCSPECIAL = .
RUST = .

TCS_CODE = g, tcslib, tcslibgs
TCS_TEST = tcsmoc, tcssim, payload1.yaml, payload1sim.yaml
TCS_RUST = g-rust
TCS_TAR = $(TCS_RUST).tar.gz
TCS_OUTPUT = compressed tar file $(TCS_TAR)
PROMPT = Generate Rust code ($(TCS_CODE)) and tests ($(TCS_TEST)), and create $(TCS_OUTPUT) from $(DESIGN)

TCS_CRATES = tcslib tcslibgs g tcsmoc tcssim payload1.yaml payload1sim.yaml

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

FIXUP = set -x; \
		echo "Project fixup..."; \
		sed -i 's/into_raw_fd/as_raw_fd/g' g/src/endpoint.rs; \
		sed -i 's/into_raw_fd/as_raw_fd/g' g/src/dh.rs;
FIXUP =

FIXUP_TEST =
FIXUP_SIM =

# Default target
all: generate build test

# Display help
help:
	@echo "Makefile for Rust Project with Claude Code"
	@echo ""
	@echo "Usage:"
	@echo "  make all         - Generate, build, and test the project"
	@echo "  make generate    - Use Claude Code to generate project files"
	@echo "  make build       - Build the Rust project"
	@echo "  make buildmoc    - Build the MOC portion of the Rust project"
	@echo "  make buildsim    - Build the simulator portion of the Rust project"
	@echo "  make test        - Run all tests"
	@echo "  make run         - Run the application"
	@echo "  make runmoc      - Run the MOC application"
	@echo "  make runsim      - Run the simulation application"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make install     - Install the binary globally"

# Build the project
.PHONY: build
build:
	( \
		set -eu; \
		$(FIXUP) \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcspecial; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the project
.PHONY: buildmoc
buildmoc:
	( \
		set -eu; \
		$(FIXUP) \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcsmoc; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Build the project
.PHONY: buildsim
buildsim:
	( \
		set -eu; \
		$(FIXUP) \
		echo "Building the project..."; \
		cd $(RUST) && cargo build $(RELEASE) --bin tcssim; \
		echo "✓ Build complete" \
	) 2>&1 | tee build.out

# Run tests
.PHONY: test
test:
	( \
		set -eu; \
		$(FIXUP_TEST) \
		echo "Running tests..."; \
		cd $(RUST) && cargo test; \
		echo "✓ Tests complete"; \
	)

# Run the tcspecial application
.PHONY: run
run:
	( \
		set -eu; \
		$(FIXUP) \
		echo "Running $(PROJECT_NAME)..."; \
		cd $(RUST) && RUST_LOG=info cargo run --bin tcspecial \
	)

# Run the MOC application
.PHONY: runmoc
runmoc:
	( \
		set -eu; \
		$(FIXUP) \
		echo "Running $(PROJECT_NAME)..."; \
		cd $(RUST) && RUST_LOG=info PAYLOAD_SIM_YAML=$(PAYLOAD_SIM_YAML) cargo run --bin tcsmoc -- $(PAYLOAD_YAML) \
	)

# Run the simulation application
.PHONY: runsim
runsim:
	( \
		set -eu; \
		$(FIXUP_SIM) \
		echo "Running $(SIM_NAME)..."; \
		cd $(RUST) && RUST_LOG=info PAYLOAD_SIM_YAML=$(PAYLOAD_SIM_YAML) cargo run --bin tcssim \
	)

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	-cargo clean
	rm -f generate.out build.out run.out test.out $(TCS_TAR)
	rm -rf $(TCS_RUST) $(TCS_TAR)
	@echo "✓ Clean complete"


# Clean everything including generated source
.PHONY: distclean
distclean: clean
	@echo "Removing all generated files..."
	rm -f Cargo.lock Cargo.toml
	rm -rf $(TCS_CRATES)
	rm -rf target
	@echo "✓ Project reset"

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

# Run with example data
.PHONY: demo
demo: build
	@echo "Running demo..."
	cd $(RUST) && cargo run -- add "Buy groceries" --desc "Milk, eggs, bread"
	cd $(RUST) && cargo run -- add "Write documentation"
	cd $(RUST) && cargo run -- add "Deploy to production"
	cd $(RUST) && cargo run -- list
	cd $(RUST) && cargo run -- complete 1
	cd $(RUST) && cargo run -- list --pending
