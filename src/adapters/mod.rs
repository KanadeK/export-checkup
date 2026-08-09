mod google;
mod instagram;

use std::collections::HashMap;

use percent_encoding::percent_decode_str;
use serde_json::Value;

use crate::model::{FileRecord, Finding, ProviderSummary, Severity};
use crate::pathing::{basename, normalize_logical_path, parent};
use crate::remediation::remediation_for;

#[derive(Debug, Clone)]
pub(crate) struct GoogleSidecarFact {
    pub path: String,
    pub source: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub(crate) struct InstagramPageFact {
    pub path: String,
    pub source: String,
    pub parent: String,
    pub page: u64,
    pub messages: u64,
    pub local_references: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct AdapterFacts {
    pub google_detected: bool,
    pub instagram_detected: bool,
    pub google_sidecars: Vec<GoogleSidecarFact>,
    pub instagram_pages: Vec<InstagramPageFact>,
}

pub(crate) fn inspect_json(
    path: &str,
    source: &str,
    bytes: &[u8],
    facts: &mut AdapterFacts,
    findings: &mut Vec<Finding>,
) {
    let lower = path.to_lowercase();
    let google_path = lower.contains("google photos/");
    let google_sidecar_name = lower.ends_with(".supplemental-metadata.json");
    let instagram_page = instagram_page_number(path);

    if google_path {
        facts.google_detected = true;
    }
    if lower.contains("your_instagram_activity/") || lower.contains("messages/inbox/") {
        facts.instagram_detected = true;
    }

    if !google_path && instagram_page.is_none() {
        return;
    }

    let parsed: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(error) => {
            let code = if instagram_page.is_some() {
                "instagram.invalid_message_json"
            } else if google_sidecar_name {
                "google.invalid_sidecar"
            } else {
                return;
            };
            findings.push(
                Finding::new(
                    Severity::Error,
                    code,
                    format!("JSON parsing failed: {error}"),
                    remediation_for(code),
                )
                .at_path(path)
                .in_source(source),
            );
            return;
        }
    };

    if google_path {
        let looks_like_sidecar = google_sidecar_name
            || parsed.get("photoTakenTime").is_some()
            || (parsed.get("creationTime").is_some() && parsed.get("title").is_some());
        if looks_like_sidecar {
            if let Some(title) = parsed.get("title").and_then(Value::as_str) {
                facts.google_sidecars.push(GoogleSidecarFact {
                    path: path.to_owned(),
                    source: source.to_owned(),
                    title: title.to_owned(),
                });
            } else {
                let code = "google.invalid_sidecar";
                findings.push(
                    Finding::new(
                        Severity::Error,
                        code,
                        "Google Photos sidecar has no string title field.",
                        remediation_for(code),
                    )
                    .at_path(path)
                    .in_source(source),
                );
            }
        }
    }

    if let Some(page) = instagram_page {
        let mut references = Vec::new();
        collect_local_uris(&parsed, &mut references);
        references.sort();
        references.dedup();
        let messages = parsed
            .get("messages")
            .and_then(Value::as_array)
            .map_or(0, |values| u64::try_from(values.len()).unwrap_or(u64::MAX));
        facts.instagram_pages.push(InstagramPageFact {
            path: path.to_owned(),
            source: source.to_owned(),
            parent: parent(path).to_owned(),
            page,
            messages,
            local_references: references,
        });
    }
}

pub(crate) fn analyze(
    records: &[FileRecord],
    facts: &AdapterFacts,
    findings: &mut Vec<Finding>,
) -> Vec<ProviderSummary> {
    let resolver = Resolver::new(records);
    vec![
        google::analyze(records, facts, &resolver, findings),
        instagram::analyze(facts, &resolver, findings),
    ]
}

pub(crate) struct Resolver<'a> {
    records: &'a [FileRecord],
    lookup: HashMap<String, Vec<usize>>,
}

impl<'a> Resolver<'a> {
    fn new(records: &'a [FileRecord]) -> Self {
        let mut lookup: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, record) in records.iter().enumerate() {
            index_path(&mut lookup, &record.logical_path, index);
            index_path(&mut lookup, &record.canonical_path, index);
        }
        Self { records, lookup }
    }

    pub fn resolve(&self, candidate: &str) -> Vec<&'a FileRecord> {
        let Some(normalized) = normalize_reference(candidate) else {
            return Vec::new();
        };
        let lower = normalized.to_lowercase();
        let mut indices = self.lookup.get(&lower).cloned().unwrap_or_default();
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .filter_map(|index| self.records.get(index))
            .collect()
    }
}

fn index_path(index: &mut HashMap<String, Vec<usize>>, path: &str, record_index: usize) {
    let lower = path.to_lowercase();
    index.entry(lower.clone()).or_default().push(record_index);
    for anchor in ["takeout/google photos/", "google photos/", "messages/"] {
        if let Some(offset) = lower.find(anchor) {
            index
                .entry(lower[offset..].to_owned())
                .or_default()
                .push(record_index);
        }
    }
}

fn normalize_reference(value: &str) -> Option<String> {
    let decoded = percent_decode_str(value).decode_utf8_lossy();
    let trimmed = decoded.trim().trim_start_matches('/');
    let lower = trimmed.to_lowercase();
    if trimmed.is_empty()
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
    {
        return None;
    }
    normalize_logical_path(trimmed)
}

fn collect_local_uris(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "uri" {
                    if let Some(uri) = child.as_str().and_then(normalize_reference) {
                        output.push(uri);
                    }
                }
                collect_local_uris(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_local_uris(child, output);
            }
        }
        _ => {}
    }
}

fn instagram_page_number(path: &str) -> Option<u64> {
    let lower = path.to_lowercase();
    if !lower.contains("messages/inbox/") {
        return None;
    }
    let name = basename(&lower);
    let number = name.strip_prefix("message_")?.strip_suffix(".json")?;
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_remote_uris_and_decodes_local_ones() {
        assert_eq!(normalize_reference("https://example.com/a"), None);
        assert_eq!(
            normalize_reference("messages/inbox/a/media/photo%20one.jpg"),
            Some("messages/inbox/a/media/photo one.jpg".to_owned())
        );
    }

    #[test]
    fn extracts_instagram_page_number() {
        assert_eq!(
            instagram_page_number("your_instagram_activity/messages/inbox/a/message_12.json"),
            Some(12)
        );
        assert_eq!(instagram_page_number("messages/message_1.json"), None);
    }
}
