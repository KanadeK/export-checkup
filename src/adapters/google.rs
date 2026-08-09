use std::collections::{BTreeMap, HashMap, HashSet};

use crate::adapters::{AdapterFacts, Resolver};
use crate::model::{FileKind, FileRecord, Finding, ProviderSummary, Severity};
use crate::pathing::{join, parent};
use crate::remediation::remediation_for;

pub(crate) fn analyze(
    records: &[FileRecord],
    facts: &AdapterFacts,
    resolver: &Resolver<'_>,
    findings: &mut Vec<Finding>,
) -> ProviderSummary {
    let media: Vec<&FileRecord> = records
        .iter()
        .filter(|record| {
            record.kind == FileKind::Media
                && record
                    .canonical_path
                    .to_lowercase()
                    .contains("google photos/")
        })
        .collect();
    let detected = facts.google_detected || !media.is_empty();
    let (matched_media, missing_media) = match_sidecars(facts, resolver, findings);
    let missing_sidecars = report_missing_sidecars(&media, &matched_media, findings);

    if detected && !media.is_empty() && facts.google_sidecars.is_empty() {
        let code = "google.no_sidecars";
        findings.push(Finding::new(
            Severity::Warning,
            code,
            "Google Photos media was detected, but no metadata sidecars were found.",
            remediation_for(code),
        ));
    }

    let mut metrics = BTreeMap::new();
    metrics.insert("media_files".to_owned(), usize_to_u64(media.len()));
    metrics.insert(
        "sidecars".to_owned(),
        usize_to_u64(facts.google_sidecars.len()),
    );
    metrics.insert(
        "matched_pairs".to_owned(),
        usize_to_u64(matched_media.len()),
    );
    metrics.insert("missing_media".to_owned(), missing_media);
    metrics.insert("missing_sidecars".to_owned(), missing_sidecars);

    ProviderSummary {
        provider: "google_photos".to_owned(),
        detected,
        metrics,
    }
}

fn match_sidecars(
    facts: &AdapterFacts,
    resolver: &Resolver<'_>,
    findings: &mut Vec<Finding>,
) -> (HashSet<String>, u64) {
    let mut matched_media = HashSet::new();
    let mut sidecars_per_media: HashMap<String, Vec<String>> = HashMap::new();
    let mut missing_media = 0_u64;

    for sidecar in &facts.google_sidecars {
        let parent = parent(&sidecar.path);
        let mut candidates = vec![join(parent, &sidecar.title)];
        if let Some(base) = sidecar.path.strip_suffix(".supplemental-metadata.json") {
            candidates.push(base.to_owned());
        }
        if let Some(base) = sidecar.path.strip_suffix(".json") {
            candidates.push(base.to_owned());
        }
        candidates.sort();
        candidates.dedup();

        let matched_record = candidates
            .iter()
            .flat_map(|candidate| resolver.resolve(candidate))
            .find(|record| record.kind == FileKind::Media && record.sha256.is_some());

        if let Some(record) = matched_record {
            matched_media.insert(record.canonical_path.to_lowercase());
            sidecars_per_media
                .entry(record.canonical_path.to_lowercase())
                .or_default()
                .push(sidecar.path.clone());
        } else {
            missing_media += 1;
            let code = "google.sidecar_missing_media";
            findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    format!(
                        "Sidecar title {:?} did not resolve to media in any supplied part.",
                        sidecar.title
                    ),
                    remediation_for(code),
                )
                .at_path(&sidecar.path)
                .in_source(&sidecar.source),
            );
        }
    }

    for (media_path, sidecars) in sidecars_per_media {
        if sidecars.len() > 1 {
            let code = "google.duplicate_sidecar";
            findings.push(
                Finding::new(
                    Severity::Warning,
                    code,
                    format!(
                        "{} sidecars resolve to the same media file.",
                        sidecars.len()
                    ),
                    remediation_for(code),
                )
                .at_path(media_path),
            );
        }
    }
    (matched_media, missing_media)
}

fn report_missing_sidecars(
    media: &[&FileRecord],
    matched_media: &HashSet<String>,
    findings: &mut Vec<Finding>,
) -> u64 {
    let mut missing_sidecars = 0_u64;
    for record in media {
        if !matched_media.contains(&record.canonical_path.to_lowercase()) {
            missing_sidecars += 1;
            let code = "google.media_missing_sidecar";
            findings.push(
                Finding::new(
                    Severity::Warning,
                    code,
                    "Media has no matching Google Photos metadata sidecar.",
                    remediation_for(code),
                )
                .at_path(&record.logical_path)
                .in_source(&record.source),
            );
        }
    }
    missing_sidecars
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
