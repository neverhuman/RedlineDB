#!/usr/bin/env bash
#
# Verify that .jankurai/audit-policy.toml and agent/audit-policy.toml are
# byte-identical. jankurai 1.5.1 reads from `./agent/audit-policy.toml`
# (see the `policy.path` field in the audit JSON output); the canonical
# source-of-truth copy lives at `.jankurai/audit-policy.toml`. If the two
# drift, an editor can update one and silently leave the other behind,
# making the audit policy invisibly diverge from what's documented.
#
# This script is wired into `ops/ci/pr-ci.sh` before the cargo steps so
# pre-flight catches drift. Exits 0 in sync, 1 otherwise.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

src=".jankurai/audit-policy.toml"
dst="agent/audit-policy.toml"

if [ ! -f "$src" ]; then
    printf 'audit policy mirror: source %s is missing\n' "$src" >&2
    exit 1
fi
if [ ! -f "$dst" ]; then
    printf 'audit policy mirror: mirror %s is missing — copy from %s\n' "$dst" "$src" >&2
    exit 1
fi
if ! cmp -s "$src" "$dst"; then
    printf 'audit policy mirror drifted: %s must match %s\n' "$dst" "$src" >&2
    printf 'fix: cp %s %s\n' "$src" "$dst" >&2
    exit 1
fi

printf 'audit policy mirror: %s == %s OK\n' "$src" "$dst"
