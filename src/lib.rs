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

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Current protocol version. Bumped per the semver rules above.
pub const PROTOCOL_VERSION: &str = "0.2.0";

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Top-level report verdict (was `Verdict` in 0.1). Summarises the overall
/// state of the run; per-finding verdicts live on [`Finding::verdict`].
/// Unknown variants are forward-mapped to [`ReportVerdict::Unknown`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportVerdict {
    /// No action required — production coverage confirms the codebase.
    Clean,
    /// One or more hot paths need attention (regression / drift).
    HotPathChangesNeeded,
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
}

/// A per-function finding combining static analysis and runtime coverage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Deterministic content hash of shape `fallow:prod:<hash>`. See
    /// [`finding_id`] for the canonical helper.
    pub id: String,
    /// Path to the source file, relative to [`Request::project_root`].
    pub file: String,
    /// Function name as reported by the static analyzer. Matches
    /// [`StaticFunction::name`].
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPath {
    /// Deterministic content hash of shape `fallow:hot:<hash>`. See
    /// [`hot_path_id`] for the canonical helper.
    pub id: String,
    /// Path to the source file, relative to [`Request::project_root`].
    pub file: String,
    /// Function name as reported by the static analyzer.
    pub function: String,
    /// 1-indexed line the function starts on.
    pub line: u32,
    /// Raw invocation count from the V8 dump.
    pub invocations: u64,
    /// Percentile rank of this function's invocation count over the
    /// invocation distribution of the current response's hot paths. `100`
    /// means the busiest function, `0` the quietest that still qualified.
    pub percentile: u8,
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

/// Canonical content hash shared by the stable ID helpers. The input order
/// (file, function, line, kind) and truncation (first 4 SHA-256 bytes → 8 hex
/// chars) are part of the wire contract; see [`finding_id`] for the rationale.
fn content_hash(file: &str, function: &str, line: u32, kind: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file.as_bytes());
    hasher.update(function.as_bytes());
    hasher.update(line.to_string().as_bytes());
    hasher.update(kind.as_bytes());
    let digest = hasher.finalize();
    hex_prefix(&digest)
}

/// Encode the first four bytes of `digest` as lowercase hex — exactly eight
/// characters. Kept separate so the truncation length is easy to audit. Total
/// by construction: `HEX` is ASCII and `char::from(u8)` is infallible, so the
/// helper never panics.
fn hex_prefix(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(8);
    for &byte in digest.iter().take(4) {
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
    fn version_constant_is_v0_2() {
        assert!(PROTOCOL_VERSION.starts_with("0.2."));
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
}
