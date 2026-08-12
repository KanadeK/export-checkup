use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use export_checkup::model::Severity;
use export_checkup::report::{
    render_scan_html, write_compare_html, write_compare_json, write_scan_html, write_scan_json,
};
use export_checkup::{CompareOptions, ScanOptions, compare_exports, scan_exports};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "export-checkup-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn healthy_google_and_instagram_export_is_clean() {
    let root = TestDirectory::new("healthy");
    create_healthy_export(root.path());

    let report = scan_exports(&[root.path().to_path_buf()], &ScanOptions::default())
        .expect("scan healthy export");

    assert_eq!(report.summary.errors, 0);
    assert_eq!(report.summary.warnings, 0);
    assert_eq!(
        provider_metric(&report, "google_photos", "matched_pairs"),
        1
    );
    assert_eq!(provider_metric(&report, "instagram", "messages"), 1);
}

#[test]
fn broken_provider_references_and_page_gap_are_actionable() {
    let root = TestDirectory::new("broken");
    let google = root.path().join("Takeout/Google Photos/2026");
    let instagram = root
        .path()
        .join("your_instagram_activity/messages/inbox/friend_123");
    fs::create_dir_all(&google).expect("google dir");
    fs::create_dir_all(&instagram).expect("instagram dir");
    fs::write(
        google.join("missing.svg.supplemental-metadata.json"),
        br#"{"title":"missing.svg","photoTakenTime":{"timestamp":"1"}}"#,
    )
    .expect("google sidecar");
    fs::write(
        instagram.join("message_1.json"),
        br#"{"participants":[],"messages":[{"photos":[{"uri":"your_instagram_activity/messages/inbox/friend_123/media/missing.svg"}]}]}"#,
    )
    .expect("message 1");
    fs::write(
        instagram.join("message_3.json"),
        br#"{"participants":[],"messages":[]}"#,
    )
    .expect("message 3");

    let report = scan_exports(&[root.path().to_path_buf()], &ScanOptions::default())
        .expect("scan broken export");
    let codes = report
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"google.sidecar_missing_media"));
    assert!(codes.contains(&"instagram.missing_attachment"));
    assert!(codes.contains(&"instagram.page_gap"));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| !finding.remediation.is_empty())
    );
}

#[test]
fn unsafe_zip_path_is_rejected_without_extraction() {
    let root = TestDirectory::new("unsafe-zip");
    let archive_path = root.path().join("unsafe.zip");
    let file = File::create(&archive_path).expect("zip file");
    let mut writer = ZipWriter::new(file);
    writer
        .start_file("../escape.txt", SimpleFileOptions::default())
        .expect("unsafe entry");
    writer.write_all(b"do not extract").expect("entry bytes");
    writer.finish().expect("finish zip");

    let report = scan_exports(&[archive_path], &ScanOptions::default()).expect("scan zip");
    assert!(report.findings.iter().any(|finding| {
        finding.code == "archive.invalid_path" && finding.severity == Severity::Error
    }));
    assert!(!root.path().join("escape.txt").exists());
}

#[test]
fn corrupted_stored_zip_entry_fails_crc_verification() {
    let root = TestDirectory::new("crc");
    let archive_path = root.path().join("crc.zip");
    let marker = b"unique-payload-for-crc";
    let file = File::create(&archive_path).expect("zip file");
    let mut writer = ZipWriter::new(file);
    writer
        .start_file(
            "Takeout/file.txt",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .expect("stored entry");
    writer.write_all(marker).expect("entry bytes");
    writer.finish().expect("finish zip");
    let mut bytes = fs::read(&archive_path).expect("read zip");
    let offset = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("payload offset");
    bytes[offset] ^= 0xff;
    fs::write(&archive_path, bytes).expect("corrupt zip");

    let report = scan_exports(&[archive_path], &ScanOptions::default()).expect("scan zip");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "archive.entry_read_failed")
    );
}

#[test]
fn multipart_conflict_is_not_silently_merged() {
    let root = TestDirectory::new("conflict");
    let part_one = root.path().join("part-one");
    let part_two = root.path().join("part-two");
    fs::create_dir_all(part_one.join("Takeout")).expect("part one");
    fs::create_dir_all(part_two.join("Takeout")).expect("part two");
    fs::write(part_one.join("Takeout/data.txt"), b"first").expect("first file");
    fs::write(part_two.join("Takeout/data.txt"), b"second").expect("second file");

    let report = scan_exports(&[part_one, part_two], &ScanOptions::default()).expect("scan parts");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "archive.conflicting_entry")
    );
}

#[test]
fn comparison_uses_canonical_provider_paths() {
    let root = TestDirectory::new("compare-wrapper");
    let before = root
        .path()
        .join("before/export-a/Takeout/Google Photos/2026");
    let after = root
        .path()
        .join("after/export-b/Takeout/Google Photos/2026");
    fs::create_dir_all(&before).expect("before");
    fs::create_dir_all(&after).expect("after");
    fs::write(before.join("note.txt"), b"same").expect("before file");
    fs::write(after.join("note.txt"), b"same").expect("after file");

    let report = compare_exports(
        &[root.path().join("before")],
        &[root.path().join("after")],
        &ScanOptions::default(),
        CompareOptions::default(),
    )
    .expect("compare");
    assert_eq!(report.summary.missing_after, 0);
    assert_eq!(report.summary.added_after, 0);
    assert_eq!(report.summary.changed, 0);
}

#[test]
fn reports_are_deterministic_and_escape_paths() {
    let root = TestDirectory::new("deterministic");
    let archive_path = root.path().join("escaped.zip");
    let file = File::create(&archive_path).expect("zip file");
    let mut writer = ZipWriter::new(file);
    writer
        .start_file("<script>.txt", SimpleFileOptions::default())
        .expect("escaped entry");
    writer.write_all(b"safe").expect("entry bytes");
    writer.finish().expect("finish zip");
    let first =
        scan_exports(std::slice::from_ref(&archive_path), &ScanOptions::default()).expect("first");
    let second = scan_exports(&[archive_path], &ScanOptions::default()).expect("second");
    assert_eq!(first, second);
    let html = render_scan_html(&first);
    assert!(!html.contains("<script>.txt"));
    assert!(html.contains("&lt;script&gt;.txt"));
}

#[test]
fn command_line_writes_reports_and_returns_documented_codes() {
    let root = TestDirectory::new("cli");
    let healthy = root.path().join("healthy");
    create_healthy_export(&healthy);
    let json = root.path().join("report.json");
    let html = root.path().join("report.html");
    let success = Command::new(env!("CARGO_BIN_EXE_export-checkup"))
        .arg("scan")
        .arg(&healthy)
        .arg("--json")
        .arg(&json)
        .arg("--html")
        .arg(&html)
        .output()
        .expect("run healthy CLI");
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert!(json.exists());
    assert!(html.exists());

    let broken = root.path().join("broken");
    fs::create_dir_all(&broken).expect("broken dir");
    fs::write(broken.join("photo.jpg"), b"not-a-jpeg").expect("broken jpg");
    let failure = Command::new(env!("CARGO_BIN_EXE_export-checkup"))
        .arg("scan")
        .arg(&broken)
        .output()
        .expect("run broken CLI");
    assert_eq!(failure.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&failure.stdout).contains("media.invalid_structure"));
}

#[test]
fn missing_unsupported_empty_and_symlink_inputs_are_explicit() {
    let root = TestDirectory::new("input-errors");
    let empty = root.path().join("empty");
    let unsupported = root.path().join("export.tar");
    fs::create_dir_all(&empty).expect("empty dir");
    fs::write(&unsupported, b"not a supported archive").expect("unsupported file");
    let missing = root.path().join("missing.zip");
    let inputs = vec![missing, unsupported, empty.clone()];

    #[cfg(unix)]
    let inputs = {
        let mut inputs = inputs;
        let linked = root.path().join("linked");
        std::os::unix::fs::symlink(&empty, &linked).expect("test symlink");
        inputs.push(root.path().to_path_buf());
        inputs
    };

    let report = scan_exports(&inputs, &ScanOptions::default()).expect("scan failures");
    let codes = finding_codes(&report);
    assert!(codes.contains(&"input.not_found"));
    assert!(codes.contains(&"input.unsupported"));
    assert!(codes.contains(&"archive.empty"));
    #[cfg(unix)]
    assert!(codes.contains(&"file.symlink_skipped"));
}

#[test]
fn identical_duplicates_and_case_collisions_are_visible() {
    let root = TestDirectory::new("duplicates");
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(&first).expect("first dir");
    fs::create_dir_all(&second).expect("second dir");
    fs::write(first.join("same.txt"), b"same").expect("first duplicate");
    fs::write(second.join("same.txt"), b"same").expect("second duplicate");
    fs::write(first.join("Case.txt"), b"one").expect("upper case");
    fs::write(second.join("case.txt"), b"two").expect("lower case");

    let report = scan_exports(&[first, second], &ScanOptions::default()).expect("scan duplicates");
    let codes = finding_codes(&report);
    assert!(codes.contains(&"archive.duplicate_entry"));
    assert!(codes.contains(&"archive.case_collision"));
}

#[test]
fn canonical_conflicts_and_same_named_inputs_keep_distinct_sources() {
    let root = TestDirectory::new("canonical-conflict");
    let first = root.path().join("one/part");
    let second = root.path().join("two/part");
    let first_export = first.join("wrapper-a/Takeout/Google Photos/2026");
    let second_export = second.join("wrapper-b/Takeout/Google Photos/2026");
    fs::create_dir_all(&first_export).expect("first export");
    fs::create_dir_all(&second_export).expect("second export");
    fs::write(first_export.join("same.txt"), b"first").expect("first file");
    fs::write(second_export.join("same.txt"), b"second").expect("second file");

    let report = scan_exports(&[first, second], &ScanOptions::default()).expect("scan conflict");
    assert!(report.findings.iter().any(|finding| {
        finding.code == "archive.conflicting_entry"
            && finding.path.as_deref() == Some("Takeout/Google Photos/2026/same.txt")
    }));
    assert_eq!(report.inputs[0].label, "part#1");
    assert_eq!(report.inputs[1].label, "part#2");
}

#[test]
fn malformed_large_and_ambiguous_provider_json_is_reported() {
    let root = TestDirectory::new("provider-errors");
    let google = root.path().join("Takeout/Google Photos/2026");
    let instagram = root
        .path()
        .join("your_instagram_activity/messages/inbox/friend");
    fs::create_dir_all(&google).expect("google dir");
    fs::create_dir_all(&instagram).expect("instagram dir");
    fs::write(google.join("invalid.svg.supplemental-metadata.json"), b"{")
        .expect("bad google JSON");
    fs::write(instagram.join("message_1.json"), b"{").expect("bad Instagram JSON");
    fs::write(
        instagram.join("message_2.json"),
        br#"{"participants":[],"messages":[{"content":"this document exceeds the tiny test cap"}]}"#,
    )
    .expect("large JSON");

    let report = scan_exports(
        &[root.path().to_path_buf()],
        &ScanOptions { max_json_bytes: 32 },
    )
    .expect("scan malformed providers");
    let codes = finding_codes(&report);
    assert!(codes.contains(&"google.invalid_sidecar"));
    assert!(codes.contains(&"instagram.invalid_message_json"));
    assert!(codes.contains(&"json.too_large"));
}

#[test]
fn duplicate_google_sidecars_and_missing_sidecar_set_are_distinct() {
    let root = TestDirectory::new("google-ambiguity");
    let with_sidecars = root.path().join("with/Takeout/Google Photos/2026");
    fs::create_dir_all(&with_sidecars).expect("with sidecars");
    fs::write(
        with_sidecars.join("photo.svg"),
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
    )
    .expect("media");
    for name in [
        "photo.svg.supplemental-metadata.json",
        "second.supplemental-metadata.json",
    ] {
        fs::write(
            with_sidecars.join(name),
            br#"{"title":"photo.svg","photoTakenTime":{"timestamp":"1"}}"#,
        )
        .expect("sidecar");
    }
    let ambiguous =
        scan_exports(&[root.path().join("with")], &ScanOptions::default()).expect("ambiguous scan");
    assert!(finding_codes(&ambiguous).contains(&"google.duplicate_sidecar"));

    let without_sidecars = root.path().join("without/Takeout/Google Photos/2026");
    fs::create_dir_all(&without_sidecars).expect("without sidecars");
    fs::write(
        without_sidecars.join("photo.svg"),
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
    )
    .expect("media only");
    let missing = scan_exports(&[root.path().join("without")], &ScanOptions::default())
        .expect("missing sidecar scan");
    assert!(finding_codes(&missing).contains(&"google.no_sidecars"));
}

#[test]
fn comparison_covers_changed_added_missing_and_new_errors() {
    let root = TestDirectory::new("compare-all");
    let before = root.path().join("before");
    let after = root.path().join("after");
    fs::create_dir_all(&before).expect("before");
    fs::create_dir_all(&after).expect("after");
    fs::write(before.join("changed.txt"), b"old").expect("old changed");
    fs::write(before.join("missing.txt"), b"missing").expect("old missing");
    fs::write(after.join("changed.txt"), b"new").expect("new changed");
    fs::write(after.join("added.txt"), b"added").expect("new added");
    fs::write(after.join("broken.jpg"), b"bad").expect("bad media");

    let report = compare_exports(
        &[before],
        &[after],
        &ScanOptions::default(),
        CompareOptions::default(),
    )
    .expect("compare all branches");
    let codes = report
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"compare.missing_after"));
    assert!(codes.contains(&"compare.changed"));
    assert!(codes.contains(&"compare.added"));
    assert!(codes.contains(&"compare.new_error"));
    assert!(report.has_regressions());
}

#[test]
fn all_report_writers_are_atomic_and_require_force() {
    let root = TestDirectory::new("writers");
    let before = root.path().join("before");
    let after = root.path().join("after");
    create_healthy_export(&before);
    create_healthy_export(&after);
    fs::write(after.join("new.txt"), b"new").expect("new file");
    let scan = scan_exports(std::slice::from_ref(&before), &ScanOptions::default()).expect("scan");
    let comparison = compare_exports(
        &[before],
        &[after],
        &ScanOptions::default(),
        CompareOptions::default(),
    )
    .expect("comparison");
    let scan_json = root.path().join("reports/scan.json");
    let scan_html = root.path().join("reports/scan.html");
    let compare_json = root.path().join("reports/compare.json");
    let compare_html = root.path().join("reports/compare.html");

    write_scan_json(&scan, &scan_json, false).expect("scan JSON");
    write_scan_html(&scan, &scan_html, false).expect("scan HTML");
    write_compare_json(&comparison, &compare_json, false).expect("compare JSON");
    write_compare_html(&comparison, &compare_html, false).expect("compare HTML");
    assert!(write_scan_json(&scan, &scan_json, false).is_err());
    write_scan_json(&scan, &scan_json, true).expect("forced scan JSON");
    assert!(
        fs::read_to_string(scan_html)
            .expect("HTML")
            .contains("Provider checks")
    );
    assert!(
        fs::read_to_string(compare_html)
            .expect("comparison HTML")
            .contains("Run health")
    );
}

#[test]
fn command_line_explain_compare_allow_missing_and_strict_modes_work() {
    let root = TestDirectory::new("cli-modes");
    let before = root.path().join("before");
    let after = root.path().join("after");
    let empty = root.path().join("empty");
    fs::create_dir_all(&before).expect("before");
    fs::create_dir_all(&after).expect("after");
    fs::create_dir_all(&empty).expect("empty");
    fs::write(before.join("lost.txt"), b"lost").expect("lost");
    let compare_json = root.path().join("compare.json");
    let compare_html = root.path().join("compare.html");

    let allowed = Command::new(env!("CARGO_BIN_EXE_export-checkup"))
        .arg("compare")
        .arg("--before")
        .arg(&before)
        .arg("--after")
        .arg(&after)
        .arg("--allow-missing")
        .arg("--json")
        .arg(&compare_json)
        .arg("--html")
        .arg(&compare_html)
        .output()
        .expect("allowed comparison");
    assert!(allowed.status.success());
    assert!(compare_json.exists());
    assert!(compare_html.exists());

    let strict = Command::new(env!("CARGO_BIN_EXE_export-checkup"))
        .arg("scan")
        .arg(&empty)
        .arg("--strict")
        .output()
        .expect("strict scan");
    assert_eq!(strict.status.code(), Some(2));

    let listed = Command::new(env!("CARGO_BIN_EXE_export-checkup"))
        .args(["explain", "--list"])
        .output()
        .expect("explain list");
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains("archive.open_failed"));

    let unknown = Command::new(env!("CARGO_BIN_EXE_export-checkup"))
        .args(["explain", "not.a.real.code"])
        .output()
        .expect("unknown explain");
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown finding code"));
}

fn create_healthy_export(root: &Path) {
    let google = root.join("Takeout/Google Photos/2026");
    let instagram = root.join("your_instagram_activity/messages/inbox/friend_123");
    let instagram_media = instagram.join("media");
    fs::create_dir_all(&google).expect("google dir");
    fs::create_dir_all(&instagram_media).expect("instagram dir");
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><path d="M0 0h2v2H0z"/></svg>"#;
    fs::write(google.join("photo.svg"), svg).expect("google media");
    fs::write(
        google.join("photo.svg.supplemental-metadata.json"),
        br#"{"title":"photo.svg","photoTakenTime":{"timestamp":"1767225600"}}"#,
    )
    .expect("google sidecar");
    fs::write(instagram_media.join("photo.svg"), svg).expect("instagram media");
    fs::write(
        instagram.join("message_1.json"),
        br#"{"participants":[{"name":"Friend"}],"messages":[{"sender_name":"Friend","timestamp_ms":1767225600000,"photos":[{"uri":"your_instagram_activity/messages/inbox/friend_123/media/photo.svg"}]}]}"#,
    )
    .expect("instagram message");
}

fn provider_metric(report: &export_checkup::ScanReport, provider: &str, metric: &str) -> u64 {
    report
        .providers
        .iter()
        .find(|summary| summary.provider == provider)
        .and_then(|summary| summary.metrics.get(metric))
        .copied()
        .unwrap_or_default()
}

fn finding_codes(report: &export_checkup::ScanReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect()
}
