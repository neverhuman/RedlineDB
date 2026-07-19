#!/usr/bin/env bash
# Prove the public GitHub workflow remains a stock-runner static mirror and
# cannot impersonate the local Jeryu release gate. The entire workflow is an
# exact allowlisted document; structural spot checks below describe the locked
# shape but are never used as a substitute for the whole-document digest.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
workflow="${1:-$repo_root/.github/workflows/jankurai.yml}"

if [ ! -f "$workflow" ] || [ -L "$workflow" ]; then
    printf 'GitHub static mirror must be a regular non-symlink file: %s\n' "$workflow" >&2
    exit 1
fi

readonly expected_workflow_sha256="43d47d2722c2f1db572e896f9d67d48a259cca70f65234fc9684dce177b31b2b"
readonly expected_workflow_bytes="858"
readonly expected_workflow_lines="29"
actual_workflow_sha256="$(sha256sum -- "$workflow" | awk '{print $1}')"
actual_workflow_bytes="$(wc -c < "$workflow" | tr -d '[:space:]')"
actual_workflow_lines="$(wc -l < "$workflow" | tr -d '[:space:]')"

if [ "$actual_workflow_sha256" != "$expected_workflow_sha256" ] \
    || [ "$actual_workflow_bytes" != "$expected_workflow_bytes" ] \
    || [ "$actual_workflow_lines" != "$expected_workflow_lines" ]; then
    printf 'GitHub static mirror differs from the exact allowlisted document: expected sha256=%s bytes=%s lines=%s; got sha256=%s bytes=%s lines=%s\n' \
        "$expected_workflow_sha256" \
        "$expected_workflow_bytes" \
        "$expected_workflow_lines" \
        "$actual_workflow_sha256" \
        "$actual_workflow_bytes" \
        "$actual_workflow_lines" >&2
    exit 1
fi

require_line() {
    local expected="$1"
    grep -Fqx "$expected" "$workflow" || {
        printf 'GitHub static mirror is missing required line: %s\n' "$expected" >&2
        exit 1
    }
}

require_line '    runs-on: ubuntu-24.04'
require_line '        uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd'
require_line '          persist-credentials: false'
require_line '        run: bash ops/ci/github-mirror-contract.sh'

uses_count="$(grep -Ec '^[[:space:]]+uses:' "$workflow" || true)"
[ "$uses_count" = 1 ] || {
    printf 'GitHub static mirror must contain exactly one pinned checkout action, found %s actions\n' \
        "$uses_count" >&2
    exit 1
}

run_count="$(grep -Ec '^[[:space:]]+run:' "$workflow" || true)"
[ "$run_count" = 1 ] || {
    printf 'GitHub static mirror must contain exactly one static run step, found %s\n' \
        "$run_count" >&2
    exit 1
}

step_count="$(grep -Ec '^[[:space:]]+- name:' "$workflow" || true)"
[ "$step_count" = 2 ] || {
    printf 'GitHub static mirror must contain exactly two ordered steps, found %s\n' \
        "$step_count" >&2
    exit 1
}

for script in \
    ops/ci/lib.sh \
    ops/ci/jankurai-audit.sh \
    ops/ci/governed-jankurai-test.sh \
    scripts/ci-local.sh; do
    bash -n "$repo_root/$script"
done
jq empty \
    "$repo_root/agent/test-map.json" \
    "$repo_root/agent/owner-map.json" \
    "$repo_root/schemas/governed-jankurai-evidence.schema.json"

if ! grep -Fqx 'jankurai-local-authority:' "$repo_root/justfile" \
    || ! grep -Fqx '  bash ops/ci/jankurai-audit.sh' "$repo_root/justfile" \
    || ! grep -Fqx '  test -s target/jankurai/repo-score.json' "$repo_root/justfile"; then
    printf 'governed local Jankurai authority recipe is missing or incomplete\n' >&2
    exit 1
fi

printf 'GitHub static mirror contract passed: local Jeryu remains release authority\n'
