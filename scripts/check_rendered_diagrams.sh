#!/usr/bin/env bash
set -euo pipefail

python scripts/render_diagrams.py

if ! git diff --quiet -- documentation/diagrams/rendered; then
    echo "Diagram render outputs are stale." >&2
    echo "Run 'python scripts/render_diagrams.py' and commit updated files under documentation/diagrams/rendered/." >&2
    git --no-pager diff -- documentation/diagrams/rendered >&2
    exit 1
fi

echo "Diagram render outputs are up to date."
