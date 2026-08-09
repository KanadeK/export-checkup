# Dated concept and competitor research

Research date: **2026-08-08 (America/Los_Angeles)**.

This document records the selection evidence; it does not claim that no repository can ever appear later.

## Problem evidence

- Google's own Takeout help says archives can fail, suggests requesting another archive or smaller per-product increments, notes recent changes may be absent, and describes Takeout as a way to retain or transfer account data: <https://support.google.com/accounts/answer/3024190>
- Meta documents a separate download path for end-to-end encrypted Instagram message media: <https://www.facebook.com/help/instagram/1224884341728748>
- Users continue to report Google Photos sidecars without the corresponding photos: <https://www.reddit.com/r/googlephotos/comments/1ohjtai/google_takeout_not_downloading_all_the_photos/>
- Instagram users and parsers have repeatedly broken when message export layout moved from a single file to per-conversation pages: <https://stackoverflow.com/questions/64968663/messages-json-missing-in-instagram-download/65379980>
- Google Photos sidecar naming and truncation changed again, requiring format-aware matching rather than one hard-coded suffix: <https://exiftool.org/forum/index.php?topic=17536.0>

The recurring user question is not merely “how do I import this?” It is “do I have a coherent local copy before I delete or replace anything?”

## GitHub search snapshot

The following `gh search repos ... --sort stars` snapshot was collected while the authenticated GitHub search API was available. Star counts are a point-in-time signal:

| Adjacent project | Stars | Primary job |
| --- | ---: | --- |
| `simulot/immich-go` | 6,672 | Import Google Photos Takeout into Immich |
| `TheLastGimbus/GooglePhotosTakeoutHelper` | 5,845 | Reorganize Takeout photos chronologically |
| `mattwilson1024/google-photos-exif` | 1,091 | Write sidecar metadata into EXIF |
| `dtinit/data-transfer-project` | 3,617 | Provider-to-provider portability framework |
| `hfaran/slack-export-viewer` | 1,129 | Browse Slack export archives |
| `greg-randall/memento-mori` | 270 | Turn an Instagram export into a site |

Exact searches for `google takeout validator`, `data export integrity checker`, `account export verifier`, `instagram export validator`, `cloud export audit`, `data portability validator`, and `takeout archive checker` returned no matching repositories in that snapshot. Broader searches returned many importers, metadata fixers, converters, analytics projects, and viewers.

That supports a narrow gap: a **read-only, cross-part internal-consistency auditor plus cross-export regression comparator**, with stable machine output and explicit repair guidance. It does not prove mathematical uniqueness.

## Why this concept was selected

Three candidate directions were screened against the existing local portfolio and likely GitHub audience:

| Candidate | Decision | Reason |
| --- | --- | --- |
| Export Checkup | Build | Distinct consumer data-portability problem; visible recurring pain; exact audit/compare combination absent in the search; meaningful CLI behavior can ship without a hosted service |
| Warranty evidence organizer | Reject | Adjacent to existing manual-to-maintenance work and many tracker apps; OCR/provider rules would dominate an honest v0.1 |
| Rental move-in evidence diff | Reject | Useful, but mature property-inspection apps exist and legal expectations vary; lower fit for a GitHub-native reusable CLI |

Export Checkup is also distinct from local adjacent tools: it is not an archive extraction security visualizer, a privacy/PII scanner, a manual parser, a data workbench, or a UI-only viewer. Its unit of truth is the relationship between export entries and between two export runs.

## Star and discovery thesis

No implementation can promise stars. The project is optimized for a plausible path to them:

- an anxiety-driven, searchable sentence: “audit Google Takeout before deleting an account”;
- support for raw multipart ZIPs instead of requiring extraction;
- zero upload and no account credentials;
- native release binaries for non-Rust users;
- evidence artifacts useful to both humans and scripts;
- a small provider-adapter contract that creates approachable contribution issues;
- synthetic fixtures so bug reports never need personal exports;
- honest limitations that build trust rather than a fake “safe to delete” score.

Future adapter work should be driven by public format evidence and user reports, not by adding logos to a dashboard.
