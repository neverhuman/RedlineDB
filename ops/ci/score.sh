#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"

jankurai="${JANKURAI_BIN:-jankurai}"
expected_version="jankurai 1.6.11"
if ! has "$jankurai"; then
    fail "missing governed Jankurai executable: $jankurai"
fi
actual_version="$("$jankurai" --version)"
if [ "$actual_version" != "$expected_version" ]; then
    fail "Jankurai version mismatch: expected '$expected_version', got '$actual_version'"
fi

ci_run scripts/check_audit_policy_mirror.sh
mkdir -p target/jankurai
rm -f target/jankurai/repo-score.json \
    target/jankurai/repo-score.md \
    target/jankurai/repair-queue.jsonl

ci_run "$jankurai" audit . \
    --full \
    --mode ratchet \
    --baseline .jankurai/baselines/accepted-baseline.json \
    --policy agent/audit-policy.toml \
    --no-score-history \
    --json target/jankurai/repo-score.json \
    --md target/jankurai/repo-score.md \
    --repair-queue-jsonl target/jankurai/repair-queue.jsonl

ci_run jq -e '
    .score >= 85
    and .raw_score >= 85
    and .decision.status == "pass"
    and .decision.passed == true
    and .decision.minimum_score == 85
    and .decision.hard_findings == 0
    and .decision.ratchet.allowed_drop == 0
    and .decision.ratchet.passed == true
    and .decision.ratchet.score_delta >= 0
    and (.caps_applied | type == "array" and length == 0)
    and (.decision.ratchet.new_caps | type == "array" and length == 0)
    and (.decision.ratchet.new_hard_findings | type == "array" and length == 0)
    and ((.blockers // []) | type == "array" and length == 0)
' target/jankurai/repo-score.json >/dev/null

score="$(jq -er '.score' target/jankurai/repo-score.json)"
raw_score="$(jq -er '.raw_score' target/jankurai/repo-score.json)"
log "score: pass score=${score} raw=${raw_score} floor=85 caps=0 hard=0 blockers=0"
