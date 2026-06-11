#!/usr/bin/env bash
# Checks that every tool required by ops/ci scripts is installed.
set -Eeuo pipefail

REQUIRED_TOOLS=(shellcheck gitleaks python3 gh jq)
MISSING=0

for tool in "${REQUIRED_TOOLS[@]}"; do
    if command -v "$tool" >/dev/null 2>&1; then
        echo "ok: $tool"
    else
        echo "MISSING: $tool"
        MISSING=1
    fi
done

if [[ "$MISSING" -eq 1 ]]; then
    echo "ERROR: one or more required tools are missing" >&2
    exit 1
fi

echo "All tools present."
