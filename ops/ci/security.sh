#!/usr/bin/env bash
# Security + supply-chain lane for the RedlineDB hub. The installer pipes to bash,
# so the shell surface and secret hygiene are the security-relevant artifacts here.
set -Eeuo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"
cd "$(repo_root)"

strict="${REDLINE_STRICT_TOOLS:-1}"

run_or_note() {
  local tool="$1"
  if command -v "$tool" >/dev/null 2>&1; then "$@"; return; fi
  if [ "$strict" = "1" ]; then
    die "$ERR_MISSING_TOOL" "security: required tool '$tool' missing"
  fi
  echo "security: '$tool' not installed; skipped (non-strict)"
}

log_step "shellcheck (installer + lanes)"
run_or_note shellcheck install.sh ops/ci/*.sh scripts/*.sh

log_step "gitleaks (secret scan)"
run_or_note gitleaks detect --no-banner --redact --source .

log_step "cargo audit (hub crate dependency vulnerability scan)"
if command -v cargo >/dev/null 2>&1 && command -v cargo-audit >/dev/null 2>&1; then
  cargo audit --file crates/hub/Cargo.lock
elif command -v cargo >/dev/null 2>&1 && [ -f crates/hub/Cargo.lock ]; then
  if cargo audit --version >/dev/null 2>&1; then
    cargo audit --file crates/hub/Cargo.lock
  else
    echo "security: cargo-audit not installed; skipped"
  fi
fi

log_step "cargo deny (supply-chain policy: licenses + advisories)"
if command -v cargo-deny >/dev/null 2>&1 || (command -v cargo >/dev/null 2>&1 && cargo deny --version >/dev/null 2>&1); then
  cargo deny --manifest-path crates/hub/Cargo.toml check advisories licenses
else
  echo "security: cargo-deny not installed; skipped (install: cargo install cargo-deny)"
fi

log_step "no checked-in binaries or release artifacts"
if git ls-files | grep -E '\.(tar\.gz|zip|exe)$|/redline$'; then
  die "$ERR_SECRET_DETECTED" "release artifacts must not be committed to the repo"
fi

log_ok "security lane OK"
