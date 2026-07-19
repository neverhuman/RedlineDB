#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
JANKURAI_VERSION="1.6.11"
JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"

if [[ ! -f "$JANKURAI_BIN" || -L "$JANKURAI_BIN" || ! -x "$JANKURAI_BIN" ]]; then
  printf 'governed Jankurai must be an executable regular non-symlink: %s\n' "$JANKURAI_BIN" >&2
  exit 1
fi
[[ "$(realpath -e -- "$JANKURAI_BIN")" == "$JANKURAI_BIN" ]] || {
  printf 'governed Jankurai resolved outside its exact path\n' >&2
  exit 1
}
[[ "$($JANKURAI_BIN --version)" == "jankurai $JANKURAI_VERSION" ]] || {
  printf 'governed Jankurai version mismatch\n' >&2
  exit 1
}
[[ "$(sha256sum -- "$JANKURAI_BIN" | awk '{print $1}')" == "$JANKURAI_SHA256" ]] || {
  printf 'governed Jankurai digest mismatch\n' >&2
  exit 1
}
command -v jq >/dev/null 2>&1 || {
  printf 'jq is required for the strict audit gate\n' >&2
  exit 1
}

mkdir -p agent target/jankurai
"$JANKURAI_BIN" audit . --mode advisory --full --no-score-history \
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
