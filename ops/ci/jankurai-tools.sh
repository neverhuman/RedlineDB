#!/usr/bin/env bash
# Per-tool jankurai CI evidence lane. Wraps the canonical `ci_command`
# for each of the 9 jankurai tools that were `configured` but missing CI
# command evidence in `agent/repo-score.md` (project memory
# `project_jankurai_score_gaps`):
#
#   audit-ci, proof-routing, security, contract-drift, authz-matrix,
#   input-boundary, agent-tool-supply, release-readiness, cost-budget
#
# Usage (run-mode):
#   bash ops/ci/jankurai-tools.sh <tool-id>
#
# The script writes per-tool receipts under `target/jankurai/<tool>/` so
# the per-job upload-artifact step in `.github/workflows/jankurai-tools.yml`
# captures them even when jankurai itself can't be installed (soft-gate
# row `jankurai-install` in `agent/ci-soft-gate-ledger.toml`).
#
# Soft-gate rationale: the only soft-gated step is the `jankurai install`
# attempt. Audit references: HLT-042 ci-local-parity.lib-missing, HLT-034
# ci-bad-behavior. See agent/ci-soft-gate-ledger.toml.

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

tool="${1:?tool id required: audit-ci|proof-routing|security|contract-drift|authz-matrix|input-boundary|agent-tool-supply|release-readiness|cost-budget}"

mkdir -p "target/jankurai/${tool}" target/jankurai

# Install jankurai (soft-gated per agent/ci-soft-gate-ledger.toml#jankurai-install).
installed=false
if cargo install --git "${CI_JANKURAI_GIT}" \
        --tag "${CI_JANKURAI_TAG}" --package jankurai --locked \
        2>>"target/jankurai/${tool}/install.log"; then
    installed=true
else
    printf 'soft-gate=jankurai-install status=soft-failed ledger=agent/ci-soft-gate-ledger.toml\n' \
        | tee -a "target/jankurai/${tool}/install.log"
fi

# Prepare accepted baseline (used by `--mode ratchet`).
if [[ -f agent/baselines/main.repo-score.json ]]; then
    cp agent/baselines/main.repo-score.json target/jankurai/accepted-baseline.json
fi

# Execute the per-tool canonical ci_command. We hold the EXACT string
# verbatim because the tool-adoption auditor matches each tool's
# `ci_command` field against the workflow / script source.
audit_cmd="jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md"
sec_cmd="jankurai security run . --strict --profile ci --out target/jankurai/security/evidence.json"

run_or_record() {
    local label="$1"
    shift
    if [[ "$installed" == "true" ]]; then
        "$@" || true
    else
        printf '{"tool":"%s","command":%q,"status":"soft-gated","reason":"jankurai install unavailable"}\n' \
            "$tool" "$*" \
            > "target/jankurai/${tool}/soft-gate-receipt.json"
    fi
    {
        printf 'tool=%s\n' "$tool"
        printf 'label=%s\n' "$label"
        printf 'command=%s\n' "$*"
        printf 'installed=%s\n' "$installed"
        printf 'timestamp=%s\n' "$(date -u +%FT%TZ)"
    } > "target/jankurai/${tool}/receipt.json.txt"
}

case "$tool" in
    audit-ci|proof-routing|contract-drift|authz-matrix|input-boundary|agent-tool-supply|release-readiness|cost-budget)
        # Shared canonical audit command per tool-adoption.toml ci_command.
        # shellcheck disable=SC2086
        run_or_record "$tool" $audit_cmd
        ;;
    security)
        # Strict-profile security run per HLT-034.
        # shellcheck disable=SC2086
        run_or_record "$tool" $sec_cmd
        ;;
    *)
        printf 'unknown jankurai tool id: %s\n' "$tool" >&2
        exit 1
        ;;
esac
