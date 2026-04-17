---
paths:
  - "**/*.rs"
  - "Cargo.toml"
---

# Rust code quality (fallow-cov-protocol)

## Crate-level policy
- `#![forbid(unsafe_code)]` at the top of `lib.rs` — also declared in `[lints.rust]` in `Cargo.toml`. Any `unsafe` is a hard reject.
- MSRV is `1.75` in `Cargo.toml`. New language/stdlib features that need a newer toolchain must bump MSRV explicitly in the same PR and land with a changelog note.
- Dependency surface is intentionally minimal: `serde`, `serde_json`, `sha2`. Adding a new dependency requires a justification in the PR description and a clippy allowlist update if needed. Transitive bloat affects every binary that pulls this crate.

## Clippy
- `[lints.clippy] pedantic = { level = "warn", priority = -1 }` is the baseline. Allow-list entries (`module_name_repetitions`, `missing_errors_doc`) are documented in `Cargo.toml`; keep that list short.
- Suppress lints with `#[expect(clippy::..., reason = "...")]` instead of `#[allow]`, so the suppression fails if the lint becomes unnecessary.
- Clippy must pass with `--all-targets -- -D warnings` in CI.

## Formatting
- `cargo fmt --all -- --check` in CI. The pre-commit hook in `.claude/settings.json` runs `cargo fmt --all` automatically on `git commit`.

## Typos
- `typos` runs on commit via the hook and in CI. All code, comments, doc strings, and test fixtures must pass. Intentional invalid identifiers in tests should use obviously synthetic names, not misspelled real words.

## Docs
- `missing_docs = "allow"` is a temporary crate-level lint relaxation until `1.0.0`. New public items should still carry rustdoc; the lint will be flipped to `deny` before the 1.0 cut.
- `cargo doc --no-deps --document-private-items` must succeed without warnings. Broken intra-doc links (`[`Foo`]`) count as warnings.

## Serde discipline
- Every wire type derives `Debug, Clone, Serialize, Deserialize`. Copyable small enums also derive `Copy, PartialEq, Eq`.
- Use `#[serde(rename_all = "snake_case")]` or `"kebab-case"` explicitly — do not rely on field-name casing that happens to match.
- New optional fields require `#[serde(default)]` or `default = "..."`. `Option<T>` fields that should be absent-when-None need `#[serde(skip_serializing_if = "Option::is_none")]`.
- Enums that may grow variants across minor versions MUST carry an `Unknown` variant with `#[serde(other)]`.

## Disallowed
- `unsafe` code.
- Direct `println!` / `eprintln!` in the library — this crate has no I/O.
- Panicking APIs (`unwrap`, `expect`) on deserialization paths. Tests may use `unwrap()` freely.
- Time / path abstraction crates (`chrono`, `camino`, etc.). The wire is stringly typed on purpose.

## CI hardening (target state)
- `permissions: {}` deny-all baseline on all workflows once CI is wired up.
- `cargo-shear` for unused dependency detection.
- `zizmor` on any GitHub Actions we add.
