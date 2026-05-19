#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same three commands run
# locally (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Soft-gate rationale: see .jankurai/ci-soft-gate-ledger.toml#cargo-deny-check
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

mkdir -p .jankurai/security

# Hard gate: cargo-audit must succeed for the lane to pass.
cargo audit

# Soft gate: cargo-deny `cargo metadata` JSON parser drift against
# rust 1.95.0 on the current workspace. See ledger for unblock.
ci_soft_gate \
    cargo-deny-check \
    .jankurai/security/cargo-deny.log \
    -- cargo deny --all-features check

# Hard gate: gitleaks must succeed for the lane to pass.
gitleaks detect --source . --redact --no-banner

# Provenance/SBOM evidence — capture the workspace dependency
# manifest so the supply-chain lane writes a reviewable artifact
# alongside the audit/deny/gitleaks outputs. Hard gate: must succeed.
cargo metadata --format-version 1 --locked \
    > .jankurai/security/sbom-cargo-metadata.json

# SBOM generation via syft — soft-gated; produces a CycloneDX SBOM
# artifact alongside the cargo-metadata evidence. Requires syft in PATH;
# installed in CI by the jankurai.yml security job.
# See ledger: .jankurai/ci-soft-gate-ledger.toml#syft-sbom.
ci_soft_gate \
    syft-sbom \
    .jankurai/security/syft.log \
    -- syft . -o cyclonedx-json=.jankurai/security/sbom-syft.json

# Workflow linting via actionlint — soft-gated; validates CI YAML for
# schema correctness and security best practices. Requires actionlint
# in PATH; installed in CI by the jankurai.yml security job.
# See ledger: .jankurai/ci-soft-gate-ledger.toml#actionlint-workflow-lint.
ci_soft_gate \
    actionlint-workflow-lint \
    .jankurai/security/actionlint.log \
    -- actionlint .github/workflows/*.yml
