#!/usr/bin/env bash

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
usage: scripts/release/publish-chain.sh <version>

Publishes the crates.io release chain in order:
  redlinedb-domain
  redlinedb-kernel
  redlinedb-sql
  redlinedb-ffi
  redlinedb

After each publish, the script waits until the new version is visible in
the crates.io index before continuing.
USAGE
}

if [ "$#" -ne 1 ]; then
    usage
    exit 64
fi

version="$1"
crates=(
    redlinedb-domain
    redlinedb-kernel
    redlinedb-sql
    redlinedb-ffi
    redlinedb
)
release_dir="${RELEASE_WITNESS_DIR:-target/release}"
witness_path="${RELEASE_WITNESS_PATH:-${release_dir}/release-witness.jsonl}"
sha256_path="${RELEASE_SHA256SUMS_PATH:-${release_dir}/SHA256SUMS}"
sbom_path="${RELEASE_SBOM_PATH:-${release_dir}/sbom.cdx.json}"
provenance_path="${RELEASE_PROVENANCE_PATH:-${release_dir}/provenance.intoto.jsonl}"
signature_path="${RELEASE_SIGNATURE_PATH:-${release_dir}/tag.sig}"
attestation_path="${RELEASE_ATTESTATION_PATH:-${release_dir}/attestation.intoto.jsonl}"

require_artifact() {
    local path="$1"
    local label="$2"
    if [ ! -s "$path" ]; then
        printf 'missing release witness artifact: %s (%s)\n' "$label" "$path" >&2
        return 1
    fi
}

write_witness_record() {
    local crate="$1"
    local published_at
    published_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    mkdir -p "$(dirname "$witness_path")"
    cat <<EOF >>"$witness_path"
{"crate":"$crate","version":"$version","published_at":"$published_at","artifacts":{"sha256sums":"$sha256_path","sbom":"$sbom_path","provenance":"$provenance_path","signature":"$signature_path","attestation":"$attestation_path"}}
EOF
}

require_artifact "$sha256_path" "SHA256SUMS"
require_artifact "$sbom_path" "sbom.cdx.json"
require_artifact "$provenance_path" "provenance.intoto.jsonl"
require_artifact "$signature_path" "tag.sig"
require_artifact "$attestation_path" "attestation.intoto.jsonl"

mkdir -p "$release_dir"
: >"$witness_path"

wait_for_index() {
    local crate="$1"
    local deadline="${RELEASE_INDEX_TIMEOUT_SECONDS:-900}"
    local search_output
    local start

    start="${SECONDS}"
    while true; do
        if search_output="$(rtk cargo search "$crate" --limit 1 2>/dev/null)" \
            && printf '%s\n' "$search_output" | grep -Fq "${crate} = \"${version}\""; then
            printf 'indexed: %s %s\n' "$crate" "$version" >&2
            return 0
        fi

        if [ $((SECONDS - start)) -ge "$deadline" ]; then
            printf 'timed out waiting for %s %s to appear in the crates.io index\n' \
                "$crate" "$version" >&2
            return 1
        fi

        sleep "${RELEASE_INDEX_POLL_SECONDS:-10}"
    done
}

for crate in "${crates[@]}"; do
    printf 'publishing: %s %s\n' "$crate" "$version" >&2
    rtk cargo publish --locked -p "$crate"
    wait_for_index "$crate"
    write_witness_record "$crate"
done
