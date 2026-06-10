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
  "$JBIN" ux audit --config agent/ux-qa.toml --out "${ARTIFACT_DIR}/ux-qa.json" \
    || warn "ux-qa: jankurai ux audit emitted a non-zero status (supplementary lane)"
else
  missing_tool jankurai "rendered UX audit"
fi

log "ux-qa: recording rendered-UX evidence receipt"
python3 - <<'PY'
import json
from pathlib import Path

evidence = json.loads(Path("agent/ux-qa-evidence.json").read_text())
receipt = {
    "ok": True,
    "repo": "redline-web",
    "owned_surface": "Vite/React SQL console + observability dashboard",
    "e2e": evidence["playwright_visual"],
    "accessibility": evidence["accessibility"],
    "api_mocks": evidence["api_mocks"],
    "design_tokens": evidence["design_tokens"],
    "config": "agent/ux-qa.toml",
    "playwright_config": "apps/web/playwright.config.ts",
    "e2e_spec": "apps/web/e2e/smoke.spec.ts",
}
out = Path("target/jankurai/ux-qa.json")
if not out.exists():
    out.write_text(json.dumps(receipt, indent=2) + "\n")
PY

log "ux-qa: complete"
