#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

version="$(tr -d '\n' < VERSION)"
test_directory="target/artifact-support-repeatability"
first_output="$test_directory/first-output"
second_output="target/artifact-support"
first_target="$ROOT/$test_directory/first-cargo-target"
second_target="$ROOT/$test_directory/second-cargo-target"
first_artifact="$first_output/redline-central-v${version}.tar.gz"
second_artifact="$second_output/redline-central-v${version}.tar.gz"
extract_directory="$test_directory/extract"
source_date_epoch="$(git show -s --format=%ct HEAD)"

rm -rf -- "$test_directory"
mkdir -p "$test_directory"
chmod 0755 "$test_directory"

if REDLINE_ARTIFACT_SUPPORT_DIR=/tmp/redline-central-escape \
    bash ops/ci/artifact-support.sh >/dev/null 2>&1; then
  printf 'artifact lane accepted output outside target/\n' >&2
  exit 1
fi
if SOURCE_DATE_EPOCH=not-a-number bash ops/ci/artifact-support.sh >/dev/null 2>&1; then
  printf 'artifact lane accepted an invalid SOURCE_DATE_EPOCH\n' >&2
  exit 1
fi
dirty_fixture=.artifact-support-dirty-fixture
[[ ! -e "$dirty_fixture" && ! -L "$dirty_fixture" ]]
printf 'hostile dirty source\n' >"$dirty_fixture"
if bash ops/ci/artifact-support.sh >/dev/null 2>&1; then
  rm -- "$dirty_fixture"
  printf 'artifact lane accepted a dirty source checkout\n' >&2
  exit 1
fi
rm -- "$dirty_fixture"

SOURCE_DATE_EPOCH="$source_date_epoch" CARGO_TARGET_DIR="$first_target" \
  REDLINE_ARTIFACT_SUPPORT_DIR="$first_output" bash ops/ci/artifact-support.sh
SOURCE_DATE_EPOCH="$source_date_epoch" CARGO_TARGET_DIR="$second_target" \
  REDLINE_ARTIFACT_SUPPORT_DIR="$second_output" bash ops/ci/artifact-support.sh

[[ "$first_target" != "$second_target" && -d "$first_target" && -d "$second_target" ]]
first_sha256="$(sha256sum "$first_artifact" | awk '{print $1}')"
second_sha256="$(sha256sum "$second_artifact" | awk '{print $1}')"
[[ "$first_sha256" == "$second_sha256" ]]
cmp --silent "$first_artifact" "$second_artifact"
cmp --silent "$first_target/release/redlinedb-client-smoke" \
  "$second_target/release/redlinedb-client-smoke"
cmp --silent "$first_target/release/db-shim-parity" \
  "$second_target/release/db-shim-parity"
[[ "$(od -An -tu4 -j4 -N4 "$second_artifact" | tr -d '[:space:]')" == "0" ]]

expected_members="$(printf '%s\n' \
  ./ \
  ./SUPPORT-ARCHIVE.md \
  ./backend-contract.toml \
  ./db-shim-parity \
  ./redlinedb-client-smoke)"
actual_members="$(tar -tzf "$second_artifact")"
[[ "$actual_members" == "$expected_members" ]]
if grep -Eq '(^|/)(Dockerfile|docker-compose[^/]*)$' <<<"$actual_members"; then
  printf 'evidence-only support archive contains deployment inputs\n' >&2
  exit 1
fi
tar --numeric-owner -tvf "$second_artifact" | awk '$2 != "0/0" { exit 1 }'

mkdir -m 0755 "$extract_directory"
tar -xzf "$second_artifact" --no-same-owner -C "$extract_directory"
while read -r expected_mode path; do
  [[ "$(stat -c '%a' "$extract_directory/$path")" == "$expected_mode" ]]
  [[ "$(stat -c '%Y' "$extract_directory/$path")" == "$source_date_epoch" ]]
done <<'EOF'
755 .
644 SUPPORT-ARCHIVE.md
644 backend-contract.toml
755 db-shim-parity
755 redlinedb-client-smoke
EOF

cmp --silent docs/support-archive.md "$extract_directory/SUPPORT-ARCHIVE.md"
cmp --silent db/backend-contract.toml "$extract_directory/backend-contract.toml"
cmp --silent "$second_target/release/redlinedb-client-smoke" \
  "$extract_directory/redlinedb-client-smoke"
cmp --silent "$second_target/release/db-shim-parity" "$extract_directory/db-shim-parity"
jq -e \
  --arg artifact_sha256 "$second_sha256" \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  --argjson source_date_epoch "$source_date_epoch" \
  --arg backend_contract_sha256 "$(sha256sum db/backend-contract.toml | awk '{print $1}')" \
  '.status == "pass"
    and .purpose == "review_evidence_only"
    and .deployable == false
    and .artifact_sha256 == $artifact_sha256
    and .commit == $commit
    and .tree == $tree
    and .source_date_epoch == $source_date_epoch
    and .backend_contract_sha256 == $backend_contract_sha256' \
  "$second_output/evidence.json" >/dev/null

printf 'artifact reproducibility ok: independent-target sha256=%s\n' "$second_sha256"
