# fallow-cov-protocol

[![Crates.io](https://img.shields.io/crates/v/fallow-cov-protocol.svg)](https://crates.io/crates/fallow-cov-protocol)
[![Docs.rs](https://docs.rs/fallow-cov-protocol/badge.svg)](https://docs.rs/fallow-cov-protocol)

Versioned JSON envelope types shared between the public [`fallow`](https://github.com/fallow-rs/fallow) CLI and the closed-source `fallow-cov` production-coverage sidecar.

## Why this crate exists

Fallow Runtime, the paid runtime intelligence layer for fallow, is a two-binary architecture. The open-source [`fallow`](https://github.com/fallow-rs/fallow) CLI handles static analysis and IO. A separate closed-source sidecar (`fallow-cov`) does V8-to-Istanbul normalization, three-state tracking, and combined scoring. Both sides marshal data through JSON on stdin/stdout. This crate is the single source of truth for that envelope so the two repositories cannot drift.

## Versioning

The `protocol_version` field is a full semver string. Consumers MUST reject mismatched majors and SHOULD forward-accept unknown fields and unknown enum variants (mapped to `Unknown` sentinel values).

## Status

Early access. Breaking changes until `1.0.0`.

## License

MIT OR Apache-2.0
