#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

mkdir -p .jankurai target/jankurai
next_json="target/jankurai/repo-score.next.json"
next_md="target/jankurai/repo-score.next.md"
rm -f "$next_json" "$next_md"

require_tool jankurai
jankurai audit . \
  --full \
  --mode advisory \
  --policy agent/audit-policy.toml \
  --json "$next_json" \
  --md "$next_md" \
  --repair-queue-jsonl target/jankurai/repair-queue.jsonl \
  --no-score-history

score="$(jq -r '.score // 0' "$next_json")"
base_score="$(jq -r '.score // 0' agent/jankurai-baseline.json)"
hard_count="$(jq -r '(.decision.hard_findings // .hard_findings // 0) | if type == "array" then length else . end' "$next_json")"
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

cp "$next_json" .jankurai/repo-score.json
cp "$next_md" .jankurai/repo-score.md
cp "$next_json" target/jankurai/repo-score.json
cp "$next_md" target/jankurai/repo-score.md
printf 'score ok: jain-split-ops\n'
