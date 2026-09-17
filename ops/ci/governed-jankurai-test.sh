#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$ROOT/ops/ci/lib.sh"

[[ "$JANKURAI_TAG" == "v1.6.11-deadlang-precision-split.2" ]]
[[ "$JANKURAI_REV" == "4dfbdfa3585f1928d5f996d7b5e14608dff14a03" ]]
[[ "$JANKURAI_SHA256" == \
  "96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa" ]]

fixture_root="$(mktemp -d /tmp/redline-central-jankurai.XXXXXX)"
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
  if "$@" >"$fixture_root/$label.log" 2>&1; then
    printf 'expected governed Jankurai rejection: %s\n' "$label" >&2
    exit 1
  fi
}

fixture="$fixture_root/jankurai"
write_fixture "$fixture" 'jankurai fixture-1'
fixture_sha256="$(sha256sum -- "$fixture" | awk '{print $1}')"
verify_jankurai_identity "$fixture" fixture-1 "$fixture_sha256"
expect_rejected missing \
  verify_jankurai_identity "$fixture_root/missing" fixture-1 "$fixture_sha256"

ln -s "$fixture" "$fixture_root/linked-jankurai"
expect_rejected symlink \
  verify_jankurai_identity "$fixture_root/linked-jankurai" fixture-1 "$fixture_sha256"
expect_rejected wrong-digest \
  verify_jankurai_identity "$fixture" fixture-1 \
    0000000000000000000000000000000000000000000000000000000000000000
expect_rejected wrong-version \
  verify_jankurai_identity "$fixture" fixture-2 "$fixture_sha256"

mkdir -p "$fixture_root/hostile-bin" "$fixture_root/empty-bin"
write_fixture "$fixture_root/hostile-bin/jankurai" 'jankurai 1.6.11-hostile'
expect_rejected hostile-source-selection \
  /usr/bin/env PATH="$fixture_root/hostile-bin:/usr/bin:/bin" \
  /usr/bin/bash -c \
  'set -euo pipefail; source "$1"; require_governed_jankurai' \
  _ "$ROOT/ops/ci/lib.sh"
expect_rejected missing-source-selection \
  /usr/bin/env PATH="$fixture_root/empty-bin:/usr/bin:/bin" \
  /usr/bin/bash -c \
  'set -euo pipefail; source "$1"; require_governed_jankurai' \
  _ "$ROOT/ops/ci/lib.sh"

PATH="$fixture_root/hostile-bin:/usr/bin:/bin"
export PATH
require_governed_jankurai
[[ "$(type -t jankurai)" == function ]]
[[ "$(jankurai --version)" == "jankurai $JANKURAI_VERSION" ]]

if grep -Fq '/home/ubuntu/.jeryu/bin/jankurai' \
  "$ROOT/ops/ci/lib.sh" "$ROOT/ops/ci/jankurai.sh" "$ROOT/scripts/ci-doctor.sh"
then
  printf 'governed Jankurai selection still depends on the user home\n' >&2
  exit 1
fi

printf 'governed Jankurai hostile identity tests passed\n'
