#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"
require_jankurai
expected_audit="$JANKURAI_BIN audit . --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md"
[[ "${JANKURAI_AUDIT_COMMAND:-$expected_audit}" == "$expected_audit" ]] || {
  printf 'Jankurai audit command contract differs from the governed lane\n' >&2
  exit 1
}
just --justfile "$repo_root/Justfile" score
[[ -z "$(git status --porcelain=v1 --untracked-files=all)" ]] || {
  printf 'governed score lane changed tracked source state\n' >&2
  exit 1
}
