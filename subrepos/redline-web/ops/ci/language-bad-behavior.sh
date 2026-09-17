#!/usr/bin/env bash
# Bind the Jankurai audit's CI/git/release detector decisions to the clean head.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

has git || fail "git is required for exact-head detector evidence"
has jq || fail "jq is required for exact-head detector evidence"
[[ -z "$(git status --porcelain)" ]] || fail "detector evidence requires a clean checkout"
score_report="${ARTIFACT_DIR}/repo-score.json"
log_file="${ARTIFACT_DIR}/language-bad-behavior.log"
[[ -s "$score_report" ]] || fail "governed score report is required for detector evidence"
expected_head="$(git rev-parse --short=7 HEAD)"
jq -e --arg head "$expected_head" '
  .auditor_version == "1.6.11"
  and .git.head == $head
  and .dirty_worktree == false
  and .git.dirty_worktree == false
  and .decision.hard_findings == 0
  and ((.caps_applied // []) | index("ci-bad-behavior") | not)
  and ((.caps_applied // []) | index("git-bad-behavior") | not)
  and ((.caps_applied // []) | index("release-bad-behavior") | not)
' "$score_report" >/dev/null
jq -n \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  --arg score_report_sha256 "$(jain_sha256 "$score_report")" \
  '{schema_version:"redline.web.language-behavior/v1",status:"pass",
    commit:$commit,tree:$tree,score_report_sha256:$score_report_sha256,
    detectors:["ci-bad-behavior","git-bad-behavior","release-bad-behavior"],
    hard_findings:0,caps_applied:[]}' >"$log_file"

log "language-bad-behavior: complete"
