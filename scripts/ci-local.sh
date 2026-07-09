#!/usr/bin/env bash
# Local CI entrypoint for the jain-split-ops control-plane repo. Mirrors the member-repo
# dispatcher shape (scripts/ci-local.sh <lane>) so split-host-ci.sh can run the required lane
# and post the jain-split-ops/required status that branch protection gates on.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

lane="${1:-required}"
case "$lane" in
  jeryu-doctor) python3 ops/split/jeryu-doctor.py ;;
  jeryu-ready) python3 ops/split/jeryu-doctor.py --setup-auth --fix-remotes ;;
  fast) bash ops/ci/fast.sh ;;
  check) bash ops/ci/check.sh ;;
  required) bash ops/ci/required.sh ;;
  score) bash ops/ci/score.sh ;;
  security) bash ops/ci/security.sh ;;
  tool-adoption) bash ops/ci/tool-adoption.sh ;;
  contract-drift) bash ops/ci/contract-drift.sh ;;
  artifact-support) bash ops/ci/artifact_support.sh ;;
  *) printf 'unknown lane: %s (jain-split-ops has: jeryu-doctor, jeryu-ready, fast, check, required, score, security, tool-adoption, contract-drift, artifact-support)\n' "$lane" >&2; exit 2 ;;
esac
