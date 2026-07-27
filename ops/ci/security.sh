#!/usr/bin/env bash
# Security lane: secret scanning, dependency advisories, license/source policy,
# npm advisories, and workflow linting. Blocking in CI (no `|| true`).
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
mkdir -p "${ROOT_DIR}/.artifacts/security"

if has gitleaks; then
  log "security: gitleaks secret scan"
  gitleaks detect --source . --config gitleaks.toml --no-banner --redact --no-git
else
  missing_tool gitleaks "secret scanning"
fi

if repo_has Cargo.lock; then
  log "security: cargo-audit"
  run_if_has cargo-audit "RustSec advisory scanning" cargo audit
elif repo_has Cargo.toml; then
  warn "skipping cargo-audit: Cargo.lock not present"
fi

if repo_has Cargo.toml && ! has cargo; then
  missing_tool cargo "Rust dependency policy"
elif cargo_workspace_ready; then
  log "security: cargo-deny"
  run_if_has cargo-deny "Rust dependency policy" cargo deny check
elif repo_has Cargo.toml; then
  warn "skipping cargo-deny: Cargo workspace metadata not ready"
fi

if [[ "${JAIN_SECURITY_NETWORK:-0}" != "1" ]]; then
  warn "skipping npm audit: needs the live registry; set JAIN_SECURITY_NETWORK=1 to run it"
elif repo_has apps/web/package-lock.json && has npm; then
  log "security: npm audit (apps/web)"
  (cd "$WEB_DIR" && npm audit --audit-level=high)
elif repo_has apps/web/package-lock.json; then
  missing_tool npm "npm advisory scanning"
fi

if has zizmor; then
  log "security: zizmor workflow lint"
  zizmor .github/workflows
else
  missing_tool zizmor "GitHub Actions security linting"
fi

if has syft; then
  log "security: SBOM"
  syft dir:. -o spdx-json=.artifacts/security/redline-web.spdx.json
else
  missing_tool syft "SBOM generation"
fi

log "security: complete"
