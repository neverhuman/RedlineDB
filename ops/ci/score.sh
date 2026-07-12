#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
require_jankurai
expected_audit="jankurai audit . --json .jankurai/repo-score.json --md .jankurai/repo-score.md"
[[ "${JANKURAI_AUDIT_COMMAND:-$expected_audit}" == "$expected_audit" ]] || {
  printf 'Jankurai audit command contract differs from the governed lane\n' >&2
  exit 1
}
mkdir -p .jankurai target/jankurai/coverage
jankurai coverage audit . \
  --config agent/coverage-sources.toml \
  --json target/jankurai/coverage/coverage-audit.json \
  --md target/jankurai/coverage/coverage-audit.md
jankurai audit . --full --mode advisory \
  --policy agent/audit-policy.toml \
  --json .jankurai/repo-score.json \
  --md .jankurai/repo-score.md \
  --repair-queue-jsonl target/jankurai/repair-queue.jsonl
./redlinectl audit-verify .jankurai/repo-score.json
