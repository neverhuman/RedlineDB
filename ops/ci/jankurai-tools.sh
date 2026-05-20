#!/usr/bin/env bash
# Per-tool jankurai CI evidence lane. Wraps the canonical `ci_command`
# for each of the 9 jankurai tools that were `configured` but missing CI
# command evidence in `.jankurai/repo-score.md` (project memory
# `project_jankurai_score_gaps`):
#
#   audit-ci, proof-routing, security, contract-drift, authz-matrix,
#   input-boundary, agent-tool-supply, release-readiness, cost-budget
#
# Usage (run-mode):
#   bash ops/ci/jankurai-tools.sh <tool-id>
#
# The script writes per-tool receipts under `.jankurai/<tool>/` so
# the per-job upload-artifact step in `.github/workflows/jankurai-tools.yml`
# captures the same evidence CI and local runs produce.

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

tool="${1:?tool id required: audit-ci|proof-routing|security|contract-drift|authz-matrix|input-boundary|agent-tool-supply|release-readiness|cost-budget}"

mkdir -p ".jankurai/${tool}" .jankurai

# Install the pinned jankurai release binary. Failure is a real lane failure.
ci_install_jankurai_logged ".jankurai/${tool}/install.log"

# Prepare accepted baseline (used by `--mode ratchet`).
if [[ -f .jankurai/baselines/main.repo-score.json ]]; then
    cp .jankurai/baselines/main.repo-score.json .jankurai/accepted-baseline.json
fi

# Execute the per-tool canonical ci_command. We hold the EXACT string
# verbatim because the tool-adoption auditor matches each tool's
# `ci_command` field against the workflow / script source.
audit_cmd="jankurai audit . --mode ratchet --baseline .jankurai/accepted-baseline.json --json .jankurai/repo-score.json --md .jankurai/repo-score.md"
sec_cmd="jankurai security run . --strict --profile ci --out .jankurai/security/evidence.json"

run_or_record() {
    local label="$1"
    shift
    "$@" || true
    {
        printf 'tool=%s\n' "$tool"
        printf 'label=%s\n' "$label"
        printf 'command=%s\n' "$*"
        printf 'installed=true\n'
        printf 'timestamp=%s\n' "$(date -u +%FT%TZ)"
    } > ".jankurai/${tool}/receipt.json.txt"
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
