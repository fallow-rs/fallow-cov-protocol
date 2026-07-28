# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in fallow-cov-protocol, please report it responsibly via [GitHub's private vulnerability reporting](https://github.com/fallow-rs/fallow-cov-protocol/security/advisories/new) instead of opening a public issue.

You should receive a response within 48 hours. Please include:

- A description of the vulnerability
- Steps to reproduce it
- Any relevant version or configuration information

## Scope

This crate is the wire contract between the [`fallow`](https://github.com/fallow-rs/fallow) CLI and the closed-source `fallow-cov` sidecar: serde-derived envelope types, a handful of enums, and two stable-ID helpers. It performs no I/O. It does not read files, spawn processes, open sockets, or execute user code, and it has no build script and no procedural macros of its own. `unsafe_code = "forbid"` is declared crate-wide in `Cargo.toml` and re-asserted at the top of `lib.rs`.

Because the crate owns no I/O, most of what would normally be a threat model here lives in the two binaries that link it. Vulnerabilities in the CLI belong in [fallow's security policy](https://github.com/fallow-rs/fallow/blob/main/SECURITY.md); the sidecar is closed-source and is covered by the same private reporting link as this repository.

## Threat model

The one security-relevant surface is **deserialization of untrusted JSON**. A consumer that reads a `Request` or `Response` from a pipe is parsing bytes it may not control, through `serde_json`. Two properties are intentional and should be treated as part of the contract:

- **Unknown fields and unknown enum variants are accepted, not rejected.** Unknown fields are ignored and unknown enum strings map to an `Unknown` sentinel. This is the forward-compatibility guarantee that lets a new encoder talk to an old decoder, and it means a decoder must not assume it has seen every field the peer sent.
- **Deserialization is not authentication.** Nothing in this crate verifies that the peer on the other end of the pipe is the process you expect. Trust in the envelope derives entirely from trust in the pipe, which the spawning binary establishes.

Resource limits are likewise the consumer's responsibility: the crate imposes no cap on input size, string length, or collection length, so a host that reads from an untrusted source should bound the reader itself.

**The stable IDs are not a security primitive.** `finding_id` and `hot_path_id` truncate SHA-256 to 8 hex characters (32 bits), and `FunctionIdentity::stable_id` to 16 hex characters (64 bits). They exist for deduplication and cross-referencing, and the widths are chosen for readability in CLI output and CI annotations. Do not use them as authentication tokens, integrity checks, or anything where an adversary benefits from a collision.

## Build-time trust boundary

This crate ships as source on crates.io. There are no released binaries, so the binary-signing and install-time verification machinery documented in fallow's policy does not apply here.

The runtime dependency surface is deliberately three crates (`serde`, `serde_json`, `sha2`) and adding a fourth requires a written justification in the pull request. The Cargo dependency graph is gated by `cargo-deny` (`deny.toml`, run in CI):

- RUSTSEC advisories deny by default (cargo-deny v2), and yanked crates are rejected.
- `[bans] wildcards = "deny"` forbids `*` version requirements; `[sources]` denies unknown registries and git sources, so a dependency cannot be pulled from an unexpected origin.
- Every `advisories.ignore` entry must carry a written justification, so suppressions are auditable rather than silent.

Dependency updates flow through Dependabot with a 7-day cooldown, and auto-merge is restricted to non-major updates, so a freshly-published (possibly compromised) version is not pulled into a build the day it lands.

## Protocol versioning as a safety property

`protocol_version` is a full semver string, also exposed as `PROTOCOL_VERSION`. Consumers MUST reject mismatched majors. A decoder that ignores the major digit and parses a future envelope anyway can silently misread fields whose meaning changed, which is a correctness and potentially a safety problem in the host, not just an inconvenience. The full policy, including what counts as a break, is in [CONTRIBUTING.md](CONTRIBUTING.md).
