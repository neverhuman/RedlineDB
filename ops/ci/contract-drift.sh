#!/usr/bin/env bash
# Contract-drift lane: guards the machine-readable contract surface.
#
# Two surfaces are covered:
#   1. the audit-policy pair — `.jankurai/audit-policy.toml` and its
#      `agent/audit-policy.toml` twin must stay byte-identical, since the
#      two tooling layers read different copies; and
#   2. every committed JSON/JSONL document under `agent/`, `.jankurai/`,
#      and `contracts/` must parse. These carry the owner map, test map,
#      proof lanes, and the C-ABI contract cell when present.
#
# The document count is asserted to be non-zero so an accidental deletion
# of the metadata surface reds the lane instead of trivially passing, and
# the `contracts/` state is always named explicitly (absent, present but
# empty, or populated) so an empty directory can never read as a check.
#
# No Python: `ops/ci/no-python-policy.sh` runs in the required lane and
# rejects any `python*` invocation in an execution surface, so documents
# are parsed with jq.
#
# Usage:
#   bash ops/ci/contract-drift.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

fail() {
    printf 'contract drift: %s\n' "$*" >&2
    exit 1
}

command -v jq >/dev/null 2>&1 \
    || fail "jq is required to parse contract documents"

bash scripts/check_audit_policy_mirror.sh

checked=0
while IFS= read -r doc; do
    [ -f "$doc" ] || continue
    case "$doc" in
        *.json)
            jq -e . "$doc" >/dev/null 2>&1 \
                || fail "malformed JSON contract document: $doc"
            ;;
        *.jsonl)
            while IFS= read -r line; do
                [ -n "${line// /}" ] || continue
                printf '%s' "$line" | jq -e . >/dev/null 2>&1 \
                    || fail "malformed JSONL record in $doc"
            done <"$doc"
            ;;
        *)
            continue
            ;;
    esac
    checked=$((checked + 1))
done < <(git ls-files -- agent .jankurai contracts)

[ "$checked" -gt 0 ] \
    || fail "no committed JSON contract documents found under agent/, .jankurai/, or contracts/"

if [ ! -d contracts ]; then
    printf 'contract drift: contracts/ is absent; no C-ABI contract cell in this checkout\n'
else
    contract_docs="$(git ls-files -- contracts | wc -l)"
    if [ "$contract_docs" -eq 0 ]; then
        printf 'contract drift: contracts/ exists but holds no committed documents\n'
    else
        printf 'contract drift: %s committed document(s) under contracts/\n' "$contract_docs"
    fi
fi

printf 'contract drift ok: %s contract document(s) parsed\n' "$checked"
