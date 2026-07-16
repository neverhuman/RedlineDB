#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

[[ -z "$(git status --porcelain)" ]] || fail "score requires a clean checkout at start"
has git || fail "git is required for exact-head score evidence"
has jq || fail "jq is required for exact-head score evidence"
JBIN="$(jankurai_bin)" || fail "governed Jankurai identity verification failed"

ensure_artifacts
report_json="${ARTIFACT_DIR}/repo-score.json"
report_md="${ARTIFACT_DIR}/repo-score.md"
"$JBIN" audit . --mode advisory --full --no-score-history \
  --policy .jankurai/audit-policy.toml \
  --json "$report_json" --md "$report_md"

expected_head="$(git rev-parse --short=7 HEAD)"
jq -e --arg head "$expected_head" '
  .auditor_version == "1.6.11"
  and .git.head == $head
  and .dirty_worktree == false
  and .git.dirty_worktree == false
  and ((.score // 0) >= 85)
  and ((.caps_applied // []) | length == 0)
  and ((.conformance_blockers // []) | length == 0)
  and ((.decision.hard_findings // .hard_findings // 0)
    | if type == "array" then length == 0 else . == 0 end)
  and (.input_fingerprint | startswith("sha256:"))
  and (.report_fingerprint | startswith("sha256:"))
' "$report_json" >/dev/null
[[ -z "$(git status --porcelain)" ]] || fail "score mutated tracked or unignored evidence"
log "score: clean exact-head governed evidence passed"
