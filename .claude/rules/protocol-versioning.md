---
paths:
  - "src/lib.rs"
  - "Cargo.toml"
  - "README.md"
---

# Protocol versioning rules

This crate defines the wire contract between the OSS `fallow` CLI and the closed-source `fallow-cov` sidecar. Versioning discipline is the whole reason the crate exists.

## What counts as a break

Hard breaks (major bump, `protocol_version` major digit changes, new release tag):

- Removing a field on a `Request` / `Response` / nested struct.
- Renaming a field's serde name (even if the Rust identifier stays).
- Changing a field's wire type (e.g. `u32` → `String`, `Option<T>` → `T` or vice versa when the default is observable).
- Adding a new required variant to an enum that has NO `#[serde(other)] Unknown` sentinel.
- Changing the canonical input order for `finding_id` / `hot_path_id` hashing, or the hash algorithm, or the truncation length. This invalidates every persisted ID downstream (CI dedup, suppression files, agent cross-references) and is always a major.
- Changing `PROTOCOL_VERSION` across the major boundary without the code changes above is still a major — the constant is a promise.

Soft changes (minor bump, forward-compatible):

- Adding an optional field with `#[serde(default)]` or `default = "..."`.
- Adding a new enum variant *only when* the enum already has `#[serde(other)] Unknown`. Old consumers will map the new variant to `Unknown`.
- Adding a new `Feature` variant — same rule, `Feature::Unknown` already exists.
- Tightening rustdoc / internal helpers / test coverage.

## Required patterns for new wire fields

1. New field on an existing struct:
   ```rust
   #[serde(default)]
   pub new_field: Option<SomeType>,
   ```
   If absent-is-observably-different-from-None, add `#[serde(skip_serializing_if = "Option::is_none")]`.

2. New bool field with a specific default:
   ```rust
   #[serde(default = "default_true")]
   pub new_flag: bool,
   ```
   Reuse the existing `default_true()` or add a similarly-named `const fn` — avoid closures.

3. New enum variant on a `Request`/`Response` enum: verify the enum has `#[serde(other)] Unknown`. If not, add it in a separate major bump first.

## Required tests for wire changes

Every PR that touches the public contract adds, at minimum:

- A forward-compat test: an unknown variant / unknown field round-trips to the `Unknown` sentinel / is ignored.
- A happy-path round-trip: `serialize -> deserialize -> assert equal`.
- For `Option<T>` with `skip_serializing_if`: one test with `Some(_)`, one test with `None`, asserting on the serialized JSON shape in both cases.
- For any new stable-ID helper: determinism, distinctness from sibling helpers, and sensitivity to every input dimension.

## Release checklist

When cutting a new protocol version:

1. Bump `PROTOCOL_VERSION` and `[package] version` in `Cargo.toml` together — they MUST match.
2. Update the `0.x overview` comment block at the top of `lib.rs` with a one-paragraph summary.
3. Update `README.md`'s "Status" section if the guarantees changed.
4. `cargo publish --dry-run` locally before tagging.
5. Tag with `v<version>` on a signed commit. The tag is the source of truth for crates.io and the sidecar repo's `Cargo.toml` dependency pin.

## Cross-repo coordination

The OSS CLI repo and the sidecar repo both depend on this crate by exact version. A protocol bump is a three-step dance:

1. Publish this crate.
2. Update the sidecar to the new version and ship a sidecar release that handles both the old and new envelopes (grace window).
3. Update the CLI to the new version only after the sidecar release has rolled out.

Never reverse that order — the CLI shipping first breaks users who haven't updated their sidecar.
