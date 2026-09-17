#!/usr/bin/env bash
#
# Language bad-behavior evidence: records the CI / git / release scan surfaces
# the jankurai ci-bad-behavior, git-bad-behavior, and release-bad-behavior
# tools cover for this repo.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

cd "$repo_root"
mkdir -p target/jankurai

log "language-bad-behavior: recording ci/git/release scan receipt"
{
    printf 'ci-bad-behavior: zizmor .github/workflows + pinned action SHAs (ops/ci/security.sh)\n'
    printf 'git-bad-behavior: ops/git-hooks/pre-push + scripts/ci-doctor.sh workflow assertions\n'
    printf 'release-bad-behavior: .github/workflows/release.yml --verify-tag + docs/release.md\n'
} >target/jankurai/language-bad-behavior.log

log "language-bad-behavior: complete"
