#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

version="$(tr -d '\n' < VERSION)"
artifact="target/artifact-support/redline-central-v${version}.tar.gz"
test_directory="target/artifact-support-repeatability"
first_artifact="$test_directory/first.tar.gz"
extract_directory="$test_directory/extract"
source_date_epoch="$(git show -s --format=%ct HEAD)"

rm -rf -- "$test_directory"
mkdir -p "$test_directory"
chmod 0755 "$test_directory"
bash ops/ci/artifact-support.sh
cp -- "$artifact" "$first_artifact"
first_sha256="$(sha256sum "$first_artifact" | awk '{print $1}')"

sleep 1
bash ops/ci/artifact-support.sh
second_sha256="$(sha256sum "$artifact" | awk '{print $1}')"
[[ "$first_sha256" == "$second_sha256" ]]
cmp --silent "$first_artifact" "$artifact"
[[ "$(od -An -tu4 -j4 -N4 "$artifact" | tr -d '[:space:]')" == "0" ]]

expected_members="$(printf '%s\n' \
  ./ \
  ./Dockerfile \
  ./README.md \
  ./docker-compose.yml \
  ./redlinedb-client-smoke \
  ./release.md)"
actual_members="$(tar -tzf "$artifact")"
[[ "$actual_members" == "$expected_members" ]]
tar --numeric-owner -tvf "$artifact" \
  | awk '$2 != "0/0" { exit 1 }'

mkdir -m 0755 "$extract_directory"
tar -xzf "$artifact" --no-same-owner -C "$extract_directory"
while read -r expected_mode path; do
  [[ "$(stat -c '%a' "$extract_directory/$path")" == "$expected_mode" ]]
  [[ "$(stat -c '%Y' "$extract_directory/$path")" == "$source_date_epoch" ]]
done <<'EOF'
755 .
644 Dockerfile
644 README.md
644 docker-compose.yml
755 redlinedb-client-smoke
644 release.md
EOF

cmp --silent README.md "$extract_directory/README.md"
cmp --silent docs/release.md "$extract_directory/release.md"
cmp --silent docker/Dockerfile "$extract_directory/Dockerfile"
cmp --silent docker/docker-compose.yml "$extract_directory/docker-compose.yml"
cmp --silent target/release/redlinedb-client-smoke \
  "$extract_directory/redlinedb-client-smoke"
jq -e \
  --arg artifact_sha256 "$second_sha256" \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  '.status == "pass"
    and .artifact_sha256 == $artifact_sha256
    and .commit == $commit
    and .tree == $tree' \
  target/artifact-support/evidence.json >/dev/null

printf 'artifact repeatability ok: sha256=%s\n' "$second_sha256"
