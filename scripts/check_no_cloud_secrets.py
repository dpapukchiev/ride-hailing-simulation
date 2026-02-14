#!/usr/bin/env python3
"""Fail if common cloud secret patterns are committed.

This is a lightweight repo check for public-demo safety.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PATTERNS = {
    "AWS access key": re.compile(r"AKIA[0-9A-Z]{16}"),
    "AWS secret key assignment": re.compile(
        r"(?i)aws(.{0,20})?(secret|access).{0,20}(=|:).{0,5}[A-Za-z0-9/+=]{40}"
    ),
    "Private key block": re.compile(r"-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "Hardcoded AWS session token": re.compile(
        r"(?i)aws_session_token.{0,10}(=|:).{0,5}[A-Za-z0-9/+=]{20,}"
    ),
}

IGNORE_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".pdf",
    ".parquet",
    ".osrm",
    ".pbf",
    ".mldgr",
    ".cell_metrics",
    ".datasource_names",
    ".ebg",
    ".cells",
    ".partition",
    ".maneuver_overrides",
    ".enw",
    ".ebg_nodes",
    ".fileIndex",
    ".cnbg_to_ebg",
    ".ramIndex",
    ".geometry",
    ".edges",
    ".tls",
    ".tld",
    ".icd",
    ".turn_penalties_index",
    ".turn_duration_penalties",
    ".turn_weight_penalties",
    ".restrictions",
    ".cnbg",
    ".nbg_nodes",
    ".properties",
    ".names",
    ".timestamp",
}


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return [ROOT / line.strip() for line in result.stdout.splitlines() if line.strip()]


def should_skip(path: Path) -> bool:
    if path.suffix in IGNORE_SUFFIXES:
        return True
    if path.name in {".env", ".env.local"}:
        return True
    return False


def main() -> int:
    violations: list[tuple[str, str, int]] = []

    for path in tracked_files():
        if should_skip(path):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for line_no, line in enumerate(text.splitlines(), start=1):
            for name, pattern in PATTERNS.items():
                if pattern.search(line):
                    violations.append((str(path.relative_to(ROOT)), name, line_no))

    if violations:
        print("Potential secrets detected:")
        for file_path, pattern_name, line_no in violations:
            print(f"- {file_path}:{line_no} ({pattern_name})")
        return 1

    print("No obvious cloud secrets detected in tracked files.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
