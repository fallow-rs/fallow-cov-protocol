# fallow-cov-protocol — Wire contract for fallow production-coverage

Versioned JSON envelope shared between the open-source [`fallow`](https://github.com/fallow-rs/fallow) CLI and the closed-source `fallow-cov` production-coverage sidecar. This crate is the single source of truth for the request/response shape so the two repositories cannot drift.

## Project structure

```
src/
  lib.rs            — Entire public API: Request/Response envelopes, enums, ID helpers, tests
Cargo.toml          — Crate metadata, pinned MSRV 1.75, pedantic clippy, unsafe_code = forbid
README.md           — crates.io front page (what/why/versioning/status)
.claude/            — Claude Code config: hooks, rules, reviewer agents
```

The crate is deliberately tiny: types + serde derives + a couple of ID helpers. Everything lives in one file so both sides can grep the contract in seconds.

## Architecture (role of this crate)

```
  fallow CLI  ──spawn──▶  fallow-cov sidecar
   (OSS)          stdin       (closed-source)
     │        Request JSON          │
     │                              │
     │     Response JSON            │
     └──────────stdout──────────────┘
                 ▲
                 │
         fallow-cov-protocol
         (this crate, shared dep)
```

Both binaries depend on this crate. The CLI writes a `Request` to the sidecar's stdin; the sidecar writes a `Response` to stdout. The crate exposes zero I/O and zero business logic — only the shape of the bytes on the pipe.

## Versioning policy (load-bearing)

- `protocol_version` is a full semver string, also exposed as `PROTOCOL_VERSION`.
- **Major bumps** are breaking. Consumers MUST reject mismatched majors.
- **Minor bumps** add optional fields or enum variants. Consumers MUST forward-accept unknown fields (via serde defaults) and SHOULD map unknown enum variants to the crate's `Unknown` sentinel (via `#[serde(other)]`).
- Every new field on an existing struct requires `#[serde(default)]` (or `default = "..."`) so old encoders stay compatible.
- Every public enum that can grow variants must carry an `Unknown` variant with `#[serde(other)]`. Adding a new variant is a *minor* bump only if the `Unknown` sentinel is present; otherwise it's a major.
- The stable ID hashes (`finding_id`, `hot_path_id`) hash a canonical, unseparated UTF-8 order. That order is part of the public contract — changing it invalidates IDs persisted by CI dedup, suppression, and agent cross-references. Treat it as a major break even if the function signature is unchanged.
- Until `1.0.0`, minor + patch may contain breaking changes; always bump the relevant digit and announce in the changelog.

## Code conventions

- `#![forbid(unsafe_code)]` — top of `lib.rs`, enforced via `[lints.rust]` in Cargo.toml.
- Serde derives on every wire type. Enums use `#[serde(rename_all = "snake_case")]` or `"kebab-case"` explicitly (see `CoverageSource` for kebab, `Verdict`/`Confidence` for snake).
- Enum `Unknown` sentinels via `#[serde(other)]` — see `ReportVerdict`, `Verdict`, `Confidence`, `Feature`, `Watermark`.
- Optional fields use `#[serde(default)]`; `Option<T>` fields skip-serialize with `skip_serializing_if = "Option::is_none"` when absent is semantically different from default.
- Default bools use a named `const fn default_true() -> bool`, not closures — keeps the wire default auditable.
- Clippy `pedantic` at `warn` (priority -1), with `module_name_repetitions` and `missing_errors_doc` allowed (tightly scoped crate, every public item is the contract).
- MSRV pinned to 1.75 in Cargo.toml; do not rely on newer features without bumping it.
- `missing_docs = "allow"` is a TODO until 1.0.0; new public items should still carry rustdoc.

## Testing conventions

Every wire-facing behavior has a unit test in the same `tests` mod in `lib.rs`:

- **Forward-compat**: unknown string variants for every `#[serde(other)]` enum round-trip to `Unknown` (see `unknown_report_verdict_round_trips`, `unknown_verdict_round_trips`, etc.).
- **Unknown top-level fields** on `Response` deserialize without erroring (`response_allows_unknown_fields`).
- **Serde rename casing** is exercised (e.g. `coverage_source_kebab_case`).
- **ID stability**: `finding_id` / `hot_path_id` must be deterministic, must differ between the two kinds for the same inputs, and must change when line number changes (see existing tests).
- **`skip_serializing_if`** on `Option<T>` fields is verified both ways (present + absent) — see `evidence_round_trips_with_untracked_reason` and `evidence_omits_untracked_reason_when_none`.

When adding a new wire field or enum variant, add the matching round-trip + forward-compat test in the same PR. No exceptions.

## Building & testing

```bash
cargo build                                       # Library build
cargo test                                        # All unit tests
cargo clippy --all-targets -- -D warnings         # Lints
cargo fmt --all -- --check                        # Formatting
cargo doc --no-deps --document-private-items      # Docs (catches broken intra-doc links)
```

## Git conventions

- Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:`
- Signed commits (`git commit -S`)
- No AI attribution in commits

## Key design decisions

- **One file for the whole contract**: keeps grep-ability and review surface small. Do not split until it's genuinely unreadable.
- **Plain `String` paths, not `PathBuf`**: the wire is JSON; paths are already stringly typed on both sides and `PathBuf` serde is lossy on Windows.
- **SHA-256 truncated to 8 hex chars for IDs**: short enough to fit in CLI output and CI annotations, long enough to avoid collisions at realistic finding counts.
- **No `thiserror` / `anyhow` dependency**: this crate never produces errors — all fallibility lives on the two binaries that own I/O.
- **No chrono / time crate**: periods are raw integers (`observation_days`, `deployments_observed`) to keep the dep tree minimal and the wire trivially parseable by non-Rust sidecars.

See `.claude/rules/` for area-specific conventions and `.claude/agents/` for reviewer personas.
