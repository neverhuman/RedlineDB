#!/usr/bin/env bash
# Validate and normalize the official redline-testing evidence bundle through
# the reviewed Rust processor bundled from Redline core.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

root="${1:-target/redline-testing}"
official_evidence="$root/official-evidence.json"
provenance="$root/redline-testing-provenance.env"
processed="$root/official-evidence.processed.json"

if [ ! -s "$official_evidence" ]; then
    printf 'redline-testing evidence processor: missing official evidence %s\n' "$official_evidence" >&2
    exit 1
fi

if [ ! -s "$provenance" ]; then
    printf 'redline-testing evidence processor: missing provenance %s\n' "$provenance" >&2
    exit 1
fi

processor_manifest="$repo_root/tools/evidence-processor/Cargo.toml"
if [ ! -f "$processor_manifest" ] || [ ! -f "$repo_root/tools/evidence-processor/Cargo.lock" ]; then
    printf 'redline-testing evidence processor: bundled Rust processor or lock is missing\n' >&2
    exit 1
fi
bash tools/evidence-processor/run.sh process-official "$root"

if [ ! -s "$processed" ]; then
    printf 'redline-testing evidence processor: canonical processor did not emit %s\n' \
        "$processed" >&2
    exit 1
fi
