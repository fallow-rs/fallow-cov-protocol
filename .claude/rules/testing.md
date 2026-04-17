---
paths:
  - "src/lib.rs"
---

# Testing conventions

All tests live in the single `#[cfg(test)] mod tests` block at the bottom of `src/lib.rs`. The crate is small enough that colocating tests with the types they cover is the clearest arrangement. Do not split into `tests/` integration files until the file becomes unreadable.

## What to cover

Every PR that changes wire-facing surface area adds tests in the matching category:

### Forward-compat (every enum with `#[serde(other)]`)
Assert that a made-up string variant deserializes to `Unknown`. Pattern:

```rust
#[test]
fn unknown_<enum_name>_round_trips() {
    let json = r#""future_variant""#;
    let parsed: EnumType = serde_json::from_str(json).unwrap();
    assert!(matches!(parsed, EnumType::Unknown));
}
```

### Unknown top-level fields on envelopes
A `Response` (and any nested struct likely to grow) must accept unknown JSON fields without erroring. See `response_allows_unknown_fields`.

### Casing
If the enum uses `#[serde(rename_all = "kebab-case")]` or a non-default casing, assert a kebab/snake JSON round-trips to the expected Rust variant. See `coverage_source_kebab_case`.

### Stable IDs (`finding_id`, `hot_path_id`, any new ID helper)
- Determinism: same inputs → same output.
- Distinctness: sibling helpers produce different IDs for identical inputs (the "kind" salt works).
- Input sensitivity: changing any hashed input (line, function, file) changes the output.
- Format: `fallow:<kind>:<8 hex chars>` length/prefix asserted.

### `skip_serializing_if` on `Option<T>`
- With `Some(x)`: the serialized JSON contains the field.
- With `None`: the serialized JSON does NOT contain the field (assert via `.contains("field_name") == false`).
- Both cases round-trip correctly.

### Defaulted fields
A JSON blob that omits a field annotated `#[serde(default)]` / `default = "..."` deserializes with the expected default. This guards against accidentally removing the `default` attribute.

## Naming
Test names read as an English assertion: `unknown_verdict_round_trips`, `finding_id_changes_with_line`, `evidence_omits_untracked_reason_when_none`. Avoid generic names like `test_1` or `it_works`.

## No external test crates
Do not add `insta`, `proptest`, `rstest`, or similar for this crate. The tests are small and literal on purpose — the wire contract should be readable without knowing a DSL.
