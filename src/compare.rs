use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;

use crate::model::{ComparisonReport, ComparisonSummary, FileRecord, Finding, Severity};
use crate::remediation::remediation_for;
use crate::scanner::{ScanOptions, scan_exports};

#[derive(Debug, Clone, Copy, Default)]
pub struct CompareOptions {
    pub allow_missing: bool,
}

/// Scans two complete export runs and compares their canonical path manifests.
///
/// # Errors
///
/// Returns an error when either underlying scan cannot be constructed.
pub fn compare_exports(
    before_inputs: &[PathBuf],
    after_inputs: &[PathBuf],
    scan_options: &ScanOptions,
    compare_options: CompareOptions,
) -> Result<ComparisonReport> {
    let before = scan_exports(before_inputs, scan_options)?;
    let after = scan_exports(after_inputs, scan_options)?;
    let before_manifest = manifest(&before.files);
    let after_manifest = manifest(&after.files);
    let mut findings = Vec::new();
    let mut summary = ComparisonSummary {
        before_errors: before.summary.errors,
        after_errors: after.summary.errors,
        ..ComparisonSummary::default()
    };

    for (path, old_hash) in &before_manifest {
        match after_manifest.get(path) {
            None => {
                summary.missing_after = summary.missing_after.saturating_add(1);
                let code = "compare.missing_after";
                findings.push(
                    Finding::new(
                        if compare_options.allow_missing {
                            Severity::Warning
                        } else {
                            Severity::Error
                        },
                        code,
                        "Path exists in the older export but not in the newer export.",
                        remediation_for(code),
                    )
                    .at_path(*path),
                );
            }
            Some(new_hash) if new_hash != old_hash => {
                summary.changed = summary.changed.saturating_add(1);
                let code = "compare.changed";
                findings.push(
                    Finding::new(
                        Severity::Warning,
                        code,
                        "Content hash changed at the same canonical path.",
                        remediation_for(code),
                    )
                    .at_path(*path),
                );
            }
            Some(_) => {}
        }
    }

    for path in after_manifest.keys() {
        if !before_manifest.contains_key(path) {
            summary.added_after = summary.added_after.saturating_add(1);
            let code = "compare.added";
            findings.push(
                Finding::new(
                    Severity::Info,
                    code,
                    "Path appears only in the newer export.",
                    remediation_for(code),
                )
                .at_path(*path),
            );
        }
    }

    if summary.after_errors > summary.before_errors {
        let code = "compare.new_error";
        findings.push(Finding::new(
            Severity::Error,
            code,
            format!(
                "Newer export has {} errors; older export has {}.",
                summary.after_errors, summary.before_errors
            ),
            remediation_for(code),
        ));
    }

    findings.sort_by(|left, right| {
        left.severity
            .rank()
            .cmp(&right.severity.rank())
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.path.cmp(&right.path))
    });

    Ok(ComparisonReport {
        schema_version: 1,
        tool: "export-checkup".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        summary,
        findings,
        before,
        after,
    })
}

fn manifest(files: &[FileRecord]) -> BTreeMap<&str, Option<&str>> {
    let mut output = BTreeMap::new();
    for file in files {
        output
            .entry(file.canonical_path.as_str())
            .or_insert_with(|| file.sha256.as_deref());
    }
    output
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    #[test]
    fn comparison_marks_removed_path_as_regression() {
        let root = test_directory();
        let before = root.join("before");
        let after = root.join("after");
        fs::create_dir_all(&before).expect("before dir");
        fs::create_dir_all(&after).expect("after dir");
        fs::write(before.join("kept.txt"), b"same").expect("kept");
        fs::write(before.join("lost.txt"), b"gone").expect("lost");
        fs::write(after.join("kept.txt"), b"same").expect("kept after");

        let report = compare_exports(
            &[before],
            &[after],
            &ScanOptions::default(),
            CompareOptions::default(),
        )
        .expect("compare");
        assert_eq!(report.summary.missing_after, 1);
        assert!(report.has_regressions());
        fs::remove_dir_all(root).expect("clean test directory");
    }

    fn test_directory() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "export-checkup-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }
}
