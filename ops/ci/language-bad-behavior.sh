#!/usr/bin/env bash
# Language bad-behavior lane: run the jankurai ci/git/release language scans and
# record a receipt. These detect mutable workflow refs, destructive git
# automation, and unverified release steps.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"
ensure_artifacts

log_file="${ARTIFACT_DIR}/language-bad-behavior.log"
: > "$log_file"

JBIN="$(jankurai_bin)" || fail "governed Jankurai identity verification failed"
for sub in ci-bad-behavior git-bad-behavior release-bad-behavior; do
  log "language-bad-behavior: jankurai ${sub}"
  "$JBIN" "$sub" . --out "$log_file" >>"$log_file" 2>&1
  printf '%s: ok\n' "$sub" >>"$log_file"
done

log "language-bad-behavior: complete"
