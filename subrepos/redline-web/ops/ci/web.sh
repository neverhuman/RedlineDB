#!/usr/bin/env bash
# Frontend lane: install, typecheck, lint, unit tests, production build. The
# build emits apps/web/dist, which the Rust binary embeds via rust-embed.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if ! has npm; then
  missing_tool npm "frontend build"
  exit 0
fi

cd "$WEB_DIR"
if [[ -f package-lock.json ]]; then
  log "web: npm ci"
  npm ci --no-audit --no-fund
else
  log "web: npm install"
  npm install --no-audit --no-fund
fi
log "web: typecheck"
npm run typecheck
log "web: lint"
npm run lint
log "web: unit tests"
npm run test
log "web: build"
npm run build
log "web: complete"
