_default:
    @just --list

# Format Rust Code
fmt:
    cargo +nightly fmt

# Lint Rust Code
lint:
    cargo clippy

# Fix lint Rust Code
lint-fix:
    cargo clippy --fix --allow-dirty
