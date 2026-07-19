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
export CARGO_NET_OFFLINE=true

# --- secret scanning ---------------------------------------------------------
if has gitleaks; then
    log "security: gitleaks secret scan"
    ci_run gitleaks detect --source . --config gitleaks.toml --no-banner --redact --no-git
else
    missing_tool gitleaks "secret scanning"
fi

# --- dependency advisory review ---------------------------------------------
if repo_has Cargo.lock; then
    rustsec_db="$(governed_advisory_db_path)" \
        || fail "fleet-governed RustSec database selection failed"
    verify_rustsec_db_identity "$rustsec_db" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
        || fail "pinned local RustSec database verification failed"
    log "security: RustSec commit $RUSTSEC_DB_COMMIT tree $RUSTSEC_DB_TREE"
    run_if_has cargo-audit "RustSec advisory scanning" \
        cargo audit --db "$rustsec_db" --no-fetch
    verify_rustsec_db_identity "$rustsec_db" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
        || fail "RustSec database identity changed during advisory scanning"
elif repo_has Cargo.toml; then
    warn "skipping cargo-audit: Cargo.lock not present"
fi

# --- dependency license / ban / source policy --------------------------------
if repo_has Cargo.toml; then
    verify_locked_cargo_closure "$repo_root/Cargo.toml" \
        || fail "offline locked Cargo metadata closure is incomplete"
    verify_cargo_deny_db_binding "${CARGO_HOME:-/home/ubuntu/.cargo}" "$rustsec_db" \
        || fail "cargo-deny is not bound to the governed RustSec identity"
    run_if_has cargo-deny "Rust dependency policy" cargo deny check --disable-fetch
    verify_rustsec_db_identity "$rustsec_db" "$RUSTSEC_DB_COMMIT" "$RUSTSEC_DB_TREE" \
        || fail "RustSec database identity changed during dependency policy scanning"
    verify_cargo_deny_db_binding "${CARGO_HOME:-/home/ubuntu/.cargo}" "$rustsec_db" \
        || fail "cargo-deny RustSec binding changed during dependency policy scanning"
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
    ci_run env SYFT_CHECK_FOR_APP_UPDATE=false \
        syft dir:. -o "spdx-json=target/security/redline-testing.spdx.json"
else
    missing_tool syft "SBOM generation"
fi

log "security: complete"
