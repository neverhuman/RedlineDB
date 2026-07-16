#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# shellcheck source=ops/ci/pinned-rustsec.sh
source ops/ci/pinned-rustsec.sh

tmp="$(mktemp -d "${TMPDIR:-/tmp}/redline-central-rustsec-test.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT

if JAIN_RUSTSEC_OBJECT_SOURCE_OVERRIDE="$tmp/missing" jain_materialize_pinned_rustsec \
  >/dev/null 2>&1; then
  printf 'missing RustSec source was accepted\n' >&2
  exit 1
fi

ln -s -- "$JAIN_RUSTSEC_OBJECT_SOURCE" "$tmp/linked-source"
if JAIN_RUSTSEC_OBJECT_SOURCE_OVERRIDE="$tmp/linked-source" jain_materialize_pinned_rustsec \
  >/dev/null 2>&1; then
  printf 'linked RustSec source was accepted\n' >&2
  exit 1
fi

mkdir -p "$tmp/wrong-source/.git"
if JAIN_RUSTSEC_OBJECT_SOURCE_OVERRIDE="$tmp/wrong-source" jain_materialize_pinned_rustsec \
  >/dev/null 2>&1; then
  printf 'RustSec source without the pinned object was accepted\n' >&2
  exit 1
fi

jain_materialize_pinned_rustsec
jq -e \
  --arg commit "$JAIN_RUSTSEC_COMMIT" \
  --arg tree "$JAIN_RUSTSEC_TREE" \
  --arg archive "$JAIN_RUSTSEC_ARCHIVE_SHA256" \
  '.schema_version == "redline-central.rustsec-snapshot/v1"
    and .status == "pass"
    and .commit == $commit
    and .tree == $tree
    and .archive_sha256 == $archive' \
  target/jankurai/security/rustsec-snapshot.json >/dev/null
[[ -d target/jankurai/security/rustsec-db/crates ]]
[[ -z "$(/usr/bin/find target/jankurai/security/rustsec-db -type l -print -quit)" ]]
printf 'pinned RustSec snapshot contract ok\n'
