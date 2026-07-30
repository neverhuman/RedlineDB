#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same three commands run
# locally (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Every applicable scanner and evidence generator is a hard gate. A missing
# tool, version probe failure, scanner failure, SBOM failure, or workflow-lint
# failure makes this lane fail nonzero.
#
# Usage:
#   bash ops/ci/security.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"

mkdir -p .jankurai/security
control_manifest="tools/evidence-processor/Cargo.toml"
control_lock="tools/evidence-processor/Cargo.lock"

gitleaks_version=""
if command -v gitleaks >/dev/null 2>&1; then
    if ! gitleaks_version="$(gitleaks version 2>/dev/null)"; then
        gitleaks_version=""
    fi
fi
if [ "$gitleaks_version" != "$CI_GITLEAKS_VERSION" ]; then
    ci_install_gitleaks
fi

if [ -f Cargo.toml ] && [ -f Cargo.lock ]; then
    cargo audit --no-fetch
    cargo deny --all-features check 2>&1 \
        | tee .jankurai/security/cargo-deny.log
    cargo metadata --format-version 1 --locked \
        > .jankurai/security/sbom-cargo-metadata.json
elif [ -e Cargo.toml ] || [ -e Cargo.lock ]; then
    printf 'incomplete Rust dependency graph: Cargo.toml and Cargo.lock must be present together\n' >&2
    exit 1
elif [ -f "$control_manifest" ] && [ -f "$control_lock" ]; then
    cargo audit --no-fetch --file "$control_lock"
    cargo deny --manifest-path "$control_manifest" check 2>&1 \
        | tee .jankurai/security/cargo-deny.log
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

gitleaks detect --source . --redact --no-banner

just security-sbom 2>&1 | tee .jankurai/security/syft.log
just security-workflows 2>&1 | tee .jankurai/security/actionlint.log
