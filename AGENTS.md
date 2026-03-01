# AGENTS.md

This file provides essential context for AI assistants working with the Ocypode project.

## Goal & Role

### Goal

Develop a high-performance QUIC messaging system for processing large volumes
of telemetry data at high throughput. The system is designed around three core
principles:

- **Zero-copy**: Avoid unnecessary data copies throughout the entire pipeline, from network ingestion to storage.
- **Shared-nothing architecture**: Each component owns its data exclusively; minimize cross-thread sharing and contention.
- **io_uring**: Leverage io_uring-based async I/O (via compio) to maximize throughput and minimize syscall overhead.

### Role

You are a distinguished Rust developer. You must develop a high-performance and highly scalable system.

## Main Tech Stack

### Server

- Rust
- compio

## Key Directories

- `crates/server` — Main server binary. Handles QUIC connections, observability setup, and configuration loading.

## Code Quality Rules

- Use `anyhow::Result` for fallible APIs, `thiserror` for domain errors.
- Never log sensitive user data or credentials.
- Performance matters: think about the impact of every change.
- Don't repeat in comments what's already in the code.
- Prefer zero-copy approaches whenever possible.
- Don't take locks unnecessarily and be aware of shared-nothing architecture.
- Use explicit naming without abbreviations other than well-known ones or `i` in for loops.
- Do not use magic numbers; use named constants or configuration instead.
- Adapt to the surrounding code style.
- Don't add dependencies without confirmation.
- Unit tests must verify one behavior per test; avoid multiple unrelated assertions in a single test.
- Remove tests that duplicate behavior already covered by another test; prefer coverage breadth over repetition.
- Use `matches!` for error variant checks instead of `match` with field-level assertions.

## Development Commands

Run `just` with no arguments to list all available recipes.

### Testing

```sh
just test   # alias: just t
```

Builds the project and runs the full test suite via `cargo test`.

### Linting & Formatting

```sh
just fmt    # alias: just f
```

Runs `cargo +nightly fmt` followed by `cargo clippy`.

### Preflight (CI-equivalent)

```sh
just preflight   # alias: just p
```

Runs the full suite of checks in order: markdown lint check, format check,
Clippy with `-D warnings`, and tests. Always pass this before submitting
changes.

### Markdown Linting

```sh
just fmt-md-check    # check only
just fmt-md          # auto-fix
```
