#!/usr/bin/env bash
# Web e2e lane: build the frontend + binary, boot the real server, and run the
# Playwright smoke (UI renders + /api/query round-trip). Browser-driven, so it
# is best-effort locally and blocking on CI runners that have browsers
# installed (REDLINE_STRICT_TOOLS=1).
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$WEB_DIR"

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
npx --no-install playwright test
log "e2e: complete"
