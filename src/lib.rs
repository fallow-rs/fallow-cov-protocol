//! Versioned envelope types shared between the public `fallow` CLI and the
//! closed-source `fallow-cov` production-coverage sidecar.
//!
//! The public CLI builds a [`Request`] from its static analysis output, spawns
//! the sidecar, writes the request to stdin, and reads a [`Response`] from
//! stdout. Both sides depend on this crate to guarantee contract alignment.
//!
//! # Versioning
//!
//! The top-level `protocol_version` field is a full semver string. Major
//! bumps indicate breaking changes; consumers MUST reject mismatched majors.
//! Minor bumps add optional fields; consumers MUST forward-accept unknown
//! fields and SHOULD map unknown enum variants to [`Feature::Unknown`],
//! [`ReportVerdict::Unknown`], or [`Verdict::Unknown`] rather than erroring.
//!
//! # 0.2 overview
//!
//! This is the first production-shaped contract. The top-level
//! [`ReportVerdict`] (previously `Verdict`) is unchanged in meaning but was
//! renamed to avoid colliding with per-finding [`Verdict`]. Each
//! [`Finding`] and [`HotPath`] now carries a deterministic [`finding_id`] /
//! [`hot_path_id`] hash, a full [`Evidence`] block, and — for findings — a
//! per-function verdict and nullable invocation count. [`Confidence`]
//! gained `VeryHigh` and `None` variants to match the decision table in
//! `.internal/spec-production-coverage.md`.
//!
//! [`StaticFunction::static_used`] and [`StaticFunction::test_covered`] are
//! intentionally required (no `#[serde(default)]`) — a silent default would
//! hide every `safe_to_delete` finding, so 0.1-shape requests must fail
//! deserialization instead of parsing into a wrong answer.
//!
//! # 0.5 changes
//!
//! - [`HotPath`] gained an `end_line` field so consumers can match a hot
//!   path against a PR diff at line granularity, not just file granularity.
//!   The field is `#[serde(default)]` for forward-tolerance with 0.4-shape
//!   sidecars; readers MUST treat a `0` value as a single-line range
//!   (`line..=line`).
//! - `ReportVerdict::HotPathChangesNeeded` was renamed to
//!   [`ReportVerdict::HotPathTouched`]. The wire string changes from
//!   `hot-path-changes-needed` to `hot-path-touched`. The verdict reads as
//!   a state observation rather than an action item; it is informational.
//!
//! # 0.6 changes
//!
//! - New [`FunctionIdentity`] type and optional `identity` field threaded
//!   through [`StaticFunction`], [`Finding`], [`HotPath`], [`BlastRadiusEntry`],
//!   and [`ImportanceEntry`]. Becomes the canonical join key between the
//!   CLI's static function inventory, V8 / Istanbul runtime coverage, test
//!   coverage from `oxc-coverage-instrument`, source-map remapped findings,
//!   and `fallow-cloud` aggregation when present. Older 0.5-shape envelopes
//!   continue to deserialize with `identity: None`; consumers SHOULD prefer
//!   `identity.stable_id` as the join key when present and fall back to the
//!   legacy `file` + `function` + `line` triple otherwise.
//! - New [`function_identity_id`] helper emitting `fallow:fn:<8 hex>`. The
//!   helper hashes only `file + name + start_line + "function"` (NOT
//!   columns) so producers with different positional fidelity (V8 byte
//!   offsets vs Istanbul UTF-16 columns vs oxc spans) agree on the join
//!   key for the same function. Columns survive on the wire as descriptive
//!   metadata for same-line disambiguation in display.
//! - New [`IdentityResolution`] enum with `Resolved` / `Fallback` /
//!   `Unresolved` / `Unknown` variants. Lets cloud aggregation record per
//!   function whether the identity came from a source-map lookup, a
//!   line-only fallback, or remains unresolved.
//! - [`StaticFunction`], [`Finding`], [`HotPath`], [`BlastRadiusEntry`],
//!   and [`ImportanceEntry`] are now `#[non_exhaustive]`. This is a
//!   one-time source-side break for downstream Rust consumers that
//!   constructed these via struct literals (the wire shape is unchanged
//!   and forward-compatible). Future field additions become pure additive
//!   changes; the CHANGELOG calls out the migration path.
//!
//! # 0.7 changes
//!
//! - [`FunctionIdentity::source_hash`] format is now pinned: the first 8
//!   bytes of `SHA-256(<canonical body bytes>)` rendered as 16 lowercase
//!   hex characters. Compute via the new [`source_hash_for`] helper.
//!   Producers that cannot canonicalize the bytes the same way as their
//!   siblings MUST omit the field rather than emit a divergent format.
//!   Closes the cross-producer non-comparability gap that the 0.6.0
//!   "producer-defined, opaque string" wording allowed.
//! - New [`source_hash_for`] helper. Reuses the existing `sha2`
//!   dependency. No new transitive deps. Anchor fixture
//!   (`source_hash_for_anchor_fixture` in the test module) pins a known
//!   input to a known output so producers can self-test.
//! - Tightened rustdoc on [`FunctionIdentity::stable_id_computed`]
//!   documenting the method as a diagnostic helper, NOT a validation
//!   gate. Consumers MUST NOT reject payloads whose `stable_id` differs
//!   from the value returned by the helper.
//! - Byte-level JSON-shape anchor fixtures added for [`FunctionIdentity`]
//!   (full + minimal) plus anchor fixtures for [`blast_radius_id`] and
//!   [`importance_id`] parallel to the existing
//!   [`function_identity_id`] fixture.
//! - [`RiskBand`] and [`CoverageSource`] gain `Unknown` sentinel variants
//!   with `#[serde(other)]`. Future producers MAY add new variants as
//!   additive minor bumps; consumers map unseen variants to `Unknown`
//!   rather than failing deserialization.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Current protocol version. Bumped per the semver rules above.
pub const PROTOCOL_VERSION: &str = "0.7.0";

// -- Request envelope -------------------------------------------------------

/// Sent by the public CLI to the sidecar via stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Semver string of the protocol version this request targets.
    pub protocol_version: String,
    /// License material the sidecar validates before running coverage analysis.
    pub license: License,
    /// Absolute path of the project root under analysis.
    pub project_root: String,
    /// One or more coverage artifacts the sidecar should ingest.
    pub coverage_sources: Vec<CoverageSource>,
    /// Static analysis output the public CLI already produced for this run.
    pub static_findings: StaticFindings,
    /// Optional runtime knobs; all fields default to forward-compatible values.
    #[serde(default)]
    pub options: Options,
}

/// The license material the sidecar should validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    /// Full JWT string, already stripped of whitespace.
    pub jwt: String,
}

/// A single coverage artifact on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CoverageSource {
    /// A single V8 `ScriptCoverage` JSON file.
    V8 {
        /// Absolute path to the V8 coverage JSON file.
        path: String,
    },
    /// A single Istanbul JSON file.
    Istanbul {
        /// Absolute path to the Istanbul coverage JSON file.
        path: String,
    },
    /// A directory containing multiple V8 dumps to merge in memory.
    V8Dir {
        /// Absolute path to the directory containing V8 dump files.
        path: String,
    },
    /// Sentinel for forward-compatibility with newer producers that add
    /// coverage source kinds (e.g. `IstanbulDir`, `TraceEvent`,
    /// `RuntimeBeacon`) the current consumer has not seen yet. Sidecars
    /// receiving an unknown `kind` map the entry here rather than
    /// failing deserialization; the payload fields associated with the
    /// unknown kind are intentionally discarded because the consumer
    /// would not know how to interpret them. Added in protocol 0.7.0.
    #[serde(other)]
    Unknown,
}

/// Static analysis output the public CLI already produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticFindings {
    /// One entry per source file the CLI analyzed.
    pub files: Vec<StaticFile>,
}

/// Static analysis results for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticFile {
    /// Path to the source file, relative to [`Request::project_root`].
    pub path: String,
    /// Functions the CLI discovered in this file.
    pub functions: Vec<StaticFunction>,
}

/// Static analysis results for a single function within a [`StaticFile`].
///
/// Marked `#[non_exhaustive]` in 0.6.0: downstream Rust consumers must
/// stop using struct-literal construction at the type's boundary
/// (destructure-with-`..` for reads still works). No `Default` impl
/// ships on this type. See CHANGELOG for the migration note. The wire
/// shape is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StaticFunction {
    /// Function identifier as reported by the static analyzer. May be an
    /// anonymous placeholder (e.g. `"<anonymous>"`) when the source has no
    /// name at the definition site.
    pub name: String,
    /// 1-indexed line where the function body starts.
    pub start_line: u32,
    /// 1-indexed line where the function body ends (inclusive).
    pub end_line: u32,
    /// Cyclomatic complexity of the function, as computed by the CLI.
    pub cyclomatic: u32,
    /// Whether this function is statically referenced by the module graph.
    /// Drives [`Evidence::static_status`] and gates [`Verdict::SafeToDelete`].
    /// Required: a missing field would silently default to "used" and hide
    /// every `safe_to_delete` finding.
    pub static_used: bool,
    /// Whether this function is covered by the project's test suite.
    /// Drives [`Evidence::test_coverage`]. Required for the same reason as
    /// [`StaticFunction::static_used`].
    pub test_covered: bool,
    /// Static caller count supplied by the CLI's module graph. Added in 0.4.0
    /// for first-class blast-radius output; defaults to zero for older CLIs.
    #[serde(default)]
    pub caller_count: u32,
    /// CODEOWNERS owner count for the containing file. `None` means no
    /// CODEOWNERS data was available; `Some(0)` means CODEOWNERS exists but
    /// no rule matched this file. Added in 0.4.0 for importance scoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_count: Option<u32>,
    /// Canonical function identity introduced in 0.6.0. When present,
    /// consumers SHOULD prefer [`FunctionIdentity::stable_id`] as the
    /// cross-surface join key over the legacy `(file, name, start_line)`
    /// triple. Optional for forward-compat with 0.5-shape CLIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<FunctionIdentity>,
}

/// Runtime knobs. All fields are optional so new options can be added without
/// a breaking change.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Options {
    /// When true the sidecar computes and returns [`Response::hot_paths`].
    /// When false, hot-path computation is skipped entirely.
    #[serde(default)]
    pub include_hot_paths: bool,
    /// Minimum invocation count a function must have to qualify as a hot path.
    /// `None` defers to the sidecar's spec default.
    #[serde(default)]
    pub min_invocations_for_hot: Option<u64>,
    /// Minimum total trace volume before `safe_to_delete` / `review_required`
    /// verdicts are allowed at high/very-high confidence. Below this the
    /// sidecar caps confidence at [`Confidence::Medium`]. Spec default `5000`.
    #[serde(default)]
    pub min_observation_volume: Option<u32>,
    /// Fraction of total `trace_count` below which an invoked function is
    /// classified as [`Verdict::LowTraffic`] instead of `active`. Spec default
    /// `0.001` (0.1%).
    #[serde(default)]
    pub low_traffic_threshold: Option<f64>,
    /// Total number of traces / request-equivalents the coverage dump covers.
    /// Used as the denominator for the low-traffic ratio and gates the
    /// minimum-observation-volume cap. When `None` the sidecar falls back to
    /// the sum of observed invocations in the current request.
    #[serde(default)]
    pub trace_count: Option<u64>,
    /// Number of days of observation the coverage dump represents. Surfaced
    /// verbatim in [`Summary::period_days`] and [`Evidence::observation_days`].
    #[serde(default)]
    pub period_days: Option<u32>,
    /// Number of distinct production deployments that contributed coverage.
    /// Surfaced verbatim in [`Summary::deployments_seen`] and
    /// [`Evidence::deployments_observed`].
    #[serde(default)]
    pub deployments_seen: Option<u32>,
    /// Total observation window in seconds. Finer-grained than
    /// [`Self::period_days`]; used to populate
    /// [`CaptureQuality::window_seconds`]. When `None` the sidecar falls back
    /// to `period_days * 86_400`. Added in protocol 0.3.0.
    #[serde(default)]
    pub window_seconds: Option<u64>,
    /// Number of distinct production instances that contributed coverage.
    /// Used to populate [`CaptureQuality::instances_observed`]. When `None`
    /// the sidecar falls back to [`Self::deployments_seen`]. Added in
    /// protocol 0.3.0.
    #[serde(default)]
    pub instances_observed: Option<u32>,
}

// -- Response envelope ------------------------------------------------------

/// Emitted by the sidecar to stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Semver string of the protocol version the sidecar produced.
    pub protocol_version: String,
    /// Top-level report verdict summarizing the overall state of the run.
    pub verdict: ReportVerdict,
    /// Aggregate statistics across the whole analysis.
    pub summary: Summary,
    /// Per-function findings, one entry per observed or tracked function.
    pub findings: Vec<Finding>,
    /// Hot-path findings, populated only when [`Options::include_hot_paths`]
    /// was set on the request. Defaults to empty.
    #[serde(default)]
    pub hot_paths: Vec<HotPath>,
    /// First-class blast-radius findings. Added in protocol 0.4.0.
    #[serde(default)]
    pub blast_radius: Vec<BlastRadiusEntry>,
    /// First-class runtime importance findings. Added in protocol 0.4.0.
    #[serde(default)]
    pub importance: Vec<ImportanceEntry>,
    /// Grace-period watermark the CLI should render in human output, if any.
    #[serde(default)]
    pub watermark: Option<Watermark>,
    /// Non-fatal errors the sidecar emitted while processing the request.
    #[serde(default)]
    pub errors: Vec<DiagnosticMessage>,
    /// Warnings the sidecar emitted while processing the request.
    #[serde(default)]
    pub warnings: Vec<DiagnosticMessage>,
}

/// Top-level report verdict for a coverage analysis run.
///
/// Was `Verdict` in 0.1. Summarises the overall state of the run;
/// per-finding verdicts live on [`Finding::verdict`]. Unknown variants
/// are forward-mapped to [`ReportVerdict::Unknown`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportVerdict {
    /// No action required — production coverage confirms the codebase.
    Clean,
    /// At least one function in the change set is on a hot path. Reviewers
    /// should pay extra attention to runtime-critical code touched by this
    /// PR. Note: the verdict is informational; matching is line-overlap
    /// against the diff when one is supplied, falling back to file-touch
    /// when only filenames are available.
    HotPathTouched,
    /// At least one finding indicates cold code that should be removed or
    /// reviewed.
    ColdCodeDetected,
    /// The license JWT has expired but the sidecar is still operating inside
    /// the configured grace window. Output is advisory.
    LicenseExpiredGrace,
    /// Sentinel for forward-compatibility with newer sidecars.
    #[serde(other)]
    Unknown,
}

/// Aggregate statistics describing the observed coverage dump.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Number of functions the sidecar could observe in the V8 dump.
    pub functions_tracked: u64,
    /// Functions that received at least one invocation.
    pub functions_hit: u64,
    /// Functions that were tracked but never invoked.
    pub functions_unhit: u64,
    /// Functions the sidecar could not track (lazy-parsed, worker thread, etc.).
    pub functions_untracked: u64,
    /// Ratio of `functions_hit / functions_tracked`, expressed as percent.
    pub coverage_percent: f64,
    /// Total number of observed invocations across all functions in the
    /// current request. Denominator for low-traffic classification.
    pub trace_count: u64,
    /// Days of observation covered by the supplied dump.
    pub period_days: u32,
    /// Distinct deployments contributing to the supplied dump.
    pub deployments_seen: u32,
    /// Quality of the capture window. Populated by the sidecar so the CLI
    /// can render a "short window" warning alongside low-confidence verdicts,
    /// and so the upgrade prompt can quantify the delta cloud mode would
    /// provide. Optional for forward compatibility with 0.2.x sidecars;
    /// 0.3.x always sets it. Added in protocol 0.3.0 per ADR 009 step 6b.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_quality: Option<CaptureQuality>,
}

/// Capture-quality telemetry surfaced alongside the aggregate summary.
///
/// First-touch local-mode captures (`fallow health --production-coverage-dir`)
/// tend to produce short windows (minutes to an hour) against a single
/// instance. Lazy-parsed scripts do not appear in V8 dumps unless they
/// actually executed during the capture window, which a first-time user
/// will read as "the tool is broken" rather than "the capture window is
/// too short." This struct gives the CLI enough information to explain the
/// state honestly and to quantify what continuous cloud monitoring would add.
///
/// Added in protocol 0.3.0 per ADR 009 step 6b, deliverable 2 of 3.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureQuality {
    /// Total observation window in seconds. Finer-grained than
    /// [`Summary::period_days`], which rounds up to whole days. A 12-minute
    /// local capture reports `window_seconds: 720` and `period_days: 1`.
    pub window_seconds: u64,
    /// Number of distinct production instances that contributed to the
    /// dump. Matches [`Summary::deployments_seen`] in the typical case but
    /// is emitted separately so future captures can distinguish "one
    /// deployment seen across many instances" from "many deployments".
    pub instances_observed: u32,
    /// True when the untracked-function ratio exceeds
    /// [`Self::LAZY_PARSE_THRESHOLD_PERCENT`]. Signals that the CLI should
    /// render a "short window" warning: many functions appearing as
    /// untracked most likely reflect lazy-parsed code rather than
    /// unreachable code, and the capture window is not long enough to
    /// distinguish the two.
    pub lazy_parse_warning: bool,
    /// `functions_untracked / functions_tracked` as a percentage. Rounded
    /// to two decimal places for JSON reproducibility. Provided so the CLI
    /// can render the exact ratio that triggered the warning.
    pub untracked_ratio_percent: f64,
}

impl CaptureQuality {
    /// Threshold above which [`Self::lazy_parse_warning`] fires. Chosen so
    /// a short window (minutes) against a typical Node app trips the
    /// warning, while a multi-day continuous capture does not.
    pub const LAZY_PARSE_THRESHOLD_PERCENT: f64 = 30.0;
}

/// A per-function finding combining static analysis and runtime coverage.
///
/// Marked `#[non_exhaustive]` in 0.6.0: downstream Rust consumers must
/// stop using struct-literal construction. The wire shape is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Finding {
    /// Deterministic content hash of shape `fallow:prod:<hash>`. See
    /// [`finding_id`] for the canonical helper. Continues to ship through
    /// 0.6 alongside [`Finding::identity`].
    ///
    /// **`Finding::id` vs [`FunctionIdentity::stable_id`].** They serve
    /// different join axes and must not be conflated:
    ///
    /// - `Finding::id` is the canonical **per-finding suppression key**.
    ///   It hashes `file + function + line + "prod"`, so the same function
    ///   produces a different `id` when its line changes. Agents writing
    ///   suppression files / baselines / CI dedup state key on this
    ///   value to suppress THIS specific finding, not every finding on
    ///   the function.
    /// - [`FunctionIdentity::stable_id`] is the canonical **cross-surface
    ///   join key**. The same function gets ONE `stable_id` across
    ///   findings, hot paths, blast-radius entries, and importance
    ///   entries. Cloud aggregation, traffic-weighted ranking, and any
    ///   "show me this function's history" join uses it.
    ///
    /// New agent suppression formats SHOULD write `identity.stable_id`
    /// when present (stable across line moves) AND retain `Finding::id`
    /// for backwards-compatibility with 0.5-era baselines. Readers MUST
    /// accept both forms during the grace window.
    pub id: String,
    /// Path to the source file, relative to [`Request::project_root`].
    pub file: String,
    /// Function name as reported by the static analyzer. Matches
    /// [`StaticFunction::name`] and [`FunctionIdentity::name`].
    pub function: String,
    /// 1-indexed line number the function starts on. Included in the ID hash
    /// so anonymous functions with identical names but different locations
    /// get distinct IDs.
    pub line: u32,
    /// Per-finding verdict. Describes what the agent should do with this
    /// specific function.
    pub verdict: Verdict,
    /// Raw invocation count from the V8 dump. `None` when the function was
    /// not tracked (lazy-parsed, worker-thread isolate, etc.).
    pub invocations: Option<u64>,
    /// Confidence the sidecar has in this finding's [`Finding::verdict`].
    pub confidence: Confidence,
    /// Evidence rows the sidecar used to arrive at the finding.
    pub evidence: Evidence,
    /// Machine-readable next-step hints for AI agents.
    #[serde(default)]
    pub actions: Vec<Action>,
    /// Canonical function identity introduced in 0.6.0. Optional for
    /// forward-compat with 0.5-shape sidecars. See [`FunctionIdentity`]
    /// for the canonical join semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<FunctionIdentity>,
}

/// Per-finding verdict. Replaces the 0.1 `CallState` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Statically unused AND never invoked in production with coverage tracked.
    SafeToDelete,
    /// Used somewhere statically / by tests / by an untracked call site but
    /// never invoked in production. Needs a human look.
    ReviewRequired,
    /// V8 could not observe the function (lazy-parsed, worker thread,
    /// dynamic code). Nothing can be said about runtime behaviour.
    CoverageUnavailable,
    /// Invoked in production but below the configured low-traffic threshold
    /// relative to `trace_count`. Effectively dead in the current period.
    LowTraffic,
    /// Function was invoked above the low-traffic threshold — not dead.
    Active,
    /// Sentinel for forward-compatibility.
    #[serde(other)]
    Unknown,
}

/// Confidence the sidecar attaches to a [`Finding::verdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Combined static + runtime signal: statically unused AND tracked AND
    /// zero invocations. Strongest delete signal the sidecar emits.
    VeryHigh,
    /// Strong signal — one of static or runtime is dispositive, the other
    /// agrees.
    High,
    /// Signals agree but observation volume or coverage fidelity tempers the
    /// call.
    Medium,
    /// Weak signal — a single data point suggests the verdict but other
    /// evidence is missing or ambiguous.
    Low,
    /// Explicit absence of confidence (e.g. coverage unavailable).
    None,
    /// Sentinel for forward-compatibility.
    #[serde(other)]
    Unknown,
}

/// How a [`FunctionIdentity`] was produced by the upstream coverage
/// pipeline.
///
/// Lets `fallow-cloud` aggregation and the CLI distinguish "this identity
/// was resolved through a source map" from "this is a best-effort
/// line-only fallback" without inspecting the column / span fields
/// directly. Added in protocol 0.6.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityResolution {
    /// Identity was produced from a fully-resolved source location, e.g.
    /// a source-map lookup succeeded for a bundled position, or a direct
    /// AST traversal yielded byte-accurate columns.
    Resolved,
    /// Identity was constructed via a best-effort fallback after a more
    /// precise resolution failed (missing source map, stale offsets, etc).
    /// [`FunctionIdentity::stable_id`] is bit-identical to what a
    /// [`IdentityResolution::Resolved`] producer would emit for the same
    /// function (the hash inputs are `file` / `name` / `start_line`
    /// only, none of which the fallback path loses); the confidence
    /// delta is about the column / span metadata, not the join key
    /// itself. Consumers that weight join confidence on this variant
    /// SHOULD apply the weight to display / disambiguation logic
    /// (column accuracy, source-map traceability), not to the join.
    Fallback,
    /// Identity could not be resolved beyond `file`, `name`, and
    /// `start_line`; columns and `source_hash` are SHOULD-be-absent.
    /// Consumers SHOULD ignore [`FunctionIdentity::start_column`],
    /// [`FunctionIdentity::end_column`], and
    /// [`FunctionIdentity::source_hash`] when `resolution ==
    /// Unresolved`, even if a non-conforming producer populated them.
    /// The protocol intentionally documents rather than enforces this
    /// (a serde-time check would force every consumer to validate);
    /// `unresolved_identity_with_columns_round_trips` locks the
    /// document-but-tolerate stance.
    Unresolved,
    /// Sentinel for forward-compatibility with newer pipelines.
    #[serde(other)]
    Unknown,
}

/// Canonical, versioned identity for a function.
///
/// Becomes the cross-surface join key between the OSS CLI's static
/// function inventory, V8 / Istanbul runtime coverage, test coverage
/// from `oxc-coverage-instrument`, source-map remapped findings, and
/// `fallow-cloud` aggregation when present.
///
/// # Name aliasing
///
/// The `name` field carries the same value as [`StaticFunction::name`]
/// and [`Finding::function`]. The three spellings exist for backwards
/// compatibility with 0.5-and-earlier envelopes: [`Finding::function`]
/// and the legacy `file` / `line` fields are preserved verbatim so
/// display surfaces (CLI human output, SARIF, GitHub annotations) keep
/// working unchanged. New code should read [`FunctionIdentity::name`]
/// when the field is present.
///
/// # Column semantics (load-bearing)
///
/// [`FunctionIdentity::start_column`] and [`FunctionIdentity::end_column`]
/// are **1-indexed UTF-16 column offsets, anchored at the function-body
/// start** (matching Istanbul `fnMap[i].loc.start`, NOT `fnMap[i].decl.start`).
/// Producers MUST normalize their native semantics to this anchor:
///
/// - **Istanbul producers** read `fnMap[i].loc.start.column` (already
///   UTF-16, 0-indexed) and add 1.
/// - **V8 producers** (`fallow-v8-coverage`, `oxc_coverage_v8`) map the
///   function's `startOffset` byte offset to a UTF-16 column via the
///   script text, then add 1.
/// - **AST-based producers** (oxc spans) convert the `Span::start`
///   byte offset to UTF-16 column, then add 1.
///
/// Pick **one** anchor and stick to it: producers picking different
/// anchors for the same function would silently produce different
/// `(start_line, start_column)` pairs for display, but they MUST still
/// produce the same [`FunctionIdentity::stable_id`] because columns are
/// intentionally NOT hashed (see below).
///
/// # Hash exclusion of columns
///
/// [`function_identity_id`] hashes only `file + name + start_line +
/// "function"`. Columns, end positions, and `source_hash` are descriptive
/// metadata for display and same-line disambiguation, but are NOT part of
/// the hash. Rationale: V8 runtime dumps frequently lack column info,
/// while Istanbul fnMap and oxc spans always have it. If columns were
/// hashed, the same function observed by two producers with different
/// fidelity would produce two different `stable_id` values and the
/// cross-surface join would silently break.
///
/// Same-line functions remain distinguishable via the column metadata on
/// the struct itself, just not via the `stable_id`. Cloud aggregation
/// that needs to disambiguate same-line functions during display can use
/// `(start_line, start_column)` as a secondary key once the stable join
/// has happened.
///
/// # Resolution confidence
///
/// [`FunctionIdentity::resolution`] is required (not `Option`) so cloud
/// aggregation can record how each identity was produced. See
/// [`IdentityResolution`] for the variants.
///
/// Added in protocol 0.6.0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionIdentity {
    /// Path to the source file, relative to [`Request::project_root`].
    /// Matches the legacy `file` field on [`Finding`], [`HotPath`],
    /// [`BlastRadiusEntry`], and [`ImportanceEntry`].
    pub file: String,
    /// Function name as reported by the producing pipeline. Matches
    /// [`StaticFunction::name`] and [`Finding::function`].
    pub name: String,
    /// 1-indexed line where the function body starts. Matches
    /// [`StaticFunction::start_line`] and the legacy `line` field on
    /// findings / hot paths / blast-radius / importance entries.
    pub start_line: u32,
    /// 1-indexed UTF-16 column of the first character of the function
    /// body (inclusive). Anchored at the function-body opening
    /// (Istanbul `loc.start`, V8 mapped from byte offset via script
    /// text, oxc `Span::start` mapped to UTF-16). Istanbul's
    /// `loc.start.column` is 0-indexed inclusive, so producers MUST
    /// add 1 when reading from Istanbul fnMap. V8 producers whose
    /// `Coverage.takePreciseCoverage()` offsets originated from a
    /// disk-loaded script source MUST decode the script through UTF-8
    /// before counting UTF-16 code units; offsets from inline-string
    /// scripts already speak UTF-16. Optional: older V8 dumps and
    /// Istanbul artifacts without column data omit this field.
    /// Descriptive metadata only; NOT part of
    /// [`FunctionIdentity::stable_id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u32>,
    /// 1-indexed line where the function body ends (inclusive). Optional.
    /// Mirrors [`StaticFunction::end_line`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    /// 1-indexed UTF-16 column of the last character of the function
    /// body (inclusive). Same indexing and anchor conventions as
    /// [`FunctionIdentity::start_column`]. Note: Istanbul's
    /// `loc.end.column` is 0-indexed AND exclusive (the column AFTER
    /// the last character), so the mapping from Istanbul to this field
    /// is identity (`protocol_end_column = istanbul_end_column`): the
    /// off-by-one between "0-indexed exclusive" and "1-indexed
    /// inclusive" cancels. V8 and oxc producers MUST convert their
    /// byte-offset / span-end to the same 1-indexed-inclusive
    /// convention. Optional. Descriptive metadata only; NOT part of
    /// [`FunctionIdentity::stable_id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
    /// Optional cross-producer tiebreaker for moved or renamed functions
    /// whose positions changed but whose source body is byte-identical.
    ///
    /// Format (pinned in protocol 0.7.0, MUST hold across producers): the
    /// first 8 bytes of `SHA-256(<canonical body bytes>)` rendered as 16
    /// lowercase hex characters. Compute via [`source_hash_for`] so every
    /// producer agrees on the value.
    ///
    /// Canonical body bytes (also pinned): the bytes the producing
    /// compiler or parser sees for the function, including the signature
    /// line and the closing brace, with NO whitespace normalization. Two
    /// producers observing the same function in the same file MUST hand
    /// the same byte slice to [`source_hash_for`].
    ///
    /// Producers that cannot compute this format MUST omit the field
    /// rather than emit a divergent string. Consumers MAY use a present
    /// value as a cross-producer comparability signal; an absent value
    /// carries no information.
    ///
    /// NOT part of [`FunctionIdentity::stable_id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    /// How this identity was produced. See [`IdentityResolution`].
    /// Required: a missing field would silently default to one of the
    /// variants and hide the resolution-confidence signal cloud
    /// aggregation needs.
    pub resolution: IdentityResolution,
    /// Deterministic cross-surface join key of shape `fallow:fn:<8 hex>`.
    /// Producers MUST compute this via [`function_identity_id`] so the
    /// CLI, sidecar, and cloud agree on the value for the same function.
    /// See the struct-level docs for the hash-input rationale.
    pub stable_id: String,
}

impl FunctionIdentity {
    /// Recompute the canonical [`FunctionIdentity::stable_id`] from
    /// `file`, `name`, and `start_line`. Diagnostic helper only: useful
    /// for logging or test assertions that a producer-supplied
    /// `stable_id` was computed via the canonical helper, and for
    /// `debug_assert!(self.stable_id == self.stable_id_computed())` in
    /// producer test suites.
    ///
    /// NOT a validation gate. Consumers MUST NOT reject payloads whose
    /// `stable_id` differs from the value returned here. A future
    /// protocol major that evolves the hash inputs would otherwise turn
    /// every such consumer into a hard-fail on upgrade, defeating the
    /// cross-surface join the value exists to provide.
    #[must_use]
    pub fn stable_id_computed(&self) -> String {
        function_identity_id(&self.file, &self.name, self.start_line)
    }
}

/// Supporting evidence for a [`Finding`]. Mirrors the rows of the decision
/// table in `.internal/spec-production-coverage.md` so the CLI can render the
/// "why" behind each verdict without re-deriving it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// `"unused"` when the CLI marked the function statically unreachable,
    /// `"used"` otherwise.
    pub static_status: String,
    /// `"covered"` or `"not_covered"` by the project's test suite.
    pub test_coverage: String,
    /// `"tracked"` when V8 observed the function, `"untracked"` otherwise.
    pub v8_tracking: String,
    /// Populated when `v8_tracking == "untracked"`. Values mirror the spec:
    /// `"lazy_parsed"`, `"worker_thread"`, `"dynamic_eval"`, `"unknown"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untracked_reason: Option<String>,
    /// Days of observation the decision rests on. Echoes [`Summary::period_days`].
    pub observation_days: u32,
    /// Distinct deployments the decision rests on. Echoes [`Summary::deployments_seen`].
    pub deployments_observed: u32,
}

/// A function the sidecar identified as a hot path in the current dump.
///
/// Marked `#[non_exhaustive]` in 0.6.0: downstream Rust consumers must
/// stop using struct-literal construction. The wire shape is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HotPath {
    /// Deterministic content hash of shape `fallow:hot:<hash>`. See
    /// [`hot_path_id`] for the canonical helper. Continues to ship through
    /// 0.6 alongside [`HotPath::identity`].
    pub id: String,
    /// Path to the source file, relative to [`Request::project_root`].
    pub file: String,
    /// Function name as reported by the static analyzer.
    pub function: String,
    /// 1-indexed line the function starts on.
    pub line: u32,
    /// 1-indexed line the function ends on (inclusive). Mirrors
    /// [`StaticFunction::end_line`] from the request envelope so consumers
    /// can match a hot path against a PR diff at line granularity, not just
    /// file granularity. Older 0.4-shape sidecars omit this field; readers
    /// that receive `0` MUST treat the hot path as a single-line range
    /// (`line..=line`) rather than a span.
    #[serde(default)]
    pub end_line: u32,
    /// Raw invocation count from the V8 dump.
    pub invocations: u64,
    /// Percentile rank of this function's invocation count over the
    /// invocation distribution of the current response's hot paths. `100`
    /// means the busiest function, `0` the quietest that still qualified.
    pub percentile: u8,
    /// Canonical function identity introduced in 0.6.0. Optional for
    /// forward-compat with 0.5-shape sidecars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<FunctionIdentity>,
}

/// Risk band for a blast-radius entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskBand {
    /// Low caller fan-in / traffic-weighted reach.
    Low,
    /// Moderate caller fan-in / traffic-weighted reach.
    Medium,
    /// High caller fan-in / traffic-weighted reach.
    High,
    /// Sentinel for forward-compatibility with newer producers that add
    /// risk bands (e.g. `Critical`, `Negligible`) the current consumer
    /// has not seen yet. Older consumers map the unknown variant here
    /// rather than failing deserialization. Added in protocol 0.7.0.
    #[serde(other)]
    Unknown,
}

/// A function with meaningful static or traffic-weighted blast radius.
///
/// Marked `#[non_exhaustive]` in 0.6.0: downstream Rust consumers must
/// stop using struct-literal construction. The wire shape is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlastRadiusEntry {
    /// Deterministic content hash of shape `fallow:blast:<hash>`.
    /// Continues to ship through 0.6 alongside [`BlastRadiusEntry::identity`].
    pub id: String,
    /// Path to the source file, relative to [`Request::project_root`].
    pub file: String,
    /// Function name as reported by the static analyzer.
    pub function: String,
    /// 1-indexed line the function starts on.
    pub line: u32,
    /// Static caller count supplied by the CLI module graph.
    pub caller_count: u32,
    /// Caller count weighted by observed traffic. Local mode uses the
    /// sidecar's current best-effort traffic proxy; cloud mode may replace
    /// this with summed caller invocations.
    pub caller_count_weighted_by_traffic: u64,
    /// Distinct git SHAs that touched this function in the observation window.
    /// Cloud-only; omitted for local coverage artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploys_touched: Option<u32>,
    /// Deterministic low / medium / high band.
    pub risk_band: RiskBand,
    /// Canonical function identity introduced in 0.6.0. Optional for
    /// forward-compat with 0.5-shape sidecars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<FunctionIdentity>,
}

/// A function ranked by runtime traffic, complexity, and ownership risk.
///
/// Marked `#[non_exhaustive]` in 0.6.0: downstream Rust consumers must
/// stop using struct-literal construction. The wire shape is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportanceEntry {
    /// Deterministic content hash of shape `fallow:importance:<hash>`.
    /// Continues to ship through 0.6 alongside [`ImportanceEntry::identity`].
    pub id: String,
    /// Path to the source file, relative to [`Request::project_root`].
    pub file: String,
    /// Function name as reported by the static analyzer.
    pub function: String,
    /// 1-indexed line the function starts on.
    pub line: u32,
    /// Raw invocation count used for the traffic component.
    pub invocations: u64,
    /// Cyclomatic complexity supplied by the CLI health pipeline.
    pub cyclomatic: u32,
    /// Number of CODEOWNERS owners; `0` means ownership is absent or unowned.
    pub owner_count: u32,
    /// 0-100 importance score. The formula is intentionally simple and
    /// documented by the sidecar implementation so it can be tuned later.
    pub importance_score: f64,
    /// Templated one-sentence explanation, not free-form model text.
    pub reason: String,
    /// Canonical function identity introduced in 0.6.0. Optional for
    /// forward-compat with 0.5-shape sidecars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<FunctionIdentity>,
}

/// Machine-readable next-step hint for AI agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Short identifier for the action kind (e.g. `"delete"`, `"inline"`,
    /// `"review"`). Free-form on the wire to keep forward compatibility.
    pub kind: String,
    /// Human-readable one-liner describing the suggested action.
    pub description: String,
    /// Whether the CLI can apply this action non-interactively.
    #[serde(default)]
    pub auto_fixable: bool,
}

/// What to render in the human output when the license is in the grace window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Watermark {
    /// The trial period has ended.
    TrialExpired,
    /// A paid license has expired but the sidecar is still inside the grace
    /// window.
    LicenseExpiredGrace,
    /// Sentinel for forward-compatibility.
    #[serde(other)]
    Unknown,
}

/// Error / warning surfaced by the sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticMessage {
    /// Stable machine-readable diagnostic code (e.g. `"COV_DUMP_PARSE"`).
    pub code: String,
    /// Human-readable description of the diagnostic.
    pub message: String,
}

// -- Stable ID helpers -----------------------------------------------------

/// Compute the deterministic [`Finding::id`] for a production-coverage finding.
///
/// Emits `fallow:prod:<hash>` where `<hash>` is the first 8 hex characters of
/// `SHA-256(file + function + line + "prod")`. The concatenation is plain,
/// unseparated UTF-8. The canonical order MUST stay identical across protocol
/// revisions; changing it breaks ID stability across runs and invalidates any
/// consumer that persists IDs (CI deduplication, suppression, agent
/// cross-references).
#[must_use]
pub fn finding_id(file: &str, function: &str, line: u32) -> String {
    format!("fallow:prod:{}", content_hash(file, function, line, "prod"))
}

/// Compute the deterministic [`HotPath::id`] for a hot-path finding. Uses the
/// same canonical order as [`finding_id`] with kind `"hot"`, emitting
/// `fallow:hot:<hash>`.
#[must_use]
pub fn hot_path_id(file: &str, function: &str, line: u32) -> String {
    format!("fallow:hot:{}", content_hash(file, function, line, "hot"))
}

/// Compute the deterministic [`BlastRadiusEntry::id`] for a blast-radius entry.
#[must_use]
pub fn blast_radius_id(file: &str, function: &str, line: u32) -> String {
    format!(
        "fallow:blast:{}",
        content_hash(file, function, line, "blast")
    )
}

/// Compute the deterministic [`ImportanceEntry::id`] for an importance entry.
#[must_use]
pub fn importance_id(file: &str, function: &str, line: u32) -> String {
    format!(
        "fallow:importance:{}",
        content_hash(file, function, line, "importance")
    )
}

/// Compute the deterministic [`FunctionIdentity::stable_id`] for a function.
///
/// Emits `fallow:fn:<hash>` where `<hash>` is the first 8 hex characters of
/// `SHA-256(file + name + start_line + "function")`. The concatenation is
/// plain, unseparated UTF-8.
///
/// # Why columns are NOT in the hash
///
/// The canonical hash inputs intentionally exclude column / span / source
/// hash metadata. Two producers observing the same function with
/// different positional fidelity (V8 dumps that lack columns vs Istanbul
/// fnMap that has them, vs oxc spans that have byte-accurate positions)
/// MUST produce the same `stable_id` so the cross-surface join holds.
/// Columns survive on the wire (see [`FunctionIdentity::start_column`])
/// for display and same-line disambiguation, but are NOT part of the
/// hash.
///
/// # Why there is no `kind` parameter
///
/// Unlike [`finding_id`] / [`hot_path_id`] / [`blast_radius_id`] /
/// [`importance_id`], which are per-surface stable IDs, this helper
/// produces ONE canonical ID per function across every surface the
/// function appears on (findings, hot paths, blast radius, importance,
/// static inventory). That is the whole point of the cross-surface join.
///
/// The canonical input order (`file`, `name`, `start_line`, then the
/// literal salt `"function"`) and truncation (first 4 SHA-256 bytes
/// rendered as 8 lowercase hex chars) are part of the wire contract.
/// Changing any of them breaks ID stability across runs and invalidates
/// any consumer that persists IDs (CI deduplication, suppression files,
/// agent cross-references) and is therefore always a major bump.
///
/// Added in protocol 0.6.0.
#[must_use]
pub fn function_identity_id(file: &str, name: &str, start_line: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file.as_bytes());
    hasher.update(name.as_bytes());
    hasher.update(start_line.to_string().as_bytes());
    hasher.update(b"function");
    let digest = hasher.finalize();
    format!("fallow:fn:{}", hex_prefix(&digest, 4))
}

/// Compute the canonical [`FunctionIdentity::source_hash`] for the given
/// canonical body bytes.
///
/// Emits 16 lowercase hex characters: the first 8 bytes of `SHA-256(body)`.
/// No `fallow:` prefix because the value is a content tiebreaker, not a
/// qualified ID; see [`FunctionIdentity::source_hash`] for the field
/// rustdoc and the canonicalization rule (signature line plus body plus
/// closing brace, no whitespace normalization).
///
/// Cross-producer comparability is the whole point: V8, Istanbul, oxc,
/// and beacon producers that all derive the same canonical body for the
/// same function MUST produce the same string from this helper. Producers
/// that cannot canonicalize the bytes the same way as their siblings MUST
/// omit [`FunctionIdentity::source_hash`] rather than emit a divergent
/// format.
///
/// Truncation (first 8 SHA-256 bytes to 16 hex chars) and lowercase hex
/// encoding are part of the wire contract. Changing either invalidates
/// every previously persisted `source_hash` value and is therefore always
/// a major bump.
///
/// Added in protocol 0.7.0.
#[must_use]
pub fn source_hash_for(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    let digest = hasher.finalize();
    hex_prefix(&digest, 8)
}

/// Canonical content hash shared by the stable ID helpers. The input order
/// (file, function, line, kind) and truncation (first 4 SHA-256 bytes to 8
/// hex chars) are part of the wire contract; see [`finding_id`] for the
/// rationale.
fn content_hash(file: &str, function: &str, line: u32, kind: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file.as_bytes());
    hasher.update(function.as_bytes());
    hasher.update(line.to_string().as_bytes());
    hasher.update(kind.as_bytes());
    let digest = hasher.finalize();
    hex_prefix(&digest, 4)
}

/// Encode the first `bytes` bytes of `digest` as lowercase hex, returning
/// a `2 * bytes`-character string. Kept as a single helper so every
/// truncation length used by the wire contract is auditable from one
/// place. Total by construction: `HEX` is ASCII and `char::from(u8)` is
/// infallible, so the helper never panics. If `bytes > digest.len()` the
/// iterator silently caps at `digest.len()`; the SHA-256 callers all
/// satisfy `bytes <= 32`.
fn hex_prefix(digest: &[u8], bytes: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes * 2);
    for &byte in digest.iter().take(bytes) {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

// -- License features -------------------------------------------------------

/// Feature flags present in the license JWT's `features` claim.
///
/// Wire format stays a string array (forward-compatible); new variants are
/// additive in minor protocol bumps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    /// Production coverage intelligence (the primary sidecar feature).
    ProductionCoverage,
    /// Portfolio dashboard for cross-project rollups. Deferred.
    PortfolioDashboard,
    /// MCP cloud tools integration. Deferred.
    McpCloudTools,
    /// Cross-repo aggregation and deduplication. Deferred.
    CrossRepoAggregation,
    /// Sentinel for forward-compatibility.
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_constant_is_v0_7() {
        assert!(PROTOCOL_VERSION.starts_with("0.7."));
    }

    #[test]
    fn unknown_report_verdict_round_trips() {
        let json = r#""something-new""#;
        let verdict: ReportVerdict = serde_json::from_str(json).unwrap();
        assert!(matches!(verdict, ReportVerdict::Unknown));
    }

    #[test]
    fn unknown_verdict_round_trips() {
        let json = r#""future_state""#;
        let verdict: Verdict = serde_json::from_str(json).unwrap();
        assert!(matches!(verdict, Verdict::Unknown));
    }

    #[test]
    fn unknown_confidence_round_trips() {
        let json = r#""ultra_high""#;
        let confidence: Confidence = serde_json::from_str(json).unwrap();
        assert!(matches!(confidence, Confidence::Unknown));
    }

    #[test]
    fn unknown_feature_round_trips() {
        let json = r#""future_feature""#;
        let feature: Feature = serde_json::from_str(json).unwrap();
        assert!(matches!(feature, Feature::Unknown));
    }

    #[test]
    fn unknown_watermark_round_trips() {
        let json = r#""something-else""#;
        let watermark: Watermark = serde_json::from_str(json).unwrap();
        assert!(matches!(watermark, Watermark::Unknown));
    }

    #[test]
    fn unknown_risk_band_round_trips() {
        // Forward-compat sentinel added in protocol 0.7.0. Future
        // producers MAY add risk bands beyond Low / Medium / High; older
        // consumers MUST map them to Unknown rather than failing
        // deserialization. Adding a new variant is a soft minor bump
        // only because this sentinel is present.
        let json = r#""critical""#;
        let band: RiskBand = serde_json::from_str(json).unwrap();
        assert!(matches!(band, RiskBand::Unknown));
    }

    #[test]
    fn unknown_coverage_source_round_trips() {
        // Forward-compat sentinel added in protocol 0.7.0. Future
        // producers MAY add coverage source kinds beyond v8 / istanbul /
        // v8-dir (e.g., istanbul-dir, trace-event, runtime-beacon);
        // older sidecars MUST map them to Unknown rather than failing
        // deserialization. The payload fields associated with the
        // unknown kind are intentionally discarded because the consumer
        // would not know how to interpret them.
        let json = r#"{"kind":"trace-event","path":"/tmp/x.trace"}"#;
        let src: CoverageSource = serde_json::from_str(json).unwrap();
        assert!(matches!(src, CoverageSource::Unknown));
    }

    #[test]
    fn coverage_source_kebab_case() {
        let json = r#"{"kind":"v8-dir","path":"/tmp/dumps"}"#;
        let src: CoverageSource = serde_json::from_str(json).unwrap();
        assert!(matches!(src, CoverageSource::V8Dir { .. }));
    }

    #[test]
    fn response_allows_unknown_fields() {
        let json = r#"{
            "protocol_version": "0.2.0",
            "verdict": "clean",
            "summary": {
                "functions_tracked": 0,
                "functions_hit": 0,
                "functions_unhit": 0,
                "functions_untracked": 0,
                "coverage_percent": 0.0,
                "trace_count": 0,
                "period_days": 0,
                "deployments_seen": 0
            },
            "findings": [],
            "future_top_level_field": 42
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.protocol_version, "0.2.0");
    }

    #[test]
    fn finding_id_is_deterministic() {
        let first = finding_id("src/a.ts", "foo", 42);
        let second = finding_id("src/a.ts", "foo", 42);
        assert_eq!(first, second);
        assert!(first.starts_with("fallow:prod:"));
        assert_eq!(first.len(), "fallow:prod:".len() + 8);
    }

    #[test]
    fn capture_quality_round_trips() {
        let q = CaptureQuality {
            window_seconds: 720,
            instances_observed: 1,
            lazy_parse_warning: true,
            untracked_ratio_percent: 42.5,
        };
        let json = serde_json::to_string(&q).unwrap();
        let parsed: CaptureQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(q, parsed);
    }

    #[test]
    fn summary_without_capture_quality_deserializes() {
        // 0.2.x sidecars produced this shape; 0.3.x deserialization must
        // still accept it so a mixed rollout (newer CLI, older sidecar)
        // does not hard-fail.
        let json = r#"{
            "functions_tracked": 10,
            "functions_hit": 5,
            "functions_unhit": 5,
            "functions_untracked": 0,
            "coverage_percent": 50.0,
            "trace_count": 100,
            "period_days": 1,
            "deployments_seen": 1
        }"#;
        let summary: Summary = serde_json::from_str(json).unwrap();
        assert!(summary.capture_quality.is_none());
    }

    #[test]
    fn summary_with_capture_quality_round_trips() {
        let summary = Summary {
            functions_tracked: 10,
            functions_hit: 5,
            functions_unhit: 5,
            functions_untracked: 3,
            coverage_percent: 50.0,
            trace_count: 100,
            period_days: 1,
            deployments_seen: 1,
            capture_quality: Some(CaptureQuality {
                window_seconds: 720,
                instances_observed: 1,
                lazy_parse_warning: true,
                untracked_ratio_percent: 30.0,
            }),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: Summary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.capture_quality, parsed.capture_quality);
    }

    #[test]
    fn lazy_parse_threshold_is_30_percent() {
        // Anchored so a bump forces a deliberate decision and a CHANGELOG
        // entry rather than a silent tweak.
        assert!((CaptureQuality::LAZY_PARSE_THRESHOLD_PERCENT - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hot_path_id_differs_from_finding_id() {
        let f = finding_id("src/a.ts", "foo", 42);
        let h = hot_path_id("src/a.ts", "foo", 42);
        assert_ne!(f[f.len() - 8..], h[h.len() - 8..]);
    }

    #[test]
    fn finding_id_changes_with_line() {
        assert_ne!(
            finding_id("src/a.ts", "foo", 10),
            finding_id("src/a.ts", "foo", 11),
        );
    }

    #[test]
    fn finding_id_changes_with_file() {
        assert_ne!(
            finding_id("src/a.ts", "foo", 42),
            finding_id("src/b.ts", "foo", 42),
        );
    }

    #[test]
    fn finding_id_changes_with_function() {
        assert_ne!(
            finding_id("src/a.ts", "foo", 42),
            finding_id("src/a.ts", "bar", 42),
        );
    }

    #[test]
    fn finding_id_is_lowercase_hex_ascii() {
        // Canonical form is lowercase hex — downstream dedup keys on string
        // equality, so an accidental uppercase switch would break persisted IDs.
        let id = finding_id("src/a.ts", "foo", 42);
        let hash = &id["fallow:prod:".len()..];
        assert!(
            hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "expected lowercase hex, got {hash}"
        );
    }

    #[test]
    fn evidence_round_trips_with_untracked_reason() {
        let evidence = Evidence {
            static_status: "used".to_owned(),
            test_coverage: "not_covered".to_owned(),
            v8_tracking: "untracked".to_owned(),
            untracked_reason: Some("lazy_parsed".to_owned()),
            observation_days: 30,
            deployments_observed: 14,
        };
        let json = serde_json::to_string(&evidence).unwrap();
        assert!(json.contains("\"untracked_reason\":\"lazy_parsed\""));
        let back: Evidence = serde_json::from_str(&json).unwrap();
        assert_eq!(back.untracked_reason.as_deref(), Some("lazy_parsed"));
    }

    #[test]
    fn static_function_requires_static_used_and_test_covered() {
        // Belt-and-suspenders: a 0.1-shape request (no static_used / test_covered)
        // must fail deserialization rather than silently defaulting to "used + covered"
        // which would hide every safe_to_delete finding.
        let json = r#"{"name":"foo","start_line":1,"end_line":2,"cyclomatic":1}"#;
        let result: Result<StaticFunction, _> = serde_json::from_str(json);
        let err = result
            .expect_err("missing static_used / test_covered must fail")
            .to_string();
        assert!(
            err.contains("static_used") || err.contains("test_covered"),
            "unexpected error text: {err}"
        );
    }

    #[test]
    fn options_defaults_when_fields_omitted() {
        let json = "{}";
        let options: Options = serde_json::from_str(json).unwrap();
        assert!(!options.include_hot_paths);
        assert!(options.min_invocations_for_hot.is_none());
        assert!(options.min_observation_volume.is_none());
        assert!(options.low_traffic_threshold.is_none());
        assert!(options.trace_count.is_none());
        assert!(options.period_days.is_none());
        assert!(options.deployments_seen.is_none());
    }

    #[test]
    fn stable_ids_are_distinct_by_kind() {
        let finding = finding_id("src/a.ts", "foo", 42);
        let hot = hot_path_id("src/a.ts", "foo", 42);
        let blast = blast_radius_id("src/a.ts", "foo", 42);
        let importance = importance_id("src/a.ts", "foo", 42);
        let function = function_identity_id("src/a.ts", "foo", 42);
        assert!(blast.starts_with("fallow:blast:"));
        assert!(importance.starts_with("fallow:importance:"));
        assert!(function.starts_with("fallow:fn:"));
        assert_eq!(blast.len(), "fallow:blast:".len() + 8);
        assert_eq!(importance.len(), "fallow:importance:".len() + 8);
        assert_eq!(function.len(), "fallow:fn:".len() + 8);
        let suffixes = [
            &finding[finding.len() - 8..],
            &hot[hot.len() - 8..],
            &blast[blast.len() - 8..],
            &importance[importance.len() - 8..],
            &function[function.len() - 8..],
        ];
        for (index, suffix) in suffixes.iter().enumerate() {
            assert!(
                suffixes.iter().skip(index + 1).all(|other| other != suffix),
                "ID suffix collision across finding kinds"
            );
        }
    }

    #[test]
    fn evidence_omits_untracked_reason_when_none() {
        let evidence = Evidence {
            static_status: "unused".to_owned(),
            test_coverage: "covered".to_owned(),
            v8_tracking: "tracked".to_owned(),
            untracked_reason: None,
            observation_days: 30,
            deployments_observed: 14,
        };
        let json = serde_json::to_string(&evidence).unwrap();
        assert!(
            !json.contains("untracked_reason"),
            "expected untracked_reason omitted, got {json}"
        );
    }

    // -- FunctionIdentity v2 (protocol 0.6.0) -----------------------------

    fn fixture_identity_full() -> FunctionIdentity {
        let stable_id = function_identity_id("src/render.tsx", "render", 42);
        FunctionIdentity {
            file: "src/render.tsx".to_owned(),
            name: "render".to_owned(),
            start_line: 42,
            start_column: Some(5),
            end_line: Some(67),
            end_column: Some(2),
            source_hash: Some(source_hash_for(b"function render() {}")),
            resolution: IdentityResolution::Resolved,
            stable_id,
        }
    }

    #[test]
    fn unknown_identity_resolution_round_trips() {
        let json = r#""future_state""#;
        let parsed: IdentityResolution = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, IdentityResolution::Unknown));
    }

    #[test]
    fn function_identity_round_trips_with_all_fields_set() {
        let identity = fixture_identity_full();
        let json = serde_json::to_string(&identity).unwrap();
        let parsed: FunctionIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity, parsed);
    }

    #[test]
    fn function_identity_omits_columns_when_none() {
        let identity = FunctionIdentity {
            file: "src/a.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 1,
            start_column: None,
            end_line: None,
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Unresolved,
            stable_id: function_identity_id("src/a.ts", "foo", 1),
        };
        let json = serde_json::to_string(&identity).unwrap();
        assert!(
            !json.contains("start_column"),
            "expected start_column omitted, got {json}"
        );
        assert!(
            !json.contains("end_line"),
            "expected end_line omitted, got {json}"
        );
        assert!(
            !json.contains("end_column"),
            "expected end_column omitted, got {json}"
        );
        assert!(
            !json.contains("source_hash"),
            "expected source_hash omitted, got {json}"
        );
    }

    #[test]
    fn function_identity_round_trips_with_some_columns() {
        let identity = FunctionIdentity {
            file: "src/b.ts".to_owned(),
            name: "bar".to_owned(),
            start_line: 10,
            start_column: Some(3),
            end_line: None,
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Fallback,
            stable_id: function_identity_id("src/b.ts", "bar", 10),
        };
        let json = serde_json::to_string(&identity).unwrap();
        assert!(json.contains("\"start_column\":3"));
        assert!(!json.contains("end_line"));
        assert!(!json.contains("end_column"));
        let parsed: FunctionIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity, parsed);
    }

    #[test]
    fn function_identity_id_is_deterministic() {
        let first = function_identity_id("src/a.ts", "foo", 42);
        let second = function_identity_id("src/a.ts", "foo", 42);
        assert_eq!(first, second);
    }

    #[test]
    fn function_identity_id_changes_with_file() {
        assert_ne!(
            function_identity_id("src/a.ts", "foo", 42),
            function_identity_id("src/b.ts", "foo", 42),
        );
    }

    #[test]
    fn function_identity_id_changes_with_name() {
        assert_ne!(
            function_identity_id("src/a.ts", "foo", 42),
            function_identity_id("src/a.ts", "bar", 42),
        );
    }

    #[test]
    fn function_identity_id_changes_with_start_line() {
        assert_ne!(
            function_identity_id("src/a.ts", "foo", 10),
            function_identity_id("src/a.ts", "foo", 11),
        );
    }

    #[test]
    fn function_identity_id_unchanged_by_columns() {
        // Cross-producer agreement test (BLOCK fix from panel review):
        // V8 producers without column info MUST produce the same
        // stable_id as Istanbul producers with column info, otherwise the
        // cross-surface join silently breaks.
        let no_columns = FunctionIdentity {
            file: "src/a.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 42,
            start_column: None,
            end_line: None,
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Unresolved,
            stable_id: function_identity_id("src/a.ts", "foo", 42),
        };
        let with_columns = FunctionIdentity {
            file: "src/a.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 42,
            start_column: Some(5),
            end_line: Some(67),
            end_column: Some(2),
            source_hash: Some(source_hash_for(b"function foo() {}")),
            resolution: IdentityResolution::Resolved,
            stable_id: function_identity_id("src/a.ts", "foo", 42),
        };
        assert_eq!(no_columns.stable_id, with_columns.stable_id);
        assert_eq!(no_columns.stable_id, no_columns.stable_id_computed());
        assert_eq!(with_columns.stable_id, with_columns.stable_id_computed());
    }

    #[test]
    fn function_identity_id_format_is_fallow_fn_8hex() {
        let id = function_identity_id("src/a.ts", "foo", 42);
        assert!(id.starts_with("fallow:fn:"));
        let hash = &id["fallow:fn:".len()..];
        assert_eq!(hash.len(), 8, "expected 8 hex chars, got {hash}");
        assert!(
            hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "expected lowercase hex, got {hash}"
        );
    }

    #[test]
    fn function_identity_stable_id_matches_helper() {
        let identity = fixture_identity_full();
        assert_eq!(identity.stable_id, identity.stable_id_computed());
    }

    #[test]
    fn function_identity_id_anchor_fixture() {
        // Conformance fixture: producers (fallow CLI, fallow-cov sidecar,
        // browser/node beacons) MUST run this exact input through their
        // own pipelines and obtain the same string. Divergence here means
        // the cross-surface join would silently break in production.
        assert_eq!(
            function_identity_id("src/render.tsx", "render", 42),
            "fallow:fn:43629542",
        );
    }

    #[test]
    fn finding_without_identity_deserializes() {
        // 0.5-shape Finding (no identity field) must continue to parse
        // with identity: None for forward-compat with older sidecars.
        let json = r#"{
            "id": "fallow:prod:deadbeef",
            "file": "src/a.ts",
            "function": "foo",
            "line": 42,
            "verdict": "active",
            "invocations": 100,
            "confidence": "high",
            "evidence": {
                "static_status": "used",
                "test_coverage": "covered",
                "v8_tracking": "tracked",
                "observation_days": 30,
                "deployments_observed": 14
            }
        }"#;
        let finding: Finding = serde_json::from_str(json).unwrap();
        assert!(finding.identity.is_none());
        assert_eq!(finding.function, "foo");
    }

    #[test]
    fn static_function_without_identity_deserializes() {
        // 0.5-shape StaticFunction (no identity field) must parse with
        // identity: None for forward-compat with older CLIs.
        let json = r#"{
            "name": "foo",
            "start_line": 1,
            "end_line": 5,
            "cyclomatic": 2,
            "static_used": true,
            "test_covered": false
        }"#;
        let func: StaticFunction = serde_json::from_str(json).unwrap();
        assert!(func.identity.is_none());
    }

    #[test]
    fn hot_path_without_identity_deserializes() {
        // 0.5-shape HotPath (no identity field) must parse with
        // identity: None for forward-compat with older sidecars.
        let json = r#"{
            "id": "fallow:hot:deadbeef",
            "file": "src/a.ts",
            "function": "foo",
            "line": 42,
            "end_line": 67,
            "invocations": 1000,
            "percentile": 95
        }"#;
        let hot: HotPath = serde_json::from_str(json).unwrap();
        assert!(hot.identity.is_none());
        assert_eq!(hot.function, "foo");
    }

    #[test]
    fn blast_radius_entry_without_identity_deserializes() {
        let json = r#"{
            "id": "fallow:blast:deadbeef",
            "file": "src/a.ts",
            "function": "foo",
            "line": 42,
            "caller_count": 10,
            "caller_count_weighted_by_traffic": 5000,
            "risk_band": "high"
        }"#;
        let entry: BlastRadiusEntry = serde_json::from_str(json).unwrap();
        assert!(entry.identity.is_none());
        assert_eq!(entry.caller_count, 10);
    }

    #[test]
    fn importance_entry_without_identity_deserializes() {
        let json = r#"{
            "id": "fallow:importance:deadbeef",
            "file": "src/a.ts",
            "function": "foo",
            "line": 42,
            "invocations": 5000,
            "cyclomatic": 7,
            "owner_count": 2,
            "importance_score": 87.5,
            "reason": "high traffic, complex, narrowly owned"
        }"#;
        let entry: ImportanceEntry = serde_json::from_str(json).unwrap();
        assert!(entry.identity.is_none());
        assert!((entry.importance_score - 87.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stable_id_field_required_on_function_identity() {
        // stable_id is the canonical cross-surface join key; a missing
        // field would silently default to an empty string and every
        // downstream dedup keyed on stable_id would collapse to one
        // bucket. Locks the explicit non-default contract.
        let json = r#"{
            "file": "src/a.ts",
            "name": "foo",
            "start_line": 42,
            "resolution": "resolved"
        }"#;
        let result: Result<FunctionIdentity, _> = serde_json::from_str(json);
        let err = result
            .expect_err("missing stable_id must fail deserialization")
            .to_string();
        assert!(err.contains("stable_id"), "unexpected error text: {err}");
    }

    #[test]
    fn identity_resolution_field_required_on_function_identity() {
        // resolution carries the source-map / fallback confidence signal
        // cloud aggregation relies on; a missing field would silently
        // default and hide the difference between Resolved and Unresolved.
        let json = r#"{
            "file": "src/a.ts",
            "name": "foo",
            "start_line": 42,
            "stable_id": "fallow:fn:43629542"
        }"#;
        let result: Result<FunctionIdentity, _> = serde_json::from_str(json);
        let err = result
            .expect_err("missing resolution must fail deserialization")
            .to_string();
        assert!(err.contains("resolution"), "unexpected error text: {err}");
    }

    #[test]
    fn unresolved_identity_with_columns_round_trips() {
        // Locks the "document, don't enforce" stance: the protocol's
        // rustdoc on IdentityResolution::Unresolved says columns should
        // be absent, but serde does not reject a non-conforming
        // producer that emits them anyway. Cloud / agent consumers
        // SHOULD ignore the columns when resolution == Unresolved.
        // A serde-time rejection would force every consumer to validate
        // and would not actually fix the producer; we tolerate and
        // document instead.
        let json = r#"{
            "file": "src/a.ts",
            "name": "foo",
            "start_line": 42,
            "start_column": 5,
            "resolution": "unresolved",
            "stable_id": "fallow:fn:43629542"
        }"#;
        let parsed: FunctionIdentity = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed.resolution, IdentityResolution::Unresolved));
        assert_eq!(parsed.start_column, Some(5));
    }

    #[test]
    fn same_line_functions_distinct_by_identity_via_column_metadata() {
        // Two anonymous callbacks on the same line of the same file with
        // the same name collide on stable_id (intentional: cross-producer
        // join). Display surfaces disambiguate via the column metadata
        // which survives on the wire even though it does not enter the
        // hash. This is the explicit panel-review BLOCK fix: columns
        // ride along for display, NOT for hashing.
        let first = FunctionIdentity {
            file: "src/a.ts".to_owned(),
            name: "<anonymous>".to_owned(),
            start_line: 7,
            start_column: Some(12),
            end_line: Some(7),
            end_column: Some(40),
            source_hash: None,
            resolution: IdentityResolution::Resolved,
            stable_id: function_identity_id("src/a.ts", "<anonymous>", 7),
        };
        let second = FunctionIdentity {
            start_column: Some(50),
            end_column: Some(78),
            ..first.clone()
        };
        assert_eq!(first.stable_id, second.stable_id);
        assert_ne!(first.start_column, second.start_column);
        // Column metadata survives serde so display can disambiguate.
        let json_first = serde_json::to_string(&first).unwrap();
        let json_second = serde_json::to_string(&second).unwrap();
        assert_ne!(json_first, json_second);
        assert!(json_first.contains("\"start_column\":12"));
        assert!(json_second.contains("\"start_column\":50"));
    }

    #[test]
    fn function_identity_full_json_shape_anchor_fixture() {
        // Byte-equal wire-shape pin (panel item 2). Catches silent
        // field-reorder regressions and skip_serializing_if drift on the
        // every-Option-Some path that the omits-when-none test cannot
        // catch in isolation. Producers and JSON-diff tooling consume this
        // exact byte sequence; changing the literal is a wire-shape break.
        let identity = fixture_identity_full();
        let json = serde_json::to_string(&identity).unwrap();
        assert_eq!(
            json,
            r#"{"file":"src/render.tsx","name":"render","start_line":42,"start_column":5,"end_line":67,"end_column":2,"source_hash":"e25ba02c5e53651f","resolution":"resolved","stable_id":"fallow:fn:43629542"}"#,
        );
    }

    #[test]
    fn function_identity_minimal_json_shape_anchor_fixture() {
        // Byte-equal wire-shape pin for the minimum required surface
        // (panel item 2 companion). The four skip_serializing_if Options
        // are absent. Pairs with the full-shape fixture above so a future
        // PR cannot regress either the Some path or the None path without
        // visibly editing a literal here.
        let identity = FunctionIdentity {
            file: "src/minimal.ts".to_owned(),
            name: "f".to_owned(),
            start_line: 1,
            start_column: None,
            end_line: None,
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Resolved,
            stable_id: function_identity_id("src/minimal.ts", "f", 1),
        };
        let json = serde_json::to_string(&identity).unwrap();
        assert_eq!(
            json,
            r#"{"file":"src/minimal.ts","name":"f","start_line":1,"resolution":"resolved","stable_id":"fallow:fn:a76cfb64"}"#,
        );
    }

    #[test]
    fn identity_resolution_unresolved_shape_fixture() {
        // Failed-join consumer fixture (panel cross-cutting item from
        // Diego and Aria). Documents the on-wire shape an MCP agent or
        // cloud aggregator sees when a producer could not resolve the
        // identity beyond file / name / start_line: columns and
        // source_hash MUST be absent, resolution MUST serialize as
        // "unresolved". The protocol documents this stance but does not
        // enforce it via serde; see IdentityResolution::Unresolved
        // rustdoc and unresolved_identity_with_columns_round_trips.
        let identity = FunctionIdentity {
            file: "src/unresolved.ts".to_owned(),
            name: "mystery_fn".to_owned(),
            start_line: 42,
            start_column: None,
            end_line: None,
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Unresolved,
            stable_id: function_identity_id("src/unresolved.ts", "mystery_fn", 42),
        };
        let json = serde_json::to_string(&identity).unwrap();
        assert_eq!(
            json,
            r#"{"file":"src/unresolved.ts","name":"mystery_fn","start_line":42,"resolution":"unresolved","stable_id":"fallow:fn:66db18d1"}"#,
        );
    }

    #[test]
    fn function_identity_id_unchanged_by_start_column() {
        // Per-field stability assertion (panel item 5). The struct-level
        // function_identity_id_unchanged_by_columns test bundles all four
        // metadata fields; the per-field cases catch a future regression
        // where the helper accidentally starts hashing one specific
        // metadata field but not the others.
        let base = function_identity_id("src/stability.ts", "foo", 10);
        let with_start_column = FunctionIdentity {
            file: "src/stability.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 10,
            start_column: Some(7),
            end_line: None,
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Fallback,
            stable_id: function_identity_id("src/stability.ts", "foo", 10),
        };
        assert_eq!(base, with_start_column.stable_id);
        assert_eq!(base, with_start_column.stable_id_computed());
    }

    #[test]
    fn function_identity_id_unchanged_by_end_line() {
        let base = function_identity_id("src/stability.ts", "foo", 10);
        let with_end_line = FunctionIdentity {
            file: "src/stability.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 10,
            start_column: None,
            end_line: Some(99),
            end_column: None,
            source_hash: None,
            resolution: IdentityResolution::Fallback,
            stable_id: function_identity_id("src/stability.ts", "foo", 10),
        };
        assert_eq!(base, with_end_line.stable_id);
        assert_eq!(base, with_end_line.stable_id_computed());
    }

    #[test]
    fn function_identity_id_unchanged_by_end_column() {
        let base = function_identity_id("src/stability.ts", "foo", 10);
        let with_end_column = FunctionIdentity {
            file: "src/stability.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 10,
            start_column: None,
            end_line: None,
            end_column: Some(42),
            source_hash: None,
            resolution: IdentityResolution::Fallback,
            stable_id: function_identity_id("src/stability.ts", "foo", 10),
        };
        assert_eq!(base, with_end_column.stable_id);
        assert_eq!(base, with_end_column.stable_id_computed());
    }

    #[test]
    fn function_identity_id_unchanged_by_source_hash() {
        let base = function_identity_id("src/stability.ts", "foo", 10);
        let with_source_hash = FunctionIdentity {
            file: "src/stability.ts".to_owned(),
            name: "foo".to_owned(),
            start_line: 10,
            start_column: None,
            end_line: None,
            end_column: None,
            source_hash: Some(source_hash_for(b"function foo() { return 1; }")),
            resolution: IdentityResolution::Fallback,
            stable_id: function_identity_id("src/stability.ts", "foo", 10),
        };
        assert_eq!(base, with_source_hash.stable_id);
        assert_eq!(base, with_source_hash.stable_id_computed());
    }

    #[test]
    fn source_hash_for_anchor_fixture() {
        // Conformance fixture for the pinned source_hash format added in
        // protocol 0.7.0. Producers (fallow CLI, fallow-cov sidecar,
        // browser / node beacons, Istanbul ingester) MUST run this exact
        // byte sequence through their own pipelines and obtain the same
        // 16-hex string. Divergence here means the cross-producer
        // tiebreaker would silently break in production.
        assert_eq!(
            source_hash_for(b"function foo() { return 1; }"),
            "74846e29a52fe863",
        );
    }

    #[test]
    fn source_hash_for_is_deterministic() {
        let first = source_hash_for(b"const greet = (name: string) => `hi, ${name}`;\n");
        let second = source_hash_for(b"const greet = (name: string) => `hi, ${name}`;\n");
        assert_eq!(first, second);
    }

    #[test]
    fn source_hash_for_differs_on_whitespace_change() {
        // The canonicalization rule says no whitespace normalization, so
        // two byte slices that differ ONLY by whitespace must produce
        // different hashes. Locks the no-normalization stance against any
        // future producer that quietly trims or collapses whitespace.
        let tight = source_hash_for(b"function foo(){return 1;}");
        let loose = source_hash_for(b"function foo() { return 1; }");
        assert_ne!(tight, loose);
    }

    #[test]
    fn source_hash_for_format_is_sixteen_lowercase_hex() {
        let hash = source_hash_for(b"function foo() { return 1; }");
        assert_eq!(hash.len(), 16, "expected 16 hex chars, got {hash}");
        assert!(
            hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "expected lowercase hex, got {hash}",
        );
    }

    #[test]
    fn source_hash_for_differs_from_sibling_id_helpers() {
        // Distinctness check parallel to the kind-salt assertions on
        // finding_id / hot_path_id / blast_radius_id / importance_id /
        // function_identity_id: source_hash_for hashes a different input
        // shape (body bytes, not file + name + line + kind salt) so its
        // output MUST NOT collide with any sibling ID helper's output for
        // any input. Locks the structural difference even though length
        // (16 vs 8 hex) and the absent `fallow:` prefix already make the
        // strings unambiguous.
        let body = b"function foo() {}";
        let source = source_hash_for(body);
        // Sibling helpers prefix `fallow:<kind>:`; source_hash carries no
        // prefix. Distinctness by construction.
        assert!(!source.contains(':'));
        assert_ne!(source, finding_id("src/x.ts", "foo", 1));
        assert_ne!(source, hot_path_id("src/x.ts", "foo", 1));
        assert_ne!(source, blast_radius_id("src/x.ts", "foo", 1));
        assert_ne!(source, importance_id("src/x.ts", "foo", 1));
        assert_ne!(source, function_identity_id("src/x.ts", "foo", 1));
    }

    #[test]
    fn source_hash_for_no_fallow_prefix() {
        // source_hash is a content tiebreaker, not a qualified ID. The
        // "fallow:" prefix used by finding_id / hot_path_id / function_identity_id
        // exists to namespace cross-surface joins; source_hash is consumed
        // raw and MUST NOT carry the prefix.
        let hash = source_hash_for(b"function foo() { return 1; }");
        assert!(
            !hash.starts_with("fallow:"),
            "source_hash must not carry the fallow: prefix, got {hash}",
        );
    }

    #[test]
    fn blast_radius_id_anchor_fixture() {
        // Conformance fixture parallel to function_identity_id_anchor_fixture.
        // Locks the canonical hash inputs + truncation for blast_radius_id
        // so producers can self-test agreement with the protocol.
        assert_eq!(
            blast_radius_id("src/blast.tsx", "handle", 100),
            "fallow:blast:d437d3d3",
        );
    }

    #[test]
    fn importance_id_anchor_fixture() {
        // Conformance fixture parallel to function_identity_id_anchor_fixture.
        assert_eq!(
            importance_id("src/importance.tsx", "important", 5),
            "fallow:importance:38ee86d9",
        );
    }
}
