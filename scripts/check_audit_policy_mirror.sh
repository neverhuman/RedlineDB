#!/usr/bin/env bash
#
# Verify that the documented `.jankurai` policy and generated-zone authorities
# are byte-identical to the `agent` paths read by current Jankurai. If either
# pair drifts, an editor can update one and silently leave the other behind,
# making the active proof authority diverge from what's documented.
#
# This script is wired into `ops/ci/pr-ci.sh` before the cargo steps so
# pre-flight catches drift. Exits 0 in sync, 1 otherwise.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

check_mirror() {
    local label="$1" src="$2" dst="$3"
    if [ ! -f "$src" ]; then
        printf '%s mirror: source %s is missing\n' "$label" "$src" >&2
        exit 1
    fi
    if [ ! -f "$dst" ]; then
        printf '%s mirror: mirror %s is missing — copy from %s\n' \
            "$label" "$dst" "$src" >&2
        exit 1
    fi
    if ! cmp -s "$src" "$dst"; then
        printf '%s mirror drifted: %s must match %s\n' "$label" "$dst" "$src" >&2
        printf 'fix: copy %s to %s\n' "$src" "$dst" >&2
        exit 1
    fi
    printf '%s mirror: %s == %s OK\n' "$label" "$src" "$dst"
}

check_mirror "audit policy" \
    ".jankurai/audit-policy.toml" "agent/audit-policy.toml"
check_mirror "generated zones" \
    ".jankurai/generated-zones.toml" "agent/generated-zones.toml"
