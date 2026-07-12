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
# cargo-audit is hard-gated when a complete Rust dependency graph exists;
# gitleaks remains hard-gated for every repository profile.
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

if [ -f Cargo.toml ] && [ -f Cargo.lock ]; then
    # Hard gate: cargo-audit must succeed for an applicable Rust graph.
    cargo audit

    # Soft gate: cargo-deny `cargo metadata` JSON parser drift against
    # rust 1.95.0 on the current workspace. See ledger for unblock.
    ci_soft_gate \
        cargo-deny-check \
        .jankurai/security/cargo-deny.log \
        -- cargo deny --all-features check

    # Hard gate: the locked dependency graph must produce reviewable SBOM
    # input whenever Rust manifests are present.
    cargo metadata --format-version 1 --locked \
        > .jankurai/security/sbom-cargo-metadata.json
elif [ -e Cargo.toml ] || [ -e Cargo.lock ]; then
    printf 'incomplete Rust dependency graph: Cargo.toml and Cargo.lock must be present together\n' >&2
    exit 1
else
    printf '%s\n' \
        '{"status":"not_applicable","reason":"thin hub has no Rust dependency graph"}' \
        > .jankurai/security/sbom-cargo-metadata.json
    printf 'cargo dependency audit: not applicable (no Cargo.toml or Cargo.lock)\n'
fi

# Hard gate: gitleaks must succeed for the lane to pass.
gitleaks detect --source . --redact --no-banner

# General SBOM generation via syft — soft-gated; produces a CycloneDX SBOM
# for both Rust workspaces and the shell/docs-only hub. Requires syft in PATH;
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
