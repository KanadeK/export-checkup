use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub remediation: String,
}

impl Finding {
    #[must_use]
    pub fn new(
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            path: None,
            source: None,
            remediation: remediation.into(),
        }
    }

    #[must_use]
    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    #[must_use]
    pub fn in_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Json,
    Html,
    Media,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub source: String,
    pub logical_path: String,
    pub canonical_path: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub kind: FileKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSummary {
    pub label: String,
    pub kind: String,
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub provider: String,
    pub detected: bool,
    pub metrics: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScanSummary {
    pub inputs: u64,
    pub files: u64,
    pub bytes: u64,
    pub errors: u64,
    pub warnings: u64,
    pub infos: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: u32,
    pub tool: String,
    pub version: String,
    pub summary: ScanSummary,
    pub inputs: Vec<InputSummary>,
    pub providers: Vec<ProviderSummary>,
    pub findings: Vec<Finding>,
    pub files: Vec<FileRecord>,
}

impl ScanReport {
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        self.summary.errors > 0
    }

    #[must_use]
    pub const fn has_warnings(&self) -> bool {
        self.summary.warnings > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ComparisonSummary {
    pub missing_after: u64,
    pub added_after: u64,
    pub changed: u64,
    pub before_errors: u64,
    pub after_errors: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub schema_version: u32,
    pub tool: String,
    pub version: String,
    pub summary: ComparisonSummary,
    pub findings: Vec<Finding>,
    pub before: ScanReport,
    pub after: ScanReport,
}

impl ComparisonReport {
    #[must_use]
    pub fn has_regressions(&self) -> bool {
        self.summary.missing_after > 0
            || self.summary.after_errors > self.summary.before_errors
            || self
                .findings
                .iter()
                .any(|finding| finding.severity == Severity::Error)
    }
}
