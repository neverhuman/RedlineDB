#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# shellcheck source=ops/ci/lib.sh
. ops/ci/lib.sh

scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT

artifact="$scratch/redline-testing.tar.gz"
receipt="$scratch/custody-receipt.json"
printf 'reviewed artifact\n' >"$artifact"
artifact_sha256="$(sha256sum "$artifact" | awk '{print $1}')"

write_receipt() {
    local bound_sha256="${1:?artifact sha256 required}"
    jq -n --arg artifact_sha256 "$bound_sha256" \
        '{schema_version:"redline.custody-receipt/v1",status:"pass",artifact_sha256:$artifact_sha256}' \
        >"$receipt"
}

write_receipt "$artifact_sha256"
REDLINE_ORACLE_CUSTODY_RECEIPT="$receipt" \
    ci_verify_redline_testing_attestation "$artifact"

printf 'tampered artifact\n' >"$artifact"
if REDLINE_ORACLE_CUSTODY_RECEIPT="$receipt" \
    ci_verify_redline_testing_attestation "$artifact"; then
    printf 'tampered artifact unexpectedly matched custody receipt\n' >&2
    exit 1
fi

printf 'reviewed artifact\n' >"$artifact"
write_receipt "$(printf 'foreign artifact\n' | sha256sum | awk '{print $1}')"
if REDLINE_ORACLE_CUSTODY_RECEIPT="$receipt" \
    ci_verify_redline_testing_attestation "$artifact"; then
    printf 'foreign custody receipt unexpectedly matched artifact\n' >&2
    exit 1
fi

write_receipt "$artifact_sha256"
jq '.status = "failed"' "$receipt" >"$scratch/stale-receipt.json"
if REDLINE_ORACLE_CUSTODY_RECEIPT="$scratch/stale-receipt.json" \
    ci_verify_redline_testing_attestation "$artifact"; then
    printf 'stale failed custody receipt unexpectedly passed\n' >&2
    exit 1
fi

if REDLINE_ORACLE_CUSTODY_RECEIPT="$scratch/missing-receipt.json" \
    ci_verify_redline_testing_attestation "$artifact"; then
    printf 'missing custody receipt unexpectedly passed\n' >&2
    exit 1
fi

printf 'oracle custody binding tests passed\n'
