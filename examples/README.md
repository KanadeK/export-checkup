# Synthetic example exports

Every file here is invented and contains no personal data.

- `healthy/` contains one Google Photos media/sidecar pair and one Instagram message attachment. A strict scan exits `0`.
- `broken/` contains a Google sidecar without media, media without sidecars, a fake JPEG, an Instagram attachment reference without a file, and a missing `message_2.json`. A scan exits `3` and every finding includes a repair procedure.
- `compare-before/` and `compare-after/` demonstrate wrapper-directory normalization plus one removed and one added path. The comparison exits `3` because removal is a regression by default.

```bash
cargo run -- scan examples/healthy --strict
cargo run -- scan examples/broken
cargo run -- compare --before examples/compare-before --after examples/compare-after
```
