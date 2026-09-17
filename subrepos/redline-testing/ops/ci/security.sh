#!/usr/bin/env bash
#
# Security lane for redline-testing: secret scanning, dependency review,
# dependency-policy/license enforcement, and GitHub Actions workflow linting.
# Wired into CI (.github/workflows/ci.yml), into the validate entrypoint
# (ops/ci/pr-ci.sh), and captured as jankurai evidence via
# `jankurai security run --strict --profile ci --script ops/ci/security.sh`.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p target/security

# --- secret scanning ---------------------------------------------------------
if has gitleaks; then
    log "security: gitleaks secret scan"
    ci_run gitleaks detect --source . --config gitleaks.toml --no-banner --redact --no-git
else
    missing_tool gitleaks "secret scanning"
fi

# --- dependency advisory review ---------------------------------------------
if repo_has Cargo.lock; then
    run_if_has cargo-audit "RustSec advisory scanning" cargo audit
elif repo_has Cargo.toml; then
    warn "skipping cargo-audit: Cargo.lock not present"
fi

# --- dependency license / ban / source policy --------------------------------
if cargo_workspace_ready; then
    run_if_has cargo-deny "Rust dependency policy" cargo deny check
elif repo_has Cargo.toml; then
    warn "skipping cargo-deny: Cargo workspace metadata is not ready"
fi

# --- npm advisory review (only if a JS lockfile exists) ----------------------
if repo_has package-lock.json && has npm; then
    log "security: npm audit"
    ci_run npm audit --audit-level=high
elif repo_has package-lock.json; then
    missing_tool npm "npm advisory scanning"
fi

# --- GitHub Actions workflow security linting --------------------------------
if has zizmor; then
    log "security: zizmor workflow lint"
    ci_run zizmor .github/workflows
else
    missing_tool zizmor "GitHub Actions security linting"
fi

# --- SBOM / provenance -------------------------------------------------------
if has syft; then
    log "security: SBOM generation"
    ci_run syft dir:. -o "spdx-json=target/security/redline-testing.spdx.json"
else
    missing_tool syft "SBOM generation"
fi

log "security: complete"
