#!/usr/bin/env python3
"""Fail when source files contain credential-shaped material."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path


PATTERNS = {
    "GitHub token": re.compile(rb"\bgh[pousr]_[A-Za-z0-9]{30,}\b"),
    "Google API key": re.compile(rb"\bAIza[0-9A-Za-z_-]{30,}\b"),
    "AWS access key": re.compile(rb"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
    "private key": re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "Slack token": re.compile(rb"\bxox[baprs]-[A-Za-z0-9-]{20,}\b"),
}

MAX_FILE_BYTES = 2 * 1024 * 1024


def candidate_files(root: Path) -> list[Path]:
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [root / Path(item.decode("utf-8")) for item in result.stdout.split(b"\0") if item]


def scan_file(path: Path) -> list[tuple[str, int]]:
    try:
        if not path.is_file() or path.stat().st_size > MAX_FILE_BYTES:
            return []
        data = path.read_bytes()
    except OSError as error:
        raise RuntimeError(f"cannot read {path}: {error}") from error
    findings: list[tuple[str, int]] = []
    for label, pattern in PATTERNS.items():
        for match in pattern.finditer(data):
            line = data.count(b"\n", 0, match.start()) + 1
            findings.append((label, line))
    return findings


def scan_history(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "log", "--format=%H%x00%B%x00", "--all"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    issues = []
    for label, pattern in PATTERNS.items():
        if pattern.search(result.stdout):
            issues.append(f"{label} shape appears in commit metadata")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--history", action="store_true", help="scan commit metadata too")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    issues: list[str] = []
    for path in candidate_files(root):
        for label, line in scan_file(path):
            issues.append(f"{path.relative_to(root)}:{line}: {label} shape")
    if args.history:
        issues.extend(scan_history(root))
    if issues:
        print("secret scan failed:", file=sys.stderr)
        for issue in issues:
            print(f"- {issue}", file=sys.stderr)
        return 1
    print("secret scan passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
