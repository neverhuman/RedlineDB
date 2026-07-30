#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

fail() {
    printf '[redline-score][error] %s\n' "$*" >&2
    exit 1
}

command -v jankurai >/dev/null 2>&1 \
    || fail "the governed Jankurai executable is missing"
command -v jq >/dev/null 2>&1 \
    || fail "jq is required to validate score evidence"
bash scripts/check_audit_policy_mirror.sh

mkdir -p target/jankurai
install -m 0644 \
    .jankurai/baselines/main.repo-score.json \
    target/jankurai/accepted-baseline.json
rm -f target/jankurai/audit-state.json \
    target/jankurai/repo-score.json \
    target/jankurai/repo-score.md

audit_rc=0
jankurai audit . \
    --mode ratchet \
    --baseline target/jankurai/accepted-baseline.json \
    --json target/jankurai/repo-score.json \
    --md target/jankurai/repo-score.md \
    --policy agent/audit-policy.toml \
    --no-score-history \
    || audit_rc=$?

if [ "$audit_rc" -ne 0 ]; then
    bash tools/evidence-processor/run.sh \
        jankurai-ratchet target/jankurai/repo-score.json
    printf '[redline-score] reviewed policy transition accepted by locked Rust ratchet gate\n'
fi

policy_sha="sha256:$(sha256sum agent/audit-policy.toml | awk '{print $1}')"
jq -e --arg policy_sha "$policy_sha" '
    .score >= 85
    and .raw_score >= 85
    and .decision.minimum_score == 85
    and .decision.hard_findings == 0
    and .decision.ratchet.allowed_drop == 0
    and .decision.ratchet.score_delta >= 0
    and (.caps_applied | type == "array" and length == 0)
    and (.decision.ratchet.new_caps | type == "array" and length == 0)
    and (.decision.ratchet.new_hard_findings | type == "array" and length == 0)
    and ((.blockers // []) | type == "array" and length == 0)
    and .policy_fingerprint == $policy_sha
    and (
        (
            .decision.status == "pass"
            and .decision.passed == true
            and .decision.ratchet.passed == true
        )
        or
        (
            .decision.status == "fail"
            and .decision.passed == false
            and .decision.ratchet.passed == false
            and .decision.ratchet.policy_changed == true
        )
    )
' target/jankurai/repo-score.json >/dev/null

score="$(jq -er '.score' target/jankurai/repo-score.json)"
raw_score="$(jq -er '.raw_score' target/jankurai/repo-score.json)"
printf '[redline-score] pass score=%s raw=%s floor=85 caps=0 hard=0 blockers=0\n' \
    "$score" "$raw_score"
