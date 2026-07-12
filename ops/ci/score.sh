#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

mkdir -p .jankurai target/jankurai
rm -f .jankurai/repo-score.json .jankurai/repo-score.md

require_tool jankurai
jankurai audit . \
  --full \
  --mode advisory \
  --policy agent/audit-policy.toml \
  --json .jankurai/repo-score.json \
  --md .jankurai/repo-score.md \
  --repair-queue-jsonl target/jankurai/repair-queue.jsonl \
  --no-score-history

score="$(jq -r '.score // 0' .jankurai/repo-score.json)"
base_score="$(jq -r '.score // 0' agent/jankurai-baseline.json)"
hard_count="$(jq -r '(.decision.hard_findings // .hard_findings // 0) | if type == "array" then length else . end' .jankurai/repo-score.json)"
allowed_drop="$(awk -F= '/allowed_score_drop[[:space:]]*=/{gsub(/[ "\t]/,"",$2); print $2; exit}' agent/audit-policy.toml)"
allowed_drop="${allowed_drop:-0}"
floor="$(awk -F= '/minimum_score[[:space:]]*=/{gsub(/[ "\t]/,"",$2); print $2; exit}' agent/audit-policy.toml)"
floor="${floor:-85}"
floor_enforced="$(awk -F= '/floor_enforced[[:space:]]*=/{gsub(/[ "\t]/,"",$2); print $2; exit}' agent/audit-policy.toml)"
floor_enforced="${floor_enforced:-true}"
errors=()
(( hard_count == 0 )) || errors+=("hard findings present: $hard_count")
(( score >= base_score - allowed_drop )) || errors+=("score regression: $score < baseline $base_score (allowed_drop=$allowed_drop)")
if [[ "$floor_enforced" == "true" ]]; then
  (( score >= floor )) || errors+=("score $score below enforced absolute floor $floor")
fi
if (( ${#errors[@]} )); then
  printf 'score check failed: %s\n' "$(IFS='; '; echo "${errors[*]}")" >&2
  exit 1
fi
printf 'score ok: %s (baseline posture; hard=%s)\n' "$score" "$hard_count"

cp .jankurai/repo-score.json target/jankurai/repo-score.json
cp .jankurai/repo-score.md target/jankurai/repo-score.md
printf 'score ok: jain-split-ops\n'
