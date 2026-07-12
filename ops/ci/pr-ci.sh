#!/usr/bin/env bash
# redline-web PR-CI gate — the single authoritative local + CI validate command.
# `bash ops/ci/pr-ci.sh` runs exactly what .github/workflows/ci.yml runs, lane
# for lane (ci-local parity). Green here means redline-web is green; a red build
# in a sibling redline repo never turns this repo red.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

log "pr-ci: fast lane"
bash ops/ci/fast.sh

log "pr-ci: frontend lane"
bash ops/ci/web.sh

log "pr-ci: backend lane"
bash ops/ci/backend.sh

log "pr-ci: contract-drift lane"
bash ops/ci/contract-drift.sh

log "pr-ci: security lane"
bash ops/ci/security.sh

log "pr-ci: web e2e lane (browser required by the canonical required dispatcher)"
bash ops/ci/e2e.sh

log "pr-ci: cost-budget + release-readiness lanes"
bash ops/ci/cost-budget.sh
bash ops/ci/release-readiness.sh

log "pr-ci: jankurai tool-suite evidence"
bash ops/ci/jankurai.sh
bash ops/ci/evidence-catalog.sh

log "pr-ci: OK"
