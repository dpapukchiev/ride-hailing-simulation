#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_DIR="$REPO_ROOT/documentation/diagrams/src"
RENDERED_DIR="$REPO_ROOT/documentation/diagrams/rendered"

if [[ ! -d "$SOURCE_DIR" ]]; then
    echo "Diagram source directory is missing: $SOURCE_DIR" >&2
    exit 1
fi

if [[ ! -d "$RENDERED_DIR" ]]; then
    echo "Diagram rendered directory is missing: $RENDERED_DIR" >&2
    echo "Run 'python scripts/render_diagrams.py' to generate expected files." >&2
    exit 1
fi

declare -A expected=()
while IFS= read -r -d '' source_file; do
    rel="${source_file#"$SOURCE_DIR"/}"
    expected["${rel%.mmd}.svg"]=1
done < <(find "$SOURCE_DIR" -type f -name '*.mmd' -print0)

if [[ ${#expected[@]} -eq 0 ]]; then
    echo "No Mermaid source files found under: $SOURCE_DIR" >&2
    exit 1
fi

declare -A actual=()
while IFS= read -r -d '' rendered_file; do
    rel="${rendered_file#"$RENDERED_DIR"/}"
    actual["$rel"]=1
done < <(find "$RENDERED_DIR" -type f -name '*.svg' -print0)

missing=0
for rel in "${!expected[@]}"; do
    if [[ -z "${actual[$rel]+x}" ]]; then
        echo "Missing rendered diagram: documentation/diagrams/rendered/$rel" >&2
        missing=1
    fi
done

unexpected=0
for rel in "${!actual[@]}"; do
    if [[ -z "${expected[$rel]+x}" ]]; then
        echo "Unexpected rendered diagram: documentation/diagrams/rendered/$rel" >&2
        unexpected=1
    fi
done

if ((missing || unexpected)); then
    echo "Diagram artifacts do not match expected names/paths from documentation/diagrams/src." >&2
    echo "Run 'python scripts/render_diagrams.py' to regenerate outputs." >&2
    exit 1
fi

echo "Diagram artifacts exist with expected names and paths."
