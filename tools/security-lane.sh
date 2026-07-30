#!/usr/bin/env bash
# Canonical security-lane wrapper.
#
# This file is the jankurai-recognised security-lane marker
# (`tools/security-lane.sh`). It delegates to the canonical hard-gated
# ops/ci/security.sh script so the same commands run locally and in CI,
# and so a jankurai audit can confirm the security lane covers secret
# scanning, dependency review, and supply-chain scanning. Audit references:
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

# Applicable cargo audit + cargo deny check, plus gitleaks detect. The bundled
# Rust evidence processor supplies the dependency graph for this thin hub, and
# a partial graph fails closed. Every applicable command is a hard gate.
bash "$ROOT/ops/ci/security.sh"
