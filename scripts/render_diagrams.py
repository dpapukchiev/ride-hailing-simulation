#!/usr/bin/env python3
"""Render Mermaid diagram sources into committed SVG artifacts."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = REPO_ROOT / "documentation" / "diagrams" / "src"
DEFAULT_RENDERED = REPO_ROOT / "documentation" / "diagrams" / "rendered"
DEFAULT_CONFIG = REPO_ROOT / "scripts" / "mermaid-config.json"
DEFAULT_CI_PUPPETEER_CONFIG = REPO_ROOT / "scripts" / "puppeteer-config-ci.json"


def run(command: list[str]) -> None:
    completed = subprocess.run(command, capture_output=True, text=True)
    if completed.returncode != 0:
        if completed.stdout:
            print(completed.stdout, end="", file=sys.stderr)
        if completed.stderr:
            print(completed.stderr, end="", file=sys.stderr)
        raise RuntimeError(f"Command failed: {' '.join(command)}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render Mermaid diagrams to SVG.")
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--rendered", type=Path, default=DEFAULT_RENDERED)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument(
        "--puppeteer-config",
        type=Path,
        default=None,
        help=(
            "Optional Puppeteer launch config file. "
            "If omitted, CI runs auto-use scripts/puppeteer-config-ci.json when present."
        ),
    )
    parser.add_argument(
        "--mermaid-version",
        default="11.4.2",
        help="Mermaid CLI version used via npx (default: 11.4.2)",
    )
    return parser.parse_args()


def is_ci_environment() -> bool:
    return os.environ.get("CI", "").strip().lower() in {"1", "true", "yes"}


def resolve_puppeteer_config(args: argparse.Namespace) -> Path | None:
    if args.puppeteer_config is not None:
        return args.puppeteer_config.resolve()

    if is_ci_environment() and DEFAULT_CI_PUPPETEER_CONFIG.exists():
        return DEFAULT_CI_PUPPETEER_CONFIG.resolve()

    return None


def main() -> int:
    args = parse_args()
    source = args.source.resolve()
    rendered = args.rendered.resolve()
    config = args.config.resolve()
    puppeteer_config = resolve_puppeteer_config(args)

    npx_executable = "npx.cmd" if os.name == "nt" else "npx"
    if shutil.which(npx_executable) is None:
        print("error: npx is required to render diagrams", file=sys.stderr)
        print("Install Node.js 20+ and retry.", file=sys.stderr)
        return 1

    if not source.exists():
        print(f"error: source directory does not exist: {source}", file=sys.stderr)
        return 1

    if not config.exists():
        print(f"error: Mermaid config does not exist: {config}", file=sys.stderr)
        return 1

    if puppeteer_config is not None and not puppeteer_config.exists():
        print(
            f"error: Puppeteer config does not exist: {puppeteer_config}",
            file=sys.stderr,
        )
        return 1

    diagram_files = sorted(source.rglob("*.mmd"))
    if not diagram_files:
        print(f"error: no .mmd files found under: {source}", file=sys.stderr)
        return 1

    if rendered.exists():
        for existing in rendered.rglob("*.svg"):
            existing.unlink()
    rendered.mkdir(parents=True, exist_ok=True)

    for diagram in diagram_files:
        rel_path = diagram.relative_to(source)
        output_file = rendered / rel_path.with_suffix(".svg")
        output_file.parent.mkdir(parents=True, exist_ok=True)

        print(
            f"render {rel_path.as_posix()} -> {output_file.relative_to(rendered).as_posix()}"
        )
        command = [
            npx_executable,
            "--yes",
            f"@mermaid-js/mermaid-cli@{args.mermaid_version}",
            "-i",
            str(diagram),
            "-o",
            str(output_file),
            "-c",
            str(config),
        ]
        if puppeteer_config is not None:
            command.extend(["--puppeteerConfigFile", str(puppeteer_config)])

        run(command)

    print(f"Rendered {len(diagram_files)} diagrams.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
