#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_lib="$repo_root/ops/ci/lib.sh"
production_broker="/opt/jain-ci/authority/release-bin/jankurai"
production_governed="/home/ubuntu/.jeryu/bin/jankurai"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-core-governed-jankurai.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT

fail() {
    printf 'governed-jankurai-test: %s\n' "$*" >&2
    exit 1
}

expect_failure() {
    local description="$1" pattern="$2"
    shift 2
    if "$@" >"$tmp/failure.log" 2>&1; then
        fail "$description: command unexpectedly succeeded"
    fi
    grep -Fq "$pattern" "$tmp/failure.log" || {
        sed -n '1,80p' "$tmp/failure.log" >&2
        fail "$description: expected failure text was absent"
    }
}

grep -Fq "$production_broker" "$source_lib" || fail "release broker path is absent"
grep -Fq "$production_governed" "$source_lib" || fail "ordinary governed path is absent"
grep -Fq '96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa' \
    "$source_lib" || fail "protected Tool digest is absent"
grep -Fq '479489f56f42045a71bf0651c3793d82f8689630' \
    "$source_lib" || fail "protected Tool main authority is absent"
grep -Fq 'jeryu-tool-v5.1.0-split.3' \
    "$source_lib" || fail "immutable Tool authority tag is absent"
if grep -Fq 'fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e' \
    "$source_lib"; then
    fail "retired Jankurai digest remains"
fi

mkdir -p "$tmp/broker/bin" "$tmp/attacker/bin" \
    "$tmp/home/.jeryu/bin" "$tmp/home/.jeryu/receipts/jankurai/sha256" \
    "$tmp/home/.local/bin"
governed_source="/usr/local/libexec/jain/jankurai"
[[ -f "$governed_source" && ! -L "$governed_source" && -x "$governed_source" ]] \
    || fail "governed Jankurai test source is unavailable"
[[ "$("$governed_source" --version)" == 'jankurai 1.6.11' ]] \
    || fail "governed Jankurai test source has the wrong version"
[[ "$(sha256sum "$governed_source" | awk '{print $1}')" == \
    '96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa' ]] \
    || fail "governed Jankurai test source has the wrong digest"

broker_bin="$tmp/broker/bin/jankurai"
attacker_bin="$tmp/attacker/bin/jankurai"
ordinary_bin="$tmp/home/.jeryu/bin/jankurai"
older_local_bin="$tmp/home/.local/bin/jankurai"
cp -- "$governed_source" "$broker_bin"
cp -- "$governed_source" "$attacker_bin"
cp -- "$governed_source" "$ordinary_bin"
chmod 0555 "$broker_bin" "$attacker_bin" "$ordinary_bin"
printf '#!/usr/bin/env bash\nprintf "jankurai 1.6.11\\n"\n' >"$older_local_bin"
chmod 0555 "$older_local_bin"

# Exercise the production bytes while substituting only the two fixed paths
# inside this automatically removed hostile fixture.
test_lib="$tmp/lib.sh"
sed -e "s#$production_broker#$broker_bin#g" \
    -e "s#$production_governed#$ordinary_bin#g" \
    "$source_lib" >"$test_lib"
# The disposable broker fixture is owned by the invoking test user. Keep the
# existing identity/mode/link hostiles scoped to that fixture; exact root
# custody is exercised separately below against both a real root-owned binary
# and an exact-byte user-owned expected-path hostile.
fixture_owner="$(stat -c '%u:%g' -- "$broker_bin")"
sed -i "s#0:0:555:1#${fixture_owner}:555:1#g" "$test_lib"
# shellcheck source=ops/ci/lib.sh
source "$test_lib"

receipt_tmp="$tmp/receipt.json"
jq -n \
    --arg remote "$JERYU_JANKURAI_SOURCE_REPO" \
    --arg commit "$JERYU_JANKURAI_SOURCE_REV" \
    --arg tag "$JERYU_JANKURAI_SOURCE_TAG" \
    --arg tree "$JERYU_JANKURAI_SOURCE_TREE" \
    --arg archive "$JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256" \
    --arg lock "$JERYU_JANKURAI_CARGO_LOCK_SHA256" \
    --arg rustc "$JERYU_JANKURAI_RUSTC_VERSION" \
    --arg cargo "$JERYU_JANKURAI_CARGO_VERSION" \
    --arg triple "$JERYU_JANKURAI_TARGET_TRIPLE" \
    --arg mode "$JERYU_JANKURAI_BUILD_MODE" \
    --arg digest "$JERYU_JANKURAI_SHA256" \
    --arg version "$JERYU_JANKURAI_VERSION" \
    --arg path "$ordinary_bin" \
    --arg manifest_repo "$JERYU_TOOL_AUTHORITY_REPO" \
    --arg manifest_sha "$JERYU_TOOL_MANIFEST_SHA256" \
    '{schema:"jeryu.jankurai-installation/v1",
      source:{remote:$remote,commit:$commit,tag:$tag,tree:$tree,
        archive_sha256:$archive,cargo_lock_sha256:$lock,
        verification:"release-authoritative"},
      build:{rustc:$rustc,cargo:$cargo,target_triple:$triple,mode:$mode,
        cargo_net_offline:true,dedicated_cargo_home:true,
        git_global_config_disabled:true,git_system_config_disabled:true,
        git_http_follow_redirects:false,git_terminal_prompt:false,
        jankurai_update_check:false,
        network_scope:"local-forge-source-plus-offline-cargo",
        no_proxy:"127.0.0.1,localhost,::1"},
      governance:{status:"governed",
        manifest_repo:$manifest_repo,
        manifest_commit:("a"*40),manifest_tree:("b"*40),
        manifest_sha256:$manifest_sha,protected_main:true,
        protection_policy:"immutable-main-v1"},
      binary:{sha256:$digest,version_output:$version},
      installation:{path:$path,atomic:true},test_mode:false,
      conclusion:"success"}' >"$receipt_tmp"
receipt_sha="$(sha256sum "$receipt_tmp" | awk '{print $1}')"
mv -- "$receipt_tmp" \
    "$tmp/home/.jeryu/receipts/jankurai/sha256/$receipt_sha.json"

ordinary_command='source "$1"; require_jankurai; [[ "$JERYU_GOVERNED_JANKURAI_BIN" == "$2" ]]'
env -i HOME="$tmp/home" \
    PATH="$tmp/home/.local/bin:$tmp/home/.jeryu/bin:/usr/bin:/bin" \
    bash -c "$ordinary_command" bash "$test_lib" "$ordinary_bin"

run_release_broker() {
    local path="$1"
    shift
    local command_text
    command_text='source "$1"; require_jankurai; [[ "$JERYU_GOVERNED_JANKURAI_BIN" == "$2" ]]'
    env -i HOME="$tmp/home" PATH="$path:/usr/bin:/bin" JAIN_RELEASE_CI=1 "$@" \
        bash -c "$command_text" bash "$test_lib" "$broker_bin"
}

run_release_broker_with() {
    local library="$1" expected_bin="$2" path="$3"
    shift 3
    local command_text
    command_text='source "$1"; require_jankurai; [[ "$JERYU_GOVERNED_JANKURAI_BIN" == "$2" ]]'
    env -i HOME="$tmp/home" PATH="$path:/usr/bin:/bin" JAIN_RELEASE_CI=1 "$@" \
        bash -c "$command_text" bash "$library" "$expected_bin"
}

root_test_lib="$tmp/root-lib.sh"
sed -e "s#$production_broker#$governed_source#g" \
    -e "s#$production_governed#$ordinary_bin#g" \
    "$source_lib" >"$root_test_lib"
run_release_broker_with "$root_test_lib" "$governed_source" \
    "$(dirname "$governed_source")"

user_owned_test_lib="$tmp/user-owned-lib.sh"
sed -e "s#$production_broker#$broker_bin#g" \
    -e "s#$production_governed#$ordinary_bin#g" \
    "$source_lib" >"$user_owned_test_lib"
expect_failure "user-owned exact-byte expected-path broker" \
    "release broker Jankurai custody mismatch" \
    run_release_broker_with "$user_owned_test_lib" "$broker_bin" "$tmp/broker/bin"

run_release_broker "$tmp/broker/bin"
run_release_broker "$tmp/broker/bin" \
    JERYU_GOVERNED_JANKURAI_BIN="$attacker_bin" \
    JERYU_JANKURAI_BIN="$attacker_bin"
expect_failure "caller receipt substitution" \
    "release broker Jankurai rejects caller receipt authority" \
    run_release_broker "$tmp/broker/bin" \
    JERYU_JANKURAI_RECEIPT="$tmp/caller.json" \
    JERYU_JANKURAI_RECEIPT_SHA256="$(printf 'a%.0s' {1..64})" \
    JERYU_JANKURAI_ALLOW_TEST_RECEIPT=1
expect_failure "ambient home auditor" "release broker Jankurai path mismatch" \
    run_release_broker "$tmp/home/.jeryu/bin"
expect_failure "caller PATH substitution" "release broker Jankurai path mismatch" \
    run_release_broker "$tmp/attacker/bin"
expect_failure "missing broker" "release broker Jankurai path mismatch" \
    run_release_broker "/usr/bin:/bin"

cp -- "$broker_bin" "$tmp/broker-backup"
chmod 0755 "$broker_bin"
printf '#!/usr/bin/env bash\nprintf "jankurai 1.6.11\\n"\n' >"$broker_bin"
chmod 0555 "$broker_bin"
expect_failure "wrong broker identity" "governed jankurai identity mismatch" \
    run_release_broker "$tmp/broker/bin"
rm -- "$broker_bin"
mv -- "$tmp/broker-backup" "$broker_bin"
chmod 0555 "$broker_bin"

chmod 0755 "$broker_bin"
expect_failure "writable broker" "release broker Jankurai custody mismatch" \
    run_release_broker "$tmp/broker/bin"
chmod 0555 "$broker_bin"

ln "$broker_bin" "$tmp/broker/bin/jankurai-linked"
expect_failure "hard-linked broker" "release broker Jankurai custody mismatch" \
    run_release_broker "$tmp/broker/bin"
rm -- "$tmp/broker/bin/jankurai-linked"

mv -- "$broker_bin" "$tmp/physical-broker"
ln -s -- "$tmp/physical-broker" "$broker_bin"
expect_failure "symlinked broker" "governed jankurai must be" \
    run_release_broker "$tmp/broker/bin"

printf 'governed Jankurai hostiles passed: ordinary broker caller shadow identity custody\n'
