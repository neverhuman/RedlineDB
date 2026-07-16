#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/ops/ci/lib.sh"

fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/redline-testing-jankurai.XXXXXX")"
cleanup() {
    rm -rf -- "$fixture_root"
}
trap cleanup EXIT

write_fixture() {
    local path="$1" version="$2"
    printf '%s\n' '#!/usr/bin/env bash' "printf '%s\\n' '$version'" >"$path"
    chmod 0755 "$path"
}

expect_rejected() {
    local label="$1"
    shift
    if verify_jankurai_identity "$@" >/dev/null 2>&1; then
        printf 'expected governed Jankurai rejection: %s\n' "$label" >&2
        exit 1
    fi
}

correct="$fixture_root/correct"
write_fixture "$correct" "$JANKURAI_VERSION"
correct_sha="$(sha256sum -- "$correct" | awk '{print $1}')"
verify_jankurai_identity "$correct" "$JANKURAI_VERSION" "$correct_sha"

expect_rejected missing "$fixture_root/missing" "$JANKURAI_VERSION" "$correct_sha"

linked="$fixture_root/linked"
ln -s "$correct" "$linked"
expect_rejected symlink "$linked" "$JANKURAI_VERSION" "$correct_sha"

expect_rejected wrong-digest "$correct" "$JANKURAI_VERSION" "$JANKURAI_SHA256"

wrong_version="$fixture_root/wrong-version"
write_fixture "$wrong_version" 'jankurai 1.6.10'
wrong_version_sha="$(sha256sum -- "$wrong_version" | awk '{print $1}')"
expect_rejected wrong-version "$wrong_version" "$JANKURAI_VERSION" "$wrong_version_sha"

printf 'governed Jankurai negative probes ok\n'
