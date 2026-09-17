#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

require_mirror() {
    local label="$1"
    local canonical="$2"
    local compatibility="$3"

    if [ ! -f "$canonical" ]; then
        printf 'missing canonical %s: %s\n' "$label" "$canonical" >&2
        exit 1
    fi
    if [ ! -f "$compatibility" ]; then
        printf 'missing compatibility %s: %s\n' "$label" "$compatibility" >&2
        exit 1
    fi
    if ! cmp -s "$canonical" "$compatibility"; then
        printf '%s mirror drifted: %s must match %s\n' \
            "$label" "$compatibility" "$canonical" >&2
        diff -u "$canonical" "$compatibility" >&2 || true
        exit 1
    fi
}

require_mirror \
    "audit policy" \
    "$repo_root/agent/audit-policy.toml" \
    "$repo_root/.jankurai/audit-policy.toml"
require_mirror \
    "generated-zones manifest" \
    "$repo_root/agent/generated-zones.toml" \
    "$repo_root/.jankurai/generated-zones.toml"
