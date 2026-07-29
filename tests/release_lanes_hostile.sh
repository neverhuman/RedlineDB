#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_identity="$repo_root/ops/ci/source-identity.sh"
tag_identity="$repo_root/ops/ci/release-tag-identity.sh"
inventory="$repo_root/ops/ci/verify-release-inventory.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

expect_failure() {
    if "$@" >"$scratch/expected-failure.log" 2>&1; then
        printf 'hostile release lane unexpectedly passed: %q' "$1" >&2
        printf ' %q' "${@:2}" >&2
        printf '\n' >&2
        exit 1
    fi
}

fixture="$scratch/source"
mkdir -p "$fixture"
git -C "$fixture" init -q
git -C "$fixture" config user.name "Redline Testing Hostile Fixture"
git -C "$fixture" config user.email "fixture@invalid"
printf '[package]\nname = "redline-testing"\nversion = "1.0.1"\n' >"$fixture/Cargo.toml"
printf 'alpha\n' >"$fixture/source.txt"
git -C "$fixture" add Cargo.toml source.txt
git -C "$fixture" commit -q -m initial
initial_head="$(git -C "$fixture" rev-parse HEAD)"
identity_receipt="$scratch/source-identity.json"

"$source_identity" snapshot "$identity_receipt" "$fixture" >/dev/null
"$source_identity" verify "$identity_receipt" "$fixture" >/dev/null

printf 'dirty\n' >>"$fixture/source.txt"
expect_failure "$source_identity" verify "$identity_receipt" "$fixture"
git -C "$fixture" restore source.txt

printf 'untracked\n' >"$fixture/untracked.txt"
expect_failure "$source_identity" verify "$identity_receipt" "$fixture"
rm "$fixture/untracked.txt"

printf 'successor\n' >>"$fixture/source.txt"
git -C "$fixture" add source.txt
git -C "$fixture" commit -q -m successor
expect_failure "$source_identity" verify "$identity_receipt" "$fixture"

tag="redline-testing-v1.0.1-jain.2"
tag_receipt="$scratch/tag-identity.json"
"$tag_identity" "$tag" "$tag_receipt" "$fixture" >/dev/null
jq -e '.tag_state == "planned"' "$tag_receipt" >/dev/null
git -C "$fixture" tag "$tag" "$initial_head"
expect_failure "$tag_identity" "$tag" "$tag_receipt" "$fixture"
git -C "$fixture" tag -d "$tag" >/dev/null
git -C "$fixture" tag "$tag"
"$tag_identity" "$tag" "$tag_receipt" "$fixture" >/dev/null
jq -e '.tag_state == "live"' "$tag_receipt" >/dev/null

package="$scratch/redline-testing-1.0.1-linux-x86_64"
mkdir -p "$package/bin" "$package/data"
printf '#!/usr/bin/env bash\nexit 0\n' >"$package/bin/redline-testing"
chmod 0755 "$package/bin/redline-testing"
printf 'data\n' >"$package/data/member.txt"
"$inventory" "$package" >/dev/null

printf 'extra\n' >"$package/bin/extra"
chmod 0755 "$package/bin/extra"
expect_failure "$inventory" "$package"
rm "$package/bin/extra"

ln -s member.txt "$package/data/link"
expect_failure "$inventory" "$package"
rm "$package/data/link"

ln "$package/data/member.txt" "$package/data/hardlink"
expect_failure "$inventory" "$package"
rm "$package/data/hardlink"

mkfifo "$package/data/fifo"
expect_failure "$inventory" "$package"
rm "$package/data/fifo"

tarball="$scratch/package.tar.gz"
tar -C "$scratch" --sort=name --owner=0 --group=0 --numeric-owner \
    -czf "$tarball" "$(basename "$package")"
"$inventory" "$package" "$tarball" >/dev/null

duplicate_tarball="$scratch/package-duplicate.tar.gz"
tar -C "$scratch" --sort=name --owner=0 --group=0 --numeric-owner \
    -czf "$duplicate_tarball" "$(basename "$package")" \
    "$(basename "$package")/bin/redline-testing"
expect_failure "$inventory" "$package" "$duplicate_tarball"

printf 'hostile release lanes: pass\n'
