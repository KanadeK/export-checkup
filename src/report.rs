use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::model::{ComparisonReport, Finding, ScanReport, Severity};

/// Writes a deterministic JSON scan report using a same-directory temporary file.
///
/// # Errors
///
/// Returns an error when serialization, directory creation, writing, or replacement fails.
pub fn write_scan_json(report: &ScanReport, path: &Path, force: bool) -> Result<()> {
    write_json(report, path, force)
}

/// Writes a deterministic JSON comparison report.
///
/// # Errors
///
/// Returns an error when serialization, directory creation, writing, or replacement fails.
pub fn write_compare_json(report: &ComparisonReport, path: &Path, force: bool) -> Result<()> {
    write_json(report, path, force)
}

/// Writes a self-contained, escaped HTML scan report.
///
/// # Errors
///
/// Returns an error when directory creation, writing, or replacement fails.
pub fn write_scan_html(report: &ScanReport, path: &Path, force: bool) -> Result<()> {
    write_atomic(path, render_scan_html(report).as_bytes(), force)
}

/// Writes a self-contained, escaped HTML comparison report.
///
/// # Errors
///
/// Returns an error when directory creation, writing, or replacement fails.
pub fn write_compare_html(report: &ComparisonReport, path: &Path, force: bool) -> Result<()> {
    write_atomic(path, render_compare_html(report).as_bytes(), force)
}

fn write_json<T: Serialize>(value: &T, path: &Path, force: bool) -> Result<()> {
    let mut output = serde_json::to_vec_pretty(value).context("serialize JSON report")?;
    output.push(b'\n');
    write_atomic(path, &output, force)
}

fn write_atomic(path: &Path, bytes: &[u8], force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "output already exists: {} (use --force to replace it)",
            path.display()
        );
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create report directory {}", parent.display()))?;
    }
    let temporary = temporary_path(path);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .with_context(|| format!("create temporary report {}", temporary.display()))?;
    let write_result = (|| -> Result<()> {
        file.write_all(bytes).context("write report")?;
        file.sync_all().context("sync report")?;
        Ok(())
    })();
    if let Err(error) = write_result {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    if force && path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("replace existing report {}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .with_context(|| format!("move report into place at {}", path.display()))?;
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("report");
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

#[must_use]
pub fn render_scan_html(report: &ScanReport) -> String {
    let title = "Export Checkup scan report";
    let mut body = String::new();
    summary_cards(
        &mut body,
        &[
            ("Files", report.summary.files),
            ("Bytes", report.summary.bytes),
            ("Errors", report.summary.errors),
            ("Warnings", report.summary.warnings),
        ],
    );
    body.push_str("<section><h2>Provider checks</h2><div class=table-wrap><table><thead><tr><th>Provider</th><th>Detected</th><th>Metrics</th></tr></thead><tbody>");
    for provider in &report.providers {
        let metrics = provider
            .metrics
            .iter()
            .map(|(key, value)| format!("{}: {}", escape(key), value))
            .collect::<Vec<_>>()
            .join(" · ");
        let _ = write!(
            body,
            "<tr><td>{}</td><td>{}</td><td>{metrics}</td></tr>",
            escape(&provider.provider),
            if provider.detected { "yes" } else { "no" }
        );
    }
    body.push_str("</tbody></table></div></section>");
    findings_section(&mut body, &report.findings);
    body.push_str("<section><details><summary>File manifest</summary><div class=table-wrap><table><thead><tr><th>Canonical path</th><th>Source</th><th>Bytes</th><th>SHA-256</th></tr></thead><tbody>");
    for file in &report.files {
        let _ = write!(
            body,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
            escape(&file.canonical_path),
            escape(&file.source),
            file.size,
            escape(file.sha256.as_deref().unwrap_or("unavailable"))
        );
    }
    body.push_str("</tbody></table></div></details></section>");
    document(
        title,
        &body,
        &format!("Export Checkup {} · offline report", report.version),
    )
}

#[must_use]
pub fn render_compare_html(report: &ComparisonReport) -> String {
    let title = "Export Checkup comparison report";
    let mut body = String::new();
    summary_cards(
        &mut body,
        &[
            ("Missing after", report.summary.missing_after),
            ("Added after", report.summary.added_after),
            ("Changed", report.summary.changed),
            ("After errors", report.summary.after_errors),
        ],
    );
    findings_section(&mut body, &report.findings);
    body.push_str("<section><h2>Run health</h2><div class=table-wrap><table><thead><tr><th>Run</th><th>Files</th><th>Bytes</th><th>Errors</th><th>Warnings</th></tr></thead><tbody>");
    for (name, scan) in [("Before", &report.before), ("After", &report.after)] {
        let _ = write!(
            body,
            "<tr><td>{name}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            scan.summary.files, scan.summary.bytes, scan.summary.errors, scan.summary.warnings
        );
    }
    body.push_str("</tbody></table></div></section>");
    document(
        title,
        &body,
        &format!("Export Checkup {} · offline comparison", report.version),
    )
}

fn summary_cards(output: &mut String, cards: &[(&str, u64)]) {
    output.push_str("<section class=cards aria-label=Summary>");
    for (label, value) in cards {
        let display = if *label == "Bytes" {
            format_bytes(*value)
        } else {
            value.to_string()
        };
        let _ = write!(
            output,
            "<article class=card><span>{}</span><strong>{display}</strong></article>",
            escape(label)
        );
    }
    output.push_str("</section>");
}

fn findings_section(output: &mut String, findings: &[Finding]) {
    output.push_str("<section><h2>Findings</h2>");
    if findings.is_empty() {
        output.push_str("<p class=ok>No findings. The supplied files are internally consistent under the checks performed.</p></section>");
        return;
    }
    output.push_str("<div class=table-wrap><table><thead><tr><th>Severity</th><th>Code</th><th>Path</th><th>Finding</th><th>Repair</th></tr></thead><tbody>");
    for finding in findings {
        let severity = match finding.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        let _ = write!(
            output,
            "<tr><td><span class=\"pill {severity}\">{severity}</span></td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
            escape(&finding.code),
            escape(finding.path.as_deref().unwrap_or("—")),
            escape(&finding.message),
            escape(&finding.remediation)
        );
    }
    output.push_str("</tbody></table></div></section>");
}

fn document(title: &str, body: &str, footer: &str) -> String {
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{}</style></head><body><main><header><p class=kicker>PRIVATE · LOCAL · REPRODUCIBLE</p><h1>{}</h1><p>This report checks internal consistency. It cannot prove that a provider exported records it never listed.</p></header>{}<footer>{}</footer></main></body></html>",
        escape(title),
        CSS,
        escape(title),
        body,
        escape(footer)
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        let mut divisor = 1024_u64;
        let mut unit = 1_usize;
        while bytes / divisor >= 1024 && unit + 1 < UNITS.len() {
            divisor = divisor.saturating_mul(1024);
            unit += 1;
        }
        let whole = bytes / divisor;
        let fraction = (bytes % divisor).saturating_mul(100) / divisor;
        format!("{whole}.{fraction:02} {}", UNITS[unit])
    }
}

const CSS: &str = r#"
:root{color-scheme:light dark;--bg:#f4f2ec;--panel:#fffdf7;--ink:#1f2925;--muted:#637069;--line:#cfd5ce;--accent:#116a55;--error:#b42318;--warning:#a15c00;--info:#175cd3}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.55 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}main{max-width:1180px;margin:auto;padding:48px 24px}header{max-width:760px;margin-bottom:30px}h1{font-size:clamp(2rem,5vw,4.8rem);line-height:.95;letter-spacing:-.055em;margin:.15em 0 .35em}h2{font-size:1.25rem;margin:0 0 12px}.kicker{font-size:.74rem;font-weight:800;letter-spacing:.16em;color:var(--accent)}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin:22px 0}.card{background:var(--panel);border:1px solid var(--line);border-radius:14px;padding:16px}.card span{display:block;color:var(--muted);font-size:.8rem}.card strong{font-size:1.8rem}section{margin:28px 0}.table-wrap{overflow:auto;border:1px solid var(--line);border-radius:14px;background:var(--panel)}table{width:100%;border-collapse:collapse}th,td{text-align:left;vertical-align:top;padding:11px 13px;border-bottom:1px solid var(--line)}th{font-size:.75rem;text-transform:uppercase;letter-spacing:.07em;color:var(--muted)}tr:last-child td{border-bottom:0}code{font:12px/1.45 ui-monospace,SFMono-Regular,Consolas,monospace;overflow-wrap:anywhere}.pill{display:inline-block;border:1px solid currentColor;border-radius:99px;padding:2px 8px;font-size:.72rem;font-weight:750}.error{color:var(--error)}.warning{color:var(--warning)}.info{color:var(--info)}.ok{border-left:4px solid var(--accent);padding:12px 16px;background:var(--panel)}summary{cursor:pointer;font-weight:700;padding:14px}footer{margin-top:44px;color:var(--muted);font-size:.8rem}@media(prefers-color-scheme:dark){:root{--bg:#101411;--panel:#171d19;--ink:#eef5ef;--muted:#a4b2aa;--line:#344239;--accent:#64d9b5;--error:#ff8a80;--warning:#ffc266;--info:#84adff}}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_report_content() {
        assert_eq!(escape("<script>'&\""), "&lt;script&gt;&#39;&amp;&quot;");
    }

    #[test]
    fn formats_binary_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
    }
}
