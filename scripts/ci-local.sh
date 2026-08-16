#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v rtk >/dev/null 2>&1; then
  rtk() {
    "$@"
  }
  export -f rtk
fi

lane="${1:-required}"
case "$lane" in
  validate|fast|required|pr-ci)
    exec bash "$repo_root/ops/ci/pr-ci.sh"
    ;;
  security)
    exec bash "$repo_root/tools/security-lane.sh"
    ;;
  score)
    exec just score
    ;;
  contract-drift)
    printf 'contract-drift: redline hub has no Jain contracts consumer surface\n'
    exit 0
    ;;
  artifact-support)
    mkdir -p target/artifact-support
    cat > target/artifact-support/redline.json <<'JSON'
{"schema_version":"jeryu.split.artifact-support/v1","repo":"redline","status":"bootstrap"}
JSON
    ;;
  doctor)
    exec bash "$repo_root/scripts/ci-doctor.sh"
    ;;
  *)
    printf 'usage: %s {validate|fast|required|pr-ci|security|score|contract-drift|artifact-support|doctor}\n' "$0" >&2
    exit 64
    ;;
esac
