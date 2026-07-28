#!/usr/bin/env bash
# Focused contract for the four release-mode dispatcher lanes. Heavy child
# lanes are replaced only inside this test; the real artifact lane is exercised
# separately by required release qualification.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

mkdir -p "$ROOT_DIR/target"
tmp="$(mktemp -d "$ROOT_DIR/target/release-dispatch-contract.XXXXXX")"
sentinel="$ROOT_DIR/release-dispatch-contract-hostile.$$"
status_before="$(git status --porcelain=v1 --untracked-files=all)"
cleanup() {
  rm -f -- "$sentinel"
  rm -rf -- "$tmp"
}
trap cleanup EXIT

mkdir -p "$tmp/bin"
cat >"$tmp/bin/bash" <<'SH'
#!/bin/sh
: "${DISPATCH_CAPTURE:?missing dispatch capture}"
printf '%s\n' "$*" >"$DISPATCH_CAPTURE"
exit "${DISPATCH_EXIT:-0}"
SH
chmod 0755 "$tmp/bin/bash"

expect_dispatch() {
  local mode="$1"
  local expected_script="$2"
  local capture="$tmp/${mode}.capture"
  local rc

  set +e
  DISPATCH_CAPTURE="$capture" DISPATCH_EXIT=73 \
    PATH="$tmp/bin:/usr/bin:/bin" \
    /bin/bash scripts/ci-local.sh "$mode" >"$tmp/${mode}.out" 2>&1
  rc=$?
  set -e
  [[ "$rc" -eq 73 ]] \
    || fail "dispatcher did not propagate child failure for ${mode}: ${rc}"
  [[ "$(cat "$capture")" == "$ROOT_DIR/$expected_script" ]] \
    || fail "dispatcher routed ${mode} to the wrong child"
}

expect_usage_failure() {
  local label="$1"
  shift
  local rc

  set +e
  /bin/bash scripts/ci-local.sh "$@" >"$tmp/${label}.usage" 2>&1
  rc=$?
  set -e
  [[ "$rc" -eq 64 ]] || fail "${label} dispatcher usage exit was ${rc}"
  grep -Fq 'security|score|contract-drift|artifact-support' \
    "$tmp/${label}.usage" || fail "${label} usage omitted release modes"
}

expect_dispatch security ops/ci/security.sh
expect_dispatch score ops/ci/score.sh
expect_dispatch contract-drift ops/ci/contract-drift.sh
expect_dispatch artifact-support ops/ci/artifact-support.sh
expect_usage_failure missing
expect_usage_failure extra required unexpected
expect_usage_failure unknown definitely-not-a-lane

set +e
REDLINE_ARTIFACT_SUPPORT_DIR=../outside \
  /bin/bash scripts/ci-local.sh artifact-support \
  >"$tmp/artifact-unsafe.out" 2>&1
unsafe_rc=$?
set -e
[[ "$unsafe_rc" -ne 0 ]] || fail "unsafe artifact output was accepted"
grep -Fq 'artifact support directory must remain beneath target/' \
  "$tmp/artifact-unsafe.out" || fail "unsafe artifact output failed for the wrong reason"

: >"$sentinel"
set +e
/bin/bash scripts/ci-local.sh score >"$tmp/score-dirty.out" 2>&1
score_dirty_rc=$?
/bin/bash scripts/ci-local.sh artifact-support >"$tmp/artifact-dirty.out" 2>&1
artifact_dirty_rc=$?
set -e
[[ "$score_dirty_rc" -ne 0 ]] || fail "score accepted a dirty source checkout"
[[ "$artifact_dirty_rc" -ne 0 ]] || fail "artifact support accepted a dirty source checkout"
grep -Fq 'score requires a clean checkout at start' "$tmp/score-dirty.out" \
  || fail "score dirty-source guard did not run"
grep -Fq 'artifact support requires a clean source checkout' \
  "$tmp/artifact-dirty.out" || fail "artifact dirty-source guard did not run"
rm -f -- "$sentinel"

/bin/bash scripts/ci-local.sh contract-drift
[[ "$(jq -r '.ok' target/jankurai/contract-drift.json)" == true ]] \
  || fail "contract-drift dispatcher did not emit passing evidence"
[[ "$(git status --porcelain=v1 --untracked-files=all)" == "$status_before" ]] \
  || fail "release dispatcher contract test changed source state"
log "release-dispatch-contract: complete"
