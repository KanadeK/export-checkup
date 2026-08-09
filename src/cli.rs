use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};

use crate::compare::{CompareOptions, compare_exports};
use crate::model::{ComparisonReport, Finding, ScanReport, Severity};
use crate::remediation;
use crate::report::{write_compare_html, write_compare_json, write_scan_html, write_scan_json};
use crate::scanner::{ScanOptions, scan_exports};

#[derive(Debug, Parser)]
#[command(
    name = "export-checkup",
    version,
    about = "Audit personal data exports before you trust or replace them",
    long_about = "Offline integrity checks for ZIPs and directories from Google Takeout and Instagram. Export Checkup verifies what is inside the supplied export; it cannot prove that a provider included data it never listed."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scan one export run. Supply every ZIP part in the same command.
    Scan(ScanArgs),
    /// Compare an older complete export run with a newer complete run.
    Compare(CompareArgs),
    /// Show the repair procedure for a finding code.
    Explain(ExplainArgs),
}

#[derive(Debug, Args)]
struct CommonReportArgs {
    /// Write the full machine-readable report.
    #[arg(long, value_name = "PATH")]
    json: Option<PathBuf>,
    /// Write a self-contained offline HTML report.
    #[arg(long, value_name = "PATH")]
    html: Option<PathBuf>,
    /// Replace report paths that already exist.
    #[arg(long)]
    force: bool,
    /// Return exit code 2 when warnings are present.
    #[arg(long)]
    strict: bool,
    /// Maximum provider JSON file size inspected in memory.
    #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u64).range(1..=4096))]
    max_json_mib: u64,
}

#[derive(Debug, Args)]
struct ScanArgs {
    /// ZIP archives and/or directories from one export run.
    #[arg(required = true, value_name = "INPUT")]
    inputs: Vec<PathBuf>,
    #[command(flatten)]
    report: CommonReportArgs,
}

#[derive(Debug, Args)]
struct CompareArgs {
    /// One part of the older run. Repeat for multipart exports.
    #[arg(long = "before", required = true, action = clap::ArgAction::Append, value_name = "INPUT")]
    before: Vec<PathBuf>,
    /// One part of the newer run. Repeat for multipart exports.
    #[arg(long = "after", required = true, action = clap::ArgAction::Append, value_name = "INPUT")]
    after: Vec<PathBuf>,
    /// Downgrade paths missing from the newer run to warnings.
    #[arg(long)]
    allow_missing: bool,
    #[command(flatten)]
    report: CommonReportArgs,
}

#[derive(Debug, Args)]
struct ExplainArgs {
    /// Finding code such as `google.sidecar_missing_media`.
    code: Option<String>,
    /// List every finding code.
    #[arg(long)]
    list: bool,
}

#[must_use]
pub fn run() -> i32 {
    match execute(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            1
        }
    }
}

fn execute(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Scan(args) => execute_scan(&args),
        Command::Compare(args) => execute_compare(&args),
        Command::Explain(args) => execute_explain(&args),
    }
}

fn execute_scan(args: &ScanArgs) -> Result<i32> {
    let options = scan_options(args.report.max_json_mib)?;
    let report = scan_exports(&args.inputs, &options)?;
    write_outputs_scan(&report, &args.report)?;
    print_scan_summary(&report);
    print_findings(&report.findings);
    Ok(scan_exit_code(&report, args.report.strict))
}

fn execute_compare(args: &CompareArgs) -> Result<i32> {
    let options = scan_options(args.report.max_json_mib)?;
    let report = compare_exports(
        &args.before,
        &args.after,
        &options,
        CompareOptions {
            allow_missing: args.allow_missing,
        },
    )?;
    write_outputs_compare(&report, &args.report)?;
    println!(
        "comparison: {} missing · {} added · {} changed · errors {} → {}",
        report.summary.missing_after,
        report.summary.added_after,
        report.summary.changed,
        report.summary.before_errors,
        report.summary.after_errors
    );
    print_findings(&report.findings);
    if report.after.has_errors()
        || report
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
    {
        Ok(3)
    } else if args.report.strict
        && (report.summary.changed > 0
            || report.before.has_warnings()
            || report.after.has_warnings())
    {
        Ok(2)
    } else {
        Ok(0)
    }
}

fn execute_explain(args: &ExplainArgs) -> Result<i32> {
    if args.list || args.code.is_none() {
        for item in remediation::all() {
            println!("{:<36} {}", item.code, item.title);
        }
        return Ok(0);
    }
    let code = args.code.as_deref().unwrap_or_default();
    let Some(item) = remediation::find(code) else {
        bail!("unknown finding code {code:?}; use `export-checkup explain --list`");
    };
    println!("{} — {}\n\n{}", item.code, item.title, item.steps);
    Ok(0)
}

fn scan_options(max_json_mib: u64) -> Result<ScanOptions> {
    let max_json_bytes = max_json_mib
        .checked_mul(1024 * 1024)
        .context("--max-json-mib is too large")?;
    Ok(ScanOptions { max_json_bytes })
}

fn write_outputs_scan(report: &ScanReport, args: &CommonReportArgs) -> Result<()> {
    if let Some(path) = &args.json {
        write_scan_json(report, path, args.force)?;
        println!("wrote JSON report: {}", path.display());
    }
    if let Some(path) = &args.html {
        write_scan_html(report, path, args.force)?;
        println!("wrote HTML report: {}", path.display());
    }
    Ok(())
}

fn write_outputs_compare(report: &ComparisonReport, args: &CommonReportArgs) -> Result<()> {
    if let Some(path) = &args.json {
        write_compare_json(report, path, args.force)?;
        println!("wrote JSON report: {}", path.display());
    }
    if let Some(path) = &args.html {
        write_compare_html(report, path, args.force)?;
        println!("wrote HTML report: {}", path.display());
    }
    Ok(())
}

fn print_scan_summary(report: &ScanReport) {
    println!(
        "scan: {} inputs · {} files · {} bytes · {} errors · {} warnings",
        report.summary.inputs,
        report.summary.files,
        report.summary.bytes,
        report.summary.errors,
        report.summary.warnings
    );
    for provider in report.providers.iter().filter(|provider| provider.detected) {
        let metrics = provider
            .metrics
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("provider {}: {metrics}", provider.provider);
    }
}

fn print_findings(findings: &[Finding]) {
    for finding in findings {
        let severity = match finding.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN ",
            Severity::Info => "INFO ",
        };
        let location = finding
            .path
            .as_deref()
            .or(finding.source.as_deref())
            .unwrap_or("-");
        println!(
            "{severity} {} [{location}] {}",
            finding.code, finding.message
        );
    }
}

const fn scan_exit_code(report: &ScanReport, strict: bool) -> i32 {
    if report.has_errors() {
        3
    } else if strict && report.has_warnings() {
        2
    } else {
        0
    }
}
