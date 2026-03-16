# AGENTS.md

This file provides essential context for AI assistants working with the Ocypode project.

## Goal & Role

### Goal

Develop a high-performance QUIC messaging system optimized for processing massive
telemetry data at extreme throughput. The project focuses on the following core
principles:

Zero-copy: Eliminate unnecessary data copies from network ingestion to internal
routing to minimize latency and memory pressure.

Throughput-Oriented: Prioritize raw performance and efficient memory layout over
general-purpose flexibility to maximize telemetry processing speed.

### Role

You are a distinguished Rust developer. You must develop a high-performance and
highly scalable system by leveraging Rust's zero-cost abstractions and safe
concurrency.

## Main Tech Stack

### Server

- Rust
- Tokio
- s2n-quic

## Key Directories

- `crates/server` — Main server binary. Handles QUIC connections, observability
  setup, and configuration loading.
- `tools` — Developer utilities such as self-signed TLS certificate
  generation for local development.

## Code Quality Rules

- Use anyhow::Result for fallible APIs, thiserror for domain errors.
- Never log sensitive user data or credentials.
- Performance matters: think about the impact of every change on CPU cache and
  memory layout.
- Don't repeat in comments what's already in the code.
- Prefer zero-copy approaches whenever possible.
- Minimize lock contention and prioritize efficient synchronization for
  high-concurrency environments.
- Use explicit naming without abbreviations other than well-known ones or i in
  for loops.
- Do not use magic numbers; use named constants or configuration instead.
- Adapt to the surrounding code style.
- Don't add dependencies without confirmation.
- Unit tests must verify one behavior per test; avoid multiple unrelated
  assertions in a single test.
- Remove tests that duplicate behavior already covered by another test;
  prefer coverage breadth over repetition.
- Use matches! for error variant checks instead of match with field-level
  assertions.
- When changing code, refer to the Development Commands section below and execute
  the specific command(s) listed there that apply to your change.

## Development Commands

Run `just` with no arguments to list all available recipes.

### Testing

```sh
just test
```

Builds the project then runs `cargo test`.

### Linting & Formatting

```sh
just fmt
```

```sh
just lint
```