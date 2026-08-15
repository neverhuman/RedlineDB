#!/usr/bin/env bash
# Score lane: ratcheted jankurai audit gate for the release proof surface.
#
# Enforcement is layered so the lane cannot pass by accident:
#   1. jankurai runs in ratchet mode, which fails the run when the score is
#      below the policy floor, when hard findings are present, or when the
#      report regresses against the reviewed baseline; and
#   2. this script re-asserts each of those properties from the emitted
#      report, so the lane still reds if a future auditor softens an exit
#      code. A non-zero audit exit is never swallowed.
#
# No Python: `ops/ci/no-python-policy.sh` runs in the required lane and
# rejects any `python*` invocation in an execution surface (see
# tools/no-python-policy.rs), so the report is read with jq.
#
# Usage:
#   bash ops/ci/score.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

fail() {
    printf 'score lane: %s\n' "$*" >&2
    exit 1
}

command -v jq >/dev/null 2>&1 \
    || fail "jq is required to evaluate the audit report"

expected_jankurai="jankurai 1.6.11"
actual_jankurai="$(jankurai --version 2>/dev/null || true)"
[ "$actual_jankurai" = "$expected_jankurai" ] \
    || fail "requires ${expected_jankurai}, found ${actual_jankurai:-no jankurai on PATH}"

required_metadata=(
    agent/audit-policy.toml
    agent/boundaries.toml
    agent/tool-adoption.toml
    agent/JANKURAI_STANDARD.md
    .jankurai/owner-map.json
    .jankurai/test-map.json
    .jankurai/generated-zones.toml
    .jankurai/proof-lanes.toml
)
for path in "${required_metadata[@]}"; do
    [ -s "$path" ] || fail "missing split metadata: $path"
done

bash scripts/check_audit_policy_mirror.sh

policy="agent/audit-policy.toml"
# Fail closed when the floor is absent. A removed `minimum_score` must not
# be silently indistinguishable from an explicitly declared floor.
floor="$(sed -n \
    's/^[[:space:]]*minimum_score[[:space:]]*=[[:space:]]*\([0-9][0-9]*\).*$/\1/p' \
    "$policy" | head -n 1)"
[ -n "$floor" ] || fail "$policy declares no minimum_score"

# Reviewed comparison input, committed under the scan-excluded baselines
# directory. Never seeded from the candidate run itself
# (HLT-034 ci.ratchet.self-generated-baseline).
baseline=".jankurai/baselines/accepted-baseline.json"
[ -s "$baseline" ] || fail "missing reviewed ratchet baseline: $baseline"

report="target/jankurai/repo-score.json"
mkdir -p target/jankurai
# CI starts from an empty target directory; local checkouts may retain
# smart-scan state from an earlier audit. Removing it forces a full scan
# while keeping the command canonical (see ops/ci/jankurai-audit.sh).
rm -f target/jankurai/audit-state.json

audit_rc=0
jankurai audit . \
    --full \
    --mode ratchet \
    --baseline "$baseline" \
    --policy "$policy" \
    --json "$report" \
    --md target/jankurai/repo-score.md \
    --no-score-history || audit_rc=$?

[ -s "$report" ] || fail "audit produced no report at $report (exit $audit_rc)"

report_field() {
    local filter="$1" value
    value="$(jq -r "$filter" "$report")" \
        || fail "cannot read $filter from $report"
    { [ -n "$value" ] && [ "$value" != "null" ]; } \
        || fail "$report carries no $filter"
    printf '%s' "$value"
}

score="$(report_field '.score')"
caps="$(report_field '.caps_applied | length')"
hard="$(report_field \
    '.decision.hard_findings | if type == "array" then length else . end')"
ratchet_passed="$(report_field '.decision.ratchet.passed')"
baseline_score="$(report_field '.decision.ratchet.baseline_score')"

errors=()
[ "$score" -ge "$floor" ] \
    || errors+=("score $score is below the policy floor $floor")
[ "$caps" -eq 0 ] \
    || errors+=("caps present: $(jq -r '.caps_applied | join(", ")' "$report")")
[ "$hard" -eq 0 ] \
    || errors+=("hard findings present: $hard")
[ "$ratchet_passed" = "true" ] \
    || errors+=("ratchet against $baseline did not pass (baseline score $baseline_score)")

if [ "${#errors[@]}" -gt 0 ]; then
    printf 'score lane: %s\n' "${errors[@]}" >&2
    exit 1
fi

# Every asserted property holds, so a non-zero audit exit means the auditor
# rejected the run for a reason this script does not model. Never pass it.
[ "$audit_rc" -eq 0 ] \
    || fail "jankurai audit exited $audit_rc despite passing every asserted check"

printf 'score ok: %s (floor %s; ratchet baseline %s; no caps; no hard findings)\n' \
    "$score" "$floor" "$baseline_score"
