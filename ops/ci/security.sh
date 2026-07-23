#!/usr/bin/env bash
# Security lane: supply-chain + secret-scan evidence.
#
# Mirrors the `security` recipe in `justfile` and the `security` job in
# `.github/workflows/jankurai.yml`, so the same hard-gated checks run
# locally (`just security`, `scripts/ci-local.sh security`) and in CI.
# Audit reference: HLT-016 supply-chain-drift, HLT-034 ci-bad-behavior.
#
# Cargo audit, cargo deny, gitleaks, Syft, and actionlint are hard-gated,
# digest-pinned local inputs. Each command retains its raw evidence log.
#
# Usage:
#   bash ops/ci/security.sh

set -euo pipefail

# shellcheck source=ops/ci/lib.sh
. "$(dirname "$0")/lib.sh"
# shellcheck source=ops/ci/pinned-rustsec.sh
. "$(dirname "$0")/pinned-rustsec.sh"
# shellcheck source=ops/ci/security-tools.sh
. "$(dirname "$0")/security-tools.sh"

mkdir -p .jankurai/security
export CARGO_NET_OFFLINE=true SYFT_CHECK_FOR_APP_UPDATE=false

rustsec_stage="$(mktemp -d "${TMPDIR:-/tmp}/redline-security-rustsec.XXXXXX")"
trap 'rm -rf -- "$rustsec_stage"' EXIT
redline_prepare_local_rustsec "$rustsec_stage"

bash ops/ci/governed-security-inputs-test.sh
bash ops/ci/pinned-rustsec-test.sh
bash ops/ci/external-release-policy-test.sh
redline_resolve_security_tools
redline_resolve_pinned_rustsec
redline_resolve_cargo_deny_rustsec

# Resolve the exact locked graph under the runner's sealed Cargo home before
# switching cargo-deny to its isolated RustSec-only home.
cargo metadata --format-version 1 --locked --offline \
    > .jankurai/security/sbom-cargo-metadata.json

# Hard gate: cargo-audit consumes only the exact clean local RustSec object.
"$REDLINE_CARGO_AUDIT_BIN" audit \
    --db "$REDLINE_PINNED_ADVISORY_DB" --no-fetch --json \
    > .jankurai/security/cargo-audit.json

# Hard gate: dependency, license, and source policy cannot refresh their DB.
CARGO_HOME="$REDLINE_CARGO_DENY_HOME" \
    "$REDLINE_CARGO_DENY_BIN" check --disable-fetch \
        --metadata-path .jankurai/security/sbom-cargo-metadata.json 2>&1 \
    | tee .jankurai/security/cargo-deny.log

# Hard gate: gitleaks must succeed for the lane to pass.
"$REDLINE_GITLEAKS_BIN" detect --source . --redact --no-banner

# Hard gate: generate the CycloneDX SBOM alongside cargo metadata.
"$REDLINE_SYFT_BIN" dir:. \
    --source-name redline-core --source-version 4.2.0 \
    --exclude './target/**' --exclude './.git/**' --exclude './.jankurai/**' \
    -o cyclonedx-json=.jankurai/security/sbom-syft.json 2>&1 \
    | tee .jankurai/security/syft.log

# Hard gate: workflow schema and shell validation.
"$REDLINE_ACTIONLINT_BIN" .github/workflows/*.yml 2>&1 \
    | tee .jankurai/security/actionlint.log

jq -n \
    --arg rustsec_commit "$REDLINE_RUSTSEC_COMMIT" \
    --arg rustsec_tree "$REDLINE_RUSTSEC_TREE" \
    --arg cargo_deny_rustsec "$REDLINE_RUSTSEC_COMMIT" \
    --arg cargo_audit "$REDLINE_CARGO_AUDIT_SHA256" \
    --arg cargo_deny "$REDLINE_CARGO_DENY_SHA256" \
    --arg gitleaks "$REDLINE_GITLEAKS_SHA256" \
    --arg syft "$REDLINE_SYFT_SHA256" \
    --arg actionlint "$REDLINE_ACTIONLINT_SHA256" \
    '{schema:"redline.security-custody/v1",status:"pass",network:"denied",
      rustsec:{commit:$rustsec_commit,tree:$rustsec_tree,
        cargo_deny_commit:$cargo_deny_rustsec},
      tools:{cargo_audit:$cargo_audit,cargo_deny:$cargo_deny,
        gitleaks:$gitleaks,syft:$syft,actionlint:$actionlint}}' \
    > .jankurai/security/custody.json
