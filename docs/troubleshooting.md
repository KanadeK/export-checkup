# Troubleshooting and repair flow

## Universal repair sequence

1. **Preserve** the original archives, the command, and the report. Do not extract over them or delete the older export.
2. **Explain** the stable code: `export-checkup explain <code>`.
3. **Confirm scope**: every part from one export run must be supplied once; do not mix two runs in one scan.
4. **Repair the source of evidence**: redownload the affected part or request a fresh provider export. Export Checkup never mutates an export.
5. **Rerun scan** with the same strictness and report paths (use new filenames, or `--force` only when replacement is intentional).
6. **Compare runs** so a repaired export does not silently lose paths present in the prior one.
7. **Review high-value files manually** before deleting a cloud account.

## Common findings

### `archive.entry_read_failed`

The ZIP reader could not fully decompress or CRC-verify an entry. Redownload that part. If it repeats, request smaller export parts and scan all of them together.

### `archive.conflicting_entry`

The same logical path has different hashes across supplied inputs. The usual cause is mixing two export runs. Separate them, scan independently, then use `compare`.

### `google.sidecar_missing_media`

The sidecar exists but its `title` and filename candidates do not resolve to media in any supplied part. Search the untouched ZIP set; if absent, request a Photos-only Takeout. Do not treat a JSON sidecar as a photo backup.

### `google.media_missing_sidecar`

Media bytes exist, but Google-only date/location/description edits may not. Preserve the file, request a fresh Photos-only export, and compare before applying any metadata repair tool.

### `instagram.missing_attachment`

A message page references a local URI that is absent. Confirm all parts and request an all-time, high-quality JSON export if needed.

### `instagram.page_gap`

At least one conversation contains a later `message_N.json` without every earlier page. Check other parts for the missing page; then request and compare a fresh export.

### `compare.missing_after`

The older run contains a canonical path absent from the newer one. If the deletion was deliberate, document it and use `--allow-missing`; otherwise keep the older copy and investigate before replacing it.

## Command or build failures

### Output already exists

Reports are not overwritten by default. Choose a new evidence filename. Use `--force` only when deliberate replacement is part of your workflow.

### Rust on Windows says `link.exe` is missing

Run `scripts/verify.ps1`. It prefers MSVC when `cl.exe` is available and otherwise uses Rust's installed `stable-x86_64-pc-windows-gnu` self-contained linker in a plain-ASCII temporary target directory. Looking only for `link.exe` is insufficient because Git for Windows may provide an unrelated program with that name. If neither toolchain exists, install one through rustup.

### JSON is larger than the inspection limit

For a trusted export, raise the per-file bound, for example `--max-json-mib 128`. This affects memory use. A safer alternative is requesting smaller provider date ranges.

Use `export-checkup explain --list` for every stable code and its packaged repair text.
