#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# shellcheck source=ops/ci/lib.sh
. ops/ci/lib.sh

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/redline-governed-jankurai.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

expect_rejected() {
    local name="$1"
    shift
    if "$@" >"$tmp_dir/$name.log" 2>&1; then
        printf 'negative probe unexpectedly accepted: %s\n' "$name" >&2
        return 1
    fi
    printf 'negative probe rejected: %s\n' "$name"
}

expect_rejected missing \
    ci_validate_jankurai_binary \
    "$tmp_dir/missing" "$CI_JANKURAI_VERSION" "$CI_JANKURAI_SHA256"

ln -s "$CI_JANKURAI_BIN" "$tmp_dir/symlinked"
expect_rejected symlink \
    ci_validate_jankurai_binary \
    "$tmp_dir/symlinked" "$CI_JANKURAI_VERSION" "$CI_JANKURAI_SHA256"
rm -f "$tmp_dir/symlinked"

cp "$CI_JANKURAI_BIN" "$tmp_dir/wrong-digest"
chmod 0755 "$tmp_dir/wrong-digest"
expect_rejected wrong-digest \
    ci_validate_jankurai_binary \
    "$tmp_dir/wrong-digest" "$CI_JANKURAI_VERSION" \
    "0000000000000000000000000000000000000000000000000000000000000000"

cp /usr/bin/true "$tmp_dir/wrong-version"
chmod 0755 "$tmp_dir/wrong-version"
wrong_version_sha="$(sha256sum "$tmp_dir/wrong-version" | awk '{print $1}')"
expect_rejected wrong-version \
    ci_validate_jankurai_binary \
    "$tmp_dir/wrong-version" "$CI_JANKURAI_VERSION" "$wrong_version_sha"

mkdir -p "$tmp_dir/hostile-bin"
printf '#!/usr/bin/env bash\nprintf "hostile PATH jankurai\\n"\n' \
    > "$tmp_dir/hostile-bin/jankurai"
chmod 0755 "$tmp_dir/hostile-bin/jankurai"
PATH="$tmp_dir/hostile-bin:/usr/bin:/bin"
export PATH
ci_require_governed_jankurai >/dev/null
[ "$(type -t jankurai)" = "function" ]
[ "$(jankurai --version)" = "jankurai $CI_JANKURAI_VERSION" ]

printf 'governed Jankurai hostile probes passed\n'
