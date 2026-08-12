use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::adapters::{AdapterFacts, Resolver};
use crate::model::{Finding, ProviderSummary, Severity};
use crate::remediation::remediation_for;

pub(crate) fn analyze(
    facts: &AdapterFacts,
    resolver: &Resolver<'_>,
    findings: &mut Vec<Finding>,
) -> ProviderSummary {
    let detected = facts.instagram_detected || !facts.instagram_pages.is_empty();
    let mut missing_attachments = 0_u64;
    let mut attachment_references = 0_u64;
    let mut message_count = 0_u64;
    let mut pages_by_thread: HashMap<&str, BTreeSet<u64>> = HashMap::new();

    for page in &facts.instagram_pages {
        pages_by_thread
            .entry(page.parent.as_str())
            .or_default()
            .insert(page.page);
        message_count = message_count.saturating_add(page.messages);
        for reference in &page.local_references {
            attachment_references = attachment_references.saturating_add(1);
            if resolver.resolve(reference).is_empty() {
                missing_attachments = missing_attachments.saturating_add(1);
                let code = "instagram.missing_attachment";
                findings.push(
                    Finding::new(
                        Severity::Error,
                        code,
                        format!("Message JSON references missing local attachment {reference:?}."),
                        remediation_for(code),
                    )
                    .at_path(&page.path)
                    .in_source(&page.source),
                );
            }
        }
    }

    let mut page_gaps = 0_u64;
    for (thread, pages) in pages_by_thread {
        let Some(max_page) = pages.last().copied() else {
            continue;
        };
        for expected in 1..=max_page {
            if !pages.contains(&expected) {
                page_gaps = page_gaps.saturating_add(1);
                let code = "instagram.page_gap";
                findings.push(
                    Finding::new(
                        Severity::Error,
                        code,
                        format!("Conversation is missing message_{expected}.json before page {max_page}."),
                        remediation_for(code),
                    )
                    .at_path(thread),
                );
            }
        }
    }

    if detected && facts.instagram_pages.is_empty() {
        let code = "instagram.no_message_pages";
        findings.push(Finding::new(
            Severity::Warning,
            code,
            "Instagram export structure was detected, but no messages/inbox/*/message_N.json pages were found.",
            remediation_for(code),
        ));
    }

    let mut metrics = BTreeMap::new();
    metrics.insert(
        "message_pages".to_owned(),
        u64::try_from(facts.instagram_pages.len()).unwrap_or(u64::MAX),
    );
    metrics.insert("messages".to_owned(), message_count);
    metrics.insert("attachment_references".to_owned(), attachment_references);
    metrics.insert("missing_attachments".to_owned(), missing_attachments);
    metrics.insert("page_gaps".to_owned(), page_gaps);

    ProviderSummary {
        provider: "instagram".to_owned(),
        detected,
        metrics,
    }
}
