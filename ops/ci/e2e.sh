#!/usr/bin/env bash
# Web e2e lane: build the frontend + binary, boot the real server, and run the
# Playwright smoke (UI renders + /api/query round-trip). The strict release
# lane records machine-readable browser and accessibility evidence.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$WEB_DIR"
ensure_artifacts
has jq || fail "jq is required for browser evidence"
if [[ "$STRICT_TOOLS" == "1" ]]; then
  [[ -z "$(git -C "$ROOT_DIR" status --porcelain)" ]] \
    || fail "strict browser evidence requires a clean checkout"
fi

if ! has npm; then
  missing_tool npm "Playwright e2e"
  exit 0
fi
if [[ ! -d node_modules/@playwright ]]; then
  log "e2e: installing npm deps"
  npm ci --no-audit --no-fund
fi

if ! npx --no-install playwright --version >/dev/null 2>&1; then
  missing_tool playwright "browser e2e smoke"
  exit 0
fi

# Ensure browsers are present; skip (non-fatal) when they cannot be fetched.
if ! npx --no-install playwright install chromium >/dev/null 2>&1; then
  if [[ "$STRICT_TOOLS" == "1" ]]; then
    fail "e2e: chromium not available and REDLINE_STRICT_TOOLS=1"
  fi
  warn "e2e: chromium browser unavailable; skipping smoke"
  exit 0
fi

log "e2e: running Playwright smoke"
ux_artifact_dir="${ARTIFACT_DIR}/ux-qa"
playwright_json="${ux_artifact_dir}/playwright.json"
mkdir -p "$ux_artifact_dir"
PLAYWRIGHT_JSON_OUTPUT_FILE="$playwright_json" \
  npx --no-install playwright test --reporter=list,json
jq -e '
  .stats.expected == 4
  and .stats.skipped == 0
  and .stats.unexpected == 0
  and .stats.flaky == 0
  and ([.suites[].specs[] | select(.ok != true)] | length) == 0
  and ([.suites[].specs[].title] | sort) == ([
    "POST /api/query round-trips through the connector",
    "SPA renders the workbench shell",
    "running a query from the UI shows results",
    "the workbench has no critical accessibility violations"
  ] | sort)
' "$playwright_json" >/dev/null
log "e2e: complete"
