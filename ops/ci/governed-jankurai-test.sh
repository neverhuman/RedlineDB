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

expect_command_rejected() {
    local label="$1"
    shift
    if "$@" >"$fixture_root/$label.log" 2>&1; then
        printf 'expected governed Jankurai command rejection: %s\n' "$label" >&2
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

mkdir -p "$fixture_root/hostile-bin"
write_fixture "$fixture_root/hostile-bin/jankurai" 'jankurai 1.6.11-hostile'
expect_command_rejected hostile-source-selection \
    /usr/bin/env PATH="$fixture_root/hostile-bin:/usr/bin:/bin" \
    /usr/bin/bash -c \
    'set -euo pipefail; . "$1"; require_jankurai' \
    _ "$repo_root/ops/ci/lib.sh"

mkdir -p "$fixture_root/empty-bin"
expect_command_rejected missing-source-selection \
    /usr/bin/env PATH="$fixture_root/empty-bin:/usr/bin:/bin" \
    /usr/bin/bash -c \
    'set -euo pipefail; . "$1"; require_jankurai' \
    _ "$repo_root/ops/ci/lib.sh"

PATH="$fixture_root/hostile-bin:/usr/bin:/bin"
export PATH
require_jankurai
[ "$(type -t jankurai)" = function ]
[ "$(jankurai --version)" = "$JANKURAI_VERSION" ]

if grep -Fq '/home/ubuntu/.jeryu/bin/jankurai' "$repo_root/ops/ci/lib.sh"; then
    printf 'governed Jankurai selection still depends on the user home\n' >&2
    exit 1
fi

printf 'governed Jankurai negative probes ok\n'
