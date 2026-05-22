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
# The script writes per-tool receipts under `target/jankurai/<tool>/` so
# the per-job upload-artifact step in `.github/workflows/jankurai-tools.yml`
# captures the same evidence CI and local runs produce.

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

tool="${1:?tool id required: audit-ci|proof-routing|security|contract-drift|authz-matrix|input-boundary|agent-tool-supply|release-readiness|cost-budget}"

LOG_DIR="target/jankurai"
mkdir -p "$LOG_DIR/${tool}" "$LOG_DIR/security" .jankurai

# Install the pinned jankurai release binary. Failure is a real lane failure.
ci_install_jankurai_logged "$LOG_DIR/${tool}/install.log"

# Prepare accepted baseline (used by `--mode ratchet`).
if [[ -f .jankurai/baselines/main.repo-score.json ]]; then
    cp .jankurai/baselines/main.repo-score.json "$LOG_DIR/accepted-baseline.json"
fi

# Execute the per-tool canonical ci_command. We hold the EXACT string
# verbatim because the tool-adoption auditor matches each tool's
# `ci_command` field against the workflow / script source.
audit_cmd="jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md --policy agent/audit-policy.toml"
sec_cmd="jankurai security run . --out target/jankurai/security/evidence.json"

audit_ratchet_acceptable() {
    jq -e '
      (.score // 0) >= (.decision.minimum_score // 85)
      and ((.decision.hard_findings // 0) == 0)
      and (((.caps_applied // []) | length) == 0)
      and ((.decision.ratchet.score_delta // -999999) >= 0)
      and (((.decision.ratchet.new_caps // []) | length) == 0)
      and (((.decision.ratchet.new_hard_findings // []) | length) == 0)
    ' "$LOG_DIR/repo-score.json" >/dev/null 2>&1
}

run_or_record() {
    local label="$1"
    shift
    local command_log="$LOG_DIR/${tool}/command.log"
    local status=0
    local accepted=false
    if [[ "$label" != "security" ]]; then
        bash scripts/check_audit_policy_mirror.sh
        # Keep the canonical ci_command exact while avoiding dirty-worktree
        # smart fast scans that lose repo-wide evidence during local mirrors.
        rm -f "$LOG_DIR/audit-state.json"
    fi
    "$@" > "$command_log" 2>&1 || status=$?
    if [[ "$status" -ne 0 ]]; then
        if [[ "$label" != "security" ]] && audit_ratchet_acceptable; then
            accepted=true
            printf 'jankurai ratchet accepted: no score drop, new caps, or new hard findings vs baseline\n' \
                | tee -a "$command_log"
        else
            cat "$command_log" >&2
            return "$status"
        fi
    fi
    {
        printf 'tool=%s\n' "$tool"
        printf 'label=%s\n' "$label"
        printf 'command=%s\n' "$*"
        printf 'status=%s\n' "$status"
        printf 'accepted=%s\n' "$accepted"
        printf 'installed=true\n'
        printf 'timestamp=%s\n' "$(date -u +%FT%TZ)"
    } > "$LOG_DIR/${tool}/receipt.json.txt"
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
