#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT
mkdir "$tmp/bin"
ln -s "$(command -v bash)" "$tmp/bin/bash"
ln -s "$(command -v git)" "$tmp/bin/git"

# The inner required run reaches the typed preflight but cannot find Cargo.
# It must stop there, must not print the success marker, and cannot recurse
# into this test because the tool inventory check precedes this invocation.
if PATH="$tmp/bin" "$tmp/bin/bash" ops/ci/required.sh >"$tmp/required.log" 2>&1; then
  printf 'required lane masked a missing-tool failure\n' >&2
  cat "$tmp/required.log" >&2
  exit 1
fi
grep -Fq 'required tool is unavailable: cargo' "$tmp/required.log"
if grep -Fq 'required ok: jain-split-ops' "$tmp/required.log"; then
  printf 'required lane printed success after a missing-tool failure\n' >&2
  exit 1
fi

printf 'required failure propagation ok\n'
