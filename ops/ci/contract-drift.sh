#!/usr/bin/env bash
#
# Contract-drift lane: guards the published contract surface of the
# release artifacts.
#
# Two surfaces are covered:
#   1. the audit-policy pair — `.jankurai/audit-policy.toml` and its
#      `agent/audit-policy.toml` twin must stay byte-identical, since the
#      two tooling layers read different copies; and
#   2. the schemas/ documents shipped inside the release tarball
#      (scripts/release-package.sh copies them), which RedlineDB CI reads
#      when it verifies downloaded reference evidence. Each must parse and
#      each `*.schema.json` must declare `$schema`.
#
# Every committed JSON/JSONL document under `agent/` and `.jankurai/` is
# parsed as well, and the count is asserted non-zero, so an accidental
# deletion of the metadata surface reds the lane instead of passing.
#
# The report is read with jq rather than Python, matching the sibling
# redlineDB lanes and keeping this repo Rust-and-shell only.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"

has jq || fail "jq is required to parse contract documents"

ci_run scripts/check_audit_policy_mirror.sh

[ -d schemas ] || fail "missing contract surface: schemas/"

schema_docs=0
while IFS= read -r doc; do
    [ -f "$doc" ] || continue
    case "$doc" in
        *.json) ;;
        *) continue ;;
    esac
    jq -e . "$doc" >/dev/null 2>&1 \
        || fail "malformed JSON schema document: $doc"
    case "$doc" in
        *.schema.json)
            jq -e 'has("$schema")' "$doc" >/dev/null 2>&1 \
                || fail "$doc declares no \$schema"
            ;;
    esac
    schema_docs=$((schema_docs + 1))
done < <(git ls-files -- schemas)

[ "$schema_docs" -gt 0 ] \
    || fail "schemas/ holds no committed JSON documents"

metadata_docs=0
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
    metadata_docs=$((metadata_docs + 1))
done < <(git ls-files -- agent .jankurai)

[ "$metadata_docs" -gt 0 ] \
    || fail "no committed JSON contract documents found under agent/ or .jankurai/"

log "contract-drift: ${schema_docs} schema document(s), ${metadata_docs} metadata document(s) parsed"
