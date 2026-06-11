#!/usr/bin/env bash
# One-command setup for working on the RedlineDB hub. The hub is docs + an
# installer + release glue (no build tree), so setup just checks the tools the
# validate gate needs and points you at it.
set -Eeuo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "RedlineDB hub — tool check:"
for t in bash python3 shellcheck gitleaks; do
  if command -v "$t" >/dev/null 2>&1; then echo "  ok   $t"; else echo "  miss $t (optional in non-strict mode)"; fi
done
echo
echo "Validate everything (same as CI):  bash ops/ci/pr-ci.sh   (or: just check)"
