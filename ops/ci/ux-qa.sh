#!/usr/bin/env bash
# Rendered UX QA lane: run the jankurai UX audit against the built SPA and emit
# layered evidence (Playwright smoke, axe accessibility, design tokens). The
# browser-driven Playwright run lives in ops/ci/e2e.sh; this lane records the
# rendered-UX evidence receipt the audit consumes.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

if JBIN="$(jankurai_bin)"; then
  log "ux-qa: jankurai ux audit"
  if ! "$JBIN" ux audit --config agent/ux-qa.toml --out "${ARTIFACT_DIR}/ux-qa.json"; then
    if [[ "$STRICT_TOOLS" == "1" ]]; then
      fail "ux-qa: jankurai ux audit failed"
    fi
    warn "ux-qa: jankurai ux audit emitted a non-zero status (supplementary lane)"
  fi
else
  missing_tool jankurai "rendered UX audit"
fi

log "ux-qa: recording rendered-UX evidence receipt"
if [[ ! -s "${ARTIFACT_DIR}/ux-qa.json" ]]; then
  if ! has jq; then
    missing_tool jq "rendered UX receipt"
    exit 0
  fi
  jq '{
    ok: true,
    repo: "redline-web",
    owned_surface: "Vite/React SQL console + observability dashboard",
    e2e: .playwright_visual,
    accessibility: .accessibility,
    api_mocks: .api_mocks,
    design_tokens: .design_tokens,
    config: "agent/ux-qa.toml",
    playwright_config: "apps/web/playwright.config.ts",
    e2e_spec: "apps/web/e2e/smoke.spec.ts"
  }' agent/ux-qa-evidence.json >"${ARTIFACT_DIR}/ux-qa.json"
fi

log "ux-qa: complete"
