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
//! fields and SHOULD map unknown enum variants to [`Feature::Unknown`] or
//! [`Verdict::Unknown`] rather than erroring.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Current protocol version. Bumped per spec rules above.
pub const PROTOCOL_VERSION: &str = "1.0.0";

// -- Request envelope -------------------------------------------------------

/// Sent by the public CLI to the sidecar via stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Semver string of the protocol version this request targets.
    pub protocol_version: String,
    pub license: License,
    /// Absolute path of the project root under analysis.
    pub project_root: String,
    pub coverage_sources: Vec<CoverageSource>,
    pub static_findings: StaticFindings,
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
    V8 { path: String },
    /// A single Istanbul JSON file.
    Istanbul { path: String },
    /// A directory containing multiple V8 dumps to merge in memory.
    V8Dir { path: String },
}

/// Static analysis output the public CLI already produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticFindings {
    pub files: Vec<StaticFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticFile {
    pub path: String,
    pub functions: Vec<StaticFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticFunction {
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub cyclomatic: u32,
}

/// Runtime knobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Options {
    #[serde(default)]
    pub include_hot_paths: bool,
    #[serde(default)]
    pub min_invocations_for_hot: Option<u64>,
}

// -- Response envelope ------------------------------------------------------

/// Emitted by the sidecar to stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub protocol_version: String,
    pub verdict: Verdict,
    pub summary: Summary,
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub hot_paths: Vec<HotPath>,
    #[serde(default)]
    pub watermark: Option<Watermark>,
    #[serde(default)]
    pub errors: Vec<DiagnosticMessage>,
    #[serde(default)]
    pub warnings: Vec<DiagnosticMessage>,
}

/// Top-level verdict. Unknown variants are forward-mapped to [`Verdict::Unknown`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Clean,
    HotPathChangesNeeded,
    ColdCodeDetected,
    LicenseExpiredGrace,
    /// Sentinel for forward-compatibility with newer sidecars.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub functions_total: u64,
    pub functions_called: u64,
    pub functions_never_called: u64,
    pub functions_coverage_unavailable: u64,
    pub percent_dead_in_production: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub file: String,
    pub function: String,
    pub state: CallState,
    pub invocations: u64,
    pub confidence: Confidence,
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// Per-function three-state tracking result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallState {
    Called,
    NeverCalled,
    CoverageUnavailable,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    High,
    Medium,
    Low,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPath {
    pub file: String,
    pub function: String,
    pub invocations: u64,
}

/// Machine-readable next-step hint for AI agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub kind: String,
    pub description: String,
    #[serde(default)]
    pub auto_fixable: bool,
}

/// What to render in the human output when the license is in the grace window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Watermark {
    TrialExpired,
    LicenseExpiredGrace,
    #[serde(other)]
    Unknown,
}

/// Error / warning surfaced by the sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticMessage {
    pub code: String,
    pub message: String,
}

// -- License features -------------------------------------------------------

/// Feature flags present in the license JWT's `features` claim.
///
/// Wire format stays a string array (forward-compatible); new variants are
/// additive in minor protocol bumps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    ProductionCoverage,
    // Deferred to later phases:
    PortfolioDashboard,
    McpCloudTools,
    CrossRepoAggregation,
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_constant_parses_as_semver_major() {
        assert!(PROTOCOL_VERSION.starts_with("1."));
    }

    #[test]
    fn unknown_verdict_round_trips() {
        let json = r#""something-new""#;
        let verdict: Verdict = serde_json::from_str(json).unwrap();
        matches!(verdict, Verdict::Unknown);
    }

    #[test]
    fn unknown_feature_round_trips() {
        let json = r#""future_feature""#;
        let feature: Feature = serde_json::from_str(json).unwrap();
        matches!(feature, Feature::Unknown);
    }

    #[test]
    fn coverage_source_kebab_case() {
        let json = r#"{"kind":"v8-dir","path":"/tmp/dumps"}"#;
        let src: CoverageSource = serde_json::from_str(json).unwrap();
        matches!(src, CoverageSource::V8Dir { .. });
    }

    #[test]
    fn response_allows_unknown_fields() {
        let json = r#"{
            "protocol_version": "1.0.0",
            "verdict": "clean",
            "summary": {
                "functions_total": 0,
                "functions_called": 0,
                "functions_never_called": 0,
                "functions_coverage_unavailable": 0,
                "percent_dead_in_production": 0.0
            },
            "findings": [],
            "future_top_level_field": 42
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.protocol_version, "1.0.0");
    }
}
