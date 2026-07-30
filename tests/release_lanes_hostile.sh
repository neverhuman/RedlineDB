#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_identity="$repo_root/ops/ci/source-identity.sh"
tag_identity="$repo_root/ops/ci/release-tag-identity.sh"
inventory="$repo_root/ops/ci/verify-release-inventory.sh"
projection="$repo_root/ops/ci/release-source-projection.sh"
manifest_validator="$repo_root/ops/ci/validate-release-manifest.sh"
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
printf '.cargo/\n' >"$fixture/.gitignore"
git -C "$fixture" add .gitignore Cargo.toml source.txt
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

mkdir -p "$fixture/.cargo"
printf '[build]\nrustc-wrapper = "/hostile/wrapper"\n' \
    >"$fixture/.cargo/config.toml"
projected="$scratch/projected"
"$projection" "$fixture" "$projected" "$(git -C "$fixture" rev-parse HEAD)" \
    >/dev/null
[[ ! -e "$projected/.cargo" && ! -L "$projected/.cargo" ]] || {
    printf 'ignored build input escaped into the release projection\n' >&2
    exit 1
}
rm -rf "$fixture/.cargo"

tag="redline-testing-v1.0.1-jain.2"
tag_receipt="$scratch/tag-identity.json"
forge="$scratch/redline-testing.git"
git init --bare -q "$forge"
git -C "$fixture" remote add governed "$forge"
git -C "$fixture" push -q governed HEAD:refs/heads/main
REDLINE_TESTING_HOSTILE_TEST=1 \
    "$tag_identity" "$tag" "$tag_receipt" "$fixture" "$forge" >/dev/null
jq -e '.tag_state == "planned"' "$tag_receipt" >/dev/null
git -C "$fixture" tag "$tag" "$initial_head"
expect_failure env REDLINE_TESTING_HOSTILE_TEST=1 \
    "$tag_identity" "$tag" "$tag_receipt" "$fixture" "$forge"
git -C "$fixture" tag -d "$tag" >/dev/null
git -C "$fixture" tag "$tag"
git -C "$fixture" push -q governed "refs/tags/$tag"
git -C "$fixture" tag -d "$tag" >/dev/null
expect_failure env REDLINE_TESTING_HOSTILE_TEST=1 \
    "$tag_identity" "$tag" "$tag_receipt" "$fixture" "$forge"
git -C "$fixture" tag "$tag"
REDLINE_TESTING_HOSTILE_TEST=1 \
    "$tag_identity" "$tag" "$tag_receipt" "$fixture" "$forge" >/dev/null
jq -e '.tag_state == "live"' "$tag_receipt" >/dev/null
git -C "$fixture" tag -f "$tag" "$initial_head" >/dev/null
expect_failure env REDLINE_TESTING_HOSTILE_TEST=1 \
    "$tag_identity" "$tag" "$tag_receipt" "$fixture" "$forge"

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
sidecar="$tarball.sha256"
tar -C "$scratch" --sort=name --mtime=@0 \
    --owner=0 --group=0 --numeric-owner --format=ustar \
    -cf - "$(basename "$package")" | gzip -n >"$tarball"
"$inventory" "$package" "$tarball" "$sidecar" >/dev/null
(cd "$scratch" && sha256sum -c "$(basename "$sidecar")" >/dev/null)

duplicate_tarball="$scratch/package-duplicate.tar.gz"
tar -C "$scratch" --sort=name --owner=0 --group=0 --numeric-owner \
    -czf "$duplicate_tarball" "$(basename "$package")" \
    "$(basename "$package")/bin/redline-testing"
expect_failure "$inventory" "$package" "$duplicate_tarball" \
    "$duplicate_tarball.sha256"

altered_root="$scratch/altered"
mkdir -p "$altered_root"
cp -a "$package" "$altered_root/$(basename "$package")"
printf 'changed\n' >"$altered_root/$(basename "$package")/data/member.txt"
altered_tarball="$scratch/package-altered.tar.gz"
tar -C "$altered_root" --sort=name --mtime=@0 \
    --owner=0 --group=0 --numeric-owner --format=ustar \
    -cf - "$(basename "$package")" | gzip -n >"$altered_tarball"
expect_failure "$inventory" "$package" "$altered_tarball" \
    "$altered_tarball.sha256"

race_tarball="$scratch/package-race.tar.gz"
cp "$tarball" "$race_tarball"
race_signal="$scratch/archive-open"
env REDLINE_TESTING_HOSTILE_TEST=1 \
    REDLINE_TESTING_TEST_ARCHIVE_OPEN_SIGNAL="$race_signal" \
    "$inventory" "$package" "$race_tarball" "$race_tarball.sha256" \
    >"$scratch/race.log" 2>&1 &
race_pid=$!
for _wait in $(seq 1 200); do
    [[ -e "$race_signal" ]] && break
    sleep 0.01
done
[[ -f "$race_signal" ]] || {
    printf 'archive replacement hostile did not reach descriptor custody\n' >&2
    kill "$race_pid" 2>/dev/null || true
    wait "$race_pid" 2>/dev/null || true
    exit 1
}
mv "$race_tarball" "$race_tarball.opened"
cp "$race_tarball.opened" "$race_tarball"
: >"${race_signal}.continue"
if wait "$race_pid"; then
    printf 'archive pathname replacement hostile unexpectedly passed\n' >&2
    exit 1
fi

valid_manifest="$scratch/release-manifest.json"
schema="$repo_root/schemas/release-manifest.schema.json"
jq -n \
    --arg forty "$(printf 'a%.0s' $(seq 1 40))" \
    --arg sixty_four "$(printf 'b%.0s' $(seq 1 64))" \
    '{
      name:"redline-testing",version:"1.0.1",target:"linux-x86_64",
      release_commit:$forty,release_tree:$forty,
      release_tag:"redline-testing-v1.0.1-jain.2",
      release_tag_state:"planned",tag_revision:2,
      source_archive_sha256:$sixty_four,binary:"bin/redline-testing",
      binary_sha256:$sixty_four,tarball_sha256_source:".sha256 sidecar",
      artifact_hashes:{"data/member.txt":$sixty_four},
      build_inputs:{
        source_mode:"sanitized-no-local",cargo_path:"/cargo",
        cargo_sha256:$sixty_four,cargo_version_sha256:$sixty_four,
        rustc_path:"/rustc",rustc_sha256:$sixty_four,
        rustc_version_sha256:$sixty_four,
        cargo_config_sha256:$sixty_four,vendor_root:"/vendor",
        vendor_inventory_sha256:$sixty_four,
        environment_sha256:$sixty_four
      },
      generated_by:"scripts/release-package.sh"
    }' >"$valid_manifest"
"$manifest_validator" "$schema" "$valid_manifest" >/dev/null
jq '.unexpected = true' "$valid_manifest" >"$scratch/unknown.json"
expect_failure "$manifest_validator" "$schema" "$scratch/unknown.json"
jq 'del(.release_tree)' "$valid_manifest" >"$scratch/missing.json"
expect_failure "$manifest_validator" "$schema" "$scratch/missing.json"
jq '.tag_revision = "2"' "$valid_manifest" >"$scratch/wrong-type.json"
expect_failure "$manifest_validator" "$schema" "$scratch/wrong-type.json"

printf 'hostile release lanes: pass\n'
