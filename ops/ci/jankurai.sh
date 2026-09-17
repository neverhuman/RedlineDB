#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

source ops/ci/lib.sh
require_governed_jankurai
command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for the strict audit gate\n' >&2
  exit 1
}

mkdir -p agent target/jankurai
jankurai audit . --mode advisory --full --no-score-history \
  --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md

case "${JANKURAI_UPDATE_REVIEWED:-0}" in
  0) ;;
  1)
    cp target/jankurai/repo-score.json agent/repo-score.json
    cp target/jankurai/repo-score.md agent/repo-score.md
    ;;
  *)
    printf 'JANKURAI_UPDATE_REVIEWED must be 0 or 1\n' >&2
    exit 1
    ;;
esac

jq -e --arg version "$JANKURAI_VERSION" '
  .auditor_version == $version
  and ((.conformance_blockers // []) | length == 0)
  and ((.caps_applied // []) | length == 0)
  and ((.hard_findings // .decision.hard_findings // 0)
    | if type == "array" then length == 0 else . == 0 end)
  and ((.score // 0) >= (.minimum_score // .decision.minimum_score // 85))
' target/jankurai/repo-score.json >/dev/null

jq -r '"jankurai strict gate: score=\(.score // 0) minimum=\(.minimum_score // .decision.minimum_score // 85) blockers=\((.conformance_blockers // []) | length) caps=\((.caps_applied // []) | length)"' \
  target/jankurai/repo-score.json
