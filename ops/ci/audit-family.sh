#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
bash ops/ci/install-github-tools.sh
# shellcheck source=ops/ci/lib.sh
source ops/ci/lib.sh
mkdir -p target/audit-family
components=(. subrepos/redline-testing subrepos/redline-web subrepos/redline-central subrepos/redline-split-ops subrepos/redline)
if [[ ${1:-all} == components ]]; then components=("${components[@]:1}"); fi
for component in "${components[@]}"; do
  name=${component##*/}; [[ $name != . ]] || name=engine
  mkdir -p "$root/target/audit-family/$name"
  report="$root/target/audit-family/$name/repo-score.json"
  baseline=()
  if [[ -f $component/.jankurai/baselines/github-monorepo.repo-score.json ]]; then
    baseline=(--mode ratchet --baseline "$root/$component/.jankurai/baselines/github-monorepo.repo-score.json")
  elif [[ -f $component/.jankurai/baselines/main.repo-score.json ]]; then
    baseline=(--mode ratchet --baseline "$root/$component/.jankurai/baselines/main.repo-score.json")
  elif [[ -f $component/agent/repo-score.json ]]; then
    baseline=(--mode ratchet --baseline "$root/$component/agent/repo-score.json")
  else
    baseline=(--mode standard)
  fi
  (cd "$root/$component"; jankurai audit . --full "${baseline[@]}" --policy "$root/$component/agent/audit-policy.toml" --json "$report" --md "$root/target/audit-family/$name/repo-score.md" --no-score-history)
  jq -e '.score >= 85 and .decision.hard_findings == 0 and (.caps_applied | length) == 0' "$report" >/dev/null
done
