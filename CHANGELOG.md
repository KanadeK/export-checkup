# Changelog

All notable changes are documented here. This project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-08-09

### Added

- Streaming directory and multipart ZIP scanner with SHA-256 and ZIP CRC verification.
- Path traversal, duplicate, conflict, and case-collision findings.
- Lightweight structural checks for common image and video extensions.
- Google Photos sidecar-to-media matching across archive parts.
- Instagram JSON message page, pagination, and attachment-reference checks.
- Canonical-path comparison between older and newer export runs.
- Deterministic JSON and self-contained escaped HTML reports.
- Stable exit codes and an embedded repair catalog available through `explain`.
- Synthetic healthy, broken, and comparison fixtures.
- Cross-platform CI, security audit, release packaging, checksums, and contributor hygiene gates.
- An 80% line-coverage gate; the v0.1.0 test suite records 88.51% locally on Linux.

[Unreleased]: https://github.com/KanadeK/export-checkup/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/KanadeK/export-checkup/releases/tag/v0.1.0
