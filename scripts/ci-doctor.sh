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

ci_workflow="$repo_root/.github/workflows/ci.yml"
release_workflow="$repo_root/.github/workflows/release.yml"

require_file "$ci_workflow" "github ci workflow is missing"
require_file "$release_workflow" "github release workflow is missing"

grep -q 'pull_request:' "$ci_workflow" || fail "github ci workflow must support pull requests"
grep -q 'workflow_dispatch' "$ci_workflow" || fail "github ci workflow must support workflow_dispatch"
grep -q 'merge_group' "$ci_workflow" || fail "github ci workflow must support merge_group"
grep -q 'branches: \[main\]' "$ci_workflow" || fail "github ci workflow must run on main pushes"
grep -q 'scripts/ci-local.sh pr-ci' "$ci_workflow" || fail "github ci workflow must run the pr-ci lane"
grep -q 'beyond-postgres:' "$ci_workflow" || fail "github ci workflow must keep the beyond-postgres lane"
grep -q 'python3 scripts/update-badge.py' "$ci_workflow" || fail "github ci workflow must keep the badge update lane"
grep -q 'actions/upload-artifact' "$ci_workflow" || fail "github ci workflow must upload the beyond-SQLite artifact"
grep -q 'workflow_dispatch' "$release_workflow" || fail "github release workflow must support workflow_dispatch"
grep -q 'tags:' "$release_workflow" || fail "github release workflow must trigger on tags"
grep -q 'scripts/ci-local.sh pr-ci' "$release_workflow" || fail "github release workflow must run the pr-ci lane"
grep -q 'scripts/ci-local.sh release' "$release_workflow" || fail "github release workflow must run the release lane"
grep -q 'actions/attest-build-provenance' "$release_workflow" || fail "github release workflow must attest release artifacts"
grep -q 'gh release create' "$release_workflow" || fail "github release workflow must publish a GitHub release"
grep -q -- '--verify-tag' "$release_workflow" || fail "github release workflow must bind the release to a verified tag (--verify-tag)"
grep -q 'ops/ci/pr-ci.sh' "$repo_root/scripts/ci-local.sh" || fail "ci-local must dispatch to ops/ci/pr-ci.sh"
grep -q 'ops/ci/release.sh' "$repo_root/scripts/ci-local.sh" || fail "ci-local must dispatch to ops/ci/release.sh"
