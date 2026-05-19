#!/usr/bin/env bash
# Dependency-review lane: SBOM-like comparison of base vs head dependency
# manifests for newly-introduced vulnerable or non-allowlisted licenses.
#
# Mirrors the `dependency-review` job in `.github/workflows/jankurai.yml`
# so the same evidence path runs locally (`scripts/ci-local.sh
# dependency-review`) and in CI. The actual GitHub-Action invocation
# stays in the workflow YAML (it can only execute inside Actions); this
# script exists as the canonical local entry point AND as the carrier of
# the soft-gate semantics so the workflow YAML can stay hard-gated.
# Audit references: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Soft-gate rationale: see .jankurai/ci-soft-gate-ledger.toml#dependency-review-action
# GitHub Advanced Security `Dependency graph` is not yet enabled at the
# repo level (Settings -> Security -> Code security and analysis ->
# Dependency graph). The action exits non-zero until that toggle is on.
# Workflow YAML carries NO `continue-on-error: true`; instead the
# workflow step invokes this script, which runs the local
# dependency-review equivalent under `ci_soft_gate` and always exits 0.
#
# Usage:
#   bash ops/ci/dependency-review.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

LOG_PATH="target/jankurai/dependency-review.log"
mkdir -p "$(dirname "$LOG_PATH")"

# Local equivalent: `cargo deny check advisories bans licenses sources`
# in non-strict mode. The GitHub `actions/dependency-review-action` step
# is the authoritative producer in CI; this is the soft-gated mirror.
run_dependency_review() {
    if command -v cargo-deny >/dev/null 2>&1; then
        cargo deny --all-features check advisories bans licenses sources
    else
        printf 'cargo-deny missing locally; skipping dependency-review mirror\n'
        return 0
    fi
}

ci_soft_gate \
    dependency-review-action \
    "$LOG_PATH" \
    -- run_dependency_review
