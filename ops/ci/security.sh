#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same three commands run
# locally (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift.
#
# Usage:
#   bash ops/ci/security.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

cargo audit
cargo deny check
gitleaks detect --source . --redact --no-banner
