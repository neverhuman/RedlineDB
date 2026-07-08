#!/usr/bin/env bash
# Local CI entrypoint for the jain-split-ops control-plane repo. Mirrors the member-repo
# dispatcher shape (scripts/ci-local.sh <lane>) so split-host-ci.sh can run the required lane
# and post the jain-split-ops/required status that branch protection gates on.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

lane="${1:-required}"
case "$lane" in
  required) bash ops/ci/required.sh ;;
  *) printf 'unknown lane: %s (jain-split-ops has only: required)\n' "$lane" >&2; exit 2 ;;
esac
