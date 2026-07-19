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

expect_rejected hostile-source-selection \
    /usr/bin/env PATH="$tmp_dir/hostile-bin:/usr/bin:/bin" \
    /usr/bin/bash -c \
    'set -euo pipefail; . "$1"; ci_require_governed_jankurai' \
    _ "$repo_root/ops/ci/lib.sh"

mkdir -p "$tmp_dir/empty-bin"
expect_rejected missing-source-selection \
    /usr/bin/env PATH="$tmp_dir/empty-bin:/usr/bin:/bin" \
    /usr/bin/bash -c \
    'set -euo pipefail; . "$1"; ci_require_governed_jankurai' \
    _ "$repo_root/ops/ci/lib.sh"

PATH="$tmp_dir/hostile-bin:/usr/bin:/bin"
export PATH
ci_require_governed_jankurai >/dev/null
[ "$(type -t jankurai)" = "function" ]
[ "$(jankurai --version)" = "jankurai $CI_JANKURAI_VERSION" ]

mkdir -p "$tmp_dir/dispatch-bin"
cat > "$tmp_dir/dispatch-bin/bash" <<'DISPATCH_STUB'
#!/usr/bin/bash
printf '<%s>\n' "$@"
DISPATCH_STUB
chmod 0755 "$tmp_dir/dispatch-bin/bash"

expect_dispatch() {
    local lane="$1"
    shift
    local actual expected
    actual="$(
        PATH="$tmp_dir/dispatch-bin:/usr/bin:/bin" \
            /usr/bin/bash "$repo_root/scripts/ci-local.sh" "$lane"
    )"
    expected="$(printf '<%s>\n' "$@")"
    if [ "$actual" != "$expected" ]; then
        printf 'ci-local dispatch mismatch for %s: expected %s, got %s\n' \
            "$lane" "$expected" "$actual" >&2
        return 1
    fi
}

expect_dispatch security "$repo_root/tools/security-lane.sh"
expect_dispatch score "$repo_root/scripts/just/run.sh" score
expect_dispatch contract-drift \
    "$repo_root/ops/ci/jankurai-tools.sh" contract-drift
expect_dispatch artifact-support "$repo_root/ops/ci/artifact_support.sh"

if PATH="$tmp_dir/dispatch-bin:/usr/bin:/bin" \
    /usr/bin/bash "$repo_root/scripts/ci-local.sh" unknown \
    >"$tmp_dir/unknown-dispatch.log" 2>&1
then
    printf 'ci-local accepted an unknown lane\n' >&2
    exit 1
fi
grep -Fq 'contract-drift|artifact-support' "$tmp_dir/unknown-dispatch.log"

if grep -Fq '/home/ubuntu/.jeryu/bin/jankurai' "$repo_root/ops/ci/lib.sh"; then
    printf 'governed Jankurai selection still depends on the user home\n' >&2
    exit 1
fi

printf 'governed Jankurai hostile probes passed\n'
