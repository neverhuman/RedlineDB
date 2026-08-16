#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v rtk >/dev/null 2>&1; then
  rtk() {
    "$@"
  }
  export -f rtk
fi

case "${1:-validate}" in
  validate|fast|required|pr-ci) exec bash "$repo_root/ops/ci/pr-ci.sh" ;;
  security) exec bash "$repo_root/tools/security-lane.sh" ;;
  doctor) exec bash "$repo_root/scripts/ci-doctor.sh" ;;
  *) printf 'usage: %s {validate|fast|required|pr-ci|security|doctor}\n' "$0" >&2; exit 64 ;;
esac
