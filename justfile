_default:
    @just --list

# Format Rust Code
fmt:
    cargo +nightly fmt

# Lint Rust Code
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Fix lint Rust Code
lint-fix:
    cargo clippy --all-targets --all-features --fix --allow-dirty
