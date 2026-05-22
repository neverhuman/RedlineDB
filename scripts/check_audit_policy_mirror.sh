#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project_policy="$repo_root/.jankurai/audit-policy.toml"
compat_policy="$repo_root/agent/audit-policy.toml"

if [ ! -f "$project_policy" ]; then
    printf 'missing project audit policy: %s\n' "$project_policy" >&2
    exit 1
fi

if [ ! -f "$compat_policy" ]; then
    printf 'missing jankurai v1.5.1 compatibility audit policy: %s\n' "$compat_policy" >&2
    exit 1
fi

if ! cmp -s "$project_policy" "$compat_policy"; then
    printf 'audit policy mirror drifted: agent/audit-policy.toml must match .jankurai/audit-policy.toml\n' >&2
    diff -u "$project_policy" "$compat_policy" >&2 || true
    exit 1
fi
