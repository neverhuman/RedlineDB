#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same commands run locally
# (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# The workflow YAML carries NO `continue-on-error: true`. cargo-audit,
# cargo-deny, gitleaks, cargo metadata SBOM evidence, syft SBOM evidence,
# and actionlint workflow linting all must succeed end-to-end for the lane
# to pass.
#
# Usage:
#   bash ops/ci/security.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

mkdir -p target/jankurai/security

# Hard gate: cargo-audit must succeed for the lane to pass.
cargo audit

# Hard gate: cargo-deny must succeed for the lane to pass.
cargo deny --all-features check

# Hard gate: gitleaks must succeed for the lane to pass.
gitleaks detect --source . --redact --no-banner

# Provenance/SBOM evidence — capture the workspace dependency
# manifest so the supply-chain lane writes a reviewable artifact
# alongside the audit/deny/gitleaks outputs. Hard gate: must succeed.
cargo metadata --format-version 1 --locked \
    > target/jankurai/security/sbom-cargo-metadata.json

# Hard gate: syft must produce a reviewable CycloneDX SBOM artifact.
syft . -o cyclonedx-json=target/jankurai/security/sbom-syft.json

# Hard gate: actionlint must accept every GitHub Actions workflow.
actionlint .github/workflows/*.yml
