# Security policy

## Supported versions

Security fixes are applied to the latest release. Before a `1.0.0` release, APIs may change between minor versions and are documented in the changelog.

## Report a vulnerability privately

Do not open a public issue containing a malicious archive, personal export, filename list, or report. Use GitHub's **Security → Report a vulnerability** private advisory flow for this repository.

Include only the smallest synthetic reproducer you can make:

- Export Checkup version and operating system;
- the command used, with personal paths replaced;
- the finding or behavior you expected;
- whether the issue requires opening a crafted archive or only scanning a directory;
- a synthetic archive if one is necessary.

Never send a real Google Takeout or Instagram export. Maintainers will acknowledge a complete report within seven days and coordinate disclosure after a fix is available.

## Security model

Export Checkup treats every input path and archive entry as untrusted.

- ZIPs are read in place and never extracted.
- parent traversal and absolute archive paths are rejected;
- directory symlinks are not followed;
- report text is HTML-escaped;
- JSON buffering is bounded per file;
- output replacement requires `--force`;
- the application has no network code, telemetry, plugins, scripting engine, or automatic repair mode.

Resource exhaustion remains possible when a user deliberately scans a very large trusted export. See [docs/supported-formats.md](docs/supported-formats.md) for limits.
