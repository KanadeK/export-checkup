<p align="center">
  <img src="docs/assets/hero.svg" alt="Export Checkup — audit the export before you trust the exit" width="100%">
</p>

# Export Checkup

[![CI](https://github.com/KanadeK/export-checkup/actions/workflows/ci.yml/badge.svg)](https://github.com/KanadeK/export-checkup/actions/workflows/ci.yml)
[![Security](https://github.com/KanadeK/export-checkup/actions/workflows/security.yml/badge.svg)](https://github.com/KanadeK/export-checkup/actions/workflows/security.yml)
[![Release](https://img.shields.io/github/v/release/KanadeK/export-checkup)](https://github.com/KanadeK/export-checkup/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-116a55.svg)](LICENSE)

**Audit Google Takeout and Instagram exports before deleting an account or replacing an older backup.** Export Checkup is a zero-upload Rust CLI that reads directories and multipart ZIPs, verifies every byte it can reach, resolves provider-specific references, compares export runs, and writes deterministic JSON plus self-contained HTML receipts.

[简体中文](README.zh-CN.md) · [Research](docs/research.md) · [Supported formats](docs/supported-formats.md) · [Troubleshooting](docs/troubleshooting.md)

## The honest promise

Export Checkup can prove internal facts about the files you supplied:

- each ZIP entry can be decompressed and passes the reader's CRC check;
- no archive path escapes its root, silently conflicts across parts, or collides only by case;
- known media signatures match their extensions;
- Google Photos metadata sidecars resolve to media and media has a sidecar;
- Instagram message pages are contiguous and local attachment URIs resolve;
- a newer export did not silently lose a canonical path present in an older run;
- the report contains stable SHA-256 hashes for later verification.

It **cannot** prove that Google, Meta, or another provider exported a record it never listed. A clean result means “internally consistent under these checks,” not “safe to delete without review.” This boundary is embedded in the CLI help and every HTML report.

## Quick start

Download the binary for your platform from [Releases](https://github.com/KanadeK/export-checkup/releases/latest), then pass every part from one export run together:

```bash
export-checkup scan takeout-001.zip takeout-002.zip \
  --json export-receipt.json \
  --html export-receipt.html \
  --strict
```

Compare two complete runs. Repeat `--before` or `--after` for multipart exports:

```bash
export-checkup compare \
  --before old-001.zip --before old-002.zip \
  --after new-001.zip --after new-002.zip \
  --json comparison.json \
  --html comparison.html
```

Get the exact repair procedure for any finding:

```bash
export-checkup explain google.sidecar_missing_media
export-checkup explain --list
```

Export Checkup never extracts an archive, follows a directory symlink, uploads data, or changes an input file.

## Real checks, not a dashboard shell

| Layer | What runs |
| --- | --- |
| Input | Directory recursion without symlink following; ZIP decompression and CRC verification; path traversal rejection |
| Multipart | Exact duplicates, conflicting same-path files, and case-only collisions across every supplied part |
| Content | Streaming SHA-256 for every readable file; bounded JSON inspection; signatures/closing markers for common image/video formats |
| Google Photos | Sidecar JSON parsing, `title`/filename matching across parts, missing media, missing sidecars, ambiguous mappings |
| Instagram | `message_N.json` parsing, page-gap detection per conversation, recursive local `uri` resolution, missing attachments |
| Regression | Wrapper-independent canonical paths; missing, added, and same-path changed content between export runs |
| Evidence | Deterministic schema-versioned JSON and escaped, offline, self-contained HTML with a file manifest |

The scanner streams large media instead of loading it into memory. Provider JSON inspection is capped at 32 MiB per file by default and can be raised with `--max-json-mib` for a trusted export.

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | No errors; warnings are allowed unless `--strict` is set |
| `1` | CLI or report-write failure |
| `2` | Warnings found under `--strict` |
| `3` | Integrity error or comparison regression |

This makes the CLI safe to use in scripts without scraping prose.

## Run the synthetic examples

The repository includes invented Google Photos and Instagram export shapes with no personal data:

```bash
cargo run -- scan examples/healthy --strict \
  --json .artifacts/healthy.json \
  --html .artifacts/healthy.html

# Intentionally exits 3 and prints repairable findings.
cargo run -- scan examples/broken

# Intentionally exits 3 because lost.txt disappeared.
cargo run -- compare \
  --before examples/compare-before \
  --after examples/compare-after
```

See [examples/README.md](examples/README.md) for the expected findings.

## Install from source

Rust 1.85 or newer is required:

```bash
git clone https://github.com/KanadeK/export-checkup.git
cd export-checkup
cargo install --path . --locked
export-checkup --version
```

No service credentials, browser cookies, database, or network access are required at runtime.

## Acceptance commands

The release gate is reproducible from a clean checkout:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets --locked
cargo llvm-cov --locked --all-targets --summary-only --fail-under-lines 80
cargo package --locked
cargo run --locked -- scan examples/healthy --strict
```

On Windows, `powershell -ExecutionPolicy Bypass -File scripts/verify.ps1` also handles a Rust GNU installation that lacks Visual Studio Build Tools. On macOS/Linux, run `bash scripts/verify.sh`.

If a gate fails, follow [docs/troubleshooting.md](docs/troubleshooting.md): preserve the source export, run `export-checkup explain <code>`, correct the input set or request a fresh export, rerun `scan`, then compare the repaired run to the prior one. Never “fix” a report by deleting the evidence that produced it.

## Privacy and security

- Processing is offline and has no telemetry.
- Reports identify archive-internal logical paths and hashes; treat them as private because filenames may reveal people or places.
- Archive entries are read in place and never extracted.
- Output replacement requires explicit `--force` and uses a same-directory temporary file.
- Security reports belong in the private channel described by [SECURITY.md](SECURITY.md).

## Scope and roadmap

Version 0.1 supports generic ZIP/directory integrity, Google Photos sidecars, and Instagram JSON message exports. Adapter boundaries are intentionally small so contributors can add Facebook, TikTok, X, Discord, iCloud, or other export formats without changing the scanner.

See [docs/research.md](docs/research.md) for the dated competitor search and why this is an audit/compare tool rather than another EXIF fixer, importer, or archive viewer. Stars are never guaranteed; the project is designed for discoverability through a sharp problem statement, zero-upload behavior, native binaries, reproducible evidence, and adapter-sized contributions.

## License

[MIT](LICENSE). Export Checkup is independent software and is not affiliated with Google or Meta.
