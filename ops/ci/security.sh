#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Runs through the authoritative local Jeryu fleet and its exact-head sandbox.
# The public `.github/workflows/jankurai.yml` is deliberately a stock-runner
# static contract only and never claims to execute these governed scanners.
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
bash ops/ci/github-mirror-contract.sh
control_manifest="tools/evidence-processor/Cargo.toml"
control_lock="tools/evidence-processor/Cargo.lock"
cargo_audit_args=()
if [ -n "${CI_CARGO_AUDIT_DB:-}" ]; then
    if [ ! -d "$CI_CARGO_AUDIT_DB" ] || [ -L "$CI_CARGO_AUDIT_DB" ]; then
        printf 'CI_CARGO_AUDIT_DB must be a non-symlink directory: %s\n' \
            "$CI_CARGO_AUDIT_DB" >&2
        exit 1
    fi
    cargo_audit_args+=(--db "$CI_CARGO_AUDIT_DB")
fi
if [ "${CI_CARGO_AUDIT_NO_FETCH:-0}" = "1" ]; then
    cargo_audit_args+=(--no-fetch)
fi

if ! command -v gitleaks >/dev/null 2>&1 \
    || [ "$(gitleaks version 2>/dev/null || true)" != "$CI_GITLEAKS_VERSION" ]; then
    ci_install_gitleaks
fi

if [ -f Cargo.toml ] && [ -f Cargo.lock ]; then
    # Hard gate: cargo-audit must succeed for an applicable Rust graph.
    cargo audit "${cargo_audit_args[@]}"

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
elif [ -f "$control_manifest" ] && [ -f "$control_lock" ]; then
    cargo audit "${cargo_audit_args[@]}" --file "$control_lock"
    ci_soft_gate \
        cargo-deny-check \
        .jankurai/security/cargo-deny.log \
        -- cargo deny --manifest-path "$control_manifest" check
    cargo metadata --manifest-path "$control_manifest" --format-version 1 --locked \
        > .jankurai/security/sbom-cargo-metadata.json
elif [ -e "$control_manifest" ] || [ -e "$control_lock" ]; then
    printf 'incomplete bundled control-tool dependency graph: %s and %s must be present together\n' \
        "$control_manifest" "$control_lock" >&2
    exit 1
else
    printf '%s\n' \
        '{"status":"not_applicable","reason":"thin hub has no Rust dependency graph"}' \
        > .jankurai/security/sbom-cargo-metadata.json
    printf 'cargo dependency audit: not applicable (no Cargo.toml or Cargo.lock)\n'
fi

# Hard gate: gitleaks must succeed for the lane to pass. Jankurai's strict
# security runner consumes the structured step record; a zero-exit scanner
# without that record is intentionally treated as missing required evidence.
gitleaks_status=0
gitleaks detect --source . --redact --no-banner \
    > .jankurai/security/gitleaks.log 2>&1 || gitleaks_status=$?
printf 'jankurai-security-step={"label":"gitleaks","tool":"gitleaks","shell_command":"gitleaks detect --source . --redact --no-banner","status":"%s","advisory":false,"exit_code":%s}\n' \
    "$([ "$gitleaks_status" -eq 0 ] && printf ran || printf failed)" \
    "$gitleaks_status"
if [ "$gitleaks_status" -ne 0 ]; then
    cat .jankurai/security/gitleaks.log >&2
    exit "$gitleaks_status"
fi

# General SBOM generation via syft — soft-gated; produces a CycloneDX SBOM
# for both Rust workspaces and the shell/docs-only hub. Requires syft in PATH;
# provisioned by the local fleet image.
# See ledger: .jankurai/ci-soft-gate-ledger.toml#syft-sbom.
ci_soft_gate syft-sbom .jankurai/security/syft.log -- just security-sbom

# Workflow linting via actionlint — soft-gated; validates CI YAML for
# schema correctness and security best practices. Requires actionlint in PATH;
# provisioned by the local fleet image.
# See ledger: .jankurai/ci-soft-gate-ledger.toml#actionlint-workflow-lint.
ci_soft_gate actionlint-workflow-lint .jankurai/security/actionlint.log -- just security-workflows
