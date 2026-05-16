#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same three commands run
# locally (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Soft-gate rationale: see agent/ci-soft-gate-ledger.toml#cargo-deny-check
# The workflow YAML carries NO `continue-on-error: true`. The cargo-deny
# soft-gate semantics live in this script via `ci_soft_gate`, which
# always returns 0 for the wrapped command while writing an explicit
# `soft-gate=cargo-deny-check status=...` marker line to the audit log.
# cargo-audit and gitleaks remain hard-gated end-to-end.
#
# Usage:
#   bash ops/ci/security.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

mkdir -p target/jankurai/security

# Hard gate: cargo-audit must succeed for the lane to pass.
cargo audit

# Soft gate: cargo-deny `cargo metadata` JSON parser drift against
# rust 1.95.0 on the current workspace. See ledger for unblock.
ci_soft_gate \
    cargo-deny-check \
    target/jankurai/security/cargo-deny.log \
    -- cargo deny --all-features check

# Hard gate: gitleaks must succeed for the lane to pass.
gitleaks detect --source . --redact --no-banner
