#!/usr/bin/env bash
# Security + supply-chain lane for the RedlineDB hub. The installer pipes to bash,
# so the shell surface and secret hygiene are the security-relevant artifacts here.
set -Eeuo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
strict="${REDLINE_STRICT_TOOLS:-1}"

run_or_note() { # tool... ; in strict mode missing tool fails, else warns
  local tool="$1"
  if command -v "$tool" >/dev/null 2>&1; then "$@"; return; fi
  if [ "$strict" = "1" ]; then echo "security: required tool '$tool' missing"; return 1; fi
  echo "security: '$tool' not installed; skipped (non-strict)"
}

echo "==> shellcheck (installer + lanes)"
run_or_note shellcheck install.sh ops/ci/*.sh scripts/*.sh

echo "==> gitleaks (secret scan)"
run_or_note gitleaks detect --no-banner --redact --source .

echo "==> no checked-in binaries / archives that should be releases"
if git ls-files | grep -E '\.(tar\.gz|zip|exe)$|/redline$' ; then
  echo "release artifacts must not be committed"; exit 1
fi

echo "==> security lane OK"
