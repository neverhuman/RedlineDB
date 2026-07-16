#!/usr/bin/env bash
# Bind the real Playwright/Axe browser run to the exact clean commit. The
# standalone Jankurai distribution does not ship its repository-local
# packages/ux-qa runtime, so release evidence must not invoke that missing path.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

has git || fail "git is required for exact-head UX evidence"
has jq || fail "jq is required for exact-head UX evidence"
[[ -z "$(git status --porcelain)" ]] || fail "UX evidence requires a clean checkout"
playwright_json="${ARTIFACT_DIR}/ux-qa/playwright.json"
[[ -s "$playwright_json" ]] || fail "Playwright UX evidence is missing"
jq -e '
  .stats.expected == 4
  and .stats.skipped == 0
  and .stats.unexpected == 0
  and .stats.flaky == 0
  and ([.suites[].specs[] | select(.ok != true)] | length) == 0
' "$playwright_json" >/dev/null

report_sha256="$(jain_sha256 "$playwright_json")"
smoke_sha256="$(jain_sha256 apps/web/e2e/smoke.spec.ts)"
config_sha256="$(jain_sha256 apps/web/playwright.config.ts)"
jq -n \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  --arg report_sha256 "$report_sha256" \
  --arg smoke_sha256 "$smoke_sha256" \
  --arg config_sha256 "$config_sha256" \
  --slurpfile report "$playwright_json" \
  '{schema_version:"redline.web.ux-qa/v1",status:"pass",
    commit:$commit,tree:$tree,playwright_report_sha256:$report_sha256,
    smoke_spec_sha256:$smoke_sha256,playwright_config_sha256:$config_sha256,
    stats:$report[0].stats,
    tests:[$report[0].suites[].specs[]|{title,ok}]}' \
  >"${ARTIFACT_DIR}/ux-qa.json"

log "ux-qa: complete"
