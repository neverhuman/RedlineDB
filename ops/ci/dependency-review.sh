#!/usr/bin/env bash
# Dependency-review lane: reproducible full-graph dependency validation for
# advisories, bans, licenses, and sources.
#
# Mirrors the `dependency-review` job in `.github/workflows/jankurai.yml`
# so the same evidence path runs locally (`scripts/ci-local.sh
# dependency-review`) and in CI. This script is the canonical entry point.
# Audit references: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Cargo-deny is required and its exit status is a hard gate.
#
# Usage:
#   bash ops/ci/dependency-review.sh

set -euo pipefail

LOG_PATH=".jankurai/dependency-review.log"
mkdir -p "$(dirname "$LOG_PATH")"

command -v cargo-deny >/dev/null 2>&1 || {
    printf 'cargo-deny is required for dependency review\n' >&2
    exit 1
}

cargo deny --all-features check advisories bans licenses sources 2>&1 \
    | tee "$LOG_PATH"
