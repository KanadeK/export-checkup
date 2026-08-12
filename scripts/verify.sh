#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo package --locked --allow-dirty
cargo build --locked --release

mkdir -p .artifacts/verify
target/release/export-checkup scan examples/healthy --strict \
  --json .artifacts/verify/healthy-a.json \
  --html .artifacts/verify/healthy.html \
  --force
target/release/export-checkup scan examples/healthy --strict \
  --json .artifacts/verify/healthy-b.json \
  --force
cmp .artifacts/verify/healthy-a.json .artifacts/verify/healthy-b.json

set +e
target/release/export-checkup scan examples/broken
broken_code=$?
target/release/export-checkup compare \
  --before examples/compare-before \
  --after examples/compare-after
compare_code=$?
set -e

test "$broken_code" -eq 3
test "$compare_code" -eq 3
target/release/export-checkup explain google.sidecar_missing_media
python3 scripts/secret_scan.py
git diff --check

echo "[verify] all release checks passed"
