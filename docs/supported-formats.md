# Supported formats and limits

## Inputs

| Input | v0.1 behavior |
| --- | --- |
| Directory | Recursively reads regular files; never follows symlinks |
| ZIP | Reads entries without extraction; verifies decompression/CRC; supports stored, Deflate, Deflate64, and Bzip2 methods |
| Multipart export | Supply every ZIP/directory as a separate positional input in one `scan` command |
| TAR/TGZ/7z/RAR | Not opened directly; extract to a new directory with a trusted tool, then scan the directory |

Encrypted ZIP entries and compression methods not compiled into the binary produce `archive.entry_read_failed` instead of being silently skipped.

## Generic checks

- unsafe absolute or parent-traversal paths;
- unreadable entries and size changes during directory scans;
- identical and conflicting exact paths across inputs;
- paths that differ only by case;
- SHA-256 for every fully readable file;
- lightweight JPEG, PNG, GIF, WebP, ISO-BMFF, and SVG structure checks.

Media checks intentionally do not decode every frame or prove perceptual correctness. A structurally valid file can still be visually damaged; open high-value media from a copy before deleting a source account.

## Google Photos Takeout

Detected under a `Google Photos/` path. v0.1 recognizes `*.supplemental-metadata.json` and JSON objects containing sidecar-like `title` plus time fields. It resolves:

1. the sidecar `title` in the same directory;
2. the sidecar filename with `.supplemental-metadata.json` removed;
3. the sidecar filename with `.json` removed;
4. matching paths across every supplied part.

It reports missing media, media without sidecars, malformed sidecars, and multiple sidecars resolving to one media file. It does not rewrite EXIF, reorganize albums, upload to another photo service, or infer a server-side photo count.

## Instagram JSON exports

Detected through `your_instagram_activity/` or `messages/inbox/` paths. v0.1 parses `messages/inbox/*/message_N.json`, counts messages, checks page numbers from 1 through the maximum page in each conversation, recursively collects local `uri` values, percent-decodes them, and resolves attachments across all supplied parts.

Remote `http:`, `https:`, and `data:` URIs are not treated as missing local attachments. HTML-format message exports are inventoried but not semantically parsed; request JSON for provider checks.

## Comparison

Canonicalization removes changing wrapper directories while retaining provider paths such as `Takeout/Google Photos/...` and `your_instagram_activity/...`. Comparison reports:

- paths missing from the newer export (error by default);
- new paths (information);
- changed SHA-256 at the same path (warning);
- a higher integrity-error count in the newer run (error).

Use `--allow-missing` only when deletion is intentional and separately documented. It does not suppress unrelated integrity errors.

## Resource limits

JSON inspection defaults to 32 MiB per file. Raising it increases peak memory. The scanner stores one compact manifest record per file, so exports with millions of entries require corresponding memory for paths and hashes. ZIP bombs can consume CPU and read bandwidth even though entries are never extracted; stop the process if expansion is unexpectedly large.
