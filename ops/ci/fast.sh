#!/usr/bin/env bash
# Fast lane: the canonical local + CI pre-merge gate.
#
# Mirrors the `fast` recipe in `justfile` so the same four commands run
# in `just fast`, `scripts/ci-local.sh fast`, the GitHub Actions `ci`
# workflow, and the `ops/git-hooks/pre-push` hook. Audit references:
# HLT-042 ci-local-parity.lib-missing, HLT-034 ci-bad-behavior.
#
# Usage:
#   bash ops/ci/fast.sh
#
# Soft-gate rationale: see agent/ci-soft-gate-ledger.toml (none — every
# command in this lane is hard-gated by design).

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

cargo fmt --check
bash scripts/check_file_sizes.sh
cargo check --workspace --locked
cargo test --workspace --quiet --locked
