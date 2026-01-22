# Makefile for cloudmon-metrics
# Rust project build, test, and quality automation

.PHONY: all build build-release build-convertor build-reporter \
        test test-verbose coverage coverage-html \
        fmt fmt-check lint lint-fix clean check \
        doc doc-serve doc-open doc-api help install-tools

# Binary names
CONVERTOR_BIN = cloudmon-metrics-convertor
REPORTER_BIN = cloudmon-metrics-reporter

# Default target
all: fmt-check lint test build

# ============================================================================
# Build targets
# ============================================================================

## Build all debug binaries
build:
	cargo build

## Build all release binaries (optimized)
build-release:
	cargo build --release

## Build only the convertor binary (debug)
build-convertor:
	cargo build --bin $(CONVERTOR_BIN)

## Build only the reporter binary (debug)
build-reporter:
	cargo build --bin $(REPORTER_BIN)

## Build only the convertor binary (release)
build-convertor-release:
	cargo build --release --bin $(CONVERTOR_BIN)

## Build only the reporter binary (release)
build-reporter-release:
	cargo build --release --bin $(REPORTER_BIN)

## Check code compiles without producing binaries (faster)
check:
	cargo check

# ============================================================================
# Test targets
# ============================================================================

## Run all tests
test:
	cargo test

## Run tests with verbose output
test-verbose:
	cargo test -- --nocapture

## Run tests with specific filter (usage: make test-filter FILTER=test_name)
test-filter:
	cargo test $(FILTER)

# ============================================================================
# Code coverage
# ============================================================================

## Run tests with coverage (requires cargo-tarpaulin)
coverage:
	cargo tarpaulin --lib --tests --exclude-files 'src/bin/*' --out Stdout --skip-clean

## Generate HTML coverage report
coverage-html:
	cargo tarpaulin --lib --tests --exclude-files 'src/bin/*' --out Html --output-dir target/coverage --skip-clean
	@echo "Coverage report generated at target/coverage/tarpaulin-report.html"

## Run coverage with 95% threshold enforcement (library code only)
coverage-check:
	cargo tarpaulin --lib --tests --exclude-files 'src/bin/*' --fail-under 95 --skip-clean

# ============================================================================
# Code formatting
# ============================================================================

## Format code using rustfmt
fmt:
	cargo fmt

## Check formatting without making changes
fmt-check:
	cargo fmt -- --check

# ============================================================================
# Linting
# ============================================================================

## Run clippy linter with warnings as errors
lint:
	cargo clippy -- -D warnings

## Run clippy and automatically fix warnings where possible
lint-fix:
	cargo clippy --fix --allow-dirty --allow-staged

# ============================================================================
# Documentation
# ============================================================================

## Build mdbook documentation
doc:
	mdbook build doc/

## Serve documentation locally with live reload
doc-serve:
	mdbook serve doc/

## Open documentation in browser
doc-open:
	mdbook build doc/ --open

## Generate Rust API documentation
doc-api:
	cargo doc --no-deps

## Generate and open Rust API documentation in browser
doc-api-open:
	cargo doc --no-deps --open

## Clean generated documentation
doc-clean:
	rm -rf docs/*

# ============================================================================
# Cleanup
# ============================================================================

## Clean build artifacts
clean:
	cargo clean

## Clean and remove Cargo.lock (full clean)
clean-all: clean
	rm -f Cargo.lock

# ============================================================================
# Development helpers
# ============================================================================

## Install required development tools
install-tools:
	rustup component add rustfmt clippy
	cargo install cargo-tarpaulin
	cargo install mdbook mdbook-mermaid mdbook-linkcheck

## Run all quality checks (CI simulation)
ci: fmt-check lint test coverage-check

## Watch for changes and run tests (requires cargo-watch)
watch:
	cargo watch -x test

## Update dependencies
update:
	cargo update

# ============================================================================
# Help
# ============================================================================

## Show this help message
help:
	@echo "Available targets:"
	@echo ""
	@echo "  Build:"
	@echo "    build                  - Build all debug binaries"
	@echo "    build-release          - Build all release binaries (optimized)"
	@echo "    build-convertor        - Build convertor binary (debug)"
	@echo "    build-reporter         - Build reporter binary (debug)"
	@echo "    build-convertor-release - Build convertor binary (release)"
	@echo "    build-reporter-release - Build reporter binary (release)"
	@echo "    check                  - Check code compiles without producing binaries"
	@echo ""
	@echo "  Test:"
	@echo "    test           - Run all tests"
	@echo "    test-verbose   - Run tests with verbose output"
	@echo "    test-filter    - Run tests matching FILTER (usage: make test-filter FILTER=name)"
	@echo ""
	@echo "  Coverage:"
	@echo "    coverage       - Run tests with coverage report"
	@echo "    coverage-html  - Generate HTML coverage report"
	@echo "    coverage-check - Run coverage with 95% threshold"
	@echo ""
	@echo "  Code Quality:"
	@echo "    fmt            - Format code"
	@echo "    fmt-check      - Check code formatting"
	@echo "    lint           - Run clippy linter"
	@echo "    lint-fix       - Fix linter warnings automatically"
	@echo ""
	@echo "  Documentation:"
	@echo "    doc            - Build mdbook documentation"
	@echo "    doc-serve      - Serve documentation locally with live reload"
	@echo "    doc-open       - Build and open documentation in browser"
	@echo "    doc-api        - Generate Rust API documentation"
	@echo "    doc-api-open   - Generate and open Rust API docs in browser"
	@echo "    doc-clean      - Clean generated documentation"
	@echo ""
	@echo "  Utilities:"
	@echo "    clean          - Clean build artifacts"
	@echo "    clean-all      - Clean everything including Cargo.lock"
	@echo "    install-tools  - Install required development tools"
	@echo "    ci             - Run all CI checks (fmt, lint, test, coverage)"
	@echo "    watch          - Watch for changes and run tests"
	@echo "    update         - Update dependencies"
	@echo ""
	@echo "  Default (all):"
	@echo "    fmt-check lint test build"
