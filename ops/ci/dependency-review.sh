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

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=ops/ci/pinned-rustsec.sh
. "$repo_root/ops/ci/pinned-rustsec.sh"
# shellcheck source=ops/ci/security-tools.sh
. "$repo_root/ops/ci/security-tools.sh"

LOG_PATH=".jankurai/dependency-review.log"
mkdir -p "$(dirname "$LOG_PATH")"

export CARGO_NET_OFFLINE=true
rustsec_stage="$(mktemp -d "${TMPDIR:-/tmp}/redline-dependency-rustsec.XXXXXX")"
trap 'rm -rf -- "$rustsec_stage"' EXIT
redline_prepare_local_rustsec "$rustsec_stage"
redline_resolve_pinned_rustsec
redline_resolve_security_tools
redline_resolve_cargo_deny_rustsec
cargo metadata --format-version 1 --locked --offline \
    > .jankurai/dependency-review-metadata.json

CARGO_HOME="$REDLINE_CARGO_DENY_HOME" \
    "$REDLINE_CARGO_DENY_BIN" check --disable-fetch \
    --metadata-path .jankurai/dependency-review-metadata.json \
    advisories bans licenses sources 2>&1 \
    | tee "$LOG_PATH"
