#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# shellcheck source=ops/ci/lib.sh
. ops/ci/lib.sh

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/redline-governed-jankurai.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

actionlint_bin="$(command -v actionlint || true)"
if [ -z "$actionlint_bin" ] || [ ! -x "$actionlint_bin" ]; then
    printf 'actionlint is required to prove hostile workflow fixtures are valid\n' >&2
    exit 1
fi

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

bash "$repo_root/ops/ci/github-mirror-contract.sh"
for hostile_workflow in \
    runtime-jankurai \
    indirect-audit \
    floating-install \
    privileged-install \
    self-hosted \
    bypass \
    just-local-authority \
    just-test \
    just-check \
    security-script \
    ci-local-score \
    run-sh-score \
    bash-c-local-authority \
    sh-c-security \
    env-audit \
    renamed-workflow \
    renamed-step \
    reordered-steps \
    extra-action \
    removed-step; do
    cp "$repo_root/.github/workflows/jankurai.yml" \
        "$tmp_dir/$hostile_workflow.yml"
done
printf '%s\n' '      - run: jankurai audit .' \
    >> "$tmp_dir/runtime-jankurai.yml"
printf '%s\n' '      - run: bash ops/ci/jankurai-audit.sh' \
    >> "$tmp_dir/indirect-audit.yml"
printf '%s\n' '      - run: cargo install jankurai' \
    >> "$tmp_dir/floating-install.yml"
printf '%s\n' '      - run: sudo install jankurai /usr/local/bin/jankurai' \
    >> "$tmp_dir/privileged-install.yml"
sed -i 's/runs-on: ubuntu-24.04/runs-on: self-hosted/' \
    "$tmp_dir/self-hosted.yml"
printf '%s\n' '        continue-on-error: true' \
    >> "$tmp_dir/bypass.yml"
printf '%s\n' '      - run: just jankurai-local-authority' \
    >> "$tmp_dir/just-local-authority.yml"
printf '%s\n' '      - run: just test' \
    >> "$tmp_dir/just-test.yml"
printf '%s\n' '      - run: just check' \
    >> "$tmp_dir/just-check.yml"
printf '%s\n' '      - run: bash tools/security-lane.sh' \
    >> "$tmp_dir/security-script.yml"
printf '%s\n' '      - run: bash scripts/ci-local.sh score' \
    >> "$tmp_dir/ci-local-score.yml"
printf '%s\n' '      - run: bash scripts/just/run.sh score' \
    >> "$tmp_dir/run-sh-score.yml"
printf '%s\n' "      - run: bash -c 'just jankurai-local-authority'" \
    >> "$tmp_dir/bash-c-local-authority.yml"
printf '%s\n' "      - run: sh -c 'bash tools/security-lane.sh'" \
    >> "$tmp_dir/sh-c-security.yml"
printf '%s\n' '      - run: env bash ops/ci/jankurai-audit.sh' \
    >> "$tmp_dir/env-audit.yml"
sed -i \
    's/name: redline-hub-jankurai-static-mirror/name: renamed-static-mirror/' \
    "$tmp_dir/renamed-workflow.yml"
sed -i \
    's/name: Reject release-authority drift/name: Renamed static step/' \
    "$tmp_dir/renamed-step.yml"
{
    sed -n '1,23p' "$repo_root/.github/workflows/jankurai.yml"
    sed -n '28,29p' "$repo_root/.github/workflows/jankurai.yml"
    sed -n '24,27p' "$repo_root/.github/workflows/jankurai.yml"
} > "$tmp_dir/reordered-steps.yml"
printf '%s\n' \
    '      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020' \
    >> "$tmp_dir/extra-action.yml"
sed -i '28,29d' "$tmp_dir/removed-step.yml"
for hostile_workflow in \
    runtime-jankurai \
    indirect-audit \
    floating-install \
    privileged-install \
    self-hosted \
    bypass \
    just-local-authority \
    just-test \
    just-check \
    security-script \
    ci-local-score \
    run-sh-score \
    bash-c-local-authority \
    sh-c-security \
    env-audit \
    renamed-workflow \
    renamed-step \
    reordered-steps \
    extra-action \
    removed-step; do
    "$actionlint_bin" "$tmp_dir/$hostile_workflow.yml"
    expect_rejected "github-$hostile_workflow" \
        bash "$repo_root/ops/ci/github-mirror-contract.sh" \
        "$tmp_dir/$hostile_workflow.yml"
done

if grep -Fq '/home/ubuntu/.jeryu/bin/jankurai' "$repo_root/ops/ci/lib.sh"; then
    printf 'governed Jankurai selection still depends on the user home\n' >&2
    exit 1
fi

printf 'governed Jankurai hostile probes passed\n'
