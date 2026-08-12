#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remediation {
    pub code: &'static str,
    pub title: &'static str,
    pub steps: &'static str,
}

const REMEDIATIONS: &[Remediation] = &[
    Remediation {
        code: "input.not_found",
        title: "Input does not exist",
        steps: "Confirm every archive part finished downloading, then pass the corrected path. Keep all parts from one export together.",
    },
    Remediation {
        code: "input.unsupported",
        title: "Unsupported input",
        steps: "Pass a directory or a ZIP archive. For TGZ/TAR exports, extract them to a new directory first and scan that directory without modifying the original.",
    },
    Remediation {
        code: "archive.open_failed",
        title: "Archive could not be opened",
        steps: "Download the affected part again. If the provider offers split-size choices, request smaller parts and scan every resulting ZIP together.",
    },
    Remediation {
        code: "archive.invalid_path",
        title: "Unsafe archive path",
        steps: "Do not extract this archive over valuable files. Request a fresh export and retain the report as evidence of the unsafe entry.",
    },
    Remediation {
        code: "archive.entry_read_failed",
        title: "Archive entry failed CRC or decompression",
        steps: "Download the named archive part again and rerun the same command. If the failure repeats, request a fresh export from the provider.",
    },
    Remediation {
        code: "archive.duplicate_entry",
        title: "Duplicate archive entry",
        steps: "Check that the command did not include the same ZIP or directory twice. Identical duplicates are safe to review but should be removed from the input set before creating a final receipt.",
    },
    Remediation {
        code: "archive.conflicting_entry",
        title: "Conflicting duplicate path",
        steps: "Separate archive parts from different export runs. Scan each run independently, then use compare; never merge same-named files with different hashes.",
    },
    Remediation {
        code: "archive.case_collision",
        title: "Case-colliding paths",
        steps: "Keep the original archives intact and restore onto a case-sensitive filesystem. Case-insensitive filesystems may overwrite one of the colliding files.",
    },
    Remediation {
        code: "archive.empty",
        title: "No files found",
        steps: "Verify the export selection and download all parts again. A directory wrapper with no files is not a usable backup.",
    },
    Remediation {
        code: "file.symlink_skipped",
        title: "Symbolic link skipped",
        steps: "Scan the link target explicitly if it belongs to the export. Export Checkup never follows links because they can escape the selected directory.",
    },
    Remediation {
        code: "file.read_failed",
        title: "File could not be read",
        steps: "Copy the export to healthy local storage, check permissions, then rerun. Preserve the original until the copy has passed.",
    },
    Remediation {
        code: "file.changed_during_scan",
        title: "File changed during scan",
        steps: "Stop sync or extraction processes that are writing the export, then scan an unchanged copy.",
    },
    Remediation {
        code: "media.invalid_structure",
        title: "Media structure does not match its extension",
        steps: "Open the named file from a copy, redownload the containing archive part, and rerun. Do not delete the cloud original based on a structurally invalid local copy.",
    },
    Remediation {
        code: "json.too_large",
        title: "Provider JSON exceeded the inspection limit",
        steps: "Rerun with --max-json-mib set high enough for this trusted export, or split the provider export into smaller time ranges.",
    },
    Remediation {
        code: "google.invalid_sidecar",
        title: "Google Photos sidecar is invalid JSON",
        steps: "Redownload the containing Takeout part. If it repeats, create a new Photos-only Takeout and compare the two runs.",
    },
    Remediation {
        code: "google.sidecar_missing_media",
        title: "Sidecar has no matching media",
        steps: "Confirm every Takeout part was supplied. Search the original ZIPs for the sidecar title; if the media is absent, request a new Photos-only Takeout before deleting anything.",
    },
    Remediation {
        code: "google.media_missing_sidecar",
        title: "Media has no matching sidecar",
        steps: "The media bytes are present, but Google-only edits may be missing. Request a fresh Photos-only Takeout or retain the file with its embedded metadata and review manually.",
    },
    Remediation {
        code: "google.duplicate_sidecar",
        title: "Several sidecars resolve to one media file",
        steps: "Keep the originals and compare sidecar JSON before choosing one. Do not apply metadata automatically while the mapping is ambiguous.",
    },
    Remediation {
        code: "google.no_sidecars",
        title: "Google Photos detected without sidecars",
        steps: "Verify that JSON metadata was included and that every Takeout part was scanned. A media-only directory cannot preserve Google-side edits.",
    },
    Remediation {
        code: "instagram.invalid_message_json",
        title: "Instagram message page is invalid JSON",
        steps: "Request JSON format again from Accounts Center and redownload the affected part. Keep the malformed file for comparison.",
    },
    Remediation {
        code: "instagram.missing_attachment",
        title: "Instagram message attachment is missing",
        steps: "Supply every export part and confirm the referenced URI is absent. Request a new all-time, high-quality JSON export before removing the account.",
    },
    Remediation {
        code: "instagram.page_gap",
        title: "Instagram message pagination has a gap",
        steps: "Check all ZIP parts for the missing message_N.json page. If it is absent, request a fresh JSON export and compare runs.",
    },
    Remediation {
        code: "instagram.no_message_pages",
        title: "Instagram messages folder has no JSON pages",
        steps: "Confirm Messages was selected, the date range was All time, and JSON format was requested. Then download and scan every part.",
    },
    Remediation {
        code: "compare.missing_after",
        title: "A prior path is absent from the newer export",
        steps: "Confirm both sides contain every part from their respective run. If the path is still absent, keep the older copy and review the provider account before deletion.",
    },
    Remediation {
        code: "compare.changed",
        title: "Content changed at the same path",
        steps: "Review the old and new copies. Expected metadata edits are normal; unexplained media-byte changes should be investigated before replacing the old export.",
    },
    Remediation {
        code: "compare.added",
        title: "New path appeared",
        steps: "No repair is required. Confirm the addition matches activity between export dates and retain it in the newer archive.",
    },
    Remediation {
        code: "compare.new_error",
        title: "Newer export has more integrity errors",
        steps: "Do not replace the older export. Repair the newer run using its finding codes, or request another export and compare again.",
    },
];

#[must_use]
pub fn remediation_for(code: &str) -> &'static str {
    REMEDIATIONS
        .iter()
        .find(|item| item.code == code)
        .map_or("Preserve the original export, inspect the finding context, and rerun after correcting the input.", |item| item.steps)
}

#[must_use]
pub const fn all() -> &'static [Remediation] {
    REMEDIATIONS
}

#[must_use]
pub fn find(code: &str) -> Option<&'static Remediation> {
    REMEDIATIONS.iter().find(|item| item.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_supports_known_and_unknown_codes() {
        assert!(all().len() > 20);
        assert_eq!(
            find("archive.open_failed").map(|item| item.title),
            Some("Archive could not be opened")
        );
        assert!(find("unknown.code").is_none());
        assert!(remediation_for("unknown.code").contains("Preserve the original"));
    }
}
