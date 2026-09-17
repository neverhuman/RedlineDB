#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

lane="${1:-required}"
case "$lane" in
  fast) bash ops/ci/fast.sh ;;
  security) bash ops/ci/security.sh ;;
  score) bash ops/ci/jankurai.sh ;;
  score-update) JANKURAI_UPDATE_REVIEWED=1 bash ops/ci/jankurai.sh ;;
  contract-drift) bash ops/ci/contract-drift.sh ;;
  artifact-support) bash ops/ci/artifact-support.sh ;;
  family-release) bash ops/ci/family-release.sh ;;
  coverage) bash ops/ci/coverage.sh ;;
  required) bash ops/ci/required.sh ;;
  *)
    printf 'unknown lane: %s\n' "$lane" >&2
    exit 1
    ;;
esac
