---
paths:
  - "**"
---

# Team assembly matrix

When spawning reviewer agents for a change, use this matrix to decide who to pull in. Spawn all relevant agents in parallel, collect their verdicts, report the consensus.

## Consensus rules

- **Ship** = zero BLOCKs and majority APPROVE
- **Fix first** = any BLOCK present (blocker must be resolved)
- **Ship with notes** = zero BLOCKs but one or more CONCERN verdicts

## Assembly by change type

### Pure internal Rust change (helper fns, tests, private doc comments)

Spawn: `rust-reviewer`

### Any change to `src/lib.rs` public surface — struct fields, enum variants, ID helpers, `PROTOCOL_VERSION`, serde attributes, doc comments on public items

Spawn: `rust-reviewer`, `protocol-reviewer`

The `protocol-reviewer` BLOCK on an unannounced breaking change is a hard veto.

### Cargo.toml dependency changes

Spawn: `rust-reviewer`, `protocol-reviewer`

Adding a new dep in a protocol crate is load-bearing — every downstream binary inherits the cost.

### README / CLAUDE.md / rule doc changes

Spawn: `rust-reviewer` (lightweight pass for accuracy only)

### Release (version bump + tag)

Spawn: `protocol-reviewer`
Optional: `user-panel` for a semver-communication sanity check (advisory, not in consensus count).

## User panel

`user-panel` is advisory only — its output is never counted toward APPROVE/CONCERN/BLOCK consensus. Use it when a change affects how CLI or sidecar maintainers interact with the contract (new fields, new enums, new ID conventions, migration stories).
