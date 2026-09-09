#!/usr/bin/env bash
# redline-core PR-CI gate — the authoritative local Jeryu check.
# Runs the canonical fast lane, fail-closed supply-chain checks, dependency
# review, and the complete Jankurai ratchet. Independent of the other Redline
# repositories: green here means every Core-owned required gate is green.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Canonical pre-merge gate (preflight checks + full test shards).
bash ops/ci/fast.sh

# All security and policy tools are required. Missing tools or upstream proof
# sources fail this protected check instead of being treated as advisory.
bash ops/ci/security.sh
bash ops/ci/dependency-review.sh
bash ops/ci/jankurai-audit.sh

echo "==> redline-core PR-CI: OK"
