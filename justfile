alias b := build
alias t := test
alias f := fmt
alias p := preflight

_default:
    @just --list

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

# Lint markdown with auto-fix
fmt-md:
    bunx markdownlint-cli2 --fix "**/*.md" "**/*.mdx"

# Lint markdown (check only)
fmt-md-check:
    bunx markdownlint-cli2 "**/*.md" "**/*.mdx"

# Run preflight checks
preflight:
    @echo "Preflight check..."
    @just fmt-md-check
    @just fmt-check
    @just test