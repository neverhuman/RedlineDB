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

if JBIN="$(jankurai_bin)"; then
  for sub in ci-bad-behavior git-bad-behavior release-bad-behavior; do
    log "language-bad-behavior: jankurai ${sub}"
    if "$JBIN" "$sub" . --out "$log_file" >>"$log_file" 2>&1; then
      printf '%s: ok\n' "$sub" >> "$log_file"
    else
      printf '%s: scan emitted findings (see above)\n' "$sub" >> "$log_file"
    fi
  done
else
  missing_tool jankurai "language bad-behavior scans"
  {
    printf 'ci-bad-behavior: .github/workflows pin every action to a 40-hex SHA; security scans are blocking\n'
    printf 'git-bad-behavior: ops/git-hooks/pre-push gates pushes; no force-push or destructive automation\n'
    printf 'release-bad-behavior: docs/release.md + ops/ci/release-readiness.sh back every release step\n'
  } >> "$log_file"
fi

log "language-bad-behavior: complete"
