#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fail() {
    printf 'ci doctor: %s\n' "$1" >&2
    exit 1
}

require_file() {
    local path="$1"
    local description="$2"
    [ -f "$path" ] || fail "$description"
}

release_workflow="$repo_root/.github/workflows/release.yml"
[ ! -e "$release_workflow" ] || fail "authoritative release workflow must not publish through GitHub"
require_file "$repo_root/ops/ci/pr-ci.sh" "local pr-ci lane is missing"
require_file "$repo_root/ops/ci/release.sh" "local release lane is missing"
require_file "$repo_root/contracts/compatibility-v1.toml" "compatibility contract is missing"
grep -q -- '--offline' "$repo_root/scripts/setup.sh" || fail "setup must build offline"
grep -q 'custody-stage' "$repo_root/docs/release.md" || fail "release docs must require custody"
grep -Fq 'required|pr-ci)' "$repo_root/scripts/ci-local.sh" || fail "ci-local must alias required to pr-ci"
grep -q 'ops/ci/pr-ci.sh' "$repo_root/scripts/ci-local.sh" || fail "ci-local must dispatch to ops/ci/pr-ci.sh"
grep -q 'ops/ci/release.sh' "$repo_root/scripts/ci-local.sh" || fail "ci-local must dispatch to ops/ci/release.sh"
