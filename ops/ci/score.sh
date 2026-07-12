#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
require_jankurai
expected_audit="jankurai audit . --json .jankurai/repo-score.json --md .jankurai/repo-score.md"
[[ "${JANKURAI_AUDIT_COMMAND:-$expected_audit}" == "$expected_audit" ]] || {
  printf 'Jankurai audit command contract differs from the governed lane\n' >&2
  exit 1
}
exec just --justfile "$repo_root/Justfile" score
