alias b := build
alias t := test
alias f := fmt
alias p := preflight
alias prof := profile

_default:
    @just --list

# Audit fmt-check
audit:
    @echo "Audit check..."
    @cargo deny check

# Build the project
build:
    @echo "Building..."
    @cargo build

# Format and linter
fmt:
    @echo "Formatting..."
    @cargo +nightly fmt
    @echo "Running lints..."
    @cargo clippy

# Format and linter check
fmt-check:
    cargo +nightly fmt --check
    cargo clippy --all-targets --all-features -- -D warnings

# Run tests
test: build
    @echo "Running tests..."
    @cargo test

# Unused dependency check
unused:
    @echo "Unused dependency check"
    @cargo machete

# Lint markdown with auto-fix
fmt-md:
    bunx markdownlint-cli2 --fix "**/*.md" "**/*.mdx"

# Lint markdown (check only)
fmt-md-check:
    bunx markdownlint-cli2 "**/*.md" "**/*.mdx"

# Run with CPU profiling enabled (Ctrl-C to stop and generate reports)
profile:
    @echo "Building with profiling enabled..."
    @cargo build --features profiling
    @echo "Running with profiler... Press Ctrl-C to stop and generate flamegraph.svg + profile.pb"
    @cargo run --features profiling

# Generate TLS certificates for tests and dev
generate_certs:
    @echo "Generating TLS certificates..."
    @bash crates/tests/certs/generate_certs.sh

# Run server in dev mode with dev-config.yaml
dev:
    @echo "Starting server in dev mode..."
    @cargo run -p server -- --config crates/tests/configs/dev-config.yaml

# Run preflight checks
preflight:
    @echo "Preflight check..."
    @just fmt-md-check
    @just fmt-check
    @just unused
    @just audit
    @just generate_certs
    @just test
