# Changelog

All notable changes to `fallow-cov-protocol` are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Pre-1.0 minor bumps may still contain breaking changes; see `CLAUDE.md` and
`.claude/rules/protocol-versioning.md` for the full policy.

## [Unreleased]

### Added

- Workspace-wide lint configuration bringing parity with the main `fallow`
  crate: `clippy::{all, pedantic, nursery, cargo}` at `warn`, targeted
  restriction lints, expanded `[lints.rust]` block, and `missing_docs = "warn"`.
- `.clippy.toml` with thresholds for excessive nesting, unit size, cognitive
  complexity, and parameter count.
- `rust-toolchain.toml` pinning the MSRV toolchain (1.75.0).
- `deny.toml` with permissive-only license allow-list and wildcard ban.
- `_typos.toml`.
- `.github/workflows/ci.yml` covering test / clippy / fmt / doc / typos /
  audit / deny / shear / zizmor / MSRV / publish dry-run on a three-OS matrix,
  with a deny-all permissions baseline and pinned action SHAs.
- `release.toml` locking the `cargo-release` mechanical knobs (signed
  commits + tags, `PROTOCOL_VERSION` replacement).
- Rustdoc on every public item (fields, variants, helpers).
- Forward-compat round-trip test for `Watermark::Unknown`.
- `finding_id_is_lowercase_hex_ascii` canonical-form test.

### Changed

- `content_hash` no longer uses `let _ = write!(...)` to build the hex
  prefix; replaced with a byte-level encoder (`hex_prefix`). The wire output
  is bit-identical — canonical input order, SHA-256, and 8-character
  truncation are unchanged.

## [0.2.0] — 2026-04-17

### Added (breaking)

- Stable `Finding::id` / `HotPath::id` hashes via `finding_id` / `hot_path_id`
  helpers (`fallow:prod:<hash>` / `fallow:hot:<hash>`, SHA-256 truncated to
  8 hex characters over `file + function + line + kind`).
- Per-finding `Verdict` enum (`safe_to_delete`, `review_required`,
  `coverage_unavailable`, `low_traffic`, `active`) replacing 0.1's
  `CallState`.
- Full `Evidence` block on each finding, mirroring the decision table in
  `spec-production-coverage.md`.
- `Finding::invocations` as `Option<u64>` (nullable when V8 did not track
  the function).
- `Confidence::VeryHigh` and `Confidence::None` variants.
- `#[serde(other)] Unknown` sentinel on every enum that can grow variants
  (`ReportVerdict`, `Verdict`, `Confidence`, `Feature`, `Watermark`).

### Changed (breaking)

- Top-level report verdict renamed: `Verdict` → `ReportVerdict`. Per-finding
  `Verdict` is the new `Verdict` type.
- `StaticFunction::static_used` and `StaticFunction::test_covered` are now
  required; 0.1-shape requests that omit them fail deserialization by
  design.

## [0.1.0] — 2026-04-15

### Added

- Initial `Request` / `Response` envelope, `CoverageSource`,
  `StaticFindings`, `Summary`, `Finding`, `HotPath`, `Feature`, and
  supporting enums.
- `PROTOCOL_VERSION` constant.
- Full test suite covering forward-compat, serde casing, and round-trip
  behaviour.
