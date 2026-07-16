#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/redline-testing-rustsec.XXXXXX")"
cleanup() {
    rm -rf -- "$fixture_root"
}
trap cleanup EXIT

db="$fixture_root/db"
git init -q "$db"
git -C "$db" config user.name redline-testing-ci
git -C "$db" config user.email redline-testing-ci@invalid
printf 'fixture\n' >"$db/advisory.md"
git -C "$db" add advisory.md
git -C "$db" commit -q -m fixture
head="$(git -C "$db" rev-parse HEAD)"
verify_rustsec_db_identity "$db" "$head"

expect_rejected() {
    local label="$1"
    shift
    if verify_rustsec_db_identity "$@" >/dev/null 2>&1; then
        printf 'expected RustSec database rejection: %s\n' "$label" >&2
        exit 1
    fi
}

expect_rejected missing "$fixture_root/missing" "$head"

linked="$fixture_root/linked"
ln -s "$db" "$linked"
expect_rejected symlink "$linked" "$head"

expect_rejected wrong-commit "$db" 0000000000000000000000000000000000000000

printf 'dirty\n' >"$db/untracked"
expect_rejected dirty "$db" "$head"
rm -f "$db/untracked"

printf 'governed security input negative probes ok\n'
