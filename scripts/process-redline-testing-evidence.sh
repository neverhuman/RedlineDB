#!/usr/bin/env bash
# Validate and normalize the official redline-testing evidence bundle.
#
# The official gate writes its raw artifacts under target/redline-testing/.
# This helper verifies the JSON contract, recomputes hashes for every declared
# output file, enforces the verified runner SHA, and emits a processed summary
# at target/redline-testing/official-evidence.processed.json.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

root="${1:-target/redline-testing}"
official_evidence="$root/official-evidence.json"
provenance="$root/redline-testing-provenance.env"

if [ ! -s "$official_evidence" ]; then
    printf 'redline-testing evidence processor: missing official evidence %s\n' "$official_evidence" >&2
    exit 1
fi

if [ ! -s "$provenance" ]; then
    printf 'redline-testing evidence processor: missing provenance %s\n' "$provenance" >&2
    exit 1
fi

set -a
# shellcheck source=/dev/null
. "$provenance"
set +a

cargo run --quiet --locked -p redlinedb-bench --bin evidence_processor -- "$root"
