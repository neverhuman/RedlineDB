#!/usr/bin/env bash
# Thin dispatcher for the Rust Jankurai score policy.

set -euo pipefail

if [ "$#" -ne 3 ]; then
    printf 'usage: %s <before.json> <after.json> <commit|push>\n' "$0" >&2
    exit 64
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec cargo run --quiet --locked --manifest-path "$repo_root/Cargo.toml" \
    -p redlinedb-bench --bin score_policy -- compare "$@"
