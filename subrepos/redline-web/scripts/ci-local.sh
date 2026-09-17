#!/usr/bin/env bash
# Local entry point: run any CI lane locally with the same script CI calls, so
# local runs never drift from CI.
set -Eeuo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  printf 'usage: %s [fast|web|backend|security|score|contract-drift|artifact-support|e2e|jankurai|cost-budget|release-readiness|required|pr-ci|doctor]\n' "$0" >&2
}

if [[ "$#" -ne 1 ]]; then
  usage
  exit 64
fi
COMMAND="$1"

case "$COMMAND" in
  fast)            exec bash "${ROOT_DIR}/ops/ci/fast.sh" ;;
  web)             exec bash "${ROOT_DIR}/ops/ci/web.sh" ;;
  backend)         exec bash "${ROOT_DIR}/ops/ci/backend.sh" ;;
  security)        exec bash "${ROOT_DIR}/ops/ci/security.sh" ;;
  score)           exec bash "${ROOT_DIR}/ops/ci/score.sh" ;;
  contract-drift)  exec bash "${ROOT_DIR}/ops/ci/contract-drift.sh" ;;
  artifact-support) exec bash "${ROOT_DIR}/ops/ci/artifact-support.sh" ;;
  e2e)             exec bash "${ROOT_DIR}/ops/ci/e2e.sh" ;;
  jankurai)        exec bash "${ROOT_DIR}/ops/ci/jankurai.sh" ;;
  cost-budget)     exec bash "${ROOT_DIR}/ops/ci/cost-budget.sh" ;;
  release-readiness) exec bash "${ROOT_DIR}/ops/ci/release-readiness.sh" ;;
  required)
    export REDLINE_STRICT_TOOLS=1
    exec bash "${ROOT_DIR}/ops/ci/pr-ci.sh"
    ;;
  pr-ci)           exec bash "${ROOT_DIR}/ops/ci/pr-ci.sh" ;;
  doctor)          exec bash "${ROOT_DIR}/scripts/ci-doctor.sh" ;;
  *)
    usage
    exit 64
    ;;
esac
