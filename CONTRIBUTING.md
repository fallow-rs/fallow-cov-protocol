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

### Local pre-push hook

Once per clone, opt into the same gates locally:

```bash
git config core.hooksPath .githooks
```

`.githooks/pre-push` runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `typos .` on every push. `RUN_TESTS=1 git push` also runs `cargo test`. `SKIP_PRE_PUSH=1 git push` bypasses it (use sparingly, e.g. WIP branches for early CI signal).

## Lint baseline

- `clippy::{all, pedantic, nursery, cargo}` at `warn` with a short, documented allow-list in `Cargo.toml`.
- `#![forbid(unsafe_code)]` at the top of `lib.rs`; also declared in `[lints.rust]`.
- MSRV is `1.85`, pinned in `rust-toolchain.toml` and verified by the `msrv` CI job. New language/stdlib features that require a newer toolchain must bump the MSRV in the same PR with a changelog entry.
- Suppress a specific lint with `#[expect(clippy::..., reason = "...")]`, not `#[allow]`, so the suppression fails if the lint becomes unnecessary.

## Semver and the wire contract

`protocol_version` (exposed as `PROTOCOL_VERSION`) is a full semver string. The discipline below is load-bearing:

- **Major bumps** are breaking. Renaming a field's serde name, changing a wire type, removing a field, or changing the canonical `finding_id` / `hot_path_id` input order is a major — even if the Rust signature looks the same.
- **Minor bumps** add optional fields (with `#[serde(default)]`) or enum variants (only when the enum already carries `#[serde(other)] Unknown`).

Every new wire field requires:

- `#[serde(default)]` (or `default = "..."`) so old encoders stay valid.
- `#[serde(skip_serializing_if = "Option::is_none")]` on `Option<T>` when absent-is-observably-different-from-`None`.
- Rustdoc describing the intended semantics, not just the type.
- A matching round-trip + forward-compat test in the same PR (see the patterns under "Testing conventions" below).

The cross-repo release dance for a wire change is: publish this crate, ship a sidecar release that accepts both old and new envelopes, then ship a CLI release that depends on the new version. Never reverse that order, the CLI shipping first breaks users who have not updated their sidecar yet.

## Testing conventions

Tests live in the single `#[cfg(test)] mod tests` block at the bottom of `src/lib.rs`. Required patterns:

- **Forward-compat**: every enum with `#[serde(other)]` has an `unknown_<enum>_round_trips` test.
- **Unknown top-level fields**: the `Response` envelope accepts unknown JSON fields without erroring.
- **Casing**: non-default `rename_all` casings (kebab/snake) are exercised explicitly.
- **Stable IDs**: determinism, distinctness from sibling helpers, sensitivity to every hashed input, and format/length are all asserted.
- **`skip_serializing_if`**: both present-and-serialized and absent-and-omitted cases are tested.

Do not add `insta`, `proptest`, `rstest`, or similar — tests are small and literal on purpose.

## Commit and PR hygiene

- Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:`. Breaking wire changes use `feat!:` / `fix!:`. Enforced on push/PR by the `commitlint` workflow against `commitlint.config.mjs` (header <= 100 chars, lower-case type and scope, conventional type set).
- Signed commits (`git commit -S`) and signed pushes. No AI attribution.
- One concern per PR. A wire change + an unrelated refactor go in separate PRs.
- PRs that touch the public surface must update `CHANGELOG.md` under `[Unreleased]`.

## Dependencies

The dependency surface is intentionally minimal: `serde`, `serde_json`, `sha2`. Adding a new dependency requires a justification in the PR description and, if it's transitive-heavy, an update to the clippy allow-list. Every downstream binary that depends on this crate inherits the cost.

Avoid time / path abstraction crates (`chrono`, `camino`, etc.) — the wire is stringly typed on purpose.

## Review

Wire-contract changes (new fields, new enum variants, new ID helpers, `PROTOCOL_VERSION` bumps, serde attribute changes) need a closer read than internal refactors. An unannounced breaking change to the public surface is a hard reject; bump the major and call it out in the PR description and CHANGELOG.
