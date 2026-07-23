#!/usr/bin/env bash
# Exact cross-version storage-format qualification.
#
# The predecessor is built from the immutable 4.1.0-jain.5 commit in an
# automatically removed no-local standalone clone. The gate proves both
# upgrade/migration and rollback writer compatibility, then proves a future
# generation cannot reach the engine open path.

set -euo pipefail

readonly LEGACY_COMMIT="2924a34bdca8263adc9ebff9220f5bb99ba4323f"
readonly LEGACY_TREE="cd6846436aa051fd1c1ab3671138a5ea1fe2b1db"
readonly GENERATION_ONE=$'redlinedb-storage-format/v1\ngeneration=1\n'

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-storage-compat.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT

legacy_checkout="$tmp/redline-core-4.1"
legacy_target="$tmp/legacy-target"
legacy_database="$tmp/legacy.redline"
current_database="$tmp/current.redline"
future_database="$tmp/future.redline"

git clone -q --no-local --no-checkout "$repo_root" "$legacy_checkout"
git -C "$legacy_checkout" checkout -q --detach "$LEGACY_COMMIT"
[[ "$(git -C "$legacy_checkout" rev-parse 'HEAD^{commit}')" == "$LEGACY_COMMIT" ]]
[[ "$(git -C "$legacy_checkout" rev-parse 'HEAD^{tree}')" == "$LEGACY_TREE" ]]
[[ ! -e "$legacy_checkout/.git/objects/info/alternates" ]]
[[ -z "$(find "$legacy_checkout" -type l -print -quit)" ]]

export CARGO_NET_OFFLINE=true
CARGO_TARGET_DIR="$legacy_target" \
    cargo build --quiet --locked --offline --manifest-path "$legacy_checkout/Cargo.toml" \
        -p redlinedb-cli --bin redlinedb-cli
cargo build --quiet --locked --offline -p redlinedb-cli --bin redlinedb-cli

legacy_bin="$legacy_target/debug/redlinedb-cli"
current_bin="$repo_root/target/debug/redlinedb-cli"

legacy_value="$($legacy_bin "$legacy_database" \
    "CREATE TABLE values_v1(value INTEGER); INSERT INTO values_v1 VALUES (41); SELECT value FROM values_v1;")"
[[ "$legacy_value" == "41" ]]
[[ ! -e "$legacy_database/STORAGE_FORMAT" ]]

upgraded_value="$($current_bin "$legacy_database" "SELECT value + 1 FROM values_v1;")"
[[ "$upgraded_value" == "42" ]]
[[ "$(<"$legacy_database/STORAGE_FORMAT")"$'\n' == "$GENERATION_ONE" ]]

current_value="$($current_bin "$current_database" \
    "CREATE TABLE rollback_v1(value INTEGER); INSERT INTO rollback_v1 VALUES (11); SELECT value FROM rollback_v1;")"
[[ "$current_value" == "11" ]]
rollback_value="$($legacy_bin "$current_database" \
    "UPDATE rollback_v1 SET value = value + 1; SELECT value FROM rollback_v1;")"
[[ "$rollback_value" == "12" ]]
same_digest_value="$($current_bin "$current_database" "SELECT value FROM rollback_v1;")"
[[ "$same_digest_value" == "12" ]]

$current_bin "$future_database" "CREATE TABLE future_v2(value INTEGER);" >/dev/null
printf 'redlinedb-storage-format/v1\ngeneration=2\n' >"$future_database/STORAGE_FORMAT"
if $current_bin "$future_database" "SELECT value FROM future_v2;" \
    >"$tmp/future.stdout" 2>"$tmp/future.stderr"; then
    printf 'future storage generation unexpectedly opened\n' >&2
    exit 1
fi
grep -Fq 'unsupported storage format generation 2' "$tmp/future.stderr"

printf 'storage-format compatibility passed: 4.1 upgrade migration rollback future-rejection\n'
