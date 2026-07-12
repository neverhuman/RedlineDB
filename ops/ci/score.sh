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

cargo run --locked --quiet -- validate-score \
  --report .jankurai/repo-score.json \
  --baseline agent/jankurai-baseline.json \
  --policy agent/audit-policy.toml \
  --receipt target/jankurai/score-validation.json

cp .jankurai/repo-score.json target/jankurai/repo-score.json
cp .jankurai/repo-score.md target/jankurai/repo-score.md
printf 'score ok: jain-split-ops\n'
