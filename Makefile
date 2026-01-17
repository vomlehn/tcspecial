# Makefile for automated Rust project creation with Claude Code

.PHONY: all setup generate build test clean install help

# Project variables
PROJECT_NAME := task-manager
SRC_DIR := src
DOCS_DIR := docs
PROMPTS_DIR := prompts

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
	@echo "  make test        - Run all tests"
	@echo "  make run         - Run the application"
	@echo "  make clean       - Remove build artifacts"
	@echo "  make install     - Install the binary globally"
	@echo "  make setup       - Initial setup (create directories)"

# Create necessary directories
setup:
	@echo "Setting up project structure..."
	@mkdir -p $(DOCS_DIR) $(PROMPTS_DIR)
	@echo "✓ Directories created"

# Generate project using Claude Code
generate:
	@echo "Generating project with Claude Code..."
	@if [ ! -f "$(DOCS_DIR)/design.rst" ]; then \
		echo "Error: design.rst not found in $(DOCS_DIR)/"; \
		exit 1; \
	fi
	@claude --non-interactive < $(PROMPTS_DIR)/create-project.txt
	@echo "✓ Project files generated"

# Alternative: Use echo to pipe commands
generate-alt:
	@echo "Generating project with Claude Code (alternative method)..."
	@echo "Read docs/design.rst and create a complete Rust project with:\n\
	1. Cargo.toml with dependencies (serde, serde_json, chrono, clap)\n\
	2. All source files: main.rs, task.rs, storage.rs, cli.rs\n\
	3. Unit tests in each module\n\
	4. Integration tests\n\
	5. README.md and .gitignore\n\
	Generate all files without confirmation." | claude --model claude-sonnet-4-5-20250929

# Build the project
build:
	@echo "Building the project..."
	@cargo build --release
	@echo "✓ Build complete"

# Run tests
test:
	@echo "Running tests..."
	@cargo test
	@echo "✓ Tests complete"

# Run the application
run:
	@echo "Running $(PROJECT_NAME)..."
	@cargo run -- list

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@rm -f tasks.json
	@echo "✓ Clean complete"

# Clean everything including generated source
distclean: clean
	@echo "Removing all generated files..."
	@rm -rf $(SRC_DIR) tests Cargo.toml Cargo.lock README.md .gitignore
	@echo "✓ Project reset"

# Install binary globally
install: build
	@echo "Installing $(PROJECT_NAME)..."
	@cargo install --path .
	@echo "✓ Installed to ~/.cargo/bin/$(PROJECT_NAME)"

# Check code quality
check:
	@echo "Running cargo check..."
	@cargo check
	@cargo clippy -- -D warnings
	@cargo fmt -- --check

# Format code
format:
	@cargo fmt

# Create release build
release: test
	@echo "Creating release build..."
	@cargo build --release
	@echo "✓ Release binary: target/release/$(PROJECT_NAME)"

# Run with example data
demo: build
	@echo "Running demo..."
	@cargo run -- add "Buy groceries" --desc "Milk, eggs, bread"
	@cargo run -- add "Write documentation"
	@cargo run -- add "Deploy to production"
	@cargo run -- list
	@cargo run -- complete 1
	@cargo run -- list --pending
