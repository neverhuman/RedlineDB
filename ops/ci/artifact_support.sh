#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"

log 'artifact-support lane: record score and policy artifacts'
mkdir -p target/artifact-support
present=()
for artifact in repos.manifest.toml agent/audit-policy.toml .jankurai/repo-score.json .jankurai/repo-score.md; do
  [[ -f "$artifact" ]] && present+=("$artifact")
done
{
  printf '{"schema":"jain-split-ops.artifact-support/v1","present":['
  for i in "${!present[@]}"; do
    (( i > 0 )) && printf ','
    printf '"%s"' "${present[$i]}"
  done
  printf ']}\n'
} > target/artifact-support/receipt.json
printf 'artifact-support ok: jain-split-ops\n'
