use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::Result;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::adapters::{self, AdapterFacts};
use crate::model::{
    FileKind, FileRecord, Finding, InputSummary, ScanReport, ScanSummary, Severity,
};
use crate::pathing::{canonical_export_path, extension, file_kind, normalize_logical_path};
use crate::remediation::remediation_for;

const BUFFER_SIZE: usize = 64 * 1024;
const SIGNATURE_SIZE: usize = 64;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub max_json_bytes: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_json_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
struct ReadResult {
    bytes_read: u64,
    sha256: String,
    prefix: Vec<u8>,
    suffix: Vec<u8>,
    json: Option<Vec<u8>>,
    json_truncated: bool,
}

#[derive(Debug, Default)]
struct ScanState {
    records: Vec<FileRecord>,
    findings: Vec<Finding>,
    facts: AdapterFacts,
}

/// Scans directories and ZIP parts without extracting or changing their contents.
///
/// # Errors
///
/// Returns an error when the report cannot be assembled. Input-specific read failures are
/// represented as findings so other supplied parts can still be inspected.
pub fn scan_exports(inputs: &[PathBuf], options: &ScanOptions) -> Result<ScanReport> {
    let mut state = ScanState::default();
    let mut input_summaries = Vec::new();

    for (index, input) in inputs.iter().enumerate() {
        let base_label = input_label(input, index);
        let matching_labels = inputs
            .iter()
            .enumerate()
            .filter(|(candidate_index, candidate)| {
                input_label(candidate, *candidate_index) == base_label
            })
            .count();
        let label = if matching_labels > 1 {
            let occurrence = inputs[..=index]
                .iter()
                .enumerate()
                .filter(|(candidate_index, candidate)| {
                    input_label(candidate, *candidate_index) == base_label
                })
                .count();
            format!("{base_label}#{occurrence}")
        } else {
            base_label
        };
        let summary = if !input.exists() {
            let code = "input.not_found";
            state.findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    "Input path does not exist.",
                    remediation_for(code),
                )
                .in_source(&label),
            );
            InputSummary {
                label,
                kind: "missing".to_owned(),
                files: 0,
                bytes: 0,
            }
        } else if input.is_dir() {
            scan_directory(input, &label, options, &mut state)
        } else if input.is_file() && extension(&input.to_string_lossy()).as_deref() == Some("zip") {
            scan_zip(input, &label, options, &mut state)
        } else {
            let code = "input.unsupported";
            state.findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    "Input is neither a directory nor a ZIP archive.",
                    remediation_for(code),
                )
                .in_source(&label),
            );
            InputSummary {
                label,
                kind: "unsupported".to_owned(),
                files: 0,
                bytes: 0,
            }
        };
        input_summaries.push(summary);
    }

    analyze_duplicate_paths(&state.records, &mut state.findings);
    let providers = adapters::analyze(&state.records, &state.facts, &mut state.findings);
    state.records.sort_by(|left, right| {
        left.canonical_path
            .cmp(&right.canonical_path)
            .then_with(|| left.source.cmp(&right.source))
    });
    sort_findings(&mut state.findings);

    let summary = summarize(&input_summaries, &state.records, &state.findings);
    Ok(ScanReport {
        schema_version: 1,
        tool: "export-checkup".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        summary,
        inputs: input_summaries,
        providers,
        findings: state.findings,
        files: state.records,
    })
}

fn scan_directory(
    root: &Path,
    label: &str,
    options: &ScanOptions,
    state: &mut ScanState,
) -> InputSummary {
    let mut summary = InputSummary {
        label: label.to_owned(),
        kind: "directory".to_owned(),
        files: 0,
        bytes: 0,
    };

    scan_directory_level(root, root, label, options, state, &mut summary);
    if summary.files == 0 {
        push_empty_finding(label, state);
    }
    summary
}

fn scan_directory_level(
    root: &Path,
    current: &Path,
    label: &str,
    options: &ScanOptions,
    state: &mut ScanState,
    summary: &mut InputSummary,
) {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) => {
            let code = "file.read_failed";
            state.findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    format!("Directory traversal failed: {error}"),
                    remediation_for(code),
                )
                .at_path(relative_path(root, current).unwrap_or_else(|| ".".to_owned()))
                .in_source(label),
            );
            return;
        }
    };
    let mut entries = entries.collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        entry.as_ref().map_or_else(
            |_| String::new(),
            |entry| entry.file_name().to_string_lossy().into_owned(),
        )
    });

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                let code = "file.read_failed";
                state.findings.push(
                    Finding::new(
                        Severity::Error,
                        code,
                        format!("Directory entry could not be read: {error}"),
                        remediation_for(code),
                    )
                    .in_source(label),
                );
                continue;
            }
        };
        let entry_path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                let path =
                    relative_path(root, &entry_path).unwrap_or_else(|| "<invalid>".to_owned());
                record_read_error(&path, label, &error, state);
                continue;
            }
        };
        if file_type.is_symlink() {
            let code = "file.symlink_skipped";
            let relative =
                relative_path(root, &entry_path).unwrap_or_else(|| "<invalid>".to_owned());
            state.findings.push(
                Finding::new(
                    Severity::Warning,
                    code,
                    "Symbolic link was not followed.",
                    remediation_for(code),
                )
                .at_path(relative)
                .in_source(label),
            );
        } else if file_type.is_dir() {
            scan_directory_level(root, &entry_path, label, options, state, summary);
        } else if file_type.is_file() {
            let Some(logical_path) = relative_path(root, &entry_path) else {
                let code = "archive.invalid_path";
                state.findings.push(
                    Finding::new(
                        Severity::Error,
                        code,
                        "Directory entry could not be normalized safely.",
                        remediation_for(code),
                    )
                    .in_source(label),
                );
                continue;
            };
            let declared_size = entry.metadata().map_or(0, |metadata| metadata.len());
            match File::open(&entry_path).map(BufReader::new) {
                Ok(reader) => consume_entry(
                    reader,
                    declared_size,
                    &logical_path,
                    label,
                    options,
                    state,
                    summary,
                ),
                Err(error) => record_read_error(&logical_path, label, &error, state),
            }
        }
    }
}

fn scan_zip(
    path: &Path,
    label: &str,
    options: &ScanOptions,
    state: &mut ScanState,
) -> InputSummary {
    let mut summary = InputSummary {
        label: label.to_owned(),
        kind: "zip".to_owned(),
        files: 0,
        bytes: 0,
    };
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            push_archive_open_error(label, error, state);
            return summary;
        }
    };
    let mut archive = match ZipArchive::new(BufReader::new(file)) {
        Ok(archive) => archive,
        Err(error) => {
            push_archive_open_error(label, error, state);
            return summary;
        }
    };

    for index in 0..archive.len() {
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                let code = "archive.entry_read_failed";
                state.findings.push(
                    Finding::new(
                        Severity::Error,
                        code,
                        format!("ZIP entry {index} could not be opened: {error}"),
                        remediation_for(code),
                    )
                    .in_source(label),
                );
                continue;
            }
        };
        if entry.is_dir() {
            continue;
        }
        if entry.enclosed_name().is_none() {
            let code = "archive.invalid_path";
            state.findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    "ZIP entry contains an absolute or parent-traversal path.",
                    remediation_for(code),
                )
                .at_path(entry.name())
                .in_source(label),
            );
            continue;
        }
        let Some(logical_path) = normalize_logical_path(entry.name()) else {
            let code = "archive.invalid_path";
            state.findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    "ZIP entry path could not be normalized safely.",
                    remediation_for(code),
                )
                .at_path(entry.name())
                .in_source(label),
            );
            continue;
        };
        let declared_size = entry.size();
        consume_entry(
            &mut entry,
            declared_size,
            &logical_path,
            label,
            options,
            state,
            &mut summary,
        );
    }
    if summary.files == 0 {
        push_empty_finding(label, state);
    }
    summary
}

fn consume_entry<R: Read>(
    reader: R,
    declared_size: u64,
    logical_path: &str,
    label: &str,
    options: &ScanOptions,
    state: &mut ScanState,
    summary: &mut InputSummary,
) {
    let capture_json = file_kind(logical_path) == FileKind::Json;
    match read_and_hash(reader, capture_json, options.max_json_bytes) {
        Ok(read) => {
            summary.files = summary.files.saturating_add(1);
            summary.bytes = summary.bytes.saturating_add(read.bytes_read);
            if declared_size != read.bytes_read {
                let code = "file.changed_during_scan";
                state.findings.push(
                    Finding::new(
                        Severity::Warning,
                        code,
                        format!(
                            "Declared size {declared_size} differs from {actual} bytes read.",
                            actual = read.bytes_read
                        ),
                        remediation_for(code),
                    )
                    .at_path(logical_path)
                    .in_source(label),
                );
            }
            let kind = file_kind(logical_path);
            if kind == FileKind::Media {
                if let Some(reason) =
                    validate_media(logical_path, read.bytes_read, &read.prefix, &read.suffix)
                {
                    let code = "media.invalid_structure";
                    state.findings.push(
                        Finding::new(Severity::Error, code, reason, remediation_for(code))
                            .at_path(logical_path)
                            .in_source(label),
                    );
                }
            }
            if read.json_truncated {
                let code = "json.too_large";
                state.findings.push(
                    Finding::new(
                        Severity::Warning,
                        code,
                        format!(
                            "JSON exceeds the {} MiB inspection limit.",
                            options.max_json_bytes / 1024 / 1024
                        ),
                        remediation_for(code),
                    )
                    .at_path(logical_path)
                    .in_source(label),
                );
            }
            if let Some(json) = read.json {
                adapters::inspect_json(
                    logical_path,
                    label,
                    &json,
                    &mut state.facts,
                    &mut state.findings,
                );
            }
            state.records.push(FileRecord {
                source: label.to_owned(),
                logical_path: logical_path.to_owned(),
                canonical_path: canonical_export_path(logical_path),
                size: read.bytes_read,
                sha256: Some(read.sha256),
                kind,
            });
        }
        Err(error) => {
            let code = "archive.entry_read_failed";
            state.findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    format!("Entry could not be fully read or verified: {error}"),
                    remediation_for(code),
                )
                .at_path(logical_path)
                .in_source(label),
            );
            state.records.push(FileRecord {
                source: label.to_owned(),
                logical_path: logical_path.to_owned(),
                canonical_path: canonical_export_path(logical_path),
                size: declared_size,
                sha256: None,
                kind: file_kind(logical_path),
            });
        }
    }
}

fn read_and_hash<R: Read>(
    mut reader: R,
    capture_json: bool,
    max_json_bytes: u64,
) -> io::Result<ReadResult> {
    let mut hasher = Sha256::new();
    let mut prefix = Vec::with_capacity(SIGNATURE_SIZE);
    let mut suffix = VecDeque::with_capacity(SIGNATURE_SIZE);
    let mut json = capture_json.then(Vec::new);
    let mut json_truncated = false;
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut bytes_read = 0_u64;

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let chunk = &buffer[..count];
        bytes_read = bytes_read.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
        hasher.update(chunk);
        let remaining = SIGNATURE_SIZE.saturating_sub(prefix.len());
        prefix.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        for byte in chunk {
            if suffix.len() == SIGNATURE_SIZE {
                suffix.pop_front();
            }
            suffix.push_back(*byte);
        }
        if let Some(captured) = json.as_mut() {
            if bytes_read <= max_json_bytes {
                captured.extend_from_slice(chunk);
            } else {
                json = None;
                json_truncated = true;
            }
        }
    }

    Ok(ReadResult {
        bytes_read,
        sha256: format!("{:x}", hasher.finalize()),
        prefix,
        suffix: suffix.into_iter().collect(),
        json,
        json_truncated,
    })
}

fn validate_media(path: &str, size: u64, prefix: &[u8], suffix: &[u8]) -> Option<String> {
    let extension = extension(path)?;
    let valid = match extension.as_str() {
        "jpg" | "jpeg" => {
            prefix.starts_with(&[0xff, 0xd8, 0xff]) && suffix.ends_with(&[0xff, 0xd9])
        }
        "png" => prefix.starts_with(b"\x89PNG\r\n\x1a\n") && contains_bytes(suffix, b"IEND"),
        "gif" => {
            (prefix.starts_with(b"GIF87a") || prefix.starts_with(b"GIF89a"))
                && suffix.last() == Some(&0x3b)
        }
        "webp" => prefix.starts_with(b"RIFF") && prefix.get(8..12) == Some(b"WEBP"),
        "mp4" | "m4v" | "mov" | "heic" | "heif" => prefix.get(4..8) == Some(b"ftyp"),
        "svg" => {
            let start = String::from_utf8_lossy(prefix).to_lowercase();
            let end = String::from_utf8_lossy(suffix).to_lowercase();
            start.contains("<svg") && (end.contains("</svg>") || start.trim_end().ends_with("/>"))
        }
        _ => true,
    };
    (!valid).then(|| {
        format!("{extension} signature or closing marker is invalid for a {size}-byte file.")
    })
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn analyze_duplicate_paths(records: &[FileRecord], findings: &mut Vec<Finding>) {
    let mut exact: HashMap<&str, Vec<&FileRecord>> = HashMap::new();
    let mut canonical: HashMap<&str, Vec<&FileRecord>> = HashMap::new();
    let mut folded: HashMap<String, Vec<&FileRecord>> = HashMap::new();
    for record in records {
        exact.entry(&record.logical_path).or_default().push(record);
        canonical
            .entry(&record.canonical_path)
            .or_default()
            .push(record);
        folded
            .entry(record.logical_path.to_lowercase())
            .or_default()
            .push(record);
    }
    for (path, duplicates) in exact {
        report_duplicate_group(path, &duplicates, "Logical path", findings);
    }
    for (path, duplicates) in canonical {
        let distinct_logical_paths = duplicates
            .iter()
            .map(|record| record.logical_path.as_str())
            .collect::<std::collections::HashSet<_>>();
        if distinct_logical_paths.len() > 1 {
            report_duplicate_group(path, &duplicates, "Canonical path", findings);
        }
    }
    for records in folded.values() {
        let mut names: Vec<&str> = records
            .iter()
            .map(|record| record.logical_path.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        if names.len() > 1 {
            let code = "archive.case_collision";
            findings.push(
                Finding::new(
                    Severity::Warning,
                    code,
                    format!("Paths differ only by case: {}", names.join(", ")),
                    remediation_for(code),
                )
                .at_path(names[0]),
            );
        }
    }
}

fn report_duplicate_group(
    path: &str,
    duplicates: &[&FileRecord],
    path_kind: &str,
    findings: &mut Vec<Finding>,
) {
    if duplicates.len() < 2 {
        return;
    }
    let hashes: Vec<Option<&str>> = duplicates
        .iter()
        .map(|record| record.sha256.as_deref())
        .collect();
    let all_equal = hashes.windows(2).all(|pair| pair[0] == pair[1]);
    let code = if all_equal {
        "archive.duplicate_entry"
    } else {
        "archive.conflicting_entry"
    };
    let severity = if all_equal {
        Severity::Warning
    } else {
        Severity::Error
    };
    findings.push(
        Finding::new(
            severity,
            code,
            format!(
                "{path_kind} occurs {} times across supplied inputs.",
                duplicates.len()
            ),
            remediation_for(code),
        )
        .at_path(path),
    );
}

fn summarize(inputs: &[InputSummary], records: &[FileRecord], findings: &[Finding]) -> ScanSummary {
    let mut summary = ScanSummary {
        inputs: u64::try_from(inputs.len()).unwrap_or(u64::MAX),
        files: u64::try_from(records.len()).unwrap_or(u64::MAX),
        bytes: records
            .iter()
            .fold(0_u64, |total, record| total.saturating_add(record.size)),
        ..ScanSummary::default()
    };
    for finding in findings {
        match finding.severity {
            Severity::Error => summary.errors = summary.errors.saturating_add(1),
            Severity::Warning => summary.warnings = summary.warnings.saturating_add(1),
            Severity::Info => summary.infos = summary.infos.saturating_add(1),
        }
    }
    summary
}

fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by(|left, right| {
        left.severity
            .rank()
            .cmp(&right.severity.rank())
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.source.cmp(&right.source))
    });
}

fn input_label(path: &Path, index: usize) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map_or_else(|| format!("input-{}", index + 1), str::to_owned)
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    normalize_logical_path(&relative.to_string_lossy())
}

fn record_read_error(path: &str, label: &str, error: &io::Error, state: &mut ScanState) {
    let code = "file.read_failed";
    state.findings.push(
        Finding::new(
            Severity::Error,
            code,
            format!("File could not be opened: {error}"),
            remediation_for(code),
        )
        .at_path(path)
        .in_source(label),
    );
}

fn push_archive_open_error(label: &str, error: impl std::fmt::Display, state: &mut ScanState) {
    let code = "archive.open_failed";
    state.findings.push(
        Finding::new(
            Severity::Error,
            code,
            format!("ZIP archive could not be opened: {error}"),
            remediation_for(code),
        )
        .in_source(label),
    );
}

fn push_empty_finding(label: &str, state: &mut ScanState) {
    let code = "archive.empty";
    state.findings.push(
        Finding::new(
            Severity::Warning,
            code,
            "Input contains no readable files.",
            remediation_for(code),
        )
        .in_source(label),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_media_signatures() {
        assert_eq!(
            validate_media("x.jpg", 4, &[0xff, 0xd8, 0xff], &[0xff, 0xd9]),
            None
        );
        assert!(validate_media("x.png", 3, b"bad", b"bad").is_some());
        assert_eq!(validate_media("x.svg", 11, b"<svg>", b"</svg>"), None);
        assert_eq!(
            validate_media("x.png", 20, b"\x89PNG\r\n\x1a\n", b"chunkIENDtail"),
            None
        );
        assert_eq!(validate_media("x.gif", 8, b"GIF89a", &[0x3b]), None);
        assert_eq!(validate_media("x.webp", 12, b"RIFF0000WEBP", b"tail"), None);
        assert_eq!(validate_media("x.mp4", 12, b"0000ftypisom", b"tail"), None);
        assert_eq!(validate_media("x.avi", 3, b"any", b"any"), None);
        assert!(validate_media("x.gif", 6, b"GIF89a", b"x").is_some());
        assert!(validate_media("x.webp", 4, b"RIFF", b"").is_some());
        assert!(validate_media("x.mov", 4, b"bad!", b"").is_some());
    }

    #[test]
    fn hash_capture_respects_json_limit() {
        let result = read_and_hash(&b"abcdef"[..], true, 3).expect("read");
        assert!(result.json.is_none());
        assert!(result.json_truncated);
        assert_eq!(result.bytes_read, 6);
    }
}
