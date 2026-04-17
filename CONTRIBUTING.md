# Contributing to fallow-cov-protocol

This crate is the wire contract between the OSS [`fallow`](https://github.com/fallow-rs/fallow) CLI and the closed-source `fallow-cov` production-coverage sidecar. The whole reason it exists is to keep the two binaries from drifting, so the contribution bar is higher than a typical utility crate: every change to the public surface is a promise.

## Quick start

```bash
cargo build                                             # Library build
cargo test                                              # All unit tests
cargo clippy --all-targets -- -D warnings               # Lints
cargo fmt --all -- --check                              # Formatting
cargo doc --no-deps --document-private-items            # Rustdoc (no broken links)
typos                                                   # Spellcheck
```

CI runs `cargo-audit`, `cargo-deny`, `cargo-shear`, `zizmor`, the MSRV job, and `cargo publish --dry-run` on top of that. Run the supply-chain tools locally before opening a PR if you changed `Cargo.toml`.

## Lint baseline

- `clippy::{all, pedantic, nursery, cargo}` at `warn` with a short, documented allow-list in `Cargo.toml`.
- `#![forbid(unsafe_code)]` at the top of `lib.rs`; also declared in `[lints.rust]`.
- MSRV is `1.75`, pinned in `rust-toolchain.toml` and verified by the `msrv` CI job. New language/stdlib features that require a newer toolchain must bump the MSRV in the same PR with a changelog entry.
- Suppress a specific lint with `#[expect(clippy::..., reason = "...")]`, not `#[allow]`, so the suppression fails if the lint becomes unnecessary (MSRV 1.81+ — until then, use `#[allow(..., reason = "...")]`).

## Semver and the wire contract

`protocol_version` (exposed as `PROTOCOL_VERSION`) is a full semver string. The discipline below is load-bearing:

- **Major bumps** are breaking. Renaming a field's serde name, changing a wire type, removing a field, or changing the canonical `finding_id` / `hot_path_id` input order is a major — even if the Rust signature looks the same.
- **Minor bumps** add optional fields (with `#[serde(default)]`) or enum variants (only when the enum already carries `#[serde(other)] Unknown`).

Every new wire field requires:

- `#[serde(default)]` (or `default = "..."`) so old encoders stay valid.
- `#[serde(skip_serializing_if = "Option::is_none")]` on `Option<T>` when absent-is-observably-different-from-`None`.
- Rustdoc describing the intended semantics, not just the type.
- A matching round-trip + forward-compat test in the same PR (see `.claude/rules/testing.md`).

See `.claude/rules/protocol-versioning.md` for the full policy, including the cross-repo release dance.

## Testing conventions

Tests live in the single `#[cfg(test)] mod tests` block at the bottom of `src/lib.rs`. Required patterns:

- **Forward-compat**: every enum with `#[serde(other)]` has an `unknown_<enum>_round_trips` test.
- **Unknown top-level fields**: the `Response` envelope accepts unknown JSON fields without erroring.
- **Casing**: non-default `rename_all` casings (kebab/snake) are exercised explicitly.
- **Stable IDs**: determinism, distinctness from sibling helpers, sensitivity to every hashed input, and format/length are all asserted.
- **`skip_serializing_if`**: both present-and-serialized and absent-and-omitted cases are tested.

Do not add `insta`, `proptest`, `rstest`, or similar — tests are small and literal on purpose.

## Commit and PR hygiene

- Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:`. Breaking wire changes use `feat!:` / `fix!:`.
- Signed commits (`git commit -S`) and signed pushes. No AI attribution.
- One concern per PR. A wire change + an unrelated refactor go in separate PRs.
- PRs that touch the public surface must update `CHANGELOG.md` under `[Unreleased]`.

## Dependencies

The dependency surface is intentionally minimal: `serde`, `serde_json`, `sha2`. Adding a new dependency requires a justification in the PR description and, if it's transitive-heavy, an update to the clippy allow-list. Every downstream binary that depends on this crate inherits the cost.

Avoid time / path abstraction crates (`chrono`, `camino`, etc.) — the wire is stringly typed on purpose.

## Review

Reviewer personas live in `.claude/agents/`; the assembly matrix in `.claude/rules/team-assembly.md` describes which reviewers to pull in for which change types. A `BLOCK` from `protocol-reviewer` on an unannounced breaking change is a hard veto.
