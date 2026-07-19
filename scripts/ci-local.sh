#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "${1:-validate}" in
  validate|fast|required|pr-ci) exec bash "$repo_root/ops/ci/pr-ci.sh" ;;
  security) exec bash "$repo_root/tools/security-lane.sh" ;;
  score) exec bash "$repo_root/scripts/just/run.sh" score ;;
  contract-drift) exec bash "$repo_root/ops/ci/jankurai-tools.sh" contract-drift ;;
  artifact-support) exec bash "$repo_root/ops/ci/artifact_support.sh" ;;
  doctor) exec bash "$repo_root/scripts/ci-doctor.sh" ;;
  *)
    printf 'usage: %s {validate|fast|required|pr-ci|security|score|contract-drift|artifact-support|doctor}\n' \
      "$0" >&2
    exit 64
    ;;
esac
