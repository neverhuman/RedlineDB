#!/usr/bin/env bash
# Prove the public GitHub workflow remains a stock-runner static mirror and
# cannot impersonate the local Jeryu release gate.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
workflow="${1:-$repo_root/.github/workflows/jankurai.yml}"

if [ ! -f "$workflow" ] || [ -L "$workflow" ]; then
    printf 'GitHub static mirror must be a regular non-symlink file: %s\n' "$workflow" >&2
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

forbidden_patterns=(
    'runs-on:.*self-hosted'
    'ci_require_governed_jankurai'
    'ops/ci/jankurai-audit\.sh'
    'scripts/ci-local\.sh'
    'scripts/just/run\.sh[[:space:]]+score'
    '(^|[[:space:]])just[[:space:]]+(score|security|release)'
    '(^|[[:space:]/])jankurai[[:space:]]+(--version|audit|security|proof|proofbind|proofmark|doctor|copy-code|rust|ux)'
    '(cargo|apt|apt-get|npm|pnpm|yarn)[[:space:]]+install'
    '(curl[[:space:]]|wget[[:space:]]|git[[:space:]]+clone)'
    'rustup[[:space:]]+toolchain[[:space:]]+install'
    '(^|[[:space:]])sudo[[:space:]]'
    '^[[:space:]]+(container|services):'
    'continue-on-error:'
)
for pattern in "${forbidden_patterns[@]}"; do
    if grep -Eiq "$pattern" "$workflow"; then
        printf 'GitHub static mirror contains forbidden release/provisioning surface: %s\n' \
            "$pattern" >&2
        exit 1
    fi
done

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

cmp -s "$repo_root/agent/audit-policy.toml" "$repo_root/.jankurai/audit-policy.toml" || {
    printf 'Jankurai audit policy mirrors differ\n' >&2
    exit 1
}
for rule in HLT-000-SCORE-DIMENSION HLT-048-CANONICAL-CI-GAP; do
    grep -Fq "\"$rule\"" "$repo_root/agent/audit-policy.toml" || {
        printf 'Jankurai local-authority exception is missing rule: %s\n' "$rule" >&2
        exit 1
    }
done
if [ ! -f "$repo_root/docs/exceptions/jankurai-local-ci-authority.md" ] \
    || [ -L "$repo_root/docs/exceptions/jankurai-local-ci-authority.md" ]; then
    printf 'Jankurai local-authority exception document is missing or a symlink\n' >&2
    exit 1
fi

printf 'GitHub static mirror contract passed: local Jeryu remains release authority\n'
