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

require_governed_jankurai
for sub in ci-bad-behavior git-bad-behavior release-bad-behavior; do
  log "language-bad-behavior: jankurai ${sub}"
  if bash ops/ci/run-jankurai.sh "$sub" . --out "$log_file" >>"$log_file" 2>&1; then
    printf '%s: ok\n' "$sub" >> "$log_file"
  else
    printf '%s: scan emitted findings (see above)\n' "$sub" >> "$log_file"
  fi
done

log "language-bad-behavior: complete"
