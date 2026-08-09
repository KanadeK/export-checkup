# Contributing

Export Checkup welcomes provider adapters, format fixtures, performance fixes, and documentation improvements. Never contribute real personal exports.

## Development setup

1. Install Rust 1.85 or newer with `rustfmt` and `clippy`.
2. Fork and clone the repository.
3. Create a focused branch.
4. Add only invented or irreversibly synthetic fixtures.
5. Run the release gate before opening a pull request.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets --locked
cargo llvm-cov --locked --all-targets --summary-only --fail-under-lines 80
cargo package --locked
cargo run --locked -- scan examples/healthy --strict
```

Windows contributors without Visual Studio Build Tools can run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify.ps1
```

## Add a provider adapter

Keep provider parsing separate from archive traversal:

1. Detect only strong, documented path or JSON-shape signals.
2. Extract the minimum facts while the scanner already has bounded JSON bytes.
3. Resolve references through the shared canonical-path index.
4. Add a stable finding code and repair text in `src/remediation.rs`.
5. Add healthy, missing-reference, malformed, and format-variation tests.
6. Document exactly which provider export options the adapter expects.

Adapters must not call provider APIs, log into accounts, upload exports, alter input bytes, or claim server-side completeness.

## Pull requests

Explain the user-visible problem, evidence for the format, test coverage, privacy impact, and compatibility impact. Keep generated data out of Git history. By contributing, you agree that your work is licensed under MIT.
