#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "${1:-validate}" in
  validate|fast|pr-ci) exec bash "$repo_root/ops/ci/pr-ci.sh" ;;
  doctor) exec bash "$repo_root/scripts/ci-doctor.sh" ;;
  *) printf 'usage: %s {validate|fast|pr-ci|doctor}\n' "$0" >&2; exit 64 ;;
esac
