#!/usr/bin/env bash
# Fast pre-push quality gate: the cheap deterministic subset of pr-ci that must
# pass before a push (full lanes run in CI / `bash ops/ci/pr-ci.sh`).
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

bash ops/ci/fast.sh
log "quality-gates: OK"
