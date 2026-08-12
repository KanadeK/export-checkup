# Release procedure

This checklist distinguishes local completion from public release closure.

1. Run `scripts/verify.ps1` or `scripts/verify.sh` from a clean checkout.
2. Confirm `git status --short` is empty and inspect the full diff from the previous tag.
3. Verify identity and contributor hygiene:
   - `git log -1 --format=fuller`
   - `git shortlog -sne HEAD`
   - `git log --format=%B | rg "Co-authored-by"` must have no output
4. Push the release branch and require green CI/Security checks.
5. Merge through the reviewed pull request.
6. Create an annotated `vX.Y.Z` tag at the exact verified default-branch commit and push it.
7. Wait for the Release workflow to build all platform archives, the crate, Cargo lockfile, and `SHA256SUMS`.
8. Download every release asset into a new directory and run the platform's SHA-256 checker against `SHA256SUMS`.
9. Verify the public repository, tag, Release, README badges, license, topics, and contributor list while signed out or through the public API.
10. Only then announce completion.

If a tag workflow fails, fix the cause on a new commit and publish a new patch tag. Do not move a public release tag silently.
