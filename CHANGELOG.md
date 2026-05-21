# Changelog

All notable changes to `fallow-cov-protocol` are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Pre-1.0 minor bumps may still contain breaking changes; see `CLAUDE.md` and
`.claude/rules/protocol-versioning.md` for the full policy.

## [0.7.1] - 2026-05-21

### Changed

- **MSRV bumped from `1.75` to `1.85`.** Required to consume `sha2 = "0.11"`, whose transitive deps (`hybrid-array`, `cpufeatures`, `block-buffer`, `crypto-common`, `digest`) all declare `edition = "2024"`. Both real consumers of this crate (the `fallow` CLI and the `fallow-cov` sidecar) are already on Rust 1.92, so the bump has no practical downstream impact. Updated `Cargo.toml`, `rust-toolchain.toml`, the `msrv` CI job, and the in-repo docs in lockstep.
- Bumped `sha2` from `0.10` to `0.11`. No public API change; the hash inputs and `source_hash_for` output format are unchanged.

## [0.7.0] - 2026-05-21

### Changed (additive producer constraint)

- **`FunctionIdentity::source_hash` format is now pinned**, closing the
  cross-producer non-comparability gap that issue #3 surfaced. The field
  was previously documented as "opaque, producer-defined", which let each
  producer pipeline (`fallow-v8-coverage`, `oxc_coverage_v8`, the Istanbul
  ingester, browser / node beacons) emit incompatible hash formats
  (`sha256-hex`, `xxh3-base64`, `blake3-truncated`) and rendered the
  tiebreaker useless across pipelines.

  Pinned format (added in protocol 0.7.0, MUST hold across producers):
  the first 8 bytes of `SHA-256(<canonical body bytes>)` rendered as 16
  lowercase hex characters. Canonical body bytes are the bytes the
  producing compiler or parser sees for the function, including the
  signature line and the closing brace, with NO whitespace
  normalization. Producers that cannot canonicalize the bytes the same
  way as their siblings MUST omit the field rather than emit a
  divergent format.

  Migration for producers shipping a non-conforming `source_hash` today:
  switch the producer to the new `source_hash_for` helper in this crate,
  or omit the field. No wire shape change for consumers; the field stays
  `Option<String>`.

### Added

- **`source_hash_for(body: &[u8]) -> String`** helper computing the
  canonical `FunctionIdentity::source_hash` value. Reuses the existing
  `sha2` dependency. No new transitive deps. Producers MUST route every
  `source_hash` value through this helper so cross-producer agreement
  holds by construction.
- **Anchor fixture** `source_hash_for_anchor_fixture` pinning
  `source_hash_for(b"function foo() { return 1; }")` to the literal
  string `"74846e29a52fe863"`. Producers self-test against this fixture
  in their own CIs to detect divergence at the source rather than at
  cross-surface join time.
- **`function_identity_full_json_shape_anchor_fixture`** and
  **`function_identity_minimal_json_shape_anchor_fixture`** locking the
  byte-level JSON output of `FunctionIdentity` for the every-field-set
  and the minimum-required (all `Option`s `None`) shapes. Catches silent
  field-reorder regressions and `skip_serializing_if` drift.
- **`identity_resolution_unresolved_shape_fixture`** documenting the
  on-wire shape an MCP agent or cloud aggregator sees for a failed-join
  entry where `resolution = "unresolved"` and columns / `source_hash`
  are absent.
- **`blast_radius_id_anchor_fixture`** and
  **`importance_id_anchor_fixture`** parallel to the existing
  `function_identity_id_anchor_fixture`. Locks the canonical hash
  inputs and truncation for the remaining stable-ID helpers so every
  producer can self-test agreement.
- **Per-field stability assertions** for `function_identity_id`:
  separate tests pin invariance under independent mutation of
  `start_column`, `end_line`, `end_column`, and `source_hash`. The
  prior `function_identity_id_unchanged_by_columns` test bundled all
  four metadata fields; the per-field cases catch a future regression
  where the helper accidentally starts hashing one specific metadata
  field but not the others.
- **Tightened rustdoc** on `FunctionIdentity::stable_id_computed`. The
  method is now explicitly documented as a diagnostic helper, NOT a
  validation gate. Consumers MUST NOT reject payloads whose `stable_id`
  differs from the computed value; doing so would turn every such
  consumer into a hard-fail on the next protocol major that evolves the
  hash inputs.
- **`Unknown` sentinel variants** on `RiskBand` and `CoverageSource`,
  closing the last two forward-compat gaps from a pre-release audit
  of every public enum in the crate. Adds `#[serde(other)] Unknown` to
  both. Future producers MAY add new variants (`Critical` / `Negligible`
  for `RiskBand`; `IstanbulDir` / `TraceEvent` / `RuntimeBeacon` for
  `CoverageSource`) as additive minor bumps; consumers that have not
  seen the new variant yet map it to `Unknown` rather than failing
  deserialization. Closes the latent gap that future variant additions
  on either enum would have required a major bump. Closes #5.

### Other

- `PROTOCOL_VERSION` bumped to `"0.7.0"`. `Cargo.toml [package].version`
  matches per `.claude/rules/protocol-versioning.md`.
- Internal `hex_prefix` refactored to take a `bytes` count so the 4-byte
  truncation used by `finding_id` / `hot_path_id` / `blast_radius_id` /
  `importance_id` / `function_identity_id` and the 8-byte truncation
  used by `source_hash_for` share one auditable implementation.

## [0.6.0] - 2026-05-20

### Added

- **`FunctionIdentity` type and optional `identity` field** on
  `StaticFunction`, `Finding`, `HotPath`, `BlastRadiusEntry`, and
  `ImportanceEntry`. Becomes the canonical cross-surface join key
  between the OSS CLI's static function inventory, V8 / Istanbul
  runtime coverage, test coverage from `oxc-coverage-instrument`,
  source-map remapped findings, and `fallow-cloud` aggregation. The
  legacy `file` / `function` / `line` triple is preserved verbatim
  for display and 0.5-era consumers.
- **`function_identity_id(file, name, start_line)` helper** emitting
  `fallow:fn:<8 hex>`. Hash inputs are `file + name + start_line +
  "function"`; columns and `source_hash` are intentionally NOT hashed
  so producers with different positional fidelity (V8 byte offsets vs
  Istanbul UTF-16 columns vs oxc spans) agree on the join key.
- **`IdentityResolution` enum** with `Resolved` / `Fallback` /
  `Unresolved` / `Unknown` variants. Lets cloud aggregation record
  per-function whether the identity came from a source-map lookup, a
  best-effort fallback, or remains unresolved. Required field on
  `FunctionIdentity` (not `#[serde(default)]`): a missing field would
  silently default and hide the resolution-confidence signal.
- **`FunctionIdentity::stable_id_computed`** convenience method for
  consumers that want to sanity-check a producer-supplied
  `stable_id`.
- **Column-semantic lock** in rustdoc on `FunctionIdentity::start_column`
  and `end_column`: 1-indexed UTF-16 column, anchored at the
  function-body start (Istanbul `loc.start`, V8 mapped from byte
  offset via script text, oxc `Span::start` mapped to UTF-16).
  Producers MUST normalize to this anchor.
- **Conformance fixture test** `function_identity_id_anchor_fixture`
  with hard-coded inputs (`"src/render.tsx"`, `"render"`, `42`) and
  expected hash `"fallow:fn:43629542"`. Producers run the same
  fixture in their CIs to catch divergence at the source.

### Changed (breaking, source-side only)

- **`StaticFunction`, `Finding`, `HotPath`, `BlastRadiusEntry`, and
  `ImportanceEntry` are now `#[non_exhaustive]`.** This is a one-time
  source-level break for downstream Rust consumers that constructed
  these via struct literals; the wire shape is unchanged and
  forward-compatible. Future field additions become pure additive
  changes that no longer require a source break.
- `PROTOCOL_VERSION` bumped to `"0.6.0"`.

### Migration

Cross-repo rollout order (load-bearing, do not invert):

1. Publish `fallow-cov-protocol` 0.6.0 (this release).
2. Update the closed-source `fallow-cov` sidecar to depend on 0.6.0,
   start populating `identity` on every emitted `Finding` /
   `HotPath` / `BlastRadiusEntry` / `ImportanceEntry` via
   `function_identity_id`, and ship a sidecar release. The wire is
   purely additive, so the upgraded sidecar's 0.6 output remains a
   valid 0.5 envelope for any CLI consumer that has not yet upgraded
   (the `identity` field is `#[serde(default)]` and `skip_serializing_if`
   on the consumer side); the sidecar therefore satisfies the grace
   window required by `.claude/rules/protocol-versioning.md` without
   needing to emit two envelope shapes in parallel. Sidecar / CLI /
   cloud repos with auto-update Dependabot or similar bot config
   SHOULD pin or delay the 0.6.0 bump (and gate auto-merge) until the
   matching consumer PR has landed in lockstep; rolling the bumps
   independently does not break the wire but reverses the
   coordination order this rule file requires.
3. Update the OSS `fallow` CLI to depend on 0.6.0. Continue reading
   the legacy `file` / `function` / `line` fields for display.
   Switch the join key to `identity.stable_id` when present (tracked
   in `fallow-rs/fallow#506`).
4. Migrate `fallow-cloud` aggregation to prefer `identity.stable_id`
   for dedup / merge keys (tracked in `fallow-rs/fallow-cloud#63`),
   and the browser / node beacon paths to emit column data (tracked
   in `fallow-rs/fallow-cloud#64`).

For downstream Rust consumers of this crate:

- Code that constructs `StaticFunction`, `Finding`, `HotPath`,
  `BlastRadiusEntry`, or `ImportanceEntry` via struct literals will
  fail to compile against 0.6.0. Replace struct literals with
  builder helpers in the producing crate, or accept the one-time
  source break.
- Legacy `Finding.id` / `HotPath.id` / `BlastRadiusEntry.id` /
  `ImportanceEntry.id` continue to ship through 0.6 alongside
  `identity.stable_id`. Existing suppression files keyed on the
  legacy IDs remain valid.
- **Suppression key vs join key.** The two IDs serve different
  axes: `Finding.id` is the per-finding suppression key (hashes the
  current `line`, so it changes when a function moves);
  `identity.stable_id` is the cross-surface join key (stable across
  line moves; same function gets one value across findings, hot
  paths, blast-radius, and importance entries). Agent tooling
  writing NEW suppression / baseline entries SHOULD prefer
  `identity.stable_id` when present so suppressions survive line
  shifts. Readers MUST accept both forms during the grace window.

## [0.3.0] - 2026-04-20

### Added

- **`Summary.capture_quality: Option<CaptureQuality>`** (ADR 009 step 6b,
  deliverable 2 of 3). Surfaces `{ window_seconds, instances_observed,
  lazy_parse_warning, untracked_ratio_percent }` so the CLI can render a
  "short window" warning alongside low-confidence verdicts and quantify
  the delta continuous cloud monitoring would provide. Optional for
  forward-compatibility with 0.2.x sidecars; 0.3.x always sets it.
- `CaptureQuality::LAZY_PARSE_THRESHOLD_PERCENT = 30.0`. Untracked ratio
  above this threshold trips the lazy-parse warning.
- `Options.window_seconds: Option<u64>` and `Options.instances_observed:
  Option<u32>`. Finer-grained inputs for `CaptureQuality`; both fall back
  to existing `period_days`/`deployments_seen` when `None`.

### Changed

- `PROTOCOL_VERSION` bumped to `"0.3.0"`.

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
