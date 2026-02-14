#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

declare -a CHECKS=(
    "AWS access key|AKIA[0-9A-Z]{16}"
    "AWS secret key assignment|[Aa][Ww][Ss].{0,20}(secret|access).{0,20}(=|:).{0,5}[A-Za-z0-9/+=]{40}"
    "Private key block|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----"
    "Hardcoded AWS session token|[Aa][Ww][Ss]_[Ss][Ee][Ss][Ss][Ii][Oo][Nn]_[Tt][Oo][Kk][Ee][Nn].{0,10}(=|:).{0,5}[A-Za-z0-9/+=]{20,}"
)

violations=0

for entry in "${CHECKS[@]}"; do
    name="${entry%%|*}"
    regex="${entry#*|}"
    matches="$(git grep -nI -E -e "$regex" -- . ":(exclude).env" ":(exclude).env.local" || true)"
    if [[ -n "$matches" ]]; then
        if [[ "$violations" -eq 0 ]]; then
            echo "Potential secrets detected:"
        fi
        while IFS= read -r line; do
            echo "- $line ($name)"
        done <<< "$matches"
        violations=1
    fi
done

if [[ "$violations" -ne 0 ]]; then
    exit 1
fi

echo "No obvious cloud secrets detected in tracked files."
