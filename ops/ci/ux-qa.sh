#!/usr/bin/env bash
# Rendered UX QA lane: run the jankurai UX audit against the built SPA and emit
# layered evidence (Playwright smoke, axe accessibility, design tokens). The
# browser-driven Playwright run lives in ops/ci/e2e.sh; this lane records the
# rendered-UX evidence receipt the audit consumes.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

JBIN="$(jankurai_bin)" || fail "governed Jankurai identity verification failed"
log "ux-qa: jankurai ux audit"
"$JBIN" ux audit --config agent/ux-qa.toml --out "${ARTIFACT_DIR}/ux-qa.json"
[[ -s "${ARTIFACT_DIR}/ux-qa.json" ]] || fail "Jankurai UX evidence is empty"

log "ux-qa: complete"
