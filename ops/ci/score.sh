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

transition=".jankurai/baselines/main.policy-transition.json"
baseline=".jankurai/baselines/main.repo-score.json"
for evidence in "$transition" "$baseline"; do
    [ -f "$evidence" ] && [ ! -L "$evidence" ] \
        || fail "$evidence must be a physical file"
done

policy_sha="sha256:$(sha256sum agent/audit-policy.toml | awk '{print $1}')"
baseline_sha="$(sha256sum "$baseline" | awk '{print $1}')"
jq -e \
    --arg policy_sha "$policy_sha" \
    --arg baseline_sha "$baseline_sha" \
    '
        .schema_version == "redline.jankurai-policy-transition/v1"
        and .protected_main_commit
            == "cdb1c5a4d4629ff555cfca9d8994c818e070c571"
        and .protected_main_tree
            == "0299b7163c427ea0a2de8853d65e75723e425a3e"
        and .previous_policy_fingerprint
            == "sha256:affaf294fc444a2dc116f4046213e166d7f8474be08d78d6278feb240d9a2c00"
        and .current_policy_fingerprint == $policy_sha
        and .refreshed_baseline_sha256 == $baseline_sha
    ' "$transition" >/dev/null \
    || fail "reviewed main policy transition evidence is invalid"
jq -e --arg policy_sha "$policy_sha" '
    .git.head == "cdb1c5a"
    and .git.dirty_worktree == false
    and .scope.mode == "full"
    and .policy_fingerprint == $policy_sha
    and .score >= 85
    and .raw_score >= 85
    and .decision.passed == true
    and .decision.hard_findings == 0
    and (.caps_applied | type == "array" and length == 0)
' "$baseline" >/dev/null \
    || fail "reviewed protected-main baseline is invalid"

mkdir -p target/jankurai
install -m 0644 \
    "$baseline" \
    target/jankurai/accepted-baseline.json
rm -f target/jankurai/audit-state.json \
    target/jankurai/repo-score.json \
    target/jankurai/repo-score.md

jankurai audit . \
    --mode ratchet \
    --baseline target/jankurai/accepted-baseline.json \
    --json target/jankurai/repo-score.json \
    --md target/jankurai/repo-score.md \
    --policy agent/audit-policy.toml \
    --no-score-history

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
    and .decision.status == "pass"
    and .decision.passed == true
    and .decision.ratchet.passed == true
    and .decision.ratchet.policy_changed == false
    and .decision.ratchet.baseline_policy_fingerprint == $policy_sha
' target/jankurai/repo-score.json >/dev/null

score="$(jq -er '.score' target/jankurai/repo-score.json)"
raw_score="$(jq -er '.raw_score' target/jankurai/repo-score.json)"
printf '[redline-score] pass score=%s raw=%s floor=85 caps=0 hard=0 blockers=0\n' \
    "$score" "$raw_score"
