#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same hard-gated checks run
# locally (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Cargo audit, cargo deny, gitleaks, Syft, and actionlint are hard-gated
# end-to-end. Each command retains its raw evidence log.
#
# Usage:
#   bash ops/ci/security.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

mkdir -p .jankurai/security

if ! command -v gitleaks >/dev/null 2>&1 \
    || [ "$(gitleaks version 2>/dev/null || true)" != "$CI_GITLEAKS_VERSION" ]; then
    ci_install_gitleaks
fi

# Hard gate: cargo-audit must succeed for the lane to pass.
cargo audit

# Hard gate: dependency, license, and source policy must all pass.
cargo deny --all-features check 2>&1 \
    | tee .jankurai/security/cargo-deny.log

# Hard gate: gitleaks must succeed for the lane to pass.
gitleaks detect --source . --redact --no-banner

# Provenance/SBOM evidence — capture the workspace dependency
# manifest so the supply-chain lane writes a reviewable artifact
# alongside the audit/deny/gitleaks outputs. Hard gate: must succeed.
cargo metadata --format-version 1 --locked \
    > .jankurai/security/sbom-cargo-metadata.json

# Hard gate: generate the CycloneDX SBOM alongside cargo metadata.
syft . -o cyclonedx-json=.jankurai/security/sbom-syft.json 2>&1 \
    | tee .jankurai/security/syft.log

# Hard gate: workflow schema and shell validation.
actionlint .github/workflows/*.yml 2>&1 \
    | tee .jankurai/security/actionlint.log
