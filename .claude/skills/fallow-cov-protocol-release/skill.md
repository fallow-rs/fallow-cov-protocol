---
name: fallow-cov-protocol-release
description: Bump version, tag, and publish fallow-cov-protocol to crates.io; coordinate dep bumps in consumer repos (fallow, fallow-cloud). Use this instead of the bare `release` slash command to avoid name collisions with sibling project skills.
---

Cut a new release of `fallow-cov-protocol`. The crate is the wire contract between the fallow CLI and the fallow-cov sidecar, so the release order matters: publish this crate first, then bump consumer deps in a grace-window release of the sidecar, then the CLI. See `.claude/rules/protocol-versioning.md` for the full policy.

## Usage

- `/fallow-cov-protocol-release patch` (patch bump, 0.7.0 to 0.7.1, backward-compatible bug fix)
- `/fallow-cov-protocol-release minor` (minor bump, 0.7.0 to 0.8.0, forward-compatible additions; unknown enum variants must already map to the crate's `Unknown` sentinel)
- `/fallow-cov-protocol-release major` (major bump, e.g. 0.7.0 to 0.8.0 while pre-1.0, or 0.7.0 to 1.0.0 once the contract is locked)

**Pre-1.0 caveat:** until 1.0.0, any visible field/enum/type change is effectively a break. Use `minor` bump for breaking changes too, per the README "Status" banner. The major channel only becomes meaningful post-1.0.

## What "done" looks like

- crates.io has the new version (published by `.github/workflows/release.yml` on tag push, not locally)
- GitHub Release at `v<version>` with the matching CHANGELOG slice attached (created by the same workflow)
- `v<version>` tag on a signed commit on `main`, pushed to origin
- `PROTOCOL_VERSION` in `src/lib.rs` matches `[package] version` in `Cargo.toml` (the release workflow re-checks this; mismatch hard-fails the job)
- Consumers (`fallow/crates/cli/Cargo.toml`, `fallow-cloud/crates/fallow-cov/Cargo.toml`) have their pin bumped in follow-up PRs in their own repos (NOT in this repo's release)

## Steps

### 1. Pre-flight

- `git status` clean; on `main`; `git pull --ff-only origin main` so HEAD is current.
- `git log --oneline origin/main..HEAD` and `git log --oneline HEAD..origin/main` both empty. Bail if a parallel session is mid-release.
- `grep '^version' Cargo.toml` to read current version. Compute new version from the requested bump.
- `git ls-remote --tags origin | grep "refs/tags/v<NEW_VERSION>"`. Tag must not already exist. If it does, bump further; never force-push a release tag.
- `cargo publish --dry-run` (catches bad metadata before the release workflow does).

### 2. Bump versions (must stay in lockstep)

`Cargo.toml` `[package] version` and `src/lib.rs` `PROTOCOL_VERSION` MUST match. The release workflow's "Verify PROTOCOL_VERSION matches Cargo.toml version" step is a hard gate.

```bash
NEW_VERSION=X.Y.Z

# Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml

# src/lib.rs PROTOCOL_VERSION constant
sed -i '' "s/^pub const PROTOCOL_VERSION: &str = \".*\"/pub const PROTOCOL_VERSION: \&str = \"$NEW_VERSION\"/" src/lib.rs
```

### 3. Update the overview block

If the wire contract shape changed (new field, new variant, renamed field, removed field), add or update the `# 0.x overview` doc block at the top of `src/lib.rs` summarizing the delta in one paragraph. Consumers read this to decide how to migrate.

For patch releases that don't touch the wire shape, skip this step.

### 4. Write the CHANGELOG entry

Add a `## [<NEW_VERSION>] - YYYY-MM-DD` section at the top of `CHANGELOG.md`, under `[Unreleased]` if present. The release workflow extracts this slice and attaches it to the GitHub release body, so write it for human consumers (one paragraph per change, "why" not just "what"). Required even for patch releases.

### 5. Pre-commit quality gates

```bash
cargo fmt --all
cargo test                                  # all unit + serde round-trip tests must pass
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps --document-private-items
typos .
```

The pre-push hook in `.githooks/pre-push` runs fmt + clippy + typos automatically on `git push`. Running them up front gets faster feedback.

### 6. Commit + tag + push (let CI publish)

This repo has a release workflow (`.github/workflows/release.yml`) that runs `cargo publish` on tag push. Do NOT run `cargo publish` locally; the workflow uses the repo-scoped `CARGO_REGISTRY_TOKEN` secret and an ephemeral runner, and the local credential set is not the source of truth.

The release commit itself must go through the branch ruleset on `main` (CI + Commit messages must pass; signed commits required). Open a release PR rather than pushing direct:

```bash
git checkout -b "release/v$NEW_VERSION"
git add Cargo.toml src/lib.rs CHANGELOG.md
git commit -S -m "chore: release v$NEW_VERSION"
git push -u origin "release/v$NEW_VERSION"
gh pr create --title "chore: release v$NEW_VERSION" --body "..."
gh pr merge --auto --squash --subject "chore: release v$NEW_VERSION (#<PR>)"
```

Once the release PR merges into `main`, tag the merge commit and push:

```bash
git checkout main && git pull --ff-only origin main
git tag -s "v$NEW_VERSION" -m "v$NEW_VERSION"
git push origin "v$NEW_VERSION"
```

The tag push triggers `release.yml`, which:
1. Verifies tag matches `Cargo.toml` version AND `PROTOCOL_VERSION` in `src/lib.rs`
2. Runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo publish --dry-run`
3. Publishes to crates.io with `CARGO_REGISTRY_TOKEN`
4. Extracts the matching CHANGELOG slice and creates the GitHub Release

Both commit and tag MUST be signed (`commit -S`, `tag -s`).

### 7. Monitor the release workflow

```bash
gh run watch $(gh run list --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId') --exit-status
```

If the workflow fails partway through (e.g. crates.io was up but the GitHub Release step failed), the workflow is idempotent on re-run: the `Check if version is already published on crates.io` step skips the publish, and the `gh release view` check skips re-creation. Use `workflow_dispatch` with the existing tag to retry.

### 8. Consumer coordination (cross-repo, NOT this skill)

The three-step dance from `.claude/rules/protocol-versioning.md`:

1. **This release** (protocol crate published, tag pushed, GH release created) - done.
2. **Sidecar** (`fallow-cloud/crates/fallow-cov/Cargo.toml`) - bump pin to the new version. Ship a sidecar release that handles both the old and new envelopes during the grace window if the change is breaking. Use `/fallow-sidecar-release`.
3. **CLI** (`fallow/crates/cli/Cargo.toml`) - bump pin LAST, only after the sidecar release has rolled out. Use `/fallow-release`.

**Do not bump the CLI pin before the sidecar has a release out.** That's the hard constraint the ordering enforces. Shipping the CLI first strands users whose sidecar hasn't upgraded.

Track consumer bumps in their own repos via dedicated issues; do not open the PRs from inside this release skill.

## Retrospective / incident log

When something goes wrong, add an entry with date, symptom, root cause, prevention step (either a new pre-flight check above or a new rule in `.claude/rules/protocol-versioning.md`).

### 2026-05-21 (v0.7.1)

- **Symptom:** the previous version of this skill body told the operator to run `cargo publish` locally as step 6, then `git push --tags` as step 7. By 2026-05-21 the repo had grown a `release.yml` workflow that does its own `cargo publish` on tag push. Following the old skill verbatim would have caused a double-publish race (local upload, then CI attempts the same and the "already published" guard saves us only after the fact) or, worse, a local upload with a different toolchain than CI.
- **Root cause:** the skill body went stale when CI was added; the "What CI does NOT handle" section literally claimed `no .github/workflows/`.
- **Prevention:** rewrote steps 6 and 7 to be tag-push-only, removed the manual `cargo publish` call, added step 7 (monitor `release.yml`), and added this incident log. Future skill edits MUST re-verify "what CI does vs what the skill does" before publishing skill changes.
