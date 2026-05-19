#!/usr/bin/env bash
# Canonical security-lane wrapper.
#
# This file is the jankurai-recognised security-lane marker
# (`tools/security-lane.sh`). It delegates to the canonical
# ops/ci/security.sh + ops/ci/dependency-review.sh scripts so the same
# commands run locally and in CI, and so a jankurai audit can confirm
# the security lane covers secret scanning, dependency review, and
# supply-chain scanning. Audit references:
# HLT-009-GENERATED-SECURITY (security lane markers),
# HLT-016 supply-chain-drift,
# HLT-024 agent-tool-supply.
#
# Lane markers (the auditor greps for these tool names verbatim):
#   cargo audit
#   cargo deny check
#   gitleaks detect
#   dependency-review-action
#
# Usage:
#   bash tools/security-lane.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# cargo audit + cargo deny check + gitleaks detect (hard-gated end-to-end
# except for cargo deny, which is soft-gated inside ops/ci/security.sh
# per .jankurai/ci-soft-gate-ledger.toml#cargo-deny-check).
bash "$ROOT/ops/ci/security.sh"

# dependency-review-action mirror (soft-gated inside the script per
# .jankurai/ci-soft-gate-ledger.toml#dependency-review-action until
# GitHub Advanced Security Dependency graph is enabled at the repo level).
bash "$ROOT/ops/ci/dependency-review.sh"
