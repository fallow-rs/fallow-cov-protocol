---
name: release
description: Bump version, tag, publish to crates.io, and coordinate dep bumps in consumer repos (fallow, fallow-cloud)
---

Cut a new release of `fallow-cov-protocol`. The crate is the wire contract between the fallow CLI and the fallow-cov sidecar, so the release order matters: publish this crate first, then bump consumer deps in a grace-window release of the sidecar, then the CLI. See `.claude/rules/protocol-versioning.md` for the full policy.

## Usage

- `/release patch` — patch bump (0.2.0 → 0.2.1, backward-compatible bug fix)
- `/release minor` — minor bump (0.2.0 → 0.3.0, forward-compatible additions; unknown enum variants must already map to the crate's `Unknown` sentinel)
- `/release major` — major bump (0.2.0 → 0.3.0 while pre-1.0, or 0.2.0 → 1.0.0 once the contract is locked)

**Pre-1.0 caveat:** until 1.0.0, any visible field/enum/type change is effectively a break — use `minor` bump for breaking changes too, per the README "Status" banner. The major channel only becomes meaningful post-1.0.

## What "done" looks like

- crates.io has the new version
- `v<version>` tag on a signed commit on `main`, pushed to origin
- `PROTOCOL_VERSION` in `src/lib.rs` matches `[package] version` in `Cargo.toml` (invariant checked by `protocol-reviewer` agent)
- Consumers (`fallow/crates/cli/Cargo.toml`, `fallow-cloud/crates/fallow-cov/Cargo.toml`) have their pin bumped in follow-up PRs — **NOT in this repo's release**
- Migration notes merged into `src/lib.rs`'s top-of-file overview block

## Steps

### 1. Pre-flight

- `git status` clean; on `main`.
- `git fetch origin && git log --oneline origin/main..HEAD && git log --oneline HEAD..origin/main` — bail if remote is ahead, a parallel session may already be releasing.
- `cat Cargo.toml | grep '^version'` — read current version.
- Compute new version from the requested bump.
- `git ls-remote --tags origin | grep "refs/tags/v<NEW_VERSION>"` — tag must not already exist. If it does, bump further; never force-push a release tag.
- `cargo publish --dry-run` — catches bad metadata (missing README, invalid license expression, non-published dep paths) before the irreversible publish.

### 2. Bump versions (keep them in sync)

```bash
NEW_VERSION=X.Y.Z

# Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# src/lib.rs PROTOCOL_VERSION constant
sed -i '' "s/^pub const PROTOCOL_VERSION: &str = \".*\"/pub const PROTOCOL_VERSION: \&str = \"$NEW_VERSION\"/" src/lib.rs
```

Both values MUST match — `protocol-reviewer` agent blocks PRs that desync them.

### 3. Update the overview block

If the wire contract shape changed (new field, new variant, renamed field, removed field), add or update the `# 0.x overview` doc block at the top of `src/lib.rs` summarizing the delta in one paragraph. Consumers read this to decide how to migrate.

For patch releases that don't touch the wire shape, skip this step.

### 4. Pre-commit quality gates

```bash
cargo fmt --all
cargo test                      # all 16+ tests must pass
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps --document-private-items
```

`cargo fmt` and `typos` also run automatically via the pre-commit hook in `.claude/settings.json`; running them up front just gets faster feedback.

### 5. Commit + tag

```bash
git add Cargo.toml src/lib.rs
git commit -S -m "$(cat <<'EOF'
chore: release v<NEW_VERSION>

<one-paragraph summary of what changed on the wire, or
"patch release — <bug fix summary>" for patch bumps>
EOF
)"

git tag -s "v$NEW_VERSION" -m "v$NEW_VERSION"
```

Both commit and tag MUST be signed (`commit -S`, `tag -s`). The tag is the source of truth for crates.io and the sidecar / CLI repos' dependency pins.

### 6. Publish

```bash
cargo publish
```

Wait ~30 seconds for the crates.io index to converge before the consumer bump PRs can resolve the new version.

### 7. Push

```bash
git push origin main
git push origin "v$NEW_VERSION"
```

### 8. Consumer coordination (cross-repo)

The three-step dance from `.claude/rules/protocol-versioning.md`:

1. **This release** — protocol crate published, tag pushed. ✓
2. **Sidecar** (`~/Sites/fallow-cloud/crates/fallow-cov/Cargo.toml`) — bump pin to the new version. Ship a sidecar release that handles both the old and new envelopes during the grace window (if the change is breaking).
3. **CLI** (`~/Sites/fallow/crates/cli/Cargo.toml`) — bump pin last, only after the sidecar release has rolled out. Shipping the CLI first strands users whose sidecar hasn't upgraded.

**Do not bump the CLI pin before the sidecar has a release out.** That's the hard constraint the ordering enforces.

### 9. Consumer pin-bump PRs

For each consumer repo (`fallow`, `fallow-cloud`), open a focused PR that only bumps the `fallow-cov-protocol` pin and refreshes `Cargo.lock`. Body should link back to this repo's tag. Don't bundle with unrelated work.

## What CI does NOT handle

This repo has **no `.github/workflows/`** — everything in this skill is manual. If you want automation, mirror `fallow/.github/workflows/release.yml` but trimmed to a single `cargo publish` job.

## Retrospective / incident log

If something goes wrong, add an entry to this section with: date, symptom, root cause, prevention step (either a new pre-flight check above or a new rule in `.claude/rules/protocol-versioning.md`).
